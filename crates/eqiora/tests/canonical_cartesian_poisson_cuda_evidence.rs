#[path = "support/canonical_cartesian_poisson.rs"]
mod canonical;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};

use eqiora::artifact::{
    ExecutionTopologyV1, LayoutArtifacts, ModelEnvelope, RealizationEnvelopeV1, RunManifestV2,
};
use eqiora::device::{
    BufferId, Completion, DeviceBufferDescriptor, DeviceCapability, DeviceDescriptor,
    DeviceElement, DeviceId, Fence, HostBufferDescriptor, MemoryRegion, QueueId, QueueSlot,
    QueueTimeline, RuntimeId, TransferDirection, TransferEvidence, TransferPlan, WaitedCompletion,
};
use eqiora::realization::{TargetCapabilities, resolve};
use eqiora::solver::{
    BackendId, ConvergenceReason, ExecutionId, ExecutionProvider, ExecutionReport,
    LinearOperatorOrientation, LinearSolver, PreconditionerPolicy, ProviderLibrary,
    ReductionPolicy, SERIAL_LINEAR_EXECUTION, SolverProvider, accept_linear_solution_with_verifier,
};
use eqiora_execution::{
    AdmittedExecution, CsrDeviceTransferEvidence, CudaExecutorDescriptor, CudaLinearExecutionTrace,
    DeploymentBinding, DeviceValueGeneration, ExecutionReceipt, ExecutionStepKind,
};
use eqiora_numerics::scalar::finalize_resolved_scalar_elliptic_cartesian;
use serde::Deserialize;
use serde::de::DeserializeOwned;

const OBSERVATION_SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-observation/v2";
const ENVIRONMENT_SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-environment/v2";
const SOURCE_IDENTITY_SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-source-identity/v1";
const MAX_ENVIRONMENT_BYTES: usize = 64 * 1024;
const MAX_SOLUTIONS_BYTES: usize = 256 * 1024;
const MAX_ARTIFACT_BYTES: usize = 1024 * 1024;
const MAX_TEXT_BYTES: usize = 16 * 1024;
const MAX_VALUE_COUNT: usize = 4096;
const MAX_TRANSFER_COUNT: usize = 7;
const MAX_DAG_STEP_COUNT: usize = 9;
const MAX_COMPLETION_SEQUENCE: u64 = 64;
const CUDA_RUNTIME: &str = "eqiora.cuda.cudarc";
const CUDA_SOLVER_BACKEND: &str = "eqiora.cuda.krylov";
const CUDA_LINEAR_EXECUTION: &str = "eqiora.cuda.single-device";
const SERIAL_EXECUTION: &str = "eqiora.host.serial";
const REGISTERED_SOURCE_COMMIT: &str = "5696f62ed84eba5457e2ff99f40fd2080c808d69";
const RECORDED_CUDA_ADAPTER_VERSION: &str = "0.1.0-alpha.1";
const RECORDED_CUDA_BINDING_TOOLKIT: &str = "12.0";
const RECORDED_CUDARC_VERSION: &str = "0.18.2";
const RECORDED_CUDA_EXECUTION_LIBRARIES: &[ProviderLibrary] = &[
    ProviderLibrary::new("cuda-binding-toolkit", RECORDED_CUDA_BINDING_TOOLKIT),
    ProviderLibrary::new("cudarc", RECORDED_CUDARC_VERSION),
];
const RECORDED_CUDA_SOLVER_PROVIDER: SolverProvider = SolverProvider::new(
    BackendId::new(CUDA_SOLVER_BACKEND),
    RECORDED_CUDA_ADAPTER_VERSION,
    &[],
);
const RECORDED_CUDA_EXECUTION_PROVIDER: ExecutionProvider = ExecutionProvider::new(
    ExecutionId::new(CUDA_LINEAR_EXECUTION),
    RECORDED_CUDA_ADAPTER_VERSION,
    RECORDED_CUDA_EXECUTION_LIBRARIES,
);
const CURRENT_MODEL_BRIDGE: &[u8] = include_bytes!(
    "../../../verify/numerics/canonical-cartesian-poisson-cuda/expected/current-model-bridge.json"
);

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentObservation {
    schema: String,
    source_commit: String,
    source_clean: bool,
    profile: String,
    rustc: String,
    target_arch: String,
    logical_cpu_count: usize,
    selected_device_count: usize,
    eqiora_device_ordinal: u16,
    adapter_version: String,
    system_load_before: SystemLoadObservation,
    system_load_after: SystemLoadObservation,
    other_compute_process_count_before: usize,
    other_compute_process_count_after: usize,
    device: DeviceObservation,
    libraries: LibraryObservation,
    observation_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SystemLoadObservation {
    one_minute: f64,
    five_minutes: f64,
    fifteen_minutes: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceObservation {
    runtime: String,
    ordinal: u16,
    name: String,
    total_memory_bytes: u64,
    capabilities: Vec<String>,
    compute_capability_major: u16,
    compute_capability_minor: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryObservation {
    driver: i32,
    cusparse: i32,
    cublas: i32,
    cudarc: String,
    binding_toolkit: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceIdentityObservation {
    schema: String,
    raw_compiler_model_ulid: String,
    symbols: Vec<SourceSymbolObservation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceSymbolObservation {
    name: String,
    kind: String,
    ulid: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolutionObservations {
    schema: String,
    methods: Vec<MethodObservation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodObservation {
    method: String,
    value_count: usize,
    values: Vec<f64>,
    producer: ProducerObservation,
    accepted_report: ReportObservation,
    method_metrics: MethodMetricsObservation,
    cpu_conformance: CpuConformanceObservation,
    execution: ExecutionObservation,
    timings_ns: TimingObservation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProducerObservation {
    reason: String,
    completed_iterations: usize,
    reported_residual_norm: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportObservation {
    backend: String,
    execution_adapter: String,
    execution_device: u16,
    verification_adapter: String,
    verification_workers: usize,
    orientation: String,
    algorithm: String,
    preconditioner: String,
    reduction: String,
    reason: String,
    completed_iterations: usize,
    initial_residual_norm: f64,
    reported_residual_norm: f64,
    true_residual_norm: f64,
    residual_target: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodMetricsObservation {
    l2_error: f64,
    boundary_quantity: f64,
    integrated_source: f64,
    relative_balance: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CpuConformanceObservation {
    maximum_absolute_error: f64,
    maximum_scaled_error: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionObservation {
    deployment: DeploymentObservation,
    queue_materialization: u64,
    operator_fingerprint_sha256: String,
    output_fingerprint_sha256: String,
    minimum_device_payload_bytes: usize,
    external_sparse_workspace_bytes: usize,
    receipt_dimension: usize,
    receipt_replay: ReceiptReplayObservation,
    dag: Vec<String>,
    transfers: Vec<TransferObservation>,
    waited_fences: WaitedFencesObservation,
    solution_generations: SolutionGenerationsObservation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploymentObservation {
    backend: String,
    adapter: String,
    runtime: String,
    device: u16,
    logical_queue_slot: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferObservation {
    slot: String,
    direction: String,
    element_type: String,
    elements: usize,
    bytes: usize,
    allocation: u64,
    completion_sequence: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitedFencesObservation {
    inputs_ready_sequence: u64,
    solve_visible_sequence: u64,
    solution_visible_sequence: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptReplayObservation {
    adapter: String,
    workers: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SolutionGenerationsObservation {
    allocation: u64,
    initial: u64,
    solved: u64,
    downloaded: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimingObservation {
    setup: u64,
    host_to_device: u64,
    solve: u64,
    device_to_host: u64,
    verification: u64,
    total: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecoderProbe {
    schema: String,
}

#[test]
fn observation_decoder_is_closed_and_bounded_before_committed_replay() {
    let valid: DecoderProbe = decode_closed(br#"{"schema":"probe/v1"}"#).unwrap();
    assert_eq!(valid.schema, "probe/v1");
    assert!(decode_closed::<DecoderProbe>(br#"{"schema":"probe/v1","extra":0}"#).is_err());
    assert!(decode_closed::<DecoderProbe>(br#"{"schema":"a","schema":"b"}"#).is_err());
    assert!(decode_closed::<DecoderProbe>(br#"{"schema": "#).is_err());
    assert!(
        read_bounded_bytes(
            &vec![b' '; MAX_ENVIRONMENT_BYTES + 1],
            MAX_ENVIRONMENT_BYTES
        )
        .is_err()
    );
}

#[test]
fn fresh_compiler_ids_alpha_normalize_to_exact_model_bytes() {
    let (recorded_program, recorded_identity) = canonical::compile_program_with_identity().unwrap();
    let recorded_model = ModelEnvelope::from_program(&recorded_program).unwrap();
    let recorded_identity = SourceIdentityObservation {
        schema: SOURCE_IDENTITY_SCHEMA.to_owned(),
        raw_compiler_model_ulid: recorded_identity.model_ulid,
        symbols: recorded_identity
            .symbols
            .into_iter()
            .map(|symbol| SourceSymbolObservation {
                name: symbol.name,
                kind: symbol.kind.to_owned(),
                ulid: symbol.ulid,
            })
            .collect(),
    };
    validate_source_identity(&recorded_identity).unwrap();

    let (fresh_program, fresh_identity) = canonical::compile_program_with_identity().unwrap();
    let fresh_model = ModelEnvelope::from_program(&fresh_program).unwrap();
    assert_ne!(
        fresh_model.canonical_json().unwrap(),
        recorded_model.canonical_json().unwrap(),
        "compiler v0 deliberately mints fresh graph IDs"
    );
    let normalized = normalize_compiled_model(
        &fresh_model,
        &fresh_identity,
        &recorded_model,
        &recorded_identity,
    )
    .unwrap();
    assert_eq!(
        normalized.canonical_json().unwrap(),
        recorded_model.canonical_json().unwrap()
    );
    assert_eq!(
        normalized.digest().unwrap(),
        recorded_model.digest().unwrap()
    );
}

#[test]
fn committed_canonical_cuda_observation_replays_on_the_host() {
    let root = case_root();
    let environment_bytes = read_bounded(
        &root.join("observations/environment.json"),
        MAX_ENVIRONMENT_BYTES,
    )
    .unwrap();
    let solutions_bytes = read_bounded(
        &root.join("observations/solutions.json"),
        MAX_SOLUTIONS_BYTES,
    )
    .unwrap();
    let source_identity_bytes = read_bounded(
        &root.join("observations/source-identity.json"),
        MAX_ENVIRONMENT_BYTES,
    )
    .unwrap();
    let environment: EnvironmentObservation = decode_closed(&environment_bytes).unwrap();
    let solutions: SolutionObservations = decode_closed(&solutions_bytes).unwrap();
    let recorded_identity: SourceIdentityObservation =
        decode_closed(&source_identity_bytes).unwrap();
    validate_environment(&environment).unwrap();
    validate_solutions(&solutions).unwrap();
    validate_source_identity(&recorded_identity).unwrap();

    let (raw_program, fresh_identity) = canonical::compile_program_with_identity().unwrap();
    let fresh_raw_model = ModelEnvelope::from_program(&raw_program).unwrap();
    let historical_model_bytes =
        read_bounded(&root.join("artifacts/model.json"), MAX_ARTIFACT_BYTES).unwrap();
    assert!(
        ModelEnvelope::from_json(&historical_model_bytes, Default::default()).is_err(),
        "the current decoder must not relabel the recorded historical Model"
    );
    let current_model_bytes = CURRENT_MODEL_BRIDGE.strip_suffix(b"\n").unwrap();
    let recorded_model = ModelEnvelope::from_json(current_model_bytes, Default::default()).unwrap();
    assert_eq!(
        recorded_model.canonical_json().unwrap(),
        current_model_bytes
    );
    let normalized_model = normalize_compiled_model(
        &fresh_raw_model,
        &fresh_identity,
        &recorded_model,
        &recorded_identity,
    )
    .unwrap();
    assert_eq!(
        normalized_model.canonical_json().unwrap(),
        current_model_bytes
    );
    assert_eq!(
        normalized_model.digest().unwrap(),
        recorded_model.digest().unwrap()
    );
    falsify_alpha_normalization(
        &fresh_raw_model,
        &fresh_identity,
        &recorded_model,
        &recorded_identity,
    );
    let program = normalized_model.to_program().unwrap();

    let capabilities = canonical::exact_capabilities(
        canonical::cuda_solver_contract(),
        TargetCapabilities::none().with_cuda_device(environment.eqiora_device_ordinal),
    );
    for ((revision, method, tag), observation) in
        canonical::METHODS.into_iter().zip(&solutions.methods)
    {
        assert_eq!(observation.method, tag);
        replay_method(
            &root,
            &program,
            &normalized_model,
            &capabilities,
            &environment,
            observation,
            revision,
            method,
            tag,
        )
        .unwrap();
    }

    falsify_closed_decoders(&environment_bytes, &source_identity_bytes, &solutions_bytes);
    falsify_numerical_reacceptance(&program, &capabilities, &environment, &solutions.methods[0]);
    falsify_artifact_linkage(&root);
}

#[allow(clippy::too_many_arguments)]
fn replay_method(
    root: &Path,
    program: &eqiora::sem::KernelProgram,
    model: &ModelEnvelope,
    capabilities: &eqiora::realization::RealizationCapabilities,
    environment: &EnvironmentObservation,
    observation: &MethodObservation,
    revision: u64,
    method: eqiora::realization::DiscretizationMethod,
    tag: &str,
) -> Result<(), String> {
    let request = canonical::request(program, method, environment.eqiora_device_ordinal, revision)?;
    let resolved = resolve(&request, canonical::requirements(), capabilities)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let fresh_realization =
        RealizationEnvelopeV1::from_resolved(model, &resolved, LayoutArtifacts::Replicated)
            .map_err(|diagnostic| diagnostic.to_string())?;
    let realization_bytes = read_bounded(
        &root.join(format!("artifacts/{tag}-realization.json")),
        MAX_ARTIFACT_BYTES,
    )?;
    let recorded_realization =
        RealizationEnvelopeV1::from_json(&realization_bytes, Default::default())
            .map_err(|diagnostic| diagnostic.to_string())?;
    if fresh_realization
        .canonical_json()
        .map_err(|diagnostic| diagnostic.to_string())?
        == realization_bytes
    {
        return Err("current Realization relabelled the historical Model lineage".to_owned());
    }
    require_equal_bytes(
        "decoded Realization canonical bytes",
        &recorded_realization
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &realization_bytes,
    )?;
    if fresh_realization
        .digest()
        .map_err(|diagnostic| diagnostic.to_string())?
        == recorded_realization
            .digest()
            .map_err(|diagnostic| diagnostic.to_string())?
    {
        return Err("current and historical Realization digests must differ".to_owned());
    }

    let run_bytes = read_bounded(
        &root.join(format!("artifacts/{tag}-run.json")),
        MAX_ARTIFACT_BYTES,
    )?;
    let recorded_run = RunManifestV2::from_json(&run_bytes, Default::default())
        .map_err(|diagnostic| diagnostic.to_string())?;
    recorded_run
        .validate_against(&recorded_realization)
        .map_err(|diagnostic| diagnostic.to_string())?;
    require_equal_bytes(
        "decoded Run canonical bytes",
        &recorded_run
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &run_bytes,
    )?;
    let fresh_run = run_from_environment(&fresh_realization, environment)?;
    fresh_run
        .validate_against(&fresh_realization)
        .map_err(|diagnostic| diagnostic.to_string())?;
    if fresh_run
        .canonical_json()
        .map_err(|diagnostic| diagnostic.to_string())?
        == run_bytes
    {
        return Err("current Run relabelled the historical Model lineage".to_owned());
    }
    if !recorded_run.outputs().is_empty() {
        return Err("the observation must not imply a durable product output artifact".to_owned());
    }

    let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian(program, &resolved)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let reason = parse_reason(&observation.producer.reason)?;
    let native_accepted = accept_linear_solution_with_verifier(
        &finalized
            .linear_problem()
            .map_err(|diagnostic| diagnostic.to_string())?,
        finalized.solver_plan(),
        RECORDED_CUDA_SOLVER_PROVIDER,
        RECORDED_CUDA_EXECUTION_PROVIDER,
        ExecutionReport::cuda(
            ExecutionId::new(CUDA_LINEAR_EXECUTION),
            environment.eqiora_device_ordinal,
        ),
        reason,
        observation.producer.completed_iterations,
        observation.producer.reported_residual_norm,
        observation.values.clone(),
        &SERIAL_LINEAR_EXECUTION,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    compare_report(native_accepted.report(), observation)?;
    let accepted =
        replay_observed_execution(&finalized, environment, observation, native_accepted)?;
    let solution = finalized
        .finish(accepted)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let metrics = canonical::method_metrics(method, &solution)?;
    require_same_float(
        "L2 error",
        metrics.l2_error,
        observation.method_metrics.l2_error,
    )?;
    require_same_float(
        "boundary quantity",
        metrics.boundary_quantity,
        observation.method_metrics.boundary_quantity,
    )?;
    require_same_float(
        "integrated source",
        metrics.integrated_source,
        observation.method_metrics.integrated_source,
    )?;
    require_same_float(
        "relative balance",
        metrics.relative_balance,
        observation.method_metrics.relative_balance,
    )?;
    let cpu = canonical::reference_cpu_solution(program, method, revision + 100)?;
    let conformance = canonical::cpu_conformance(&cpu, &solution)?;
    require_same_float(
        "CPU maximum absolute error",
        conformance.maximum_absolute_error,
        observation.cpu_conformance.maximum_absolute_error,
    )?;
    require_same_float(
        "CPU maximum scaled error",
        conformance.maximum_scaled_error,
        observation.cpu_conformance.maximum_scaled_error,
    )?;
    Ok(())
}

fn validate_environment(environment: &EnvironmentObservation) -> Result<(), String> {
    if environment.schema != ENVIRONMENT_SCHEMA
        || !environment.source_clean
        || environment.profile != "release"
    {
        return Err(
            "environment identity, source commit, cleanliness, or profile differs".to_owned(),
        );
    }
    if !is_full_lower_hex(&environment.source_commit) {
        return Err("source commit is not full lowercase 40-hex".to_owned());
    }
    if environment.source_commit != REGISTERED_SOURCE_COMMIT {
        return Err("source commit differs from the registered public source".to_owned());
    }
    for (label, value, allow_empty) in [
        ("rustc", environment.rustc.as_str(), false),
        (
            "target architecture",
            environment.target_arch.as_str(),
            false,
        ),
        (
            "adapter_version",
            environment.adapter_version.as_str(),
            false,
        ),
        ("device name", environment.device.name.as_str(), false),
        ("cudarc", environment.libraries.cudarc.as_str(), false),
        (
            "binding toolkit",
            environment.libraries.binding_toolkit.as_str(),
            false,
        ),
    ] {
        validate_text(label, value, allow_empty)?;
    }
    validate_system_load("before", &environment.system_load_before)?;
    validate_system_load("after", &environment.system_load_after)?;
    if environment.other_compute_process_count_before != 0
        || environment.other_compute_process_count_after != 0
    {
        return Err("selected device was not isolated from other compute processes".to_owned());
    }
    if environment.logical_cpu_count == 0
        || environment.selected_device_count != 1
        || environment.eqiora_device_ordinal != 0
        || environment.device.ordinal != 0
        || environment.device.runtime != CUDA_RUNTIME
        || environment.adapter_version != RECORDED_CUDA_ADAPTER_VERSION
        || environment.libraries.cudarc != RECORDED_CUDARC_VERSION
        || environment.libraries.binding_toolkit != RECORDED_CUDA_BINDING_TOOLKIT
        || environment.device.total_memory_bytes == 0
        || environment.device.compute_capability_major == 0
        || environment.libraries.driver <= 0
        || environment.libraries.cusparse <= 0
        || environment.libraries.cublas <= 0
        || environment.observation_kind != "selected-device-run; not hardware-attestation"
    {
        return Err("recorded device/library identity is incomplete or contradictory".to_owned());
    }
    let expected = [
        "float32",
        "float64",
        "csr-matrix-vector-product",
        "dense-vector-level-1",
        "asynchronous-queue",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let observed = environment
        .device
        .capabilities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if observed != expected || observed.len() != environment.device.capabilities.len() {
        return Err("device capabilities are missing, duplicated, or unknown".to_owned());
    }
    Ok(())
}

fn validate_system_load(label: &str, observation: &SystemLoadObservation) -> Result<(), String> {
    for (interval, value) in [
        ("one minute", observation.one_minute),
        ("five minutes", observation.five_minutes),
        ("fifteen minutes", observation.fifteen_minutes),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!(
                "{label} system load for {interval} must be finite and non-negative"
            ));
        }
    }
    Ok(())
}

fn validate_source_identity(identity: &SourceIdentityObservation) -> Result<(), String> {
    if identity.schema != SOURCE_IDENTITY_SCHEMA
        || !is_ulid(&identity.raw_compiler_model_ulid)
        || identity.symbols.is_empty()
        || identity.symbols.len() > 64
    {
        return Err("source identity schema, model ULID, or symbol count is invalid".to_owned());
    }
    let mut previous_name = None;
    let mut ulids = BTreeSet::new();
    for symbol in &identity.symbols {
        validate_text("source symbol name", &symbol.name, false)?;
        if !matches!(
            symbol.kind.as_str(),
            "domain"
                | "representation"
                | "field"
                | "parameter"
                | "port"
                | "relation"
                | "activation"
                | "connection"
                | "clock-domain"
        ) || !is_ulid(&symbol.ulid)
            || !ulids.insert(symbol.ulid.as_str())
            || previous_name.is_some_and(|previous| previous >= symbol.name.as_str())
        {
            return Err(
                "source symbols are unknown, duplicated, or not lexically ordered".to_owned(),
            );
        }
        previous_name = Some(symbol.name.as_str());
    }
    if ulids.contains(identity.raw_compiler_model_ulid.as_str()) {
        return Err("source model and declaration identities must be distinct".to_owned());
    }
    Ok(())
}

fn normalize_compiled_model(
    fresh: &ModelEnvelope,
    fresh_identity: &canonical::SourceIdentity,
    recorded: &ModelEnvelope,
    recorded_identity: &SourceIdentityObservation,
) -> Result<ModelEnvelope, String> {
    let fresh_bytes = fresh
        .canonical_json()
        .map_err(|diagnostic| diagnostic.to_string())?;
    let recorded_bytes = recorded
        .canonical_json()
        .map_err(|diagnostic| diagnostic.to_string())?;
    let mut fresh_value: serde_json::Value =
        serde_json::from_slice(&fresh_bytes).map_err(|error| error.to_string())?;
    let recorded_value: serde_json::Value =
        serde_json::from_slice(&recorded_bytes).map_err(|error| error.to_string())?;
    let fresh_model_ulid = model_ulid(&fresh_value)?;
    let recorded_model_ulid = model_ulid(&recorded_value)?;
    if fresh_model_ulid != fresh_identity.model_ulid
        || recorded_model_ulid != recorded_identity.raw_compiler_model_ulid
        || fresh_identity.symbols.len() != recorded_identity.symbols.len()
    {
        return Err("raw compiler Model and source identity records contradict".to_owned());
    }

    let mut mapping = BTreeMap::<String, String>::new();
    insert_mapping(
        &mut mapping,
        fresh_identity.model_ulid.clone(),
        recorded_identity.raw_compiler_model_ulid.clone(),
    )?;
    for (fresh, recorded) in fresh_identity
        .symbols
        .iter()
        .zip(&recorded_identity.symbols)
    {
        if fresh.name != recorded.name || fresh.kind != recorded.kind {
            return Err("fresh and recorded source declarations differ".to_owned());
        }
        insert_mapping(&mut mapping, fresh.ulid.clone(), recorded.ulid.clone())?;
    }

    let fresh_activations = activation_by_relation(&fresh_value)?;
    let recorded_activations = activation_by_relation(&recorded_value)?;
    if fresh_activations.len() != recorded_activations.len() {
        return Err("fresh and recorded activation counts differ".to_owned());
    }
    for (fresh_relation, fresh_activation) in fresh_activations {
        let recorded_relation = mapping
            .get(&fresh_relation)
            .ok_or_else(|| "activation refers to an unnamed or unmapped relation".to_owned())?;
        let recorded_activation = recorded_activations
            .get(recorded_relation)
            .ok_or_else(|| "recorded relation has no unique activation".to_owned())?;
        insert_mapping(&mut mapping, fresh_activation, recorded_activation.clone())?;
    }

    let fresh_ulids = collect_model_ulids(&fresh_value)?;
    let recorded_ulids = collect_model_ulids(&recorded_value)?;
    let mapped_sources = mapping.keys().cloned().collect::<BTreeSet<_>>();
    let mapped_targets = mapping.values().cloned().collect::<BTreeSet<_>>();
    if mapped_sources.len() != mapping.len()
        || mapped_targets.len() != mapping.len()
        || mapped_sources != fresh_ulids
        || mapped_targets != recorded_ulids
    {
        return Err(
            "alpha-renaming is incomplete, non-bijective, or contains an unused correspondence"
                .to_owned(),
        );
    }
    rewrite_model_ulids(&mut fresh_value, &mapping)?;
    let normalized_bytes = serde_json::to_vec(&fresh_value).map_err(|error| error.to_string())?;
    ModelEnvelope::from_json(&normalized_bytes, Default::default())
        .map_err(|diagnostic| diagnostic.to_string())
}

fn activation_by_relation(model: &serde_json::Value) -> Result<BTreeMap<String, String>, String> {
    let edges = model
        .get("edges")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Model artifact has no edge array".to_owned())?;
    let mut activations = BTreeMap::new();
    for edge in edges {
        if edge.get("kind").and_then(serde_json::Value::as_str) != Some("activates") {
            continue;
        }
        let from = edge
            .get("from")
            .ok_or_else(|| "activation edge has no source".to_owned())?;
        let to = edge
            .get("to")
            .ok_or_else(|| "activation edge has no target".to_owned())?;
        if from.get("kind").and_then(serde_json::Value::as_str) != Some("activation")
            || to.get("kind").and_then(serde_json::Value::as_str) != Some("relation")
        {
            return Err("activates edge has non-activation/relation endpoints".to_owned());
        }
        let activation = id_ulid(from)?.to_owned();
        let relation = id_ulid(to)?.to_owned();
        if activations.insert(relation, activation).is_some() {
            return Err("one relation has duplicate activation edges".to_owned());
        }
    }
    Ok(activations)
}

fn collect_model_ulids(model: &serde_json::Value) -> Result<BTreeSet<String>, String> {
    let mut ulids = BTreeSet::new();
    ulids.insert(model_ulid(model)?);
    collect_id_ulids(model, &mut ulids)?;
    Ok(ulids)
}

fn collect_id_ulids(value: &serde_json::Value, ulids: &mut BTreeSet<String>) -> Result<(), String> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(value) = object.get("ulid") {
                let ulid = value
                    .as_str()
                    .filter(|value| is_ulid(value))
                    .ok_or_else(|| "typed ID owns a malformed ULID".to_owned())?;
                ulids.insert(ulid.to_owned());
            }
            for value in object.values() {
                collect_id_ulids(value, ulids)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_id_ulids(value, ulids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn rewrite_model_ulids(
    value: &mut serde_json::Value,
    mapping: &BTreeMap<String, String>,
) -> Result<(), String> {
    match value {
        serde_json::Value::Object(object) => {
            for key in ["model_ulid", "ulid"] {
                if let Some(value) = object.get_mut(key) {
                    let source = value
                        .as_str()
                        .ok_or_else(|| format!("{key} is not a string"))?;
                    let target = mapping
                        .get(source)
                        .ok_or_else(|| format!("{key} has no alpha-renaming correspondence"))?;
                    *value = serde_json::Value::String(target.clone());
                }
            }
            for value in object.values_mut() {
                rewrite_model_ulids(value, mapping)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                rewrite_model_ulids(value, mapping)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn model_ulid(model: &serde_json::Value) -> Result<String, String> {
    model
        .get("model_ulid")
        .and_then(serde_json::Value::as_str)
        .filter(|value| is_ulid(value))
        .map(str::to_owned)
        .ok_or_else(|| "Model artifact has no valid model ULID".to_owned())
}

fn id_ulid(id: &serde_json::Value) -> Result<&str, String> {
    id.get("ulid")
        .and_then(serde_json::Value::as_str)
        .filter(|value| is_ulid(value))
        .ok_or_else(|| "typed ID has no valid ULID".to_owned())
}

fn insert_mapping(
    mapping: &mut BTreeMap<String, String>,
    source: String,
    target: String,
) -> Result<(), String> {
    if mapping.insert(source, target).is_some() {
        return Err("alpha-renaming source identity is duplicated".to_owned());
    }
    Ok(())
}

fn expected_cuda_dag() -> [&'static str; MAX_DAG_STEP_COUNT] {
    [
        "transfer-inputs-to-cuda",
        "await-cuda-inputs-ready",
        "solve-on-cuda",
        "await-cuda-solve-completion",
        "transfer-candidate-to-host",
        "await-host-visibility",
        "accept-with-native-host-verification",
        "replay-true-residual-on-host",
        "accept-host-complete",
    ]
}

fn expected_transfer_signatures() -> [(&'static str, &'static str, &'static str); MAX_TRANSFER_COUNT]
{
    [
        ("row-offsets", "host-to-device", "signed-index-64"),
        ("column-indices", "host-to-device", "signed-index-64"),
        ("matrix-values", "host-to-device", "f64"),
        ("right-hand-side", "host-to-device", "f64"),
        ("zero-initial-solution", "host-to-device", "f64"),
        ("jacobi-inverse-diagonal", "host-to-device", "f64"),
        ("complete-solution", "device-to-host", "f64"),
    ]
}

fn validate_completion_sequence(sequence: u64) -> Result<(), String> {
    if sequence == 0 || sequence > MAX_COMPLETION_SEQUENCE {
        return Err("completion sequence is outside the replay bound".to_owned());
    }
    Ok(())
}

fn validate_solutions(solutions: &SolutionObservations) -> Result<(), String> {
    if solutions.schema != OBSERVATION_SCHEMA || solutions.methods.len() != canonical::METHODS.len()
    {
        return Err("solution schema or exact method count differs".to_owned());
    }
    for ((_, _, expected), method) in canonical::METHODS.iter().zip(&solutions.methods) {
        validate_text("method", &method.method, false)?;
        if method.method != *expected
            || method.value_count == 0
            || method.value_count > MAX_VALUE_COUNT
            || method.values.len() != method.value_count
            || method
                .value_count
                .checked_mul(size_of::<f64>())
                .filter(|bytes| *bytes <= MAX_VALUE_COUNT * size_of::<f64>())
                .is_none()
        {
            return Err("method tag or bounded solution shape differs".to_owned());
        }
        for value in method.values.iter().chain([
            &method.producer.reported_residual_norm,
            &method.accepted_report.initial_residual_norm,
            &method.accepted_report.reported_residual_norm,
            &method.accepted_report.true_residual_norm,
            &method.accepted_report.residual_target,
            &method.method_metrics.l2_error,
            &method.method_metrics.boundary_quantity,
            &method.method_metrics.integrated_source,
            &method.method_metrics.relative_balance,
            &method.cpu_conformance.maximum_absolute_error,
            &method.cpu_conformance.maximum_scaled_error,
        ]) {
            require_canonical_float(*value)?;
        }
        let execution = &method.execution;
        for (label, value) in [
            ("deployment backend", execution.deployment.backend.as_str()),
            ("deployment adapter", execution.deployment.adapter.as_str()),
            ("deployment runtime", execution.deployment.runtime.as_str()),
            (
                "receipt replay adapter",
                execution.receipt_replay.adapter.as_str(),
            ),
        ] {
            validate_text(label, value, false)?;
        }
        if !is_lower_hex_64(&execution.operator_fingerprint_sha256)
            || !is_lower_hex_64(&execution.output_fingerprint_sha256)
            || execution.minimum_device_payload_bytes == 0
            || execution.queue_materialization == 0
            || execution.receipt_dimension != method.value_count
            || execution.receipt_replay.workers == 0
            || execution.dag.len() != MAX_DAG_STEP_COUNT
            || execution
                .dag
                .iter()
                .map(String::as_str)
                .ne(expected_cuda_dag())
            || execution.transfers.len() != MAX_TRANSFER_COUNT
        {
            return Err(
                "execution fingerprint, payload, DAG, or transfer count is invalid".to_owned(),
            );
        }
        for (transfer, (slot, direction, element_type)) in execution
            .transfers
            .iter()
            .zip(expected_transfer_signatures())
        {
            if transfer.slot != slot
                || transfer.direction != direction
                || transfer.element_type != element_type
                || transfer.elements == 0
                || transfer.elements > MAX_VALUE_COUNT * 8
                || transfer.bytes == 0
                || transfer.bytes > MAX_SOLUTIONS_BYTES
                || transfer.allocation == 0
            {
                return Err(
                    "transfer slot, type, extent, byte count, or allocation is invalid".to_owned(),
                );
            }
            validate_completion_sequence(transfer.completion_sequence)?;
        }
        for sequence in [
            execution.waited_fences.inputs_ready_sequence,
            execution.waited_fences.solve_visible_sequence,
            execution.waited_fences.solution_visible_sequence,
        ] {
            validate_completion_sequence(sequence)?;
        }
        if execution.solution_generations.allocation == 0
            || execution.solution_generations.initial == 0
            || execution.solution_generations.solved == 0
            || execution.solution_generations.downloaded == 0
            || method.timings_ns.total == 0
            || [
                method.timings_ns.setup,
                method.timings_ns.host_to_device,
                method.timings_ns.solve,
                method.timings_ns.device_to_host,
                method.timings_ns.verification,
            ]
            .into_iter()
            .any(|phase| phase > method.timings_ns.total)
        {
            return Err("operational counts or timings are invalid".to_owned());
        }
    }
    Ok(())
}

fn replay_observed_execution(
    finalized: &eqiora_numerics::scalar::FinalizedScalarEllipticCartesianProblem,
    environment: &EnvironmentObservation,
    observation: &MethodObservation,
    native_accepted: eqiora::solver::LinearSolution,
) -> Result<eqiora::solver::LinearSolution, String> {
    let system = finalized.canonical_csr_system_view();
    if observation.value_count != system.rows() {
        return Err("recorded solution count differs from the re-finalized system".to_owned());
    }
    let execution = &observation.execution;
    let deployment = &execution.deployment;
    if deployment.backend != CUDA_SOLVER_BACKEND
        || deployment.adapter != CUDA_LINEAR_EXECUTION
        || deployment.runtime != CUDA_RUNTIME
        || deployment.runtime != environment.device.runtime
        || deployment.device != environment.device.ordinal
        || deployment.logical_queue_slot != 0
    {
        return Err("recorded deployment contradicts the bounded CUDA slice".to_owned());
    }
    let device = device_from_environment(environment)?;
    let slot = QueueSlot::new(device.id(), deployment.logical_queue_slot);
    let binding = DeploymentBinding::bind_cuda(
        finalized.portable_realization(),
        CudaExecutorDescriptor::new(
            RECORDED_CUDA_SOLVER_PROVIDER,
            RECORDED_CUDA_EXECUTION_PROVIDER,
            device.clone(),
            slot,
            canonical::cuda_solver_contract(),
        )
        .map_err(|diagnostic| diagnostic.to_string())?,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let admitted =
        AdmittedExecution::admit_cuda_linear(finalized.portable_realization(), system, binding)
            .map_err(|diagnostic| diagnostic.to_string())?;
    if admitted.minimum_device_payload_bytes() != Some(execution.minimum_device_payload_bytes) {
        return Err("recorded resident payload differs from canonical admission".to_owned());
    }

    let trace = reconstruct_trace(execution, &device, slot)?;
    let accepted = admitted
        .accept_cuda(native_accepted, trace)
        .map_err(|diagnostic| diagnostic.to_string())?;
    compare_receipt(accepted.receipt(), execution, trace)?;
    Ok(accepted.into_parts().0)
}

fn device_from_environment(
    environment: &EnvironmentObservation,
) -> Result<DeviceDescriptor, String> {
    DeviceDescriptor::new(
        DeviceId::new(RuntimeId::new(CUDA_RUNTIME), environment.device.ordinal),
        environment.device.name.clone(),
        NonZeroU64::new(environment.device.total_memory_bytes)
            .ok_or_else(|| "recorded device memory is zero".to_owned())?,
        [
            DeviceCapability::Float32,
            DeviceCapability::Float64,
            DeviceCapability::CsrMatrixVectorProduct,
            DeviceCapability::DenseVectorLevel1,
            DeviceCapability::AsynchronousQueue,
        ],
    )
    .map_err(|diagnostic| diagnostic.to_string())
}

fn reconstruct_trace(
    execution: &ExecutionObservation,
    device: &DeviceDescriptor,
    slot: QueueSlot,
) -> Result<CudaLinearExecutionTrace, String> {
    let queue = QueueId::new(
        slot,
        NonZeroU64::new(execution.queue_materialization)
            .ok_or_else(|| "queue materialization identity is zero".to_owned())?,
    );
    let transfers = &execution.transfers;
    let typed = CsrDeviceTransferEvidence::new(
        reconstruct_transfer::<i64>(
            &transfers[0],
            "row-offsets",
            "signed-index-64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?,
        reconstruct_transfer::<i64>(
            &transfers[1],
            "column-indices",
            "signed-index-64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?,
        reconstruct_transfer::<f64>(
            &transfers[2],
            "matrix-values",
            "f64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?,
        reconstruct_transfer::<f64>(
            &transfers[3],
            "right-hand-side",
            "f64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?,
        reconstruct_transfer::<f64>(
            &transfers[4],
            "zero-initial-solution",
            "f64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?,
        Some(reconstruct_transfer::<f64>(
            &transfers[5],
            "jacobi-inverse-diagonal",
            "f64",
            TransferDirection::HostToDevice,
            device,
            queue,
        )?),
        reconstruct_transfer::<f64>(
            &transfers[6],
            "complete-solution",
            "f64",
            TransferDirection::DeviceToHost,
            device,
            queue,
        )?,
    );
    let waited = &execution.waited_fences;
    let inputs_ready = reconstruct_wait(waited.inputs_ready_sequence, queue)?;
    let solve_visible = reconstruct_wait(waited.solve_visible_sequence, queue)?;
    let solution_visible = reconstruct_wait(waited.solution_visible_sequence, queue)?;
    let generations = &execution.solution_generations;
    let solution_buffer = BufferId::new(
        device.id(),
        NonZeroU64::new(generations.allocation)
            .ok_or_else(|| "solution allocation identity is zero".to_owned())?,
    );
    let initial = DeviceValueGeneration::new(
        solution_buffer,
        NonZeroU64::new(generations.initial)
            .ok_or_else(|| "initial solution generation is zero".to_owned())?,
    );
    let solved = DeviceValueGeneration::new(
        solution_buffer,
        NonZeroU64::new(generations.solved)
            .ok_or_else(|| "solved solution generation is zero".to_owned())?,
    );
    let downloaded = DeviceValueGeneration::new(
        solution_buffer,
        NonZeroU64::new(generations.downloaded)
            .ok_or_else(|| "downloaded solution generation is zero".to_owned())?,
    );
    CudaLinearExecutionTrace::new(
        typed,
        inputs_ready,
        solve_visible,
        solution_visible,
        initial,
        solved,
        downloaded,
        execution.external_sparse_workspace_bytes,
    )
    .map_err(|diagnostic| diagnostic.to_string())
}

fn reconstruct_transfer<T: DeviceElement>(
    observation: &TransferObservation,
    expected_slot: &str,
    expected_element_type: &str,
    expected_direction: TransferDirection,
    device: &DeviceDescriptor,
    queue: QueueId,
) -> Result<TransferEvidence<T>, String> {
    if observation.slot != expected_slot
        || observation.element_type != expected_element_type
        || observation.direction != transfer_direction_name(expected_direction)
    {
        return Err(format!(
            "recorded transfer `{expected_slot}` changed its typed contract"
        ));
    }
    let elements = NonZeroUsize::new(observation.elements)
        .ok_or_else(|| format!("recorded transfer `{expected_slot}` is empty"))?;
    let allocation = NonZeroU64::new(observation.allocation)
        .ok_or_else(|| format!("recorded transfer `{expected_slot}` has zero allocation"))?;
    let buffer = DeviceBufferDescriptor::<T>::new(BufferId::new(device.id(), allocation), elements);
    let host = HostBufferDescriptor::<T>::new(elements);
    let plan = match expected_direction {
        TransferDirection::HostToDevice => {
            TransferPlan::new(MemoryRegion::Host(host), MemoryRegion::Device(buffer))
        }
        TransferDirection::DeviceToHost => {
            TransferPlan::new(MemoryRegion::Device(buffer), MemoryRegion::Host(host))
        }
        TransferDirection::DeviceToDevice => {
            return Err("device-to-device transfer is outside this evidence slice".to_owned());
        }
    }
    .map_err(|diagnostic| diagnostic.to_string())?;
    if plan.bytes().map_err(|diagnostic| diagnostic.to_string())? != observation.bytes {
        return Err(format!(
            "recorded transfer `{expected_slot}` byte count differs"
        ));
    }
    let completion = reconstruct_completion(observation.completion_sequence, queue)?;
    TransferEvidence::new(plan, completion).map_err(|diagnostic| diagnostic.to_string())
}

#[derive(Debug)]
struct ObservedSuccessfulFence {
    completion: Completion,
}

impl Fence for ObservedSuccessfulFence {
    fn completion(&self) -> Completion {
        self.completion
    }

    fn wait(&self) -> Result<(), eqiora::Diagnostic> {
        Ok(())
    }
}

fn reconstruct_wait(sequence: u64, queue: QueueId) -> Result<WaitedCompletion, String> {
    let fence = ObservedSuccessfulFence {
        completion: reconstruct_completion(sequence, queue)?,
    };
    WaitedCompletion::wait(&fence).map_err(|diagnostic| diagnostic.to_string())
}

fn reconstruct_completion(sequence: u64, queue: QueueId) -> Result<Completion, String> {
    if sequence == 0 || sequence > MAX_COMPLETION_SEQUENCE {
        return Err("recorded completion sequence is zero or exceeds the replay bound".to_owned());
    }
    let mut timeline = QueueTimeline::new(queue);
    let mut submission = None;
    for _ in 0..sequence {
        submission = Some(
            timeline
                .next_submission()
                .map_err(|diagnostic| diagnostic.to_string())?,
        );
    }
    Ok(Completion::new(submission.expect(
        "positive bounded sequence constructs one submission",
    )))
}

fn compare_receipt(
    receipt: &ExecutionReceipt,
    observation: &ExecutionObservation,
    trace: CudaLinearExecutionTrace,
) -> Result<(), String> {
    let executor = receipt
        .binding()
        .cuda_executor()
        .ok_or_else(|| "replayed receipt lost its CUDA deployment".to_owned())?;
    let replay = receipt.acceptance_verification();
    let replay_workers = match replay.topology() {
        eqiora::solver::ExecutionTopology::Host { workers } => workers.get(),
        _ => return Err("receipt replay is not a host execution".to_owned()),
    };
    if executor.backend().as_str() != observation.deployment.backend
        || executor.adapter().as_str() != observation.deployment.adapter
        || executor.device().id().runtime().as_str() != observation.deployment.runtime
        || executor.device().id().ordinal() != observation.deployment.device
        || executor.queue().ordinal() != observation.deployment.logical_queue_slot
        || trace.queue().materialization().get() != observation.queue_materialization
        || receipt.minimum_device_payload_bytes() != Some(observation.minimum_device_payload_bytes)
        || receipt.cuda_trace() != Some(trace)
        || trace.external_sparse_workspace_bytes() != observation.external_sparse_workspace_bytes
        || receipt.dimension() != observation.receipt_dimension
        || replay.adapter().as_str() != observation.receipt_replay.adapter
        || replay_workers != observation.receipt_replay.workers
    {
        return Err("replayed receipt deployment, payload, or trace differs".to_owned());
    }
    if hex_32(receipt.operator().as_bytes()) != observation.operator_fingerprint_sha256
        || hex_32(receipt.output().as_bytes()) != observation.output_fingerprint_sha256
    {
        return Err("replayed operator or output fingerprint differs".to_owned());
    }
    let replayed_dag = receipt
        .dag()
        .steps()
        .iter()
        .copied()
        .map(ExecutionStepKind::canonical_name)
        .collect::<Vec<_>>();
    if replayed_dag
        .iter()
        .copied()
        .ne(observation.dag.iter().map(String::as_str))
    {
        return Err("replayed fixed execution DAG differs".to_owned());
    }
    Ok(())
}

fn compare_report(
    report: &eqiora::solver::SolveReport,
    observation: &MethodObservation,
) -> Result<(), String> {
    let recorded = &observation.accepted_report;
    if observation.producer.reason != recorded.reason
        || observation.producer.completed_iterations != recorded.completed_iterations
        || observation.producer.reported_residual_norm.to_bits()
            != recorded.reported_residual_norm.to_bits()
        || recorded.backend != CUDA_SOLVER_BACKEND
        || recorded.execution_adapter != CUDA_LINEAR_EXECUTION
        || recorded.execution_device != observation.execution.deployment.device
        || recorded.verification_adapter != SERIAL_EXECUTION
        || recorded.verification_workers != 1
        || recorded.orientation != "normal"
        || recorded.algorithm != "conjugate-gradient"
        || recorded.preconditioner != "jacobi"
        || recorded.reduction != "fast"
    {
        return Err("producer and accepted report identities contradict".to_owned());
    }
    if report.backend().as_str() != recorded.backend
        || report.execution().adapter().as_str() != recorded.execution_adapter
        || report.verification().adapter().as_str() != recorded.verification_adapter
        || report.orientation() != LinearOperatorOrientation::Normal
        || report.algorithm() != LinearSolver::ConjugateGradient
        || report.preconditioner() != PreconditionerPolicy::Jacobi
        || report.reduction() != ReductionPolicy::Fast
        || reason_name(report.reason()) != recorded.reason
        || report.completed_iterations() != recorded.completed_iterations
    {
        return Err("host reacceptance report identity differs from the observation".to_owned());
    }
    for (label, replayed, observed) in [
        (
            "initial residual",
            report.initial_residual_norm(),
            recorded.initial_residual_norm,
        ),
        (
            "reported residual",
            report.reported_residual_norm(),
            recorded.reported_residual_norm,
        ),
        (
            "true residual",
            report.true_residual_norm(),
            recorded.true_residual_norm,
        ),
        (
            "residual target",
            report.residual_target(),
            recorded.residual_target,
        ),
    ] {
        require_same_float(label, replayed, observed)?;
    }
    Ok(())
}

fn run_from_environment(
    realization: &RealizationEnvelopeV1,
    environment: &EnvironmentObservation,
) -> Result<RunManifestV2, String> {
    let native_libraries = [
        ("cusparse", environment.libraries.cusparse.to_string()),
        ("cublas", environment.libraries.cublas.to_string()),
    ];
    let execution = eqiora::artifact::ExecutionProvenanceV1::from_provider_releases(
        RECORDED_CUDA_SOLVER_PROVIDER,
        RECORDED_CUDA_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Cuda {
            device: environment.device.ordinal,
            device_name: environment.device.name.clone(),
            compute_capability_major: environment.device.compute_capability_major,
            compute_capability_minor: environment.device.compute_capability_minor,
            driver_version: environment.libraries.driver.to_string(),
        },
        ReductionPolicy::Fast,
        native_libraries,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    RunManifestV2::new(realization, execution).map_err(|diagnostic| diagnostic.to_string())
}

fn falsify_closed_decoders(environment: &[u8], source_identity: &[u8], solutions: &[u8]) {
    let unknown =
        String::from_utf8(environment.to_vec())
            .unwrap()
            .replacen('{', "{\"unknown\":0,", 1);
    assert!(decode_closed::<EnvironmentObservation>(unknown.as_bytes()).is_err());
    let duplicate = String::from_utf8(environment.to_vec()).unwrap().replacen(
        '{',
        &format!("{{\"schema\":\"{ENVIRONMENT_SCHEMA}\","),
        1,
    );
    assert!(decode_closed::<EnvironmentObservation>(duplicate.as_bytes()).is_err());
    assert!(
        decode_closed::<EnvironmentObservation>(&environment[..environment.len() - 2]).is_err()
    );
    assert!(
        read_bounded_bytes(
            &vec![b' '; MAX_ENVIRONMENT_BYTES + 1],
            MAX_ENVIRONMENT_BYTES
        )
        .is_err()
    );

    let source_unknown = String::from_utf8(source_identity.to_vec())
        .unwrap()
        .replacen('{', "{\"unknown\":0,", 1);
    assert!(decode_closed::<SourceIdentityObservation>(source_unknown.as_bytes()).is_err());
    let source_duplicate = String::from_utf8(source_identity.to_vec())
        .unwrap()
        .replacen(
            '{',
            &format!("{{\"schema\":\"{SOURCE_IDENTITY_SCHEMA}\","),
            1,
        );
    assert!(decode_closed::<SourceIdentityObservation>(source_duplicate.as_bytes()).is_err());

    let solution_unknown =
        String::from_utf8(solutions.to_vec())
            .unwrap()
            .replacen('{', "{\"unknown\":0,", 1);
    assert!(decode_closed::<SolutionObservations>(solution_unknown.as_bytes()).is_err());

    let mut negative_zero: SolutionObservations = decode_closed(solutions).unwrap();
    negative_zero.methods[0].values[0] = -0.0;
    assert!(validate_solutions(&negative_zero).is_err());
    let mut excessive = negative_zero;
    excessive.methods[0].value_count = MAX_VALUE_COUNT + 1;
    assert!(validate_solutions(&excessive).is_err());
    let mut unbounded_completion: SolutionObservations = decode_closed(solutions).unwrap();
    unbounded_completion.methods[0].execution.transfers[0].completion_sequence =
        MAX_COMPLETION_SEQUENCE + 1;
    assert!(validate_solutions(&unbounded_completion).is_err());
}

fn falsify_alpha_normalization(
    fresh: &ModelEnvelope,
    fresh_identity: &canonical::SourceIdentity,
    recorded: &ModelEnvelope,
    recorded_identity: &SourceIdentityObservation,
) {
    let mut duplicate = recorded_identity.clone();
    duplicate.symbols[0].ulid = duplicate.symbols[1].ulid.clone();
    assert!(validate_source_identity(&duplicate).is_err());
    assert!(normalize_compiled_model(fresh, fresh_identity, recorded, &duplicate).is_err());

    let mut missing = recorded_identity.clone();
    missing.symbols.pop();
    assert!(normalize_compiled_model(fresh, fresh_identity, recorded, &missing).is_err());

    let mut unused = recorded_identity.clone();
    unused.symbols.push(SourceSymbolObservation {
        name: "zz_unknown".to_owned(),
        kind: "field".to_owned(),
        ulid: "00000000000000000000000000".to_owned(),
    });
    assert!(normalize_compiled_model(fresh, fresh_identity, recorded, &unused).is_err());
}

fn falsify_numerical_reacceptance(
    program: &eqiora::sem::KernelProgram,
    capabilities: &eqiora::realization::RealizationCapabilities,
    environment: &EnvironmentObservation,
    observation: &MethodObservation,
) {
    let (revision, method) = canonical::method_from_tag(&observation.method).unwrap();
    let request =
        canonical::request(program, method, environment.device.ordinal, revision).unwrap();
    let resolved = resolve(&request, canonical::requirements(), capabilities).unwrap();
    let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian(program, &resolved).unwrap();
    let mut values = observation.values.clone();
    values[0] += 1.0e-3;
    let result = accept_linear_solution_with_verifier(
        &finalized.linear_problem().unwrap(),
        finalized.solver_plan(),
        RECORDED_CUDA_SOLVER_PROVIDER,
        RECORDED_CUDA_EXECUTION_PROVIDER,
        ExecutionReport::cuda(
            ExecutionId::new(CUDA_LINEAR_EXECUTION),
            environment.device.ordinal,
        ),
        parse_reason(&observation.producer.reason).unwrap(),
        observation.producer.completed_iterations,
        observation.producer.reported_residual_norm,
        values,
        &SERIAL_LINEAR_EXECUTION,
    );
    assert!(result.is_err());
}

fn falsify_artifact_linkage(root: &Path) {
    let bytes = read_bounded(&root.join("artifacts/q1-fem-run.json"), MAX_ARTIFACT_BYTES).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["realization_sha256"] = serde_json::Value::String("00".repeat(32));
    let forged = serde_json::to_vec(&value).unwrap();
    let run = RunManifestV2::from_json(&forged, Default::default()).unwrap();
    let realization_bytes = read_bounded(
        &root.join("artifacts/q1-fem-realization.json"),
        MAX_ARTIFACT_BYTES,
    )
    .unwrap();
    let realization =
        RealizationEnvelopeV1::from_json(&realization_bytes, Default::default()).unwrap();
    assert!(run.validate_against(&realization).is_err());
}

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verify/numerics/canonical-cartesian-poisson-cuda")
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    read_bounded_bytes(&bytes, maximum)?;
    Ok(bytes)
}

fn read_bounded_bytes(bytes: &[u8], maximum: usize) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(format!(
            "evidence bytes require 1..={maximum} bytes, found {}",
            bytes.len()
        ));
    }
    Ok(())
}

fn decode_closed<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let decoded = T::deserialize(&mut deserializer).map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(decoded)
}

fn validate_text(label: &str, value: &str, allow_empty: bool) -> Result<(), String> {
    if value.len() > MAX_TEXT_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || (!allow_empty && value.trim().is_empty())
    {
        return Err(format!(
            "{label} text is empty, oversized, or contains controls"
        ));
    }
    Ok(())
}

fn require_canonical_float(value: f64) -> Result<(), String> {
    if !value.is_finite() || value.to_bits() == (-0.0_f64).to_bits() {
        return Err("evidence float must be finite with positive zero canonicalization".to_owned());
    }
    Ok(())
}

fn require_same_float(label: &str, replayed: f64, observed: f64) -> Result<(), String> {
    require_canonical_float(replayed)?;
    require_canonical_float(observed)?;
    if replayed.to_bits() != observed.to_bits() {
        return Err(format!(
            "{label} differs: replayed {replayed:e}, observed {observed:e}"
        ));
    }
    Ok(())
}

fn require_equal_bytes(label: &str, left: &[u8], right: &[u8]) -> Result<(), String> {
    if left != right {
        return Err(format!("{label} differ"));
    }
    Ok(())
}

fn parse_reason(value: &str) -> Result<ConvergenceReason, String> {
    match value {
        "initial-residual-satisfied" => Ok(ConvergenceReason::InitialResidualSatisfied),
        "residual-tolerance-satisfied" => Ok(ConvergenceReason::ResidualToleranceSatisfied),
        _ => Err(format!("unknown convergence reason `{value}`")),
    }
}

fn reason_name(value: ConvergenceReason) -> &'static str {
    match value {
        ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
        ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
    }
}

fn transfer_direction_name(direction: TransferDirection) -> &'static str {
    match direction {
        TransferDirection::HostToDevice => "host-to-device",
        TransferDirection::DeviceToHost => "device-to-host",
        TransferDirection::DeviceToDevice => "device-to-device",
    }
}

fn hex_32(bytes: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(value, "{byte:02x}").expect("String writes cannot fail");
    }
    value
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_full_lower_hex(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_ulid(value: &str) -> bool {
    value.len() == 26
        && value.bytes().enumerate().all(|(index, byte)| {
            (byte.is_ascii_digit()
                || matches!(byte, b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'T' | b'V'..=b'Z'))
                && (index != 0 || byte <= b'7')
        })
}
