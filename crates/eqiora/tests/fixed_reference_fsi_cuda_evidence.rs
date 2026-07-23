#![cfg(feature = "cuda")]

#[path = "support/fixed_reference_fsi_cuda_observation.rs"]
mod observation;

use std::fs;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};

use eqiora::artifact::{
    DecoderLimits, ExecutionTopologyV1, LayoutArtifacts, ModelEnvelopeV4, RealizationEnvelopeV3,
    RunManifestV2,
};
use eqiora::backends::cuda::{
    CUDA_LINEAR_EXECUTION, CUDA_LINEAR_EXECUTION_PROVIDER, CUDA_LINEAR_SOLVER_PROVIDER,
    CUDA_RUNTIME_ID, CudaLinearSolver,
};
use eqiora::device::{DeviceDescriptor, DeviceId, QueueSlot};
use eqiora::numerics::{
    ResolvedFixedReferenceFsiSolution2d, finalize_resolved_fixed_reference_fsi_step_2d,
    lower_fixed_reference_fsi_cartesian_2d,
};
use eqiora::realization::{
    AlgebraicBlock, CoupledFieldwiseRealizationRequest, DiscretizationMethod, MeshKind,
    RealizationCapabilities, ScalarType, SpatialDimensionSupport, TargetCapabilities,
    VectorLayoutKind, resolve_coupled_fieldwise,
};
use eqiora::solver::{
    ConvergenceReason, ExecutionReport, LinearOperatorProperties, LinearSolver,
    LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    SERIAL_LINEAR_EXECUTION, SolverCapabilities, SolverCapability, SolverPlan,
    accept_linear_solution_with_verifier,
};
use eqiora_execution::{
    CUDA_LINEAR_DEVICE_CAPABILITIES, CudaExecutorDescriptor, DeploymentBinding,
};
use observation::{
    Conformance, MAX_ARTIFACT_BYTES, MAX_OBSERVATION_BYTES, Observation, Physics, error_pair,
    require_same_float,
};
use sha2::{Digest, Sha256};
use support::fixed_reference_fsi::{execution_context, prestrained_state, spatial_context};

mod support;

const REGISTERED_SOURCE_COMMIT: &str = "5696f62ed84eba5457e2ff99f40fd2080c808d69";

#[test]
fn observation_decoder_is_closed_and_bounded() {
    let valid = synthetic_observation();
    valid.validate().expect("the bounded probe is valid");
    let bytes = serde_json::to_vec(&valid).expect("serialize bounded probe");
    let decoded: Observation = decode_closed(&bytes).expect("decode bounded probe");
    assert_eq!(decoded, valid);

    let mut unknown = serde_json::to_value(&valid).expect("probe JSON value");
    unknown["unknown"] = serde_json::json!(true);
    assert!(decode_closed::<Observation>(&serde_json::to_vec(&unknown).unwrap()).is_err());
    assert!(decode_closed::<Observation>(&bytes[..bytes.len() - 1]).is_err());

    let mut negative_zero = valid.clone();
    negative_zero.values[0] = -0.0;
    assert!(negative_zero.validate().is_err());
    assert!(
        read_bounded_bytes(
            &vec![b' '; MAX_OBSERVATION_BYTES + 1],
            MAX_OBSERVATION_BYTES
        )
        .is_err()
    );
}

#[test]
fn committed_fixed_reference_fsi_cuda_observation_replays_on_the_host() {
    replay_committed_observation().expect("the bounded CUDA FSI observation replays");
}

fn replay_committed_observation() -> Result<(), String> {
    let root = case_root();
    let observation_bytes = read_bounded(
        &root.join("observations/observation.json"),
        MAX_OBSERVATION_BYTES,
    )?;
    let observed: Observation = decode_closed(&observation_bytes)?;
    observed.validate()?;
    if observed.source_commit != REGISTERED_SOURCE_COMMIT {
        return Err("source commit differs from the registered public source".to_owned());
    }

    let model_bytes = read_bounded(&root.join("artifacts/model.json"), MAX_ARTIFACT_BYTES)?;
    let model = ModelEnvelopeV4::from_json(&model_bytes, DecoderLimits::default())
        .map_err(|diagnostic| diagnostic.to_string())?;
    require_equal_bytes(
        "decoded Model canonical bytes",
        &model
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &model_bytes,
    )?;
    let program = model
        .to_program()
        .map_err(|diagnostics| format!("cannot replay Model: {diagnostics:?}"))?;
    let canonical = lower_fixed_reference_fsi_cartesian_2d(&program)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let spatial = spatial_context(&program, &canonical);
    let host = execution_context(&program, &canonical, &spatial);
    let previous = prestrained_state(&spatial);

    let realization_bytes = read_bounded(
        &root.join("artifacts/cuda-realization.json"),
        MAX_ARTIFACT_BYTES,
    )?;
    let recorded_realization =
        RealizationEnvelopeV3::from_json(&realization_bytes, DecoderLimits::default())
            .map_err(|diagnostic| diagnostic.to_string())?;
    recorded_realization
        .validate_model_artifact(&model)
        .map_err(|diagnostic| diagnostic.to_string())?;
    recorded_realization
        .validate_mesh_artifact(&spatial.mesh_artifact)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let resolved = resolve_recorded_cuda(&program, &recorded_realization, &observed)?;
    let fresh_realization =
        RealizationEnvelopeV3::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)
            .map_err(|diagnostic| diagnostic.to_string())?;
    require_equal_bytes(
        "fresh and recorded coupled Realization canonical bytes",
        &fresh_realization
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &realization_bytes,
    )?;

    let run_bytes = read_bounded(&root.join("artifacts/cuda-run.json"), MAX_ARTIFACT_BYTES)?;
    let recorded_run = RunManifestV2::from_json(&run_bytes, DecoderLimits::default())
        .map_err(|diagnostic| diagnostic.to_string())?;
    recorded_run
        .validate_against(&fresh_realization)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let fresh_run = run_from_observation(&fresh_realization, &observed)?;
    require_equal_bytes(
        "fresh and recorded Run canonical bytes",
        &fresh_run
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &run_bytes,
    )?;
    if !recorded_run.outputs().is_empty() {
        return Err("the bounded observation cannot imply a durable result artifact".to_owned());
    }

    let host_finalized = finalize_resolved_fixed_reference_fsi_step_2d(
        &canonical,
        &host.resolved,
        host.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        &previous,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let cuda_finalized = finalize_resolved_fixed_reference_fsi_step_2d(
        &canonical,
        &resolved,
        host.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        &previous,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let fingerprint = cuda_finalized.linear_system().agreement_fingerprint();
    if fingerprint != host_finalized.linear_system().agreement_fingerprint()
        || hex(fingerprint.as_bytes()) != observed.operator_sha256
    {
        return Err("CPU, CUDA, or recorded finalized CSR/RHS identity differs".to_owned());
    }
    let plan = cuda_finalized.solver_plan();
    if plan.algorithm() != LinearSolver::MinimumResidual
        || plan.preconditioner() != PreconditionerPolicy::Identity
        || plan.reduction() != ReductionPolicy::Fast
    {
        return Err("recorded Realization changed the exact CUDA MINRES tuple".to_owned());
    }
    let device = synthetic_device(&observed)?;
    let admitted = cuda_finalized
        .admit_cuda(
            DeploymentBinding::bind_cuda(
                cuda_finalized.realization_graph(),
                CudaExecutorDescriptor::new(
                    CUDA_LINEAR_SOLVER_PROVIDER,
                    CUDA_LINEAR_EXECUTION_PROVIDER,
                    device.clone(),
                    QueueSlot::new(device.id(), 0),
                    CudaLinearSolver::capabilities(),
                )
                .map_err(|diagnostic| diagnostic.to_string())?,
            )
            .map_err(|diagnostic| diagnostic.to_string())?,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    if admitted.minimum_device_payload_bytes()
        != Some(observed.receipt.minimum_device_payload_bytes)
        || admitted.solver_plan() != plan
        || admitted.system().agreement_fingerprint() != fingerprint
        || observed.receipt.dimension != cuda_finalized.linear_system().columns()
    {
        return Err("recorded receipt differs from host-reconstructed CUDA admission".to_owned());
    }
    if accepted_output_sha256(&observed.values)? != observed.output_sha256 {
        return Err("recorded output identity differs from the recorded coefficients".to_owned());
    }

    let cpu_linear = REFERENCE_LINEAR_SOLVER
        .solve(
            &host_finalized
                .linear_system()
                .linear_problem()
                .map_err(|diagnostic| diagnostic.to_string())?,
            host_finalized.solver_plan(),
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    let cpu = host_finalized
        .finish(cpu_linear)
        .map_err(|diagnostic| diagnostic.to_string())?;

    let recorded_linear = accept_linear_solution_with_verifier(
        &cuda_finalized
            .linear_system()
            .linear_problem()
            .map_err(|diagnostic| diagnostic.to_string())?,
        cuda_finalized.solver_plan(),
        CUDA_LINEAR_SOLVER_PROVIDER,
        CUDA_LINEAR_EXECUTION_PROVIDER,
        ExecutionReport::cuda(CUDA_LINEAR_EXECUTION, observed.environment.device_ordinal),
        ConvergenceReason::ResidualToleranceSatisfied,
        observed.producer.completed_iterations,
        observed.producer.reported_residual_norm,
        observed.values.clone(),
        &SERIAL_LINEAR_EXECUTION,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    require_same_float(
        "recorded host true residual",
        recorded_linear.report().true_residual_norm(),
        observed.producer.true_residual_norm,
    )?;
    let cuda = cuda_finalized
        .finish(recorded_linear)
        .map_err(|diagnostic| diagnostic.to_string())?;
    compare_physics(&cuda, &observed.physics)?;
    compare_conformance(&cpu, &cuda, &observed.conformance)?;

    let mut forged = observed.values.clone();
    forged[0] += 1.0e-3;
    if accept_linear_solution_with_verifier(
        &finalize_resolved_fixed_reference_fsi_step_2d(
            &canonical,
            &resolved,
            host.mesh_reference,
            &spatial.mesh,
            &spatial.partition,
            &previous,
        )
        .map_err(|diagnostic| diagnostic.to_string())?
        .linear_system()
        .linear_problem()
        .map_err(|diagnostic| diagnostic.to_string())?,
        recorded_realization
            .plan()
            .map_err(|diagnostic| diagnostic.to_string())?
            .solver(),
        CUDA_LINEAR_SOLVER_PROVIDER,
        CUDA_LINEAR_EXECUTION_PROVIDER,
        ExecutionReport::cuda(CUDA_LINEAR_EXECUTION, observed.environment.device_ordinal),
        ConvergenceReason::ResidualToleranceSatisfied,
        observed.producer.completed_iterations,
        observed.producer.reported_residual_norm,
        forged,
        &SERIAL_LINEAR_EXECUTION,
    )
    .is_ok()
    {
        return Err("a perturbed recorded candidate passed host reacceptance".to_owned());
    }
    Ok(())
}

fn resolve_recorded_cuda(
    program: &eqiora::sem::KernelProgram,
    realization: &RealizationEnvelopeV3,
    observed: &Observation,
) -> Result<eqiora::realization::ResolvedCoupledFieldwiseRealization, String> {
    let device = observed.environment.device_ordinal;
    let plan = realization
        .plan()
        .map_err(|diagnostic| diagnostic.to_string())?;
    let solver_plan = plan.solver();
    let request = CoupledFieldwiseRealizationRequest::explicit(
        program.model(),
        realization.semantic_revision(),
        realization.realization_revision(),
        plan,
    );
    resolve_coupled_fieldwise(
        &request,
        realization
            .requirements()
            .map_err(|diagnostic| diagnostic.to_string())?,
        &cuda_capabilities(device, solver_plan)?,
    )
    .map_err(|diagnostic| diagnostic.to_string())
}

fn cuda_capabilities(device: u16, plan: SolverPlan) -> Result<RealizationCapabilities, String> {
    CudaLinearSolver::capabilities()
        .require_problem(
            plan,
            ScalarType::F64,
            LinearOperatorProperties::SymmetricIndefinite,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    let solver = SolverCapabilities::exact([SolverCapability {
        algorithm: plan.algorithm(),
        operator_properties: LinearOperatorProperties::SymmetricIndefinite,
        preconditioner: plan.preconditioner(),
        reduction: plan.reduction(),
        scalar_type: ScalarType::F64,
    }])
    .map_err(|diagnostic| diagnostic.to_string())?;
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two is non-zero")),
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_cuda_device(device),
    )
    .map_err(|diagnostic| diagnostic.to_string())
}

fn run_from_observation(
    realization: &RealizationEnvelopeV3,
    observed: &Observation,
) -> Result<RunManifestV2, String> {
    let environment = &observed.environment;
    validate_recorded_provider_environment(environment)?;
    let native_libraries = [
        ("cusparse", environment.cusparse.to_string()),
        ("cublas", environment.cublas.to_string()),
    ];
    let provenance = eqiora::artifact::ExecutionProvenanceV1::from_provider_releases(
        CUDA_LINEAR_SOLVER_PROVIDER,
        CUDA_LINEAR_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Cuda {
            device: environment.device_ordinal,
            device_name: environment.device_name.clone(),
            compute_capability_major: environment.compute_capability_major,
            compute_capability_minor: environment.compute_capability_minor,
            driver_version: environment.driver.to_string(),
        },
        ReductionPolicy::Fast,
        native_libraries,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    RunManifestV2::new(realization, provenance).map_err(|diagnostic| diagnostic.to_string())
}

fn validate_recorded_provider_environment(
    environment: &observation::Environment,
) -> Result<(), String> {
    if environment.runtime != CUDA_RUNTIME_ID.as_str()
        || environment.adapter_version != CUDA_LINEAR_SOLVER_PROVIDER.implementation_version()
        || environment.adapter_version != CUDA_LINEAR_EXECUTION_PROVIDER.implementation_version()
    {
        return Err(
            "recorded CUDA provider identity differs from the declared provider release".to_owned(),
        );
    }
    for (name, recorded) in [
        ("cudarc", environment.cudarc.as_str()),
        ("cuda-binding-toolkit", environment.binding_toolkit.as_str()),
    ] {
        let expected = CUDA_LINEAR_EXECUTION_PROVIDER
            .libraries()
            .iter()
            .find(|library| library.name() == name)
            .map(|library| library.version());
        if expected != Some(recorded) {
            return Err(format!(
                "recorded CUDA provider library `{name}` differs from the declared provider release"
            ));
        }
    }
    Ok(())
}

fn synthetic_device(observed: &Observation) -> Result<DeviceDescriptor, String> {
    DeviceDescriptor::new(
        DeviceId::new(CUDA_RUNTIME_ID, observed.environment.device_ordinal),
        observed.environment.device_name.clone(),
        NonZeroU64::new(observed.environment.total_memory_bytes)
            .ok_or_else(|| "recorded device memory is zero".to_owned())?,
        CUDA_LINEAR_DEVICE_CAPABILITIES,
    )
    .map_err(|diagnostic| diagnostic.to_string())
}

fn compare_physics(
    solution: &ResolvedFixedReferenceFsiSolution2d,
    expected: &Physics,
) -> Result<(), String> {
    let actual = solution.numerical_evidence();
    for (label, actual, expected) in [
        (
            "residual norm",
            actual.residual_norm(),
            expected.residual_norm,
        ),
        (
            "continuity residual norm",
            actual.continuity_residual_norm(),
            expected.continuity_residual_norm,
        ),
        (
            "kinematic residual norm",
            actual.kinematic_residual_norm(),
            expected.kinematic_residual_norm,
        ),
        (
            "interface velocity jump norm",
            actual.interface_velocity_jump_norm(),
            expected.interface_velocity_jump_norm,
        ),
        (
            "interface action imbalance norm",
            actual.interface_action_imbalance_norm(),
            expected.interface_action_imbalance_norm,
        ),
        (
            "energy defect",
            actual.energy_balance().defect(),
            expected.energy_defect,
        ),
    ] {
        require_same_float(label, actual, expected)?;
    }
    Ok(())
}

fn compare_conformance(
    cpu: &ResolvedFixedReferenceFsiSolution2d,
    cuda: &ResolvedFixedReferenceFsiSolution2d,
    expected: &Conformance,
) -> Result<(), String> {
    require_same_field_layout(cpu, cuda)?;
    let velocity_scale = field_scale(cpu, cpu.fields().fluid_velocity())?;
    if velocity_scale != field_scale(cpu, cpu.fields().solid_velocity())? {
        return Err("fluid and solid velocity scales differ".to_owned());
    }
    let actual = Conformance {
        algebraic: error_pair(
            cpu.numerical_evidence().algebraic_values().iter().copied(),
            cuda.numerical_evidence().algebraic_values().iter().copied(),
        )?,
        vertex_velocity_over_u: error_pair(
            flatten_scaled(cpu.vertex_velocity_coefficients(), velocity_scale),
            flatten_scaled(cuda.vertex_velocity_coefficients(), velocity_scale),
        )?,
        bubble_velocity_over_u: error_pair(
            flatten_scaled(cpu.fluid_velocity_bubble_coefficients(), velocity_scale),
            flatten_scaled(cuda.fluid_velocity_bubble_coefficients(), velocity_scale),
        )?,
        pressure_over_p: error_pair(
            cpu.fluid_pressure_coefficients()
                .iter()
                .map(|value| value / field_scale(cpu, cpu.fields().fluid_pressure()).unwrap()),
            cuda.fluid_pressure_coefficients()
                .iter()
                .map(|value| value / field_scale(cpu, cpu.fields().fluid_pressure()).unwrap()),
        )?,
        displacement_over_l: error_pair(
            flatten_scaled(
                cpu.solid_displacement_coefficients(),
                displacement_scale(cpu),
            ),
            flatten_scaled(
                cuda.solid_displacement_coefficients(),
                displacement_scale(cpu),
            ),
        )?,
    };
    for (label, actual, expected) in [
        ("algebraic", actual.algebraic, expected.algebraic),
        (
            "vertex velocity / U",
            actual.vertex_velocity_over_u,
            expected.vertex_velocity_over_u,
        ),
        (
            "bubble velocity / U",
            actual.bubble_velocity_over_u,
            expected.bubble_velocity_over_u,
        ),
        (
            "pressure / P",
            actual.pressure_over_p,
            expected.pressure_over_p,
        ),
        (
            "displacement / L",
            actual.displacement_over_l,
            expected.displacement_over_l,
        ),
    ] {
        require_same_float(
            &format!("{label} maximum absolute error"),
            actual.maximum_absolute_error,
            expected.maximum_absolute_error,
        )?;
        require_same_float(
            &format!("{label} maximum scaled error"),
            actual.maximum_scaled_error,
            expected.maximum_scaled_error,
        )?;
    }
    Ok(())
}

fn require_same_field_layout(
    cpu: &ResolvedFixedReferenceFsiSolution2d,
    cuda: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<(), String> {
    if cpu.model() != cuda.model()
        || cpu.semantic_revision() != cuda.semantic_revision()
        || cpu.realization_revision() != cuda.realization_revision()
        || cpu.mesh_artifact() != cuda.mesh_artifact()
        || cpu.fields() != cuda.fields()
        || cpu.fluid_velocity_vertices() != cuda.fluid_velocity_vertices()
        || cpu.fluid_velocity_cells() != cuda.fluid_velocity_cells()
        || cpu.fluid_pressure_vertices() != cuda.fluid_pressure_vertices()
        || cpu.solid_velocity_vertices() != cuda.solid_velocity_vertices()
        || cpu.solid_displacement_vertices() != cuda.solid_displacement_vertices()
        || cpu.solid_cells() != cuda.solid_cells()
        || cpu.interface_facets() != cuda.interface_facets()
    {
        return Err("CPU and CUDA physical Field identity/support/order differs".to_owned());
    }
    Ok(())
}

fn field_scale(
    solution: &ResolvedFixedReferenceFsiSolution2d,
    field: eqiora::Id<eqiora::kinds::Field>,
) -> Result<f64, String> {
    solution
        .realization_plan()
        .scaling()
        .block_scales()
        .iter()
        .find_map(|entry| {
            (entry.block() == AlgebraicBlock::Field(field))
                .then(|| entry.scale().quantity().value())
        })
        .ok_or_else(|| "represented physical Field has no exact scale".to_owned())
}

fn displacement_scale(solution: &ResolvedFixedReferenceFsiSolution2d) -> f64 {
    solution
        .realization_plan()
        .time_step()
        .eliminated_state()
        .state_scale()
        .quantity()
        .value()
}

fn flatten_scaled(values: &[[f64; 2]], scale: f64) -> impl Iterator<Item = f64> + '_ {
    values
        .iter()
        .flat_map(move |value| value.iter().map(move |component| component / scale))
}

fn decode_closed<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid closed observation JSON: {error}"))
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    read_bounded_bytes(&bytes, maximum)
}

fn read_bounded_bytes(bytes: &[u8], maximum: usize) -> Result<Vec<u8>, String> {
    if bytes.len() > maximum {
        return Err(format!("evidence exceeds {maximum} bytes"));
    }
    Ok(bytes.to_vec())
}

fn require_equal_bytes(label: &str, actual: &[u8], expected: &[u8]) -> Result<(), String> {
    if actual != expected {
        return Err(format!("{label} differ"));
    }
    Ok(())
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn accepted_output_sha256(values: &[f64]) -> Result<String, String> {
    let length = u64::try_from(values.len())
        .map_err(|_| "accepted output length exceeds portable u64".to_owned())?;
    let mut hash = Sha256::new();
    hash.update(b"eqiora.accepted-host-output/v1\0");
    hash.update(length.to_le_bytes());
    for value in values {
        if !value.is_finite() {
            return Err("accepted output contains a non-finite value".to_owned());
        }
        let normalized = if *value == 0.0 { 0.0 } else { *value };
        hash.update(normalized.to_bits().to_le_bytes());
    }
    Ok(hex(hash.finalize().into()))
}

fn synthetic_observation() -> Observation {
    Observation {
        schema: observation::SCHEMA.to_owned(),
        source_commit: "0".repeat(40),
        source_clean: true,
        environment: observation::Environment {
            runtime: "eqiora.cuda.cudarc".to_owned(),
            device_ordinal: 0,
            device_name: "synthetic".to_owned(),
            total_memory_bytes: 1,
            compute_capability_major: 1,
            compute_capability_minor: 0,
            driver: 1,
            cusparse: 1,
            cublas: 1,
            cudarc: "test".to_owned(),
            binding_toolkit: "test".to_owned(),
            adapter_version: "test".to_owned(),
            observation_kind:
                "public-source-selected-device-run; no-host-identity; not-hardware-attestation"
                    .to_owned(),
        },
        operator_sha256: "0".repeat(64),
        output_sha256: "0".repeat(64),
        values: vec![1.0],
        producer: observation::Producer {
            reason: "residual-tolerance-satisfied".to_owned(),
            completed_iterations: 1,
            reported_residual_norm: 0.0,
            true_residual_norm: 0.0,
        },
        receipt: observation::Receipt {
            dimension: 1,
            minimum_device_payload_bytes: 1,
            external_sparse_workspace_bytes: 0,
            dag: observation::cuda_dag().map(str::to_owned).collect(),
            transfer_count: 6,
            inverse_diagonal_present: false,
            inputs_ready_sequence: 1,
            solve_visible_sequence: 2,
            output_transfer_sequence: 3,
            solution_visible_sequence: 4,
        },
        physics: Physics {
            residual_norm: 0.0,
            continuity_residual_norm: 0.0,
            kinematic_residual_norm: 0.0,
            interface_velocity_jump_norm: 0.0,
            interface_action_imbalance_norm: 0.0,
            energy_defect: 0.0,
        },
        conformance: Conformance {
            algebraic: observation::ErrorPair {
                maximum_absolute_error: 0.0,
                maximum_scaled_error: 0.0,
            },
            vertex_velocity_over_u: observation::ErrorPair {
                maximum_absolute_error: 0.0,
                maximum_scaled_error: 0.0,
            },
            bubble_velocity_over_u: observation::ErrorPair {
                maximum_absolute_error: 0.0,
                maximum_scaled_error: 0.0,
            },
            pressure_over_p: observation::ErrorPair {
                maximum_absolute_error: 0.0,
                maximum_scaled_error: 0.0,
            },
            displacement_over_l: observation::ErrorPair {
                maximum_absolute_error: 0.0,
                maximum_scaled_error: 0.0,
            },
        },
    }
}

fn case_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify/fsi/fixed-reference-cuda-solve-2d")
}
