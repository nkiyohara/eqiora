//! Transport-independent projection of one accepted structural solve.

use std::num::{NonZeroU16, NonZeroUsize};

use eqiora::DimExponents;
use eqiora::artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, LayoutArtifacts, ModelEnvelopeV4,
    RealizationEnvelopeV1, RunManifestV2,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::kernel::KernelNode;
use eqiora::meshing::{MeshEntity, MeshTopology};
use eqiora::numerics::solve_resolved_isotropic_elasticity_cartesian_2d;
use eqiora::realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, SemanticRevision, Space, Target, VectorLayoutKind, resolve,
};
use eqiora::solver::{
    ConvergenceReason, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    REFERENCE_LINEAR_SOLVER, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType, SolverPlan,
};
use serde::Serialize;

const DEMO_PROTOCOL: &str = "eqiora.studio.mixed-boundary-elasticity-demo/v1";
const EXAMPLE_ID: &str = "mixed-boundary-linear-elasticity";
const SCIENTIFIC_CASE_ID: &str = "solid.mixed-boundary-elasticity-2d";
const SCIENTIFIC_CASE: &str =
    include_str!("../../../../verify/solid/mixed-boundary-elasticity-2d/case.toml");
const MODEL_SOURCE: &str =
    include_str!("../../../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi");

const CELLS_PER_AXIS: usize = 16;
const REALIZATION_REVISION: u64 = 1;
const RELATIVE_TOLERANCE: f64 = 1.0e-12;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-14;
const MAXIMUM_ITERATIONS: usize = 10_000;
const DISPLACEMENT_DIMENSION: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};

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
    validate_scientific_case(SCIENTIFIC_CASE)?;
    let document = ExactModelCodec::V4
        .compile("mixed-boundary-elasticity.eqi", MODEL_SOURCE)
        .map_err(diagnostics)?;
    let displacement = document
        .aliases()
        .get("displacement")
        .copied()
        .ok_or_else(|| "structural Model omitted the displacement alias".to_owned())?;
    validate_displacement_field(document.program(), displacement)?;
    let resolved = resolve_realization(document.program())?;
    let (_, solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
        document.program(),
        &resolved,
        &REFERENCE_LINEAR_SOLVER,
    )
    .map_err(error)?;

    let mesh = solution.displacement().mesh();
    let vertices = project_vertices(mesh.entity_count(0), |vertex| {
        mesh.vertex_coordinates(vertex)
    })?;
    let cells = project_cells(mesh.entity_count(2), |cell| mesh.entity_vertices(cell))?;
    let displacements = (0..vertices.len())
        .map(|vertex| {
            solution
                .displacement()
                .vertex_values(vertex)
                .and_then(|value| <[f64; 2]>::try_from(value).ok())
                .ok_or_else(|| format!("structural result omitted displacement vertex {vertex}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_payload(&vertices, &cells, &displacements)?;

    let report = solution.solve_report();
    if report.algorithm() != LinearSolver::ConjugateGradient
        || report.preconditioner() != PreconditionerPolicy::Identity
        || report.reduction() != ReductionPolicy::Reproducible
    {
        return Err("structural result used a solver tuple outside the frozen example".to_owned());
    }
    let assembly = solution.assembly_report();
    let model = ModelEnvelopeV4::from_program(document.program()).map_err(error)?;
    let realization =
        RealizationEnvelopeV1::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)
            .map_err(error)?;
    let run = RunManifestV2::new(&realization, execution_provenance()?).map_err(error)?;
    run.validate_against(&realization).map_err(error)?;

    Ok(StructuralDemoResult {
        protocol: DEMO_PROTOCOL,
        example_id: EXAMPLE_ID,
        mesh: MeshProjection {
            spatial_dimension: 2,
            cells_per_axis: CELLS_PER_AXIS,
            vertices,
            cells,
        },
        displacement: DisplacementProjection {
            unit: "m",
            values_m: displacements,
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
            model_digest: model.digest().map_err(error)?.to_string(),
            realization_digest: realization.digest().map_err(error)?.to_string(),
            run_digest: run.digest().map_err(error)?.to_string(),
            semantic_revision: document.program().revision().0,
            realization_revision: REALIZATION_REVISION,
            output_artifacts: run.outputs().len(),
        },
        evidence: EvidenceAttribution {
            case_id: SCIENTIFIC_CASE_ID,
            status: "verified",
        },
    })
}

fn resolve_realization(
    program: &eqiora::sem::KernelProgram,
) -> Result<eqiora::realization::ResolvedRealization, String> {
    let plan = RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(CELLS_PER_AXIS)
                    .expect("positive frozen refinement"),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("positive frozen quadrature"),
            },
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            RELATIVE_TOLERANCE,
            ABSOLUTE_TOLERANCE,
            NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("positive frozen iteration limit"),
        )
        .map_err(error)?,
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .map_err(error)?;
    resolve(
        &RealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(REALIZATION_REVISION),
            plan,
        ),
        RealizationRequirements::new(
            NonZeroUsize::new(2).expect("positive frozen dimension"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
        &RealizationCapabilities::isotropic_elasticity_2d_reference(),
    )
    .map_err(error)
}

fn validate_displacement_field(
    program: &eqiora::sem::KernelProgram,
    displacement: eqiora::RawId,
) -> Result<(), String> {
    let Some(KernelNode::Field(field)) = program.node(displacement) else {
        return Err("structural displacement alias does not identify a Field".to_owned());
    };
    if field.dimension() != DISPLACEMENT_DIMENSION {
        return Err("structural displacement Field is not measured in metres".to_owned());
    }
    Ok(())
}

fn project_vertices(
    count: Option<usize>,
    coordinates_for: impl Fn(MeshEntity) -> Option<Vec<f64>>,
) -> Result<Vec<VertexProjection>, String> {
    let count = count.ok_or_else(|| "structural mesh omitted its vertex count".to_owned())?;
    (0..count)
        .map(|index| {
            let coordinates = coordinates_for(MeshEntity::new(0, index))
                .ok_or_else(|| format!("structural mesh omitted vertex {index}"))?;
            let coordinates_m = <[f64; 2]>::try_from(coordinates)
                .map_err(|_| format!("structural mesh vertex {index} is not two-dimensional"))?;
            Ok(VertexProjection {
                index,
                coordinates_m,
            })
        })
        .collect()
}

fn project_cells(
    count: Option<usize>,
    vertices_for: impl Fn(MeshEntity) -> Option<Vec<MeshEntity>>,
) -> Result<Vec<CellProjection>, String> {
    let count = count.ok_or_else(|| "structural mesh omitted its cell count".to_owned())?;
    (0..count)
        .map(|index| {
            let vertices = vertices_for(MeshEntity::new(2, index))
                .ok_or_else(|| format!("structural mesh omitted cell {index} connectivity"))?
                .into_iter()
                .map(MeshEntity::index)
                .collect::<Vec<_>>();
            let vertices = <[usize; 4]>::try_from(vertices)
                .map_err(|_| format!("structural mesh cell {index} is not Q1 quadrilateral"))?;
            Ok(CellProjection { index, vertices })
        })
        .collect()
}

fn validate_payload(
    vertices: &[VertexProjection],
    cells: &[CellProjection],
    displacements: &[[f64; 2]],
) -> Result<(), String> {
    let expected_vertices = (CELLS_PER_AXIS + 1).pow(2);
    let expected_cells = CELLS_PER_AXIS.pow(2);
    if vertices.len() != expected_vertices
        || displacements.len() != expected_vertices
        || cells.len() != expected_cells
    {
        return Err("structural result shape differs from the frozen Q1 mesh".to_owned());
    }
    if vertices
        .iter()
        .any(|vertex| !vertex.coordinates_m.into_iter().all(f64::is_finite))
        || displacements
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || cells
            .iter()
            .flat_map(|cell| cell.vertices)
            .any(|vertex| vertex >= expected_vertices)
    {
        return Err("structural result contains nonfinite or out-of-range mesh data".to_owned());
    }
    Ok(())
}

fn execution_provenance() -> Result<ExecutionProvenanceV1, String> {
    ExecutionProvenanceV1::from_provider_releases(
        REFERENCE_LINEAR_SOLVER.provider(),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
        std::iter::empty::<(&str, &str)>(),
    )
    .map_err(error)
}

fn convergence_reason(reason: ConvergenceReason) -> &'static str {
    match reason {
        ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
        ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
    }
}

fn validate_scientific_case(manifest: &str) -> Result<(), String> {
    let exact_line = |key: &str| {
        manifest
            .lines()
            .find(|line| line.starts_with(key))
            .map(str::trim)
    };
    if exact_line("id") != Some("id = \"solid.mixed-boundary-elasticity-2d\"")
        || exact_line("status") != Some("status = \"verified\"")
    {
        return Err(format!(
            "registered scientific case `{SCIENTIFIC_CASE_ID}` is missing or no longer verified"
        ));
    }
    Ok(())
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
    fn scientific_case_reference_fails_closed_when_stale() {
        assert!(validate_scientific_case(SCIENTIFIC_CASE).is_ok());
        assert!(
            validate_scientific_case(
                &SCIENTIFIC_CASE.replace("status = \"verified\"", "status = \"candidate\"")
            )
            .is_err()
        );
        assert!(
            validate_scientific_case(
                &SCIENTIFIC_CASE.replace(SCIENTIFIC_CASE_ID, "solid.another-case")
            )
            .is_err()
        );
    }

    #[test]
    fn demo_projects_only_solver_owned_structural_evidence() {
        let result = prepare_demo().expect("accepted structural demonstration");
        assert_eq!(result.mesh.vertices.len(), 289);
        assert_eq!(result.mesh.cells.len(), 256);
        assert_eq!(result.displacement.values_m.len(), 289);
        assert_eq!(result.lineage.output_artifacts, 0);
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
        assert_eq!(encoded["evidence"]["caseId"], SCIENTIFIC_CASE_ID);
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
