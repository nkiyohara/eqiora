//! Studio projection of the shared accepted structural result.

use eqiora::api::{MixedBoundaryElasticityResult2d, ModelDocument};
use eqiora::solver::{ConvergenceReason, REFERENCE_LINEAR_SOLVER};
use serde::Serialize;

const DEMO_PROTOCOL: &str = "eqiora.studio.mixed-boundary-elasticity-demo/v1";
const EXAMPLE_ID: &str = "mixed-boundary-linear-elasticity";
const MODEL_SOURCE: &str =
    include_str!("../../../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi");

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuralDemoResult {
    protocol: &'static str,
    example_id: &'static str,
    mesh: MeshProjection,
    displacement: DisplacementProjection,
    balance: BalanceEvidence,
    execution: ExecutionEvidence,
    lineage: LineageEvidence,
    evidence: EvidenceAttribution,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshProjection {
    spatial_dimension: usize,
    cells_per_axis: usize,
    vertices: Vec<VertexProjection>,
    cells: Vec<CellProjection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VertexProjection {
    index: usize,
    coordinates_m: [f64; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CellProjection {
    index: usize,
    vertices: [usize; 4],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DisplacementProjection {
    unit: &'static str,
    values_m: Vec<[f64; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BalanceEvidence {
    unit: &'static str,
    constrained_reaction_n: [f64; 2],
    integrated_body_force_n: [f64; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionEvidence {
    method: &'static str,
    mesh: &'static str,
    space: &'static str,
    quadrature: &'static str,
    scalar_type: &'static str,
    placement: &'static str,
    solver: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    convergence_reason: &'static str,
    relative_tolerance: f64,
    absolute_tolerance: f64,
    completed_iterations: usize,
    true_residual_norm: f64,
    residual_target: f64,
    assembly_packets: usize,
    assembly_targets: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LineageEvidence {
    model_digest: String,
    realization_digest: String,
    run_digest: String,
    semantic_revision: u64,
    realization_revision: u64,
    output_artifacts: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceAttribution {
    case_id: &'static str,
    status: &'static str,
}

pub(super) fn prepare_demo() -> Result<StructuralDemoResult, String> {
    let document = ModelDocument::compile("mixed-boundary-elasticity.eqi", MODEL_SOURCE)
        .map_err(diagnostics)?;
    let result =
        MixedBoundaryElasticityResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
            .map_err(error)?;
    let solution = result.solution();
    let report = solution.solve_report();
    let assembly = solution.assembly_report();
    let vertices = result
        .vertices_m()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, coordinates_m)| VertexProjection {
            index,
            coordinates_m,
        })
        .collect();
    let cells = result
        .cells()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, vertices)| CellProjection {
            index,
            vertices: vertices.map(|vertex| vertex as usize),
        })
        .collect();

    Ok(StructuralDemoResult {
        protocol: DEMO_PROTOCOL,
        example_id: EXAMPLE_ID,
        mesh: MeshProjection {
            spatial_dimension: 2,
            cells_per_axis: result.cells_per_axis(),
            vertices,
            cells,
        },
        displacement: DisplacementProjection {
            unit: "m",
            values_m: result.displacements_m().to_vec(),
        },
        balance: BalanceEvidence {
            unit: "N",
            constrained_reaction_n: solution.boundary_reaction(),
            integrated_body_force_n: solution.integrated_body_force(),
        },
        execution: ExecutionEvidence {
            method: "continuous-galerkin",
            mesh: "generated-uniform-cartesian",
            space: "continuous-q1-two-component",
            quadrature: "gauss-legendre-2-per-axis",
            scalar_type: "f64",
            placement: "one-host-one-worker",
            solver: "conjugate-gradient",
            preconditioner: "identity",
            reduction: "reproducible",
            convergence_reason: convergence_reason(report.reason()),
            relative_tolerance: report.solver_plan().relative_tolerance(),
            absolute_tolerance: report.solver_plan().absolute_tolerance(),
            completed_iterations: report.completed_iterations(),
            true_residual_norm: report.true_residual_norm(),
            residual_target: report.residual_target(),
            assembly_packets: assembly.packet_count(),
            assembly_targets: assembly.target_count(),
        },
        lineage: LineageEvidence {
            model_digest: result.model().digest().map_err(error)?.to_string(),
            realization_digest: result.realization().digest().map_err(error)?.to_string(),
            run_digest: result.run().digest().map_err(error)?.to_string(),
            semantic_revision: result.semantic_revision(),
            realization_revision: result.realization_revision(),
            output_artifacts: result.run().outputs().len(),
        },
        evidence: EvidenceAttribution {
            case_id: result.scientific_case_id(),
            status: "verified",
        },
    })
}

fn convergence_reason(reason: ConvergenceReason) -> &'static str {
    match reason {
        ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
        ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
    }
}

fn diagnostics(diagnostics: Vec<eqiora::Diagnostic>) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn error(error: eqiora::Diagnostic) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_projects_only_solver_owned_structural_evidence() {
        let result = prepare_demo().expect("accepted structural demonstration");
        assert_eq!(result.mesh.vertices.len(), 289);
        assert_eq!(result.mesh.cells.len(), 256);
        assert_eq!(result.displacement.values_m.len(), 289);
        assert_eq!(result.lineage.output_artifacts, 1);
        assert!(result.execution.true_residual_norm <= result.execution.residual_target);
        assert!(
            result
                .displacement
                .values_m
                .iter()
                .any(|value| value[0].abs() > 0.0)
        );

        let encoded = serde_json::to_value(&result).expect("serialize closed result");
        assert_eq!(encoded["protocol"], DEMO_PROTOCOL);
        assert_eq!(
            encoded["evidence"]["caseId"],
            "solid.mixed-boundary-elasticity-2d"
        );
        for forbidden in [
            "stress",
            "strain",
            "traction",
            "exactSolution",
            "errorNorm",
            "convergenceOrder",
        ] {
            assert!(
                !encoded
                    .as_object()
                    .expect("result object")
                    .contains_key(forbidden)
            );
        }
    }
}
