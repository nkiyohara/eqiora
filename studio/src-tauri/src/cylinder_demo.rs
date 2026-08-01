//! One immutable exact-cylinder example composed from accepted public seams.

use eqiora::Diagnostic;
use eqiora::api::{CircularHoleSteadyStokesResult2d, UnstructuredP1ScalarFieldProjection2d};
use eqiora::artifact::ModelEnvelope;
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::diagnostic::codes;
use eqiora::geometry::{CanonicalGeometryLimits, CanonicalGeometryV1, CircularHoleChordalMeshV1};
use eqiora::meshing::MeshQualityGate;
use eqiora::solver::{LinearSolver, PreconditionerPolicy, ReductionPolicy};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::unstructured_field::UnstructuredFieldContext;
use super::{AppState, BridgeEnvelope, PROTOCOL, studio_error};

const DEMO_PROTOCOL: &str = "eqiora.studio.cylinder-stokes-demo/v1";
const EXAMPLE_ID: &str = "steady-flow-past-cylinder";
const GEOMETRY: &[u8] = include_bytes!("../../../examples/steady-flow-past-cylinder.geometry.json");
const MODEL: &[u8] = include_bytes!("../../../examples/steady-flow-past-cylinder.model.json");

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
    let source = CanonicalGeometryV1::decode_circular_hole_canonical(
        embedded_json(GEOMETRY),
        CanonicalGeometryLimits::default(),
    )?;
    let model = ModelEnvelope::from_json(embedded_json(MODEL), Default::default())?;
    let owner =
        CircularHoleChordalMeshV1::from_exact(&source, 1.0e-4, 50, MeshQualityGate::new(1.0e-5)?)?;
    let result = CircularHoleSteadyStokesResult2d::solve_reference(
        &model,
        &source,
        &owner,
        &FaerLinearSolver,
    )?;
    evidence(&result)
}

fn embedded_json(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

fn evidence(result: &CircularHoleSteadyStokesResult2d) -> Result<PreparedCylinderDemo, Diagnostic> {
    let solution = result.solution();
    let constrained = solution.boundary_reaction();
    let body = solution.integrated_body_force();
    let traction = solution.integrated_boundary_traction();
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
        projection: result.pressure_projection().clone(),
        geometry: GeometryEvidence {
            exact_source_digest: encode_digest(result.source().digest_bytes()),
            realized_geometry_digest: result.realized_geometry().digest()?.to_string(),
            requested_max_boundary_error_m: result.owner().requested_max_boundary_error_m(),
            boundary_evaluation_allowance_m: result.owner().boundary_evaluation_allowance_m(),
            boundary_error_bound_m: result.owner().boundary_error_bound_m(),
            circle_segments: result.owner().circle_segments(),
        },
        cylinder_reaction: CylinderReactionEvidence {
            convention: "constraint-force-on-fluid",
            force_on_fluid_n_m: result.cylinder_force_on_fluid(),
        },
        flux_balance: FluxBalanceEvidence {
            convention: "physical-parent-outward",
            inlet_m2_s: result.inlet_flux(),
            outlet_m2_s: result.outlet_flux(),
            net_m2_s: result.net_flux(),
        },
        momentum_balance: MomentumBalanceEvidence {
            constrained_reaction_n_m: constrained,
            integrated_body_force_n_m: body,
            integrated_traction_n_m: traction,
            closure_n_m: result.momentum_closure(),
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
