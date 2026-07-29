//! One immutable exact-cylinder example composed from accepted public seams.

use std::num::NonZeroUsize;

use eqiora::api::UnstructuredP1ScalarFieldProjection2d;
use eqiora::artifact::{
    DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, FieldSnapshotEnvelopeV1,
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, LayoutArtifacts, ModelEnvelopeV7,
    RealizationEnvelopeV2, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CanonicalCircularHoleGeometryV1, CanonicalGeometryLimits, CanonicalGeometryRef,
    CircularHoleChordalMeshV1,
};
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshQualityGate,
};
use eqiora::numerics::{
    IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d,
    solve_resolved_steady_stokes_geometry_mini_2d,
};
use eqiora::realization::{
    DiscretizationMethod, FieldwiseRealizationRequest, MeshKind, RealizationCapabilities,
    RealizationRevision, SemanticRevision, SpatialDimensionSupport, TargetCapabilities,
    VectorLayoutKind, resolve_fieldwise,
};
use eqiora::sem::KernelProgram;
use eqiora::solver::{
    LinearSolver, LinearSolverBackend, PreconditionerPolicy, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, SolverPlan,
};
use eqiora::{Diagnostic, DimExponents, DynQuantity};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::unstructured_field::UnstructuredFieldContext;
use super::{AppState, BridgeEnvelope, PROTOCOL, studio_error};

const DEMO_PROTOCOL: &str = "eqiora.studio.cylinder-stokes-demo/v1";
const EXAMPLE_ID: &str = "steady-flow-past-cylinder";
// The application composition owns a new lineage revision while retaining the
// accepted scientific plan and its independently verified observations.
const DEMO_REALIZATION_REVISION: u64 = 132;
const GEOMETRY: &[u8] = include_bytes!("../../../examples/steady-flow-past-cylinder.geometry.json");
const MODEL: &[u8] = include_bytes!("../../../examples/steady-flow-past-cylinder.model-v7.json");

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CylinderDemoRequest {
    protocol: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CylinderDemoResult {
    protocol: &'static str,
    example_id: &'static str,
    context: UnstructuredFieldContext,
    geometry: GeometryEvidence,
    cylinder_reaction: CylinderReactionEvidence,
    flux_balance: FluxBalanceEvidence,
    momentum_balance: MomentumBalanceEvidence,
    solver: SolverEvidence,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryEvidence {
    exact_source_digest: String,
    realized_geometry_digest: String,
    requested_max_boundary_error_m: f64,
    boundary_evaluation_allowance_m: f64,
    boundary_error_bound_m: f64,
    circle_segments: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CylinderReactionEvidence {
    convention: &'static str,
    force_on_fluid_n_m: [f64; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FluxBalanceEvidence {
    convention: &'static str,
    inlet_m2_s: f64,
    outlet_m2_s: f64,
    net_m2_s: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MomentumBalanceEvidence {
    constrained_reaction_n_m: [f64; 2],
    integrated_body_force_n_m: [f64; 2],
    integrated_traction_n_m: [f64; 2],
    closure_n_m: [f64; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SolverEvidence {
    algorithm: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    relative_tolerance: f64,
    absolute_tolerance: f64,
    completed_iterations: usize,
    residual_target: f64,
    true_residual_norm: f64,
    continuity_residual_norm: f64,
}

struct PreparedCylinderDemo {
    projection: UnstructuredP1ScalarFieldProjection2d,
    geometry: GeometryEvidence,
    cylinder_reaction: CylinderReactionEvidence,
    flux_balance: FluxBalanceEvidence,
    momentum_balance: MomentumBalanceEvidence,
    solver: SolverEvidence,
}

#[tauri::command]
pub(super) async fn run_cylinder_demo(
    request: CylinderDemoRequest,
    state: State<'_, AppState>,
) -> Result<BridgeEnvelope<CylinderDemoResult>, ()> {
    if request.protocol != PROTOCOL {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio cylinder-demo request protocol",
        )]));
    }
    let prepared = match tauri::async_runtime::spawn_blocking(prepare_demo).await {
        Ok(Ok(prepared)) => prepared,
        Ok(Err(diagnostic)) => return Ok(BridgeEnvelope::failure(vec![diagnostic.into()])),
        Err(error) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                format!("native cylinder-demo worker failed: {error}"),
            )]));
        }
    };
    let context = match state.unstructured_fields.lock() {
        Ok(mut cache) => match cache.publish(prepared.projection) {
            Ok(context) => context,
            Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![*diagnostic])),
        },
        Err(_) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native unstructured Field cache is unavailable",
            )]));
        }
    };
    Ok(BridgeEnvelope::success(CylinderDemoResult {
        protocol: DEMO_PROTOCOL,
        example_id: EXAMPLE_ID,
        context,
        geometry: prepared.geometry,
        cylinder_reaction: prepared.cylinder_reaction,
        flux_balance: prepared.flux_balance,
        momentum_balance: prepared.momentum_balance,
        solver: prepared.solver,
    }))
}

fn prepare_demo() -> Result<PreparedCylinderDemo, Diagnostic> {
    let source = CanonicalCircularHoleGeometryV1::decode_canonical(
        embedded_json(GEOMETRY),
        CanonicalGeometryLimits::default(),
    )?;
    let model = ModelEnvelopeV7::from_json(embedded_json(MODEL), Default::default())?;
    let program = replay_program(&model, &source)?;
    let owner =
        CircularHoleChordalMeshV1::from_exact(&source, 1.0e-4, 50, MeshQualityGate::new(1.0e-5)?)?;
    let geometry = GeometryDefinitionV1::from_region(owner.region());
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())?;
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(&geometry, &mesh)?;
    let binding = SteadyStokesGeometryBinding2d::new(
        &program,
        source.clone(),
        owner.clone(),
        geometry.clone(),
        mesh.clone(),
        correspondence.clone(),
    )?;
    let scales = IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(0.41, LENGTH),
        DynQuantity::new(0.3, VELOCITY),
        DynQuantity::new(0.001 * 0.3 / 0.41, PRESSURE),
    )?;
    let solver_plan = SolverPlan::new(
        LinearSolver::SparseLu,
        1.0e-6,
        1.0e-13,
        NonZeroUsize::new(10_000).expect("nonzero constant"),
    )?
    .with_reduction(ReductionPolicy::Fast);
    let plan = binding.mini_plan(mesh.artifact_reference()?, scales, solver_plan)?;
    let resolved = resolve_fieldwise(
        &FieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(DEMO_REALIZATION_REVISION),
            plan,
        ),
        binding.fieldwise_requirements(),
        &reference_capabilities()?,
    )?;
    let realization =
        RealizationEnvelopeV2::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)?;
    let solution = solve_resolved_steady_stokes_geometry_mini_2d(
        &program,
        &resolved,
        &binding,
        &FaerLinearSolver,
    )?;
    let pressure_payload = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Scalar,
        solution.pressure().vertex_values().to_vec(),
    )?;
    let pressure_block = DiscreteFieldEnvelopeV1::from_payload(&mesh, &pressure_payload)?;
    let snapshot = FieldSnapshotEnvelopeV1::new_authored_fieldwise(
        &model,
        &realization,
        &source,
        &owner,
        &geometry,
        &correspondence,
        &mesh,
        solution.pressure_field(),
        std::slice::from_ref(&pressure_block),
    )?;
    let execution = ExecutionProvenanceV1::from_provider_releases(
        FaerLinearSolver.provider(),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Fast,
        std::iter::empty::<(&str, &str)>(),
    )?;
    let run = RunManifestV2::new(&realization, execution)?.with_output(snapshot.digest()?);
    let projection = UnstructuredP1ScalarFieldProjection2d::from_authored_fieldwise_snapshot(
        &model,
        &realization,
        &source,
        &owner,
        &geometry,
        &correspondence,
        &mesh,
        &run,
        &snapshot,
        &pressure_block,
    )?;
    evidence(projection, &source, &owner, &geometry, &solution)
}

fn embedded_json(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn replay_program(
    model: &ModelEnvelopeV7,
    source: &CanonicalCircularHoleGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let (transaction, model_id) = model.to_transaction().map_err(first_diagnostic)?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(first_diagnostic)?;
    KernelProgram::from_snapshot_with_geometry(
        &store.snapshot(),
        model_id,
        &[CanonicalGeometryRef::from(source)],
    )
    .map_err(first_diagnostic)
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics.into_iter().next().unwrap_or_else(|| {
        Diagnostic::error(
            codes::INVALID_ARTIFACT,
            "Model v7 replay failed without a diagnostic",
        )
    })
}

fn reference_capabilities() -> Result<RealizationCapabilities, Diagnostic> {
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("nonzero constant")),
        )],
        [VectorLayoutKind::Replicated],
        FaerLinearSolver.capabilities(),
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
}

fn evidence(
    projection: UnstructuredP1ScalarFieldProjection2d,
    source: &CanonicalCircularHoleGeometryV1,
    owner: &CircularHoleChordalMeshV1,
    geometry: &GeometryDefinitionV1,
    solution: &eqiora::numerics::SteadyStokesMiniSolution2d,
) -> Result<PreparedCylinderDemo, Diagnostic> {
    let cylinder_reaction = solution
        .named_boundary_reaction("cylinder")
        .ok_or_else(|| missing_evidence("cylinder reaction"))?;
    let inlet = solution
        .named_boundary_flux("inlet")
        .ok_or_else(|| missing_evidence("inlet flux"))?;
    let outlet = solution
        .named_boundary_flux("outlet")
        .ok_or_else(|| missing_evidence("outlet flux"))?;
    let constrained = solution.boundary_reaction();
    let body = solution.integrated_body_force();
    let traction = solution.integrated_boundary_traction();
    let closure = std::array::from_fn(|component| {
        constrained[component] + body[component] + traction[component]
    });
    let dimensionless = solution.dimensionless_solution();
    let report = dimensionless.solve_report();
    if report.algorithm() != LinearSolver::SparseLu
        || report.preconditioner() != PreconditionerPolicy::Identity
        || report.reduction() != ReductionPolicy::Fast
    {
        return Err(missing_evidence("frozen sparse-LU solver tuple"));
    }
    let solver_plan = report.solver_plan();
    Ok(PreparedCylinderDemo {
        projection,
        geometry: GeometryEvidence {
            exact_source_digest: encode_digest(source.digest_bytes()),
            realized_geometry_digest: geometry.digest()?.to_string(),
            requested_max_boundary_error_m: owner.requested_max_boundary_error_m(),
            boundary_evaluation_allowance_m: owner.boundary_evaluation_allowance_m(),
            boundary_error_bound_m: owner.boundary_error_bound_m(),
            circle_segments: owner.circle_segments(),
        },
        cylinder_reaction: CylinderReactionEvidence {
            convention: "constraint-force-on-fluid",
            force_on_fluid_n_m: cylinder_reaction,
        },
        flux_balance: FluxBalanceEvidence {
            convention: "physical-parent-outward",
            inlet_m2_s: inlet,
            outlet_m2_s: outlet,
            net_m2_s: inlet + outlet,
        },
        momentum_balance: MomentumBalanceEvidence {
            constrained_reaction_n_m: constrained,
            integrated_body_force_n_m: body,
            integrated_traction_n_m: traction,
            closure_n_m: closure,
        },
        solver: SolverEvidence {
            algorithm: "sparse-lu",
            preconditioner: "identity",
            reduction: "fast",
            relative_tolerance: solver_plan.relative_tolerance(),
            absolute_tolerance: solver_plan.absolute_tolerance(),
            completed_iterations: report.completed_iterations(),
            residual_target: report.residual_target(),
            true_residual_norm: report.true_residual_norm(),
            continuity_residual_norm: dimensionless.continuity_residual_norm(),
        },
    })
}

fn missing_evidence(name: &str) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_ARTIFACT,
        format!("accepted cylinder demo is missing {name} evidence"),
    )
}

fn encode_digest(bytes: [u8; 32]) -> String {
    use std::fmt::Write;

    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing into a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unstructured_field::UnstructuredFieldCache;

    #[test]
    fn immutable_example_prepares_one_complete_studio_result() {
        let prepared = prepare_demo().expect("accepted exact-cylinder demonstration");
        assert_eq!(prepared.projection.vertices_m().len(), 104);
        assert_eq!(prepared.projection.triangles().len(), 104);
        assert_eq!(prepared.projection.values().len(), 104);
        assert_eq!(prepared.geometry.circle_segments, 50);
        assert!(
            prepared.geometry.boundary_error_bound_m
                <= prepared.geometry.requested_max_boundary_error_m
        );
        assert!(prepared.solver.true_residual_norm <= prepared.solver.residual_target);

        let mut cache = UnstructuredFieldCache::default();
        let context = cache
            .publish(prepared.projection.clone())
            .expect("publish exact projection");
        assert_eq!(
            cache
                .publish(prepared.projection)
                .expect("reuse exact retained projection"),
            context
        );
        let encoded = serde_json::to_value(CylinderDemoResult {
            protocol: DEMO_PROTOCOL,
            example_id: EXAMPLE_ID,
            context,
            geometry: prepared.geometry,
            cylinder_reaction: prepared.cylinder_reaction,
            flux_balance: prepared.flux_balance,
            momentum_balance: prepared.momentum_balance,
            solver: prepared.solver,
        })
        .expect("serialize closed result");

        assert_eq!(encoded["protocol"], DEMO_PROTOCOL);
        assert_eq!(encoded["exampleId"], EXAMPLE_ID);
        assert_eq!(encoded["context"]["semanticRevision"], "1");
        assert_eq!(encoded["context"]["mesh"]["kind"], "affine-triangle-2d");
        assert_eq!(
            encoded["context"]["field"]["coherentSiUnit"],
            "kg·m^-1·s^-2"
        );
        assert_eq!(
            encoded["cylinderReaction"]["convention"],
            "constraint-force-on-fluid"
        );
        assert_eq!(encoded["solver"]["algorithm"], "sparse-lu");
    }
}
