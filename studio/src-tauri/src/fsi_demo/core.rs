//! Closed transport projection over one accepted two-step FSI composition.

use eqiora::meshing::{MeshEntity, MeshTopology};
use eqiora::numerics::ResolvedFixedReferenceFsiSolution2d;
use eqiora::solver::ConvergenceReason;
use serde::Serialize;

use super::composition::{AcceptedComposition, compose};

const DEMO_PROTOCOL: &str = "eqiora.studio.fixed-reference-fsi-demo/v1";
const EXAMPLE_ID: &str = "fixed-reference-monolithic-fsi-step";
const STEP_CASE_ID: &str = "fsi.fixed-reference-monolithic-step-2d";
const TRAJECTORY_CASE_ID: &str = "artifacts.fixed-reference-fsi-spatial-trajectory";
const STEP_CASE: &str =
    include_str!("../../../../verify/fsi/fixed-reference-monolithic-step-2d/case.toml");
const TRAJECTORY_CASE: &str =
    include_str!("../../../../verify/artifacts/fixed-reference-fsi-spatial-trajectory/case.toml");
const MODEL_SOURCE: &str =
    include_str!("../../../../verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi");

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FsiDemoResult {
    protocol: &'static str,
    example_id: &'static str,
    mesh: MeshProjection,
    steps: Vec<StepProjection>,
    execution: ExecutionContract,
    lineage: LineageEvidence,
    evidence: Vec<EvidenceAttribution>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshProjection {
    vertices: Vec<VertexProjection>,
    cells: Vec<CellProjection>,
    interface_facets: Vec<InterfaceFacetProjection>,
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
    vertices: [usize; 3],
    region: Region,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Region {
    Fluid,
    Solid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InterfaceFacetProjection {
    index: usize,
    vertices: [usize; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StepProjection {
    step: u64,
    time_s: f64,
    velocity: VectorFieldProjection,
    fluid_bubble_velocity: VectorFieldProjection,
    pressure: PressureProjection,
    displacement: VectorFieldProjection,
    interface_actions: Vec<InterfaceActionProjection>,
    energy: EnergyProjection,
    physics_acceptance: PhysicsAcceptance,
    solver_stopping: SolverStopping,
    assembly: AssemblyEvidence,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VectorFieldProjection {
    unit: &'static str,
    values: Vec<[f64; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PressureProjection {
    unit: &'static str,
    support_vertices: Vec<usize>,
    values: Vec<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InterfaceActionProjection {
    vertex: usize,
    unit: &'static str,
    fluid: [f64; 2],
    solid: [f64; 2],
    imbalance: [f64; 2],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnergyProjection {
    unit: &'static str,
    previous_kinetic: f64,
    next_kinetic: f64,
    previous_elastic: f64,
    next_elastic: f64,
    kinetic_increment: f64,
    elastic_increment: f64,
    viscous_dissipation: f64,
    defect: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsAcceptance {
    numerical_residual_norm: f64,
    continuity_residual_norm: f64,
    kinematic_residual_norm: f64,
    interface_velocity_jump_norm: f64,
    interface_action_imbalance_n_per_m: f64,
    absolute_energy_defect_j_per_m: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SolverStopping {
    convergence_reason: &'static str,
    completed_iterations: usize,
    true_residual_norm: f64,
    residual_target: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssemblyEvidence {
    packet_count: usize,
    target_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionContract {
    method: &'static str,
    fluid_space: &'static str,
    solid_space: &'static str,
    time_method: &'static str,
    time_step_s: f64,
    length_scale_m: f64,
    velocity_scale_m_per_s: f64,
    pressure_scale_pa: f64,
    scalar_type: &'static str,
    placement: &'static str,
    solver: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    relative_tolerance: f64,
    absolute_tolerance: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LineageEvidence {
    model_digest: String,
    geometry_digest: String,
    correspondence_digest: String,
    mesh_digest: String,
    realization_digest: String,
    run_digest: String,
    state_digests: Vec<String>,
    trajectory_digest: String,
    semantic_revision: u64,
    realization_revision: u64,
    run_output_artifacts: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceAttribution {
    case_id: &'static str,
    status: &'static str,
}

pub(super) fn prepare_demo() -> Result<FsiDemoResult, String> {
    validate_scientific_case(STEP_CASE, STEP_CASE_ID)?;
    validate_scientific_case(TRAJECTORY_CASE, TRAJECTORY_CASE_ID)?;
    let accepted = compose(MODEL_SOURCE)?;
    let mesh = project_mesh(&accepted)?;
    let steps = vec![
        project_step(1, 0.05, &accepted.first),
        project_step(2, 0.10, &accepted.second),
    ];
    let plan = accepted
        .second
        .numerical_evidence()
        .solve_report()
        .solver_plan();
    let result = FsiDemoResult {
        protocol: DEMO_PROTOCOL,
        example_id: EXAMPLE_ID,
        mesh,
        steps,
        execution: ExecutionContract {
            method: "fixed-reference-monolithic-fsi",
            fluid_space: "continuous-mini-velocity-p1-pressure",
            solid_space: "continuous-p1-velocity-displacement",
            time_method: "backward-euler",
            time_step_s: 0.05,
            length_scale_m: 2.0,
            velocity_scale_m_per_s: 0.5,
            pressure_scale_pa: 4.0,
            scalar_type: "f64",
            placement: "one-host-one-worker",
            solver: "minimum-residual",
            preconditioner: "identity",
            reduction: "reproducible",
            relative_tolerance: plan.relative_tolerance(),
            absolute_tolerance: plan.absolute_tolerance(),
        },
        lineage: LineageEvidence {
            model_digest: accepted.spatial.model.digest().map_err(error)?.to_string(),
            geometry_digest: accepted
                .spatial
                .geometry
                .digest()
                .map_err(error)?
                .to_string(),
            correspondence_digest: accepted
                .spatial
                .correspondence
                .digest()
                .map_err(error)?
                .to_string(),
            mesh_digest: accepted
                .spatial
                .mesh_artifact
                .digest()
                .map_err(error)?
                .to_string(),
            realization_digest: accepted.realization.digest().map_err(error)?.to_string(),
            run_digest: accepted.run.digest().map_err(error)?.to_string(),
            state_digests: vec![
                accepted.first_state.digest().map_err(error)?.to_string(),
                accepted.second_state.digest().map_err(error)?.to_string(),
            ],
            trajectory_digest: accepted.trajectory.digest().map_err(error)?.to_string(),
            semantic_revision: accepted.document.program().revision().0,
            realization_revision: accepted.second.realization_revision().get(),
            run_output_artifacts: accepted.run.outputs().len(),
        },
        evidence: vec![
            EvidenceAttribution {
                case_id: STEP_CASE_ID,
                status: "verified",
            },
            EvidenceAttribution {
                case_id: TRAJECTORY_CASE_ID,
                status: "verified",
            },
        ],
    };
    validate_payload(&result)?;
    Ok(result)
}

fn project_mesh(accepted: &AcceptedComposition) -> Result<MeshProjection, String> {
    let mesh = &accepted.spatial.mesh;
    let vertices = mesh
        .vertices()
        .iter()
        .enumerate()
        .map(|(index, coordinates)| {
            <[f64; 2]>::try_from(coordinates.as_slice())
                .map(|coordinates_m| VertexProjection {
                    index,
                    coordinates_m,
                })
                .map_err(|_| format!("FSI mesh vertex {index} is not two-dimensional"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cells = (0..mesh
        .entity_count(2)
        .ok_or_else(|| "FSI mesh omitted its cell count".to_owned())?)
        .map(|index| {
            let cell = eqiora::meshing::CellId::new(index);
            let vertices = mesh
                .entity_vertices(MeshEntity::new(2, index))
                .ok_or_else(|| format!("FSI mesh omitted cell {index} connectivity"))?
                .into_iter()
                .map(MeshEntity::index)
                .collect::<Vec<_>>();
            let vertices = <[usize; 3]>::try_from(vertices)
                .map_err(|_| format!("FSI mesh cell {index} is not triangular"))?;
            let region = if accepted
                .spatial
                .partition
                .fluid_cells()
                .binary_search(&cell)
                .is_ok()
            {
                Region::Fluid
            } else if accepted
                .spatial
                .partition
                .solid_cells()
                .binary_search(&cell)
                .is_ok()
            {
                Region::Solid
            } else {
                return Err(format!("FSI mesh cell {index} has no physical region"));
            };
            Ok(CellProjection {
                index,
                vertices,
                region,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let interface_facets = accepted
        .spatial
        .partition
        .interface_facets()
        .iter()
        .copied()
        .map(|facet| {
            let vertices = mesh
                .entity_vertices(MeshEntity::new(1, facet.index()))
                .ok_or_else(|| format!("FSI mesh omitted interface facet {}", facet.index()))?
                .into_iter()
                .map(MeshEntity::index)
                .collect::<Vec<_>>();
            Ok(InterfaceFacetProjection {
                index: facet.index(),
                vertices: <[usize; 2]>::try_from(vertices)
                    .map_err(|_| "FSI interface facet is not an edge".to_owned())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(MeshProjection {
        vertices,
        cells,
        interface_facets,
    })
}

fn project_step(
    step: u64,
    time_s: f64,
    solution: &ResolvedFixedReferenceFsiSolution2d,
) -> StepProjection {
    let numerical = solution.numerical_evidence();
    let energy = numerical.energy_balance();
    let report = numerical.solve_report();
    let assembly = numerical.assembly_report();
    StepProjection {
        step,
        time_s,
        velocity: VectorFieldProjection {
            unit: "m/s",
            values: solution.vertex_velocity_coefficients().to_vec(),
        },
        fluid_bubble_velocity: VectorFieldProjection {
            unit: "m/s",
            values: solution.fluid_velocity_bubble_coefficients().to_vec(),
        },
        pressure: PressureProjection {
            unit: "kg m^-1 s^-2",
            support_vertices: solution
                .fluid_pressure_vertices()
                .iter()
                .map(|vertex| vertex.index())
                .collect(),
            values: solution.fluid_pressure_coefficients().to_vec(),
        },
        displacement: VectorFieldProjection {
            unit: "m",
            values: solution.solid_displacement_coefficients().to_vec(),
        },
        interface_actions: numerical
            .interface_actions()
            .iter()
            .copied()
            .map(|action| InterfaceActionProjection {
                vertex: action.vertex().index(),
                unit: "N/m",
                fluid: action.fluid(),
                solid: action.solid(),
                imbalance: action.imbalance(),
            })
            .collect(),
        energy: EnergyProjection {
            unit: "J/m",
            previous_kinetic: energy.previous_kinetic(),
            next_kinetic: energy.next_kinetic(),
            previous_elastic: energy.previous_elastic(),
            next_elastic: energy.next_elastic(),
            kinetic_increment: energy.kinetic_increment(),
            elastic_increment: energy.elastic_increment(),
            viscous_dissipation: energy.viscous_dissipation(),
            defect: energy.defect(),
        },
        physics_acceptance: PhysicsAcceptance {
            numerical_residual_norm: numerical.residual_norm(),
            continuity_residual_norm: numerical.continuity_residual_norm(),
            kinematic_residual_norm: numerical.kinematic_residual_norm(),
            interface_velocity_jump_norm: numerical.interface_velocity_jump_norm(),
            interface_action_imbalance_n_per_m: numerical.interface_action_imbalance_norm(),
            absolute_energy_defect_j_per_m: energy.defect().abs(),
        },
        solver_stopping: SolverStopping {
            convergence_reason: convergence_reason(report.reason()),
            completed_iterations: report.completed_iterations(),
            true_residual_norm: report.true_residual_norm(),
            residual_target: report.residual_target(),
        },
        assembly: AssemblyEvidence {
            packet_count: assembly.packet_count(),
            target_count: assembly.target_count(),
        },
    }
}

fn validate_payload(result: &FsiDemoResult) -> Result<(), String> {
    if result.mesh.vertices.len() != 9
        || result.mesh.cells.len() != 8
        || result.mesh.interface_facets.len() != 2
        || result
            .mesh
            .cells
            .iter()
            .filter(|cell| matches!(cell.region, Region::Fluid))
            .count()
            != 4
        || result
            .mesh
            .cells
            .iter()
            .filter(|cell| matches!(cell.region, Region::Solid))
            .count()
            != 4
    {
        return Err("FSI result shape differs from the frozen two-body mesh".to_owned());
    }
    let expected_cells = [
        [0, 1, 3],
        [0, 3, 2],
        [2, 3, 5],
        [2, 5, 4],
        [1, 6, 7],
        [1, 7, 3],
        [3, 7, 8],
        [3, 8, 5],
    ];
    if result
        .mesh
        .cells
        .iter()
        .map(|cell| cell.vertices)
        .ne(expected_cells)
    {
        return Err("FSI result changed the frozen ordered cell connectivity".to_owned());
    }
    if result.steps.len() != 2
        || result.steps[0].step != 1
        || result.steps[0].time_s != 0.05
        || result.steps[1].step != 2
        || result.steps[1].time_s != 0.10
        || result.steps[0].displacement.values == result.steps[1].displacement.values
    {
        return Err("FSI result omitted two distinct consecutive accepted steps".to_owned());
    }
    for step in &result.steps {
        let report = &step.solver_stopping;
        let acceptance = &step.physics_acceptance;
        if step.velocity.values.len() != 9
            || step.fluid_bubble_velocity.values.len() != 4
            || step.pressure.support_vertices != [0, 1, 2, 3, 4, 5]
            || step.pressure.values.len() != 6
            || step.displacement.values.len() != 9
            || step.interface_actions.len() != 1
            || step.interface_actions[0].vertex != 3
            || report.true_residual_norm > report.residual_target
            || acceptance.numerical_residual_norm >= 1.0e-9
            || acceptance.continuity_residual_norm >= 1.0e-9
            || acceptance.kinematic_residual_norm >= 1.0e-14
            || acceptance.interface_velocity_jump_norm != 0.0
            || acceptance.interface_action_imbalance_n_per_m >= 1.0e-9
            || acceptance.absolute_energy_defect_j_per_m >= 1.0e-9
        {
            return Err(
                "FSI result failed its frozen structural or acceptance boundary".to_owned(),
            );
        }
        if step
            .velocity
            .values
            .iter()
            .chain(&step.fluid_bubble_velocity.values)
            .chain(&step.displacement.values)
            .flatten()
            .chain(step.pressure.values.iter())
            .any(|value| !value.is_finite())
        {
            return Err("FSI result contains a nonfinite physical value".to_owned());
        }
        if [0, 2, 4]
            .into_iter()
            .any(|vertex| step.displacement.values[vertex] != [0.0, 0.0])
        {
            return Err("FSI result has nonzero displacement outside the solid closure".to_owned());
        }
    }
    if result.execution.solver != "minimum-residual"
        || result.execution.preconditioner != "identity"
        || result.execution.reduction != "reproducible"
        || result.execution.relative_tolerance != 1.0e-11
        || result.execution.absolute_tolerance != 1.0e-13
        || result.steps.iter().any(|step| {
            !matches!(
                step.solver_stopping.convergence_reason,
                "initial-residual-satisfied" | "residual-tolerance-satisfied"
            )
        })
        || result.lineage.state_digests.len() != 2
        || result.lineage.run_output_artifacts != 1
    {
        return Err("FSI result changed the frozen execution or lineage contract".to_owned());
    }
    Ok(())
}

fn convergence_reason(reason: ConvergenceReason) -> &'static str {
    match reason {
        ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
        ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
    }
}

fn validate_scientific_case(manifest: &str, case_id: &str) -> Result<(), String> {
    let expected_id = format!("id = \"{case_id}\"");
    let exact_line = |key: &str| {
        manifest
            .lines()
            .find(|line| line.starts_with(key))
            .map(str::trim)
    };
    if exact_line("id") != Some(expected_id.as_str())
        || exact_line("status") != Some("status = \"verified\"")
    {
        return Err(format!(
            "registered scientific case `{case_id}` is missing or no longer verified"
        ));
    }
    Ok(())
}

fn error(error: eqiora::Diagnostic) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scientific_case_references_fail_closed_when_stale() {
        for (manifest, case_id) in [
            (STEP_CASE, STEP_CASE_ID),
            (TRAJECTORY_CASE, TRAJECTORY_CASE_ID),
        ] {
            assert!(validate_scientific_case(manifest, case_id).is_ok());
            assert!(
                validate_scientific_case(
                    &manifest.replace("status = \"verified\"", "status = \"candidate\""),
                    case_id,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn demo_projects_two_solver_owned_fsi_steps() {
        let result = prepare_demo().expect("accepted FSI demonstration");
        assert_eq!(result.mesh.vertices.len(), 9);
        assert_eq!(result.mesh.cells.len(), 8);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.lineage.run_output_artifacts, 1);
        assert_ne!(
            result.steps[0].displacement.values,
            result.steps[1].displacement.values
        );

        let encoded = serde_json::to_value(&result).expect("serialize closed result");
        assert_eq!(encoded["protocol"], DEMO_PROTOCOL);
        assert_eq!(encoded["steps"][0]["interfaceActions"][0]["unit"], "N/m");
        assert_eq!(encoded["steps"][0]["energy"]["unit"], "J/m");
        for forbidden in ["stress", "drag", "lift", "exactSolution", "analyticError"] {
            assert!(
                !encoded
                    .as_object()
                    .expect("result object")
                    .contains_key(forbidden)
            );
        }
    }
}
