//! Collect one bounded canonical FEM/TPFA CUDA observation.
//!
//! This executable is case-specific verification tooling. It is neither a
//! product result format nor a general CUDA evidence API.

#[path = "../tests/support/canonical_cartesian_poisson.rs"]
mod canonical;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use eqiora::artifact::{
    ExecutionTopologyV1, LayoutArtifacts, ModelEnvelope, RealizationEnvelopeV1, RunManifestV2,
};
use eqiora::backends::cuda::{
    CUDA_ADAPTER_VERSION, CUDA_BINDING_TOOLKIT, CUDA_LINEAR_EXECUTION_PROVIDER,
    CUDA_LINEAR_SOLVER_PROVIDER, CUDARC_VERSION, CudaLinearSolveEvidence, CudaLinearSolver,
    CudaRuntime,
};
use eqiora::device::{
    Completion, DeviceCapability, DeviceElement, DeviceElementType, MemoryRegion, QueueId,
    QueueSlot, TransferDirection, TransferEvidence,
};
use eqiora::realization::{TargetCapabilities, resolve};
use eqiora::solver::ConvergenceReason;
use eqiora_backend_cuda::CudaAdmittedExecutionAdapter;
use eqiora_execution::{
    AdmittedExecution, CudaExecutorDescriptor, DeploymentBinding, ExecutionReceipt,
    ExecutionStepKind,
};
use eqiora_numerics::{
    scalar::ResolvedScalarEllipticCartesianSolution,
    scalar::finalize_resolved_scalar_elliptic_cartesian,
};

const SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-observation/v2";
const ENVIRONMENT_SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-environment/v2";
const SOURCE_IDENTITY_SCHEMA: &str = "eqiora.canonical-cartesian-poisson-cuda-source-identity/v1";

#[derive(Debug)]
struct MethodObservation {
    tag: &'static str,
    value_count: usize,
    values: Vec<f64>,
    producer_reason: &'static str,
    completed_iterations: usize,
    reported_residual_norm: f64,
    report: ReportObservation,
    l2_error: f64,
    boundary_quantity: f64,
    integrated_source: f64,
    relative_balance: f64,
    cpu_maximum_absolute_error: f64,
    cpu_maximum_scaled_error: f64,
    execution: ExecutionObservation,
    timings: TimingObservation,
}

#[derive(Debug)]
struct ReportObservation {
    backend: &'static str,
    execution_adapter: &'static str,
    execution_device: u16,
    verification_adapter: &'static str,
    verification_workers: usize,
    orientation: &'static str,
    algorithm: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    reason: &'static str,
    completed_iterations: usize,
    initial_residual_norm: f64,
    reported_residual_norm: f64,
    true_residual_norm: f64,
    residual_target: f64,
}

#[derive(Debug)]
struct TransferObservation {
    slot: &'static str,
    direction: &'static str,
    element_type: &'static str,
    elements: usize,
    bytes: usize,
    allocation: u64,
    completion_sequence: u64,
}

#[derive(Debug)]
struct ExecutionObservation {
    backend: &'static str,
    adapter: &'static str,
    runtime: &'static str,
    device: u16,
    logical_queue_slot: u32,
    queue_materialization: u64,
    operator_fingerprint_sha256: String,
    output_fingerprint_sha256: String,
    receipt_dimension: usize,
    minimum_device_payload_bytes: usize,
    external_sparse_workspace_bytes: usize,
    receipt_replay_adapter: &'static str,
    receipt_replay_workers: usize,
    dag: Vec<&'static str>,
    transfers: Vec<TransferObservation>,
    inputs_ready_sequence: u64,
    solve_visible_sequence: u64,
    solution_visible_sequence: u64,
    solution_allocation: u64,
    initial_generation: u64,
    solved_generation: u64,
    downloaded_generation: u64,
}

#[derive(Debug)]
struct TimingObservation {
    setup_ns: u128,
    host_to_device_ns: u128,
    solve_ns: u128,
    device_to_host_ns: u128,
    verification_ns: u128,
    total_ns: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HardwareObservation {
    runtime: &'static str,
    ordinal: u16,
    name: String,
    total_memory_bytes: u64,
    capabilities: Vec<&'static str>,
    compute_major: u16,
    compute_minor: u16,
    driver: i32,
    cusparse: i32,
    cublas: i32,
    cudarc: &'static str,
    binding_toolkit: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct SystemLoadObservation {
    one_minute: f64,
    five_minutes: f64,
    fifteen_minutes: f64,
}

#[derive(Debug)]
struct CollectedMethod {
    tag: &'static str,
    realization_json: Vec<u8>,
    run_json: Vec<u8>,
    observation: MethodObservation,
}

#[derive(Debug)]
struct Collection {
    model_json: Vec<u8>,
    source_identity: canonical::SourceIdentity,
    methods: Vec<CollectedMethod>,
    hardware: HardwareObservation,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("canonical CUDA evidence collection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("collection must be built with --release".to_owned());
    }
    let mut arguments = std::env::args_os().skip(1);
    let output = arguments.next().map(PathBuf::from).ok_or_else(|| {
        "usage: canonical_cartesian_poisson_cuda_collect <new-output-directory>".to_owned()
    })?;
    if arguments.next().is_some() {
        return Err("collector accepts exactly one output directory".to_owned());
    }
    if output.exists() {
        return Err(format!(
            "output directory {} already exists",
            output.display()
        ));
    }

    // The selector is used only to isolate the physical run. It is
    // deliberately absent from the persisted observation because a UUID or
    // PCI selector identifies the collection host.
    let selected_device = selected_visible_device()?;
    let source_commit = clean_source_commit()?;
    let system_load_before = system_load()?;
    let other_compute_process_count_before = gpu_compute_process_count(&selected_device)?;
    if other_compute_process_count_before != 0 {
        return Err("the selected device already has a compute process".to_owned());
    }

    let collection = collect()?;
    let environment = render_environment(
        &source_commit,
        &selected_device,
        system_load_before,
        other_compute_process_count_before,
        &collection.hardware,
    )?;
    persist(&output, &collection, &environment)
}

fn collect() -> Result<Collection, String> {
    let device_ordinal = 0;
    let device = CudaRuntime
        .discover()
        .map_err(|diagnostic| diagnostic.to_string())?
        .into_iter()
        .find(|candidate| candidate.id().ordinal() == device_ordinal)
        .ok_or_else(|| {
            "the single visible CUDA device was not discovered as ordinal zero".to_owned()
        })?;
    CudaLinearSolver::new(device_ordinal)
        .admit_device(&device)
        .map_err(|diagnostic| diagnostic.to_string())?;
    if CudaLinearSolver::capabilities() != canonical::cuda_solver_contract() {
        return Err(
            "the collector's fixed CUDA solver contract drifted from the adapter".to_owned(),
        );
    }

    let (program, source_identity) = canonical::compile_program_with_identity()?;
    let model =
        ModelEnvelope::from_program(&program).map_err(|diagnostic| diagnostic.to_string())?;
    let model_json = model
        .canonical_json()
        .map_err(|diagnostic| diagnostic.to_string())?;
    let capabilities = canonical::exact_capabilities(
        CudaLinearSolver::capabilities(),
        TargetCapabilities::none().with_cuda_device(device_ordinal),
    );
    let mut hardware = None;
    let mut methods = Vec::with_capacity(canonical::METHODS.len());

    for (revision, method, tag) in canonical::METHODS {
        let request = canonical::request(&program, method, device_ordinal, revision)?;
        let resolved = resolve(&request, canonical::requirements(), &capabilities)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let realization =
            RealizationEnvelopeV1::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)
                .map_err(|diagnostic| diagnostic.to_string())?;
        let (_, finalized) = finalize_resolved_scalar_elliptic_cartesian(&program, &resolved)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let binding = DeploymentBinding::bind_cuda(
            finalized.portable_realization(),
            CudaExecutorDescriptor::new(
                CUDA_LINEAR_SOLVER_PROVIDER,
                CUDA_LINEAR_EXECUTION_PROVIDER,
                device.clone(),
                QueueSlot::new(device.id(), 0),
                CudaLinearSolver::capabilities(),
            )
            .map_err(|diagnostic| diagnostic.to_string())?,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
        let admitted = AdmittedExecution::admit_cuda_linear(
            finalized.portable_realization(),
            finalized.canonical_csr_system_view(),
            binding,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
        let cuda = CudaLinearSolver::new(device_ordinal)
            .execute_admitted(admitted)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let (accepted, cuda_evidence) = cuda.into_parts();
        let observed_hardware = hardware_observation(&cuda_evidence)?;
        match &hardware {
            Some(expected) if expected != &observed_hardware => {
                return Err("device or CUDA library identity changed between methods".to_owned());
            }
            Some(_) => {}
            None => hardware = Some(observed_hardware),
        }
        let report = accepted.solution().report().clone();
        let (linear_solution, receipt) = accepted.into_parts();
        let solution = finalized
            .finish(linear_solution)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let metrics = canonical::method_metrics(method, &solution)?;
        let cpu = canonical::reference_cpu_solution(&program, method, revision + 100)?;
        let conformance = canonical::cpu_conformance(&cpu, &solution)?;

        let versions = cuda_evidence.versions();
        let compute = cuda_evidence.compute_capability();
        let native_libraries = [
            ("cusparse", versions.cusparse().to_string()),
            (
                "cublas",
                versions
                    .cublas()
                    .expect("the admitted CUDA Krylov path requires cuBLAS")
                    .to_string(),
            ),
        ];
        let execution = eqiora::artifact::ExecutionProvenanceV1::from_provider_releases(
            receipt.solver_provider(),
            receipt.execution_provider(),
            ExecutionTopologyV1::Cuda {
                device: device_ordinal,
                device_name: cuda_evidence.device().name().to_owned(),
                compute_capability_major: compute.major(),
                compute_capability_minor: compute.minor(),
                driver_version: versions.driver().to_string(),
            },
            eqiora::solver::ReductionPolicy::Fast,
            native_libraries,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
        let run = RunManifestV2::new(&realization, execution)
            .map_err(|diagnostic| diagnostic.to_string())?;
        let observation = method_observation(
            tag,
            &solution,
            &report,
            &receipt,
            &cuda_evidence,
            metrics,
            conformance,
        )?;
        methods.push(CollectedMethod {
            tag,
            realization_json: realization
                .canonical_json()
                .map_err(|diagnostic| diagnostic.to_string())?,
            run_json: run
                .canonical_json()
                .map_err(|diagnostic| diagnostic.to_string())?,
            observation,
        });
    }
    Ok(Collection {
        model_json,
        source_identity,
        methods,
        hardware: hardware.ok_or_else(|| "collector produced no method observations".to_owned())?,
    })
}

fn method_observation(
    tag: &'static str,
    solution: &ResolvedScalarEllipticCartesianSolution,
    report: &eqiora::solver::SolveReport,
    receipt: &ExecutionReceipt,
    cuda: &CudaLinearSolveEvidence,
    metrics: canonical::MethodMetrics,
    conformance: canonical::CpuConformance,
) -> Result<MethodObservation, String> {
    let reason = reason_name(report.reason());
    let execution_device = match report.execution().topology() {
        eqiora::solver::ExecutionTopology::Cuda { device } => device,
        _ => return Err("CUDA result carried a non-CUDA execution topology".to_owned()),
    };
    let verification_workers = match report.verification().topology() {
        eqiora::solver::ExecutionTopology::Host { workers } => workers.get(),
        _ => return Err("CUDA result carried a non-host verification topology".to_owned()),
    };
    let values = canonical::algebraic_values(solution);
    if receipt.dimension() != values.len() {
        return Err("receipt dimension differs from the reconstructed solution".to_owned());
    }
    let timings = cuda.timings();
    Ok(MethodObservation {
        tag,
        value_count: receipt.dimension(),
        values: values.to_vec(),
        producer_reason: reason,
        completed_iterations: report.completed_iterations(),
        reported_residual_norm: report.reported_residual_norm(),
        report: ReportObservation {
            backend: report.backend().as_str(),
            execution_adapter: report.execution().adapter().as_str(),
            execution_device,
            verification_adapter: report.verification().adapter().as_str(),
            verification_workers,
            orientation: "normal",
            algorithm: "conjugate-gradient",
            preconditioner: "jacobi",
            reduction: "fast",
            reason,
            completed_iterations: report.completed_iterations(),
            initial_residual_norm: report.initial_residual_norm(),
            reported_residual_norm: report.reported_residual_norm(),
            true_residual_norm: report.true_residual_norm(),
            residual_target: report.residual_target(),
        },
        l2_error: metrics.l2_error,
        boundary_quantity: metrics.boundary_quantity,
        integrated_source: metrics.integrated_source,
        relative_balance: metrics.relative_balance,
        cpu_maximum_absolute_error: conformance.maximum_absolute_error,
        cpu_maximum_scaled_error: conformance.maximum_scaled_error,
        execution: execution_observation(receipt, cuda, report)?,
        timings: TimingObservation {
            setup_ns: timings.setup().as_nanos(),
            host_to_device_ns: timings.host_to_device().as_nanos(),
            solve_ns: timings.solve().as_nanos(),
            device_to_host_ns: timings.device_to_host().as_nanos(),
            verification_ns: timings.verification().as_nanos(),
            total_ns: timings.total().as_nanos(),
        },
    })
}

fn execution_observation(
    receipt: &ExecutionReceipt,
    cuda: &CudaLinearSolveEvidence,
    report: &eqiora::solver::SolveReport,
) -> Result<ExecutionObservation, String> {
    let executor = receipt
        .binding()
        .cuda_executor()
        .ok_or_else(|| "CUDA receipt lost its device deployment binding".to_owned())?;
    let trace = receipt
        .cuda_trace()
        .ok_or_else(|| "CUDA receipt omitted its execution trace".to_owned())?;
    if executor.device() != cuda.device()
        || receipt.report() != report
        || receipt.report().verification() != eqiora::solver::ExecutionReport::host_serial()
        || receipt.acceptance_verification() != eqiora::solver::ExecutionReport::host_serial()
        || trace.external_sparse_workspace_bytes() != cuda.workspace_bytes()
        || trace.inputs_ready() != cuda.inputs_ready()
        || trace.solve_visible() != cuda.solve_visible()
        || trace.solution_visible() != cuda.solution_visible()
    {
        return Err("receipt and paired CUDA adapter evidence disagree".to_owned());
    }
    let receipt_replay_workers = match receipt.acceptance_verification().topology() {
        eqiora::solver::ExecutionTopology::Host { workers } => workers.get(),
        _ => return Err("receipt replay was not performed on a host verifier".to_owned()),
    };
    let transfers = trace.transfers();
    let adapter_transfers = cuda.transfers();
    if transfers.row_offsets() != adapter_transfers.row_offsets()
        || transfers.column_indices() != adapter_transfers.column_indices()
        || transfers.values() != adapter_transfers.values()
        || transfers.right_hand_side() != adapter_transfers.right_hand_side()
        || transfers.zero_initial_solution() != adapter_transfers.initial_guess()
        || transfers.inverse_diagonal() != adapter_transfers.inverse_diagonal()
        || transfers.complete_solution() != adapter_transfers.solution()
    {
        return Err("receipt and adapter carry different transfer evidence".to_owned());
    }
    let queue = trace.queue();
    if queue.slot() != executor.queue() {
        return Err("materialized queue differs from the deployment slot".to_owned());
    }
    let mut observed_transfers = Vec::with_capacity(7);
    observed_transfers.push(transfer_observation(
        "row-offsets",
        transfers.row_offsets(),
        queue,
    )?);
    observed_transfers.push(transfer_observation(
        "column-indices",
        transfers.column_indices(),
        queue,
    )?);
    observed_transfers.push(transfer_observation(
        "matrix-values",
        transfers.values(),
        queue,
    )?);
    observed_transfers.push(transfer_observation(
        "right-hand-side",
        transfers.right_hand_side(),
        queue,
    )?);
    observed_transfers.push(transfer_observation(
        "zero-initial-solution",
        transfers.zero_initial_solution(),
        queue,
    )?);
    if let Some(diagonal) = transfers.inverse_diagonal() {
        observed_transfers.push(transfer_observation(
            "jacobi-inverse-diagonal",
            diagonal,
            queue,
        )?);
    }
    observed_transfers.push(transfer_observation(
        "complete-solution",
        transfers.complete_solution(),
        queue,
    )?);

    Ok(ExecutionObservation {
        backend: executor.backend().as_str(),
        adapter: executor.adapter().as_str(),
        runtime: executor.device().id().runtime().as_str(),
        device: executor.device().id().ordinal(),
        logical_queue_slot: executor.queue().ordinal(),
        queue_materialization: queue.materialization().get(),
        operator_fingerprint_sha256: hex(receipt.operator().as_bytes()),
        output_fingerprint_sha256: hex(receipt.output().as_bytes()),
        receipt_dimension: receipt.dimension(),
        minimum_device_payload_bytes: receipt
            .minimum_device_payload_bytes()
            .ok_or_else(|| "CUDA receipt omitted its resident-payload lower bound".to_owned())?,
        external_sparse_workspace_bytes: trace.external_sparse_workspace_bytes(),
        receipt_replay_adapter: receipt.acceptance_verification().adapter().as_str(),
        receipt_replay_workers,
        dag: receipt
            .dag()
            .steps()
            .iter()
            .copied()
            .map(ExecutionStepKind::canonical_name)
            .collect(),
        transfers: observed_transfers,
        inputs_ready_sequence: completion_sequence(trace.inputs_ready().completion(), queue)?,
        solve_visible_sequence: completion_sequence(trace.solve_visible().completion(), queue)?,
        solution_visible_sequence: completion_sequence(
            trace.solution_visible().completion(),
            queue,
        )?,
        solution_allocation: trace.solved_solution().buffer().allocation().get(),
        initial_generation: trace.initial_solution().generation().get(),
        solved_generation: trace.solved_solution().generation().get(),
        downloaded_generation: trace.downloaded_solution().generation().get(),
    })
}

fn transfer_observation<T: DeviceElement>(
    slot: &'static str,
    transfer: TransferEvidence<T>,
    queue: QueueId,
) -> Result<TransferObservation, String> {
    let plan = transfer.plan();
    let buffer = match (plan.direction(), plan.source(), plan.destination()) {
        (TransferDirection::HostToDevice, MemoryRegion::Host(_), MemoryRegion::Device(buffer))
        | (TransferDirection::DeviceToHost, MemoryRegion::Device(buffer), MemoryRegion::Host(_)) => {
            buffer
        }
        _ => return Err(format!("{slot} has unsupported transfer endpoints")),
    };
    Ok(TransferObservation {
        slot,
        direction: match plan.direction() {
            TransferDirection::HostToDevice => "host-to-device",
            TransferDirection::DeviceToHost => "device-to-host",
            TransferDirection::DeviceToDevice => "device-to-device",
        },
        element_type: match T::ELEMENT_TYPE {
            DeviceElementType::SignedIndex64 => "signed-index-64",
            DeviceElementType::Scalar(eqiora::solver::ScalarType::F32) => "f32",
            DeviceElementType::Scalar(eqiora::solver::ScalarType::F64) => "f64",
        },
        elements: buffer.elements().get(),
        bytes: plan.bytes().map_err(|diagnostic| diagnostic.to_string())?,
        allocation: buffer.id().allocation().get(),
        completion_sequence: completion_sequence(transfer.completion(), queue)?,
    })
}

fn completion_sequence(completion: Completion, queue: QueueId) -> Result<u64, String> {
    let submission = completion.submission();
    if submission.queue() != queue {
        return Err("execution observation mixed materialized queues".to_owned());
    }
    let sequence = submission.sequence().get();
    if sequence > 64 {
        return Err("bounded CUDA evidence exceeded 64 queue submissions".to_owned());
    }
    Ok(sequence)
}

fn hex(bytes: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(value, "{byte:02x}").expect("String writes cannot fail");
    }
    value
}

fn hardware_observation(cuda: &CudaLinearSolveEvidence) -> Result<HardwareObservation, String> {
    let descriptor = cuda.device();
    let versions = cuda.versions();
    let cublas = versions
        .cublas()
        .ok_or_else(|| "CUDA Krylov evidence omitted cuBLAS".to_owned())?;
    if versions.cudarc() != CUDARC_VERSION || versions.binding_toolkit() != CUDA_BINDING_TOOLKIT {
        return Err("live CUDA dependency identity differs from the compiled adapter".to_owned());
    }
    Ok(HardwareObservation {
        runtime: descriptor.id().runtime().as_str(),
        ordinal: descriptor.id().ordinal(),
        name: descriptor.name().to_owned(),
        total_memory_bytes: descriptor.total_memory_bytes().get(),
        capabilities: descriptor
            .capabilities()
            .iter()
            .copied()
            .map(capability_name)
            .collect(),
        compute_major: cuda.compute_capability().major(),
        compute_minor: cuda.compute_capability().minor(),
        driver: versions.driver(),
        cusparse: versions.cusparse(),
        cublas,
        cudarc: versions.cudarc(),
        binding_toolkit: versions.binding_toolkit(),
    })
}

fn capability_name(capability: DeviceCapability) -> &'static str {
    match capability {
        DeviceCapability::Float32 => "float32",
        DeviceCapability::Float64 => "float64",
        DeviceCapability::CsrMatrixVectorProduct => "csr-matrix-vector-product",
        DeviceCapability::DenseVectorLevel1 => "dense-vector-level-1",
        DeviceCapability::AsynchronousQueue => "asynchronous-queue",
    }
}

fn reason_name(reason: ConvergenceReason) -> &'static str {
    match reason {
        ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
        ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
    }
}

fn persist(output: &Path, collection: &Collection, environment: &str) -> Result<(), String> {
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "output directory requires a UTF-8 final component".to_owned())?;
    let staging = parent.join(format!(".{name}.staging-{}", std::process::id()));
    if staging.exists() {
        return Err(format!(
            "staging directory {} already exists",
            staging.display()
        ));
    }
    let write_result = (|| {
        fs::create_dir(&staging)?;
        fs::create_dir(staging.join("artifacts"))?;
        fs::create_dir(staging.join("observations"))?;
        fs::write(staging.join("artifacts/model.json"), &collection.model_json)?;
        for method in &collection.methods {
            fs::write(
                staging.join(format!("artifacts/{}-realization.json", method.tag)),
                &method.realization_json,
            )?;
            fs::write(
                staging.join(format!("artifacts/{}-run.json", method.tag)),
                &method.run_json,
            )?;
        }
        fs::write(
            staging.join("observations/solutions.json"),
            render_solutions(&collection.methods),
        )?;
        fs::write(
            staging.join("observations/source-identity.json"),
            render_source_identity(&collection.source_identity),
        )?;
        fs::write(staging.join("observations/environment.json"), environment)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("cannot write staged collection: {error}"));
    }
    if let Err(error) = fs::rename(&staging, output) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("cannot publish collection atomically: {error}"));
    }
    Ok(())
}

fn render_source_identity(identity: &canonical::SourceIdentity) -> String {
    let mut output = format!(
        concat!(
            "{{\n",
            "  \"schema\": \"{}\",\n",
            "  \"raw_compiler_model_ulid\": \"{}\",\n",
            "  \"symbols\": [\n"
        ),
        SOURCE_IDENTITY_SCHEMA,
        json_escape(&identity.model_ulid),
    );
    for (index, symbol) in identity.symbols.iter().enumerate() {
        writeln!(
            output,
            "    {{\"name\":\"{}\",\"kind\":\"{}\",\"ulid\":\"{}\"}}{}",
            json_escape(&symbol.name),
            json_escape(symbol.kind),
            json_escape(&symbol.ulid),
            if index + 1 == identity.symbols.len() {
                ""
            } else {
                ","
            }
        )
        .expect("String writes cannot fail");
    }
    output.push_str("  ]\n}\n");
    output
}

fn render_solutions(methods: &[CollectedMethod]) -> String {
    let mut output = format!("{{\n  \"schema\": \"{SCHEMA}\",\n  \"methods\": [\n");
    for (method_index, method) in methods.iter().enumerate() {
        let value = &method.observation;
        writeln!(output, "    {{").expect("String writes cannot fail");
        writeln!(output, "      \"method\": \"{}\",", value.tag)
            .expect("String writes cannot fail");
        writeln!(output, "      \"value_count\": {},", value.value_count)
            .expect("String writes cannot fail");
        output.push_str("      \"values\": [");
        for (index, value) in value.values.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            write!(output, "{:.17e}", normalize_zero(*value)).expect("String writes cannot fail");
        }
        output.push_str("],\n");
        writeln!(
            output,
            "      \"producer\": {{\"reason\":\"{}\",\"completed_iterations\":{},\"reported_residual_norm\":{:.17e}}},",
            value.producer_reason,
            value.completed_iterations,
            normalize_zero(value.reported_residual_norm),
        )
        .expect("String writes cannot fail");
        let report = &value.report;
        writeln!(
            output,
            concat!(
                "      \"accepted_report\": {{",
                "\"backend\":\"{}\",\"execution_adapter\":\"{}\",\"execution_device\":{},",
                "\"verification_adapter\":\"{}\",\"verification_workers\":{},",
                "\"orientation\":\"{}\",\"algorithm\":\"{}\",",
                "\"preconditioner\":\"{}\",\"reduction\":\"{}\",\"reason\":\"{}\",",
                "\"completed_iterations\":{},\"initial_residual_norm\":{:.17e},",
                "\"reported_residual_norm\":{:.17e},\"true_residual_norm\":{:.17e},",
                "\"residual_target\":{:.17e}}},"
            ),
            report.backend,
            report.execution_adapter,
            report.execution_device,
            report.verification_adapter,
            report.verification_workers,
            report.orientation,
            report.algorithm,
            report.preconditioner,
            report.reduction,
            report.reason,
            report.completed_iterations,
            normalize_zero(report.initial_residual_norm),
            normalize_zero(report.reported_residual_norm),
            normalize_zero(report.true_residual_norm),
            normalize_zero(report.residual_target),
        )
        .expect("String writes cannot fail");
        writeln!(
            output,
            concat!(
                "      \"method_metrics\": {{\"l2_error\":{:.17e},",
                "\"boundary_quantity\":{:.17e},\"integrated_source\":{:.17e},",
                "\"relative_balance\":{:.17e}}},"
            ),
            normalize_zero(value.l2_error),
            normalize_zero(value.boundary_quantity),
            normalize_zero(value.integrated_source),
            normalize_zero(value.relative_balance),
        )
        .expect("String writes cannot fail");
        writeln!(
            output,
            "      \"cpu_conformance\": {{\"maximum_absolute_error\":{:.17e},\"maximum_scaled_error\":{:.17e}}},",
            normalize_zero(value.cpu_maximum_absolute_error),
            normalize_zero(value.cpu_maximum_scaled_error),
        )
        .expect("String writes cannot fail");
        render_execution(&mut output, &value.execution);
        writeln!(
            output,
            concat!(
                "      \"timings_ns\": {{\"setup\":{},\"host_to_device\":{},",
                "\"solve\":{},\"device_to_host\":{},\"verification\":{},\"total\":{}}}"
            ),
            value.timings.setup_ns,
            value.timings.host_to_device_ns,
            value.timings.solve_ns,
            value.timings.device_to_host_ns,
            value.timings.verification_ns,
            value.timings.total_ns,
        )
        .expect("String writes cannot fail");
        writeln!(
            output,
            "    }}{}",
            if method_index + 1 == methods.len() {
                ""
            } else {
                ","
            }
        )
        .expect("String writes cannot fail");
    }
    output.push_str("  ]\n}\n");
    output
}

fn render_execution(output: &mut String, execution: &ExecutionObservation) {
    writeln!(output, "      \"execution\": {{").expect("String writes cannot fail");
    writeln!(
        output,
        concat!(
            "        \"deployment\": {{\"backend\":\"{}\",\"adapter\":\"{}\",",
            "\"runtime\":\"{}\",\"device\":{},\"logical_queue_slot\":{}}},"
        ),
        execution.backend,
        execution.adapter,
        execution.runtime,
        execution.device,
        execution.logical_queue_slot,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"queue_materialization\": {},",
        execution.queue_materialization,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"operator_fingerprint_sha256\": \"{}\",",
        execution.operator_fingerprint_sha256,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"output_fingerprint_sha256\": \"{}\",",
        execution.output_fingerprint_sha256,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"receipt_dimension\": {},",
        execution.receipt_dimension,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"minimum_device_payload_bytes\": {},",
        execution.minimum_device_payload_bytes,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"external_sparse_workspace_bytes\": {},",
        execution.external_sparse_workspace_bytes,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        "        \"receipt_replay\": {{\"adapter\":\"{}\",\"workers\":{}}},",
        execution.receipt_replay_adapter, execution.receipt_replay_workers,
    )
    .expect("String writes cannot fail");
    output.push_str("        \"dag\": [");
    for (index, step) in execution.dag.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        write!(output, "\"{step}\"").expect("String writes cannot fail");
    }
    output.push_str("],\n        \"transfers\": [\n");
    for (index, transfer) in execution.transfers.iter().enumerate() {
        writeln!(
            output,
            concat!(
                "          {{\"slot\":\"{}\",\"direction\":\"{}\",",
                "\"element_type\":\"{}\",\"elements\":{},\"bytes\":{},",
                "\"allocation\":{},\"completion_sequence\":{}}}{}"
            ),
            transfer.slot,
            transfer.direction,
            transfer.element_type,
            transfer.elements,
            transfer.bytes,
            transfer.allocation,
            transfer.completion_sequence,
            if index + 1 == execution.transfers.len() {
                ""
            } else {
                ","
            },
        )
        .expect("String writes cannot fail");
    }
    writeln!(output, "        ],").expect("String writes cannot fail");
    writeln!(
        output,
        concat!(
            "        \"waited_fences\": {{\"inputs_ready_sequence\":{},",
            "\"solve_visible_sequence\":{},\"solution_visible_sequence\":{}}},"
        ),
        execution.inputs_ready_sequence,
        execution.solve_visible_sequence,
        execution.solution_visible_sequence,
    )
    .expect("String writes cannot fail");
    writeln!(
        output,
        concat!(
            "        \"solution_generations\": {{\"allocation\":{},",
            "\"initial\":{},\"solved\":{},\"downloaded\":{}}}"
        ),
        execution.solution_allocation,
        execution.initial_generation,
        execution.solved_generation,
        execution.downloaded_generation,
    )
    .expect("String writes cannot fail");
    writeln!(output, "      }},").expect("String writes cannot fail");
}

fn render_environment(
    source_commit: &str,
    selected_device: &str,
    system_load_before: SystemLoadObservation,
    other_compute_process_count_before: usize,
    hardware: &HardwareObservation,
) -> Result<String, String> {
    let rustc = command_output("rustc", &["--version", "--verbose"])?;
    let other_compute_process_count_after = gpu_compute_process_count(selected_device)?;
    if other_compute_process_count_after != 0 {
        return Err("the selected device gained a compute process during collection".to_owned());
    }
    let system_load_after = system_load()?;
    let mut capabilities = String::new();
    for (index, capability) in hardware.capabilities.iter().enumerate() {
        if index > 0 {
            capabilities.push(',');
        }
        write!(capabilities, "\"{}\"", json_escape(capability)).expect("String writes cannot fail");
    }
    Ok(format!(
        concat!(
            "{{\n",
            "  \"schema\": \"{}\",\n",
            "  \"source_commit\": \"{}\",\n",
            "  \"source_clean\": true,\n",
            "  \"profile\": \"release\",\n",
            "  \"rustc\": \"{}\",\n",
            "  \"target_arch\": \"{}\",\n",
            "  \"logical_cpu_count\": {},\n",
            "  \"selected_device_count\": 1,\n",
            "  \"eqiora_device_ordinal\": 0,\n",
            "  \"adapter_version\": \"{}\",\n",
            "  \"system_load_before\": {{\"one_minute\":{},\"five_minutes\":{},",
            "\"fifteen_minutes\":{}}},\n",
            "  \"system_load_after\": {{\"one_minute\":{},\"five_minutes\":{},",
            "\"fifteen_minutes\":{}}},\n",
            "  \"other_compute_process_count_before\": {},\n",
            "  \"other_compute_process_count_after\": {},\n",
            "  \"device\": {{\"runtime\":\"{}\",\"ordinal\":{},\"name\":\"{}\",",
            "\"total_memory_bytes\":{},\"capabilities\":[{}],",
            "\"compute_capability_major\":{},\"compute_capability_minor\":{}}},\n",
            "  \"libraries\": {{\"driver\":{},\"cusparse\":{},\"cublas\":{},",
            "\"cudarc\":\"{}\",\"binding_toolkit\":\"{}\"}},\n",
            "  \"observation_kind\": \"selected-device-run; not hardware-attestation\"\n",
            "}}\n"
        ),
        ENVIRONMENT_SCHEMA,
        json_escape(source_commit),
        json_escape(&rustc),
        std::env::consts::ARCH,
        std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1),
        json_escape(CUDA_ADAPTER_VERSION),
        system_load_before.one_minute,
        system_load_before.five_minutes,
        system_load_before.fifteen_minutes,
        system_load_after.one_minute,
        system_load_after.five_minutes,
        system_load_after.fifteen_minutes,
        other_compute_process_count_before,
        other_compute_process_count_after,
        json_escape(hardware.runtime),
        hardware.ordinal,
        json_escape(&hardware.name),
        hardware.total_memory_bytes,
        capabilities,
        hardware.compute_major,
        hardware.compute_minor,
        hardware.driver,
        hardware.cusparse,
        hardware.cublas,
        json_escape(hardware.cudarc),
        json_escape(hardware.binding_toolkit),
    ))
}

fn selected_visible_device() -> Result<String, String> {
    let value = std::env::var("CUDA_VISIBLE_DEVICES")
        .map_err(|_| "CUDA_VISIBLE_DEVICES must select one physical device".to_owned())?;
    if value.trim().is_empty() || value.contains(',') || value.chars().any(char::is_whitespace) {
        return Err("CUDA_VISIBLE_DEVICES must contain exactly one device selector".to_owned());
    }
    Ok(value)
}

fn clean_source_commit() -> Result<String, String> {
    let commit = command_output("git", &["rev-parse", "HEAD"])?;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("git HEAD must be one full lowercase 40-hex commit".to_owned());
    }
    if !command_output("git", &["status", "--porcelain"])?.is_empty() {
        return Err("source worktree must be clean before collection".to_owned());
    }
    Ok(commit)
}

fn command_output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot execute {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("{program} output is not UTF-8: {error}"))
}

fn gpu_compute_process_count(device: &str) -> Result<usize, String> {
    let output = command_output(
        "nvidia-smi",
        &[
            "--query-compute-apps=used_gpu_memory",
            "--format=csv,noheader,nounits",
            "-i",
            device,
        ],
    )?;
    Ok(output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count())
}

fn system_load() -> Result<SystemLoadObservation, String> {
    let source = fs::read_to_string("/proc/loadavg")
        .map_err(|error| format!("cannot read system load: {error}"))?;
    let mut values = source.split_whitespace();
    Ok(SystemLoadObservation {
        one_minute: parse_system_load(values.next(), "one minute")?,
        five_minutes: parse_system_load(values.next(), "five minutes")?,
        fifteen_minutes: parse_system_load(values.next(), "fifteen minutes")?,
    })
}

fn parse_system_load(value: Option<&str>, interval: &str) -> Result<f64, String> {
    let value = value
        .ok_or_else(|| format!("system load is missing the {interval} interval"))?
        .parse::<f64>()
        .map_err(|error| format!("{interval} system load is not numeric: {error}"))?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(format!(
            "{interval} system load must be finite and non-negative"
        ))
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", character as u32).expect("String writes cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped
}
