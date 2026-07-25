//! Collect one bounded fixed-reference FSI CUDA observation.
//!
//! This executable is case-specific verification tooling. It is neither a
//! product result format nor a general CUDA evidence API.

#[allow(dead_code)]
#[path = "../tests/support/fixed_reference_fsi.rs"]
mod fsi;
#[allow(dead_code)]
#[path = "../tests/support/fixed_reference_fsi_cuda_observation.rs"]
mod observation;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;

use eqiora::artifact::{
    ExecutionTopologyV1, LayoutArtifacts, RealizationEnvelopeV3, RunManifestV2,
};
use eqiora::backends::cuda::{
    CUDA_ADAPTER_VERSION, CUDA_LINEAR_EXECUTION_PROVIDER, CUDA_LINEAR_SOLVER_PROVIDER,
    CUDA_RUNTIME_ID, CudaLinearSolver, CudaRuntime,
};
use eqiora::device::QueueSlot;
use eqiora::realization::{
    AlgebraicBlock, CoupledFieldwiseRealizationPlan, CoupledFieldwiseRealizationRequest,
    DiscretizationMethod, MeshKind, RealizationCapabilities, RealizationRevision,
    ResolvedCoupledFieldwiseRealization, SemanticRevision, SpatialDimensionSupport,
    TargetCapabilities, VectorLayoutKind, resolve_coupled_fieldwise,
};
use eqiora::solver::{
    ConvergenceReason, LinearOperatorProperties, LinearSolverBackend, REFERENCE_LINEAR_SOLVER,
    ReductionPolicy, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
};
use eqiora_backend_cuda::CudaAdmittedExecutionAdapter;
use eqiora_execution::{CudaExecutorDescriptor, DeploymentBinding};
use eqiora_numerics::{
    fsi::FixedReferenceFsiCartesianModel2d, fsi::FixedReferenceFsiScaleProfile2d,
    fsi::ResolvedFixedReferenceFsiSolution2d, fsi::finalize_resolved_fixed_reference_fsi_step_2d,
    fsi::lower_fixed_reference_fsi_cartesian_2d,
};
use eqiora_numerics::{
    fsi::fixed_reference_fsi_cuda_plan_2d, fsi::fixed_reference_fsi_requirements_2d,
};
use fsi::{direct_document, execution_context, prestrained_state, spatial_context};
use observation::{Conformance, Environment, Observation, Physics, Producer, Receipt, error_pair};

fn main() {
    if let Err(error) = run() {
        eprintln!("fixed-reference FSI CUDA evidence collection failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("collection must be built with --release".to_owned());
    }
    let mut arguments = std::env::args_os().skip(1);
    let output = arguments.next().map(PathBuf::from).ok_or_else(|| {
        "usage: fixed_reference_fsi_cuda_collect <new-output-directory>".to_owned()
    })?;
    if arguments.next().is_some() || output.exists() {
        return Err("collector requires one new output directory".to_owned());
    }
    require_single_visible_device()?;
    let source_commit = clean_source_commit()?;
    let collected = collect(source_commit)?;
    persist(&output, &collected)
}

struct Collected {
    model: Vec<u8>,
    realization: Vec<u8>,
    run: Vec<u8>,
    observation: Observation,
}

fn collect(source_commit: String) -> Result<Collected, String> {
    let device_ordinal = selected_device_ordinal()?;
    if device_ordinal != 0 {
        return Err("the collector requires one visible device as Eqiora ordinal zero".to_owned());
    }
    let device = CudaRuntime
        .discover()
        .map_err(|diagnostic| diagnostic.to_string())?
        .into_iter()
        .find(|candidate| candidate.id().ordinal() == device_ordinal)
        .ok_or_else(|| "the selected CUDA device is not visible".to_owned())?;
    CudaLinearSolver::new(device_ordinal)
        .admit_device(&device)
        .map_err(|diagnostic| diagnostic.to_string())?;

    let document = direct_document();
    let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program())
        .map_err(|diagnostic| diagnostic.to_string())?;
    let spatial = spatial_context(document.program(), &canonical);
    let host = execution_context(document.program(), &canonical, &spatial);
    let previous = prestrained_state(&spatial);
    let resolved = resolve_cuda(&canonical, &host, device_ordinal)?;
    let realization = RealizationEnvelopeV3::from_resolved(
        &spatial.model,
        &resolved,
        LayoutArtifacts::Replicated,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;

    let host_finalized = finalize_resolved_fixed_reference_fsi_step_2d(
        &canonical,
        &host.resolved,
        host.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        &previous,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let host_operator = host_finalized.linear_system().agreement_fingerprint();
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

    let cuda_finalized = finalize_resolved_fixed_reference_fsi_step_2d(
        &canonical,
        &resolved,
        host.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        &previous,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let operator = cuda_finalized.linear_system().agreement_fingerprint();
    if operator != host_operator {
        return Err("CPU and CUDA Realizations finalized different CSR/RHS bytes".to_owned());
    }
    let binding = DeploymentBinding::bind_cuda(
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
    .map_err(|diagnostic| diagnostic.to_string())?;
    let admitted = cuda_finalized
        .admit_cuda(binding)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let executed = CudaLinearSolver::new(device_ordinal)
        .execute_admitted(admitted)
        .map_err(|diagnostic| diagnostic.to_string())?;
    let (accepted, evidence) = executed.into_parts();
    let values = accepted.solution().values().to_vec();
    let report = accepted.receipt().report();
    if report.reason() != ConvergenceReason::ResidualToleranceSatisfied {
        return Err("the selected observation did not converge iteratively".to_owned());
    }
    let receipt = accepted.receipt();
    let solver_provider = receipt.solver_provider();
    let execution_provider = receipt.execution_provider();
    let trace = receipt
        .cuda_trace()
        .ok_or_else(|| "accepted CUDA execution has no generic trace".to_owned())?;
    let transfers = trace.transfers();
    if transfers.inverse_diagonal().is_some() {
        return Err("identity-preconditioned MINRES transferred a diagonal".to_owned());
    }
    let receipt_observation = Receipt {
        dimension: receipt.dimension(),
        minimum_device_payload_bytes: receipt
            .minimum_device_payload_bytes()
            .ok_or_else(|| "CUDA receipt has no resident-payload lower bound".to_owned())?,
        external_sparse_workspace_bytes: trace.external_sparse_workspace_bytes(),
        dag: receipt
            .dag()
            .steps()
            .iter()
            .map(|step| step.canonical_name().to_owned())
            .collect(),
        transfer_count: 6,
        inverse_diagonal_present: false,
        inputs_ready_sequence: trace
            .inputs_ready()
            .completion()
            .submission()
            .sequence()
            .get(),
        solve_visible_sequence: trace
            .solve_visible()
            .completion()
            .submission()
            .sequence()
            .get(),
        output_transfer_sequence: transfers
            .complete_solution()
            .completion()
            .submission()
            .sequence()
            .get(),
        solution_visible_sequence: trace
            .solution_visible()
            .completion()
            .submission()
            .sequence()
            .get(),
    };
    let output_sha256 = hex(receipt.output().as_bytes());
    let producer = Producer {
        reason: "residual-tolerance-satisfied".to_owned(),
        completed_iterations: report.completed_iterations(),
        reported_residual_norm: canonical_zero(report.reported_residual_norm()),
        true_residual_norm: canonical_zero(report.true_residual_norm()),
    };
    let (cuda, _) = cuda_finalized
        .finish_cuda(accepted)
        .map_err(|diagnostic| diagnostic.to_string())?;
    require_same_field_layout(&cpu, &cuda)?;
    let physics = physics(&cuda);
    let conformance = conformance(&cpu, &cuda)?;

    let versions = evidence.versions();
    let compute = evidence.compute_capability();
    let environment = Environment {
        runtime: CUDA_RUNTIME_ID.as_str().to_owned(),
        device_ordinal,
        device_name: evidence.device().name().to_owned(),
        total_memory_bytes: evidence.device().total_memory_bytes().get(),
        compute_capability_major: compute.major(),
        compute_capability_minor: compute.minor(),
        driver: versions.driver(),
        cusparse: versions.cusparse(),
        cublas: versions
            .cublas()
            .ok_or_else(|| "MINRES execution did not report cuBLAS".to_owned())?,
        cudarc: versions.cudarc().to_owned(),
        binding_toolkit: versions.binding_toolkit().to_owned(),
        adapter_version: CUDA_ADAPTER_VERSION.to_owned(),
        observation_kind:
            "public-source-selected-device-run; no-host-identity; not-hardware-attestation"
                .to_owned(),
    };
    let run = RunManifestV2::new(
        &realization,
        execution_provenance(solver_provider, execution_provider, &environment)
            .map_err(|diagnostic| diagnostic.to_string())?,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let observation = Observation {
        schema: observation::SCHEMA.to_owned(),
        source_commit,
        source_clean: true,
        environment,
        operator_sha256: hex(operator.as_bytes()),
        output_sha256,
        values: values.into_iter().map(canonical_zero).collect(),
        producer,
        receipt: receipt_observation,
        physics,
        conformance,
    };
    observation.validate()?;
    Ok(Collected {
        model: spatial
            .model
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        realization: realization
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        run: run
            .canonical_json()
            .map_err(|diagnostic| diagnostic.to_string())?,
        observation,
    })
}

fn resolve_cuda(
    canonical: &FixedReferenceFsiCartesianModel2d,
    host: &fsi::ExecutionContext,
    device: u16,
) -> Result<ResolvedCoupledFieldwiseRealization, String> {
    let host_plan = host.resolved.plan();
    let velocity = canonical
        .fluid()
        .velocity()
        .downcast::<eqiora::kinds::Field>()
        .ok_or_else(|| "canonical fluid velocity is not a Field".to_owned())?;
    let pressure = canonical
        .fluid()
        .pressure()
        .downcast::<eqiora::kinds::Field>()
        .ok_or_else(|| "canonical fluid pressure is not a Field".to_owned())?;
    let plan = fixed_reference_fsi_cuda_plan_2d(
        canonical,
        host.mesh_reference,
        host_plan.time_step().duration(),
        FixedReferenceFsiScaleProfile2d::new(
            host_plan.spatial().coordinate_length_scale().quantity(),
            plan_field_scale(host_plan, velocity)?,
            plan_field_scale(host_plan, pressure)?,
        )
        .map_err(|diagnostic| diagnostic.to_string())?,
        host_plan.solver().with_reduction(ReductionPolicy::Fast),
        device,
    )
    .map_err(|diagnostic| diagnostic.to_string())?;
    let solver_plan = plan.solver();
    resolve_coupled_fieldwise(
        &CoupledFieldwiseRealizationRequest::explicit(
            host.resolved.model(),
            SemanticRevision::new(canonical.semantic_revision()),
            RealizationRevision::new(1),
            plan,
        ),
        fixed_reference_fsi_requirements_2d(canonical),
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

fn execution_provenance(
    solver: eqiora::solver::SolverProvider,
    execution: eqiora::solver::ExecutionProvider,
    environment: &Environment,
) -> Result<eqiora::artifact::ExecutionProvenanceV1, eqiora::Diagnostic> {
    let native_libraries = [
        ("cusparse", environment.cusparse.to_string()),
        ("cublas", environment.cublas.to_string()),
    ];
    eqiora::artifact::ExecutionProvenanceV1::from_provider_releases(
        solver,
        execution,
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
}

fn physics(solution: &ResolvedFixedReferenceFsiSolution2d) -> Physics {
    let numerical = solution.numerical_evidence();
    Physics {
        residual_norm: canonical_zero(numerical.residual_norm()),
        continuity_residual_norm: canonical_zero(numerical.continuity_residual_norm()),
        kinematic_residual_norm: canonical_zero(numerical.kinematic_residual_norm()),
        interface_velocity_jump_norm: canonical_zero(numerical.interface_velocity_jump_norm()),
        interface_action_imbalance_norm: canonical_zero(
            numerical.interface_action_imbalance_norm(),
        ),
        energy_defect: canonical_zero(numerical.energy_balance().defect()),
    }
}

fn conformance(
    cpu: &ResolvedFixedReferenceFsiSolution2d,
    cuda: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<Conformance, String> {
    let velocity_scale = field_scale(cpu, cpu.fields().fluid_velocity())?;
    let pressure_scale = field_scale(cpu, cpu.fields().fluid_pressure())?;
    let displacement_scale = cpu
        .realization_plan()
        .time_step()
        .eliminated_state()
        .state_scale()
        .quantity()
        .value();
    Ok(Conformance {
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
                .map(|value| value / pressure_scale),
            cuda.fluid_pressure_coefficients()
                .iter()
                .map(|value| value / pressure_scale),
        )?,
        displacement_over_l: error_pair(
            flatten_scaled(cpu.solid_displacement_coefficients(), displacement_scale),
            flatten_scaled(cuda.solid_displacement_coefficients(), displacement_scale),
        )?,
    })
}

fn require_same_field_layout(
    cpu: &ResolvedFixedReferenceFsiSolution2d,
    cuda: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<(), String> {
    if cpu.model() != cuda.model()
        || cpu.fields() != cuda.fields()
        || cpu.mesh_artifact() != cuda.mesh_artifact()
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

fn plan_field_scale(
    plan: &CoupledFieldwiseRealizationPlan,
    field: eqiora::Id<eqiora::kinds::Field>,
) -> Result<eqiora::DynQuantity, String> {
    plan.scaling()
        .block_scales()
        .iter()
        .find_map(|entry| {
            (entry.block() == AlgebraicBlock::Field(field)).then(|| entry.scale().quantity())
        })
        .ok_or_else(|| "host Realization has no exact scale for a represented Field".to_owned())
}

fn flatten_scaled(values: &[[f64; 2]], scale: f64) -> impl Iterator<Item = f64> + '_ {
    values
        .iter()
        .flat_map(move |value| value.iter().map(move |component| component / scale))
}

fn require_single_visible_device() -> Result<(), String> {
    let visible = std::env::var("CUDA_VISIBLE_DEVICES")
        .map_err(|_| "set CUDA_VISIBLE_DEVICES to exactly one physical device".to_owned())?;
    if visible.trim().is_empty()
        || visible.contains(',')
        || visible.chars().any(char::is_whitespace)
    {
        return Err("CUDA_VISIBLE_DEVICES must name exactly one device".to_owned());
    }
    Ok(())
}

fn selected_device_ordinal() -> Result<u16, String> {
    std::env::var("EQIORA_CUDA_DEVICE")
        .map_err(|_| "set EQIORA_CUDA_DEVICE=0 for collection".to_owned())?
        .parse::<u16>()
        .map_err(|_| "EQIORA_CUDA_DEVICE must be a u16 ordinal".to_owned())
}

fn clean_source_commit() -> Result<String, String> {
    let status = command_output(
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    if !status.is_empty() {
        return Err("collector requires a clean source tree".to_owned());
    }
    let commit = command_output("git", &["rev-parse", "HEAD"])?;
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("git did not return one full source commit".to_owned());
    }
    Ok(commit)
}

fn command_output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} failed with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("{program} output is not UTF-8: {error}"))
}

fn persist(output: &Path, collected: &Collected) -> Result<(), String> {
    let parent = output
        .parent()
        .ok_or_else(|| "output directory has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let stage = parent.join(format!(".eqiora-fsi-cuda-stage-{}", std::process::id()));
    if stage.exists() {
        return Err(format!(
            "staging directory {} already exists",
            stage.display()
        ));
    }
    fs::create_dir(&stage)
        .map_err(|error| format!("cannot create {}: {error}", stage.display()))?;
    let result = (|| {
        fs::create_dir(stage.join("artifacts")).map_err(|error| error.to_string())?;
        fs::create_dir(stage.join("observations")).map_err(|error| error.to_string())?;
        fs::write(stage.join("artifacts/model.json"), &collected.model)
            .map_err(|error| error.to_string())?;
        fs::write(
            stage.join("artifacts/cuda-realization.json"),
            &collected.realization,
        )
        .map_err(|error| error.to_string())?;
        fs::write(stage.join("artifacts/cuda-run.json"), &collected.run)
            .map_err(|error| error.to_string())?;
        let observation =
            serde_json::to_vec_pretty(&collected.observation).map_err(|error| error.to_string())?;
        fs::write(stage.join("observations/observation.json"), observation)
            .map_err(|error| error.to_string())?;
        fs::rename(&stage, output).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
