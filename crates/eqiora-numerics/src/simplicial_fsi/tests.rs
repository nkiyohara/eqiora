//! Focused falsifiers for the fixed-reference CPU realization.

use std::num::NonZeroUsize;

use eqiora_core::{Diagnostic, diagnostic::codes};
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearSolveRequest, LinearSolver,
    PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy, SolverPlan,
};

use crate::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyResult, AssemblyTarget,
    CellId, DofId, FacetId, IndexedAssemblyWork, LocalContribution, LocalUnknown, MeshEntity,
    MeshQualityGate, MeshTopology, QuadratureRule, REFERENCE_ASSEMBLY_BACKEND, SimplicialMesh,
    TargetAssemblyMap, VertexId, simplex_duffy_gauss_legendre, triangle_duffy_gauss_legendre,
};

use super::*;

#[test]
fn exact_partition_rejects_missing_and_extra_interface_facets() {
    let mesh = two_domain_mesh();
    let (fluid, solid, interface) = inventories(&mesh);
    assert!(
        FixedReferenceFsiPartition2d::new(&mesh, fluid.clone(), solid.clone(), interface.clone())
            .is_ok()
    );
    assert!(
        FixedReferenceFsiPartition2d::new(&mesh, fluid.clone(), solid.clone(), Vec::new()).is_err()
    );
    let mut incomplete_cells = fluid;
    incomplete_cells.pop();
    assert!(FixedReferenceFsiPartition2d::new(&mesh, incomplete_cells, solid, interface).is_err());
}

#[test]
fn tetrahedral_contract_replays_one_exact_interface_and_dimensioned_state() {
    let mesh = two_tetrahedron_mesh();
    let interface = shared_tetrahedron_interface(&mesh);
    let partition = FixedReferenceFsiPartition3d::new(
        &mesh,
        vec![CellId::new(0)],
        vec![CellId::new(1)],
        vec![interface],
    )
    .unwrap();
    let witness = partition.interface_witnesses()[0];
    assert_eq!(witness.facet(), interface);
    assert_eq!(witness.fluid_cell(), CellId::new(0));
    assert_eq!(witness.solid_cell(), CellId::new(1));
    let boundary = FixedReferenceFsiBoundary3d::homogeneous_exterior(&mesh).unwrap();
    let previous = FixedReferenceFsiState3d::new(
        &mesh,
        &partition,
        vec![[0.0; 3]; mesh.vertices().len()],
        vec![[0.0; 3]; partition.fluid_cells().len()],
        vec![[0.0; 3]; mesh.vertices().len()],
    )
    .unwrap();
    let material = FixedReferenceFsiMaterial3d::new(1.0, 0.1, 2.0, 3.0, 1.0).unwrap();
    let scale = FixedReferenceFsiScale3d::new(2.0, 5.0, 3.0).unwrap();
    assert_eq!(scale.action(), 12.0);
    assert_eq!(scale.energy(), 24.0);
    assert_eq!(scale.power(), 60.0);
    let config =
        FixedReferenceFsiStepConfig3d::new(0.25, material, scale, FixedReferenceFsiLoad3d::Zero)
            .unwrap();
    let under_integrated = validate_problem(
        &mesh,
        &partition,
        &boundary,
        &previous,
        config,
        &simplex_duffy_gauss_legendre(3, 5).unwrap(),
    )
    .unwrap_err();
    assert!(under_integrated.message().contains("degree 8"));
    validate_problem(
        &mesh,
        &partition,
        &boundary,
        &previous,
        config,
        &simplex_duffy_gauss_legendre(3, 6).unwrap(),
    )
    .unwrap();
    let problem_2d = fixture_problem();
    assert_eq!(problem_2d.quadrature.polynomial_exactness(), Some(6));
    validate_problem(
        &problem_2d.mesh,
        &problem_2d.partition,
        &problem_2d.boundary,
        &problem_2d.previous,
        problem_2d.config,
        &problem_2d.quadrature,
    )
    .unwrap();

    assert!(
        FixedReferenceFsiPartition3d::new(
            &mesh,
            vec![CellId::new(0)],
            vec![CellId::new(1)],
            Vec::new(),
        )
        .is_err()
    );
    assert!(
        FixedReferenceFsiPartition3d::new(
            &two_domain_mesh(),
            vec![CellId::new(0)],
            vec![CellId::new(1)],
            vec![FacetId::new(0)],
        )
        .is_err()
    );
}

#[test]
fn material_coercivity_depends_on_the_admitted_spatial_dimension() {
    assert!(FixedReferenceFsiMaterial2d::new(1.0, 0.1, 1.0, 1.0, -0.8).is_ok());
    let error = FixedReferenceFsiMaterial3d::new(1.0, 0.1, 1.0, 1.0, -0.8).unwrap_err();
    assert!(error.message().contains("coercive"));
}

#[test]
fn finalized_operator_is_symmetric_and_constant_pressure_is_closed_by_interface_action() {
    let problem = fixture_problem();
    let finalized = finalize_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
    )
    .unwrap();
    assert_eq!(
        finalized.linear_system().properties(),
        LinearOperatorProperties::SymmetricIndefinite
    );
    let system = finalized.linear_system();
    for row in 0..system.rows() {
        for column in 0..system.columns() {
            let left = canonical_entry(system, row, column);
            let right = canonical_entry(system, column, row);
            assert!((left - right).abs() < 2.0e-13);
        }
    }
}

#[test]
fn finalized_step_retains_exact_reduced_and_full_target_roles() {
    let problem = fixture_problem();
    let finalized = finalize_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
    )
    .unwrap();
    let roles = finalized.assembly_target_roles();
    assert_eq!(roles.reduced().index(), 0);
    assert_eq!(roles.full().index(), 1);
    assert_ne!(roles.reduced(), roles.full());
}

#[test]
fn monolithic_step_closes_residual_kinematics_interface_and_energy() {
    let problem = fixture_problem();
    let solution = solve_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();
    assert!(solution.residual_norm() < 1.0e-9);
    assert!(solution.continuity_residual_norm() < 1.0e-9);
    assert!(solution.kinematic_residual_norm() < 1.0e-14);
    assert_eq!(solution.interface_velocity_jump_norm(), 0.0);
    assert!(!solution.interface_actions().is_empty());
    assert!(solution.interface_action_imbalance_norm() < 1.0e-9);
    assert!(solution.energy_balance().defect().abs() < 1.0e-9);
    assert!(
        solution
            .vertex_velocity()
            .iter()
            .flatten()
            .any(|value| value.abs() > 1.0e-10)
    );
}

#[test]
fn tetrahedral_monolithic_step_closes_the_same_physical_acceptance() {
    let problem = fixture_problem_3d();
    let solution = solve_fixed_reference_fsi_step_3d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();

    assert_eq!(solution.vertex_velocity().first().unwrap().len(), 3);
    assert!(solution.residual_norm() < 1.0e-9);
    assert!(solution.continuity_residual_norm() < 1.0e-9);
    assert!(solution.kinematic_residual_norm() < 1.0e-14);
    assert_eq!(solution.interface_velocity_jump_norm(), 0.0);
    assert_eq!(solution.interface_actions().len(), 1);
    assert!(solution.interface_action_imbalance_norm() < 1.0e-9);
    assert!(solution.energy_balance().defect().abs() < 1.0e-9);
    assert!(
        solution
            .vertex_velocity()
            .iter()
            .flatten()
            .any(|value| value.abs() > 1.0e-10)
    );
}

#[test]
fn tetrahedral_physical_step_is_invariant_under_dimensioned_scale_profiles() {
    let problem = fixture_problem_3d();
    let reference = solve_fixed_reference_fsi_step_3d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();
    let rescaled_config = FixedReferenceFsiStepConfig3d::new(
        problem.config.time_step(),
        problem.config.material(),
        FixedReferenceFsiScale3d::new(4.0, 0.25, 3.0).unwrap(),
        FixedReferenceFsiLoad3d::Zero,
    )
    .unwrap();
    let rescaled = solve_fixed_reference_fsi_step_3d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        rescaled_config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();

    assert_close(
        reference.vertex_velocity().iter().flatten().copied(),
        rescaled.vertex_velocity().iter().flatten().copied(),
    );
    assert_close(
        reference.fluid_pressure().iter().copied(),
        rescaled.fluid_pressure().iter().copied(),
    );
    assert_close(
        reference.solid_displacement().iter().flatten().copied(),
        rescaled.solid_displacement().iter().flatten().copied(),
    );
    assert_eq!(
        reference
            .interface_actions()
            .iter()
            .map(|action| action.vertex())
            .collect::<Vec<_>>(),
        rescaled
            .interface_actions()
            .iter()
            .map(|action| action.vertex())
            .collect::<Vec<_>>()
    );
    assert_close(
        reference
            .interface_actions()
            .iter()
            .flat_map(|action| action.fluid().into_iter().chain(action.solid())),
        rescaled
            .interface_actions()
            .iter()
            .flat_map(|action| action.fluid().into_iter().chain(action.solid())),
    );
}

#[test]
fn physical_step_is_invariant_under_admitted_scale_profiles() {
    let problem = fixture_problem();
    let reference = solve_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();
    let rescaled_config = FixedReferenceFsiStepConfig2d::new(
        problem.config.time_step(),
        problem.config.material(),
        FixedReferenceFsiScale2d::new(2.0, 0.25, 3.0).unwrap(),
        FixedReferenceFsiLoad2d::Zero,
    )
    .unwrap();
    let rescaled = solve_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        rescaled_config,
        &problem.quadrature,
        reference_solver(),
    )
    .unwrap();
    for (left, right) in reference
        .vertex_velocity()
        .iter()
        .flatten()
        .zip(rescaled.vertex_velocity().iter().flatten())
    {
        assert!((left - right).abs() < 2.0e-9);
    }
    for (left, right) in reference
        .fluid_pressure()
        .iter()
        .zip(rescaled.fluid_pressure())
    {
        assert!((left - right).abs() < 2.0e-9);
    }
    for (left, right) in reference
        .solid_displacement()
        .iter()
        .flatten()
        .zip(rescaled.solid_displacement().iter().flatten())
    {
        assert!((left - right).abs() < 2.0e-9);
    }
}

#[test]
fn zero_interface_action_rejects_an_unclosed_pressure_mode() {
    let problem = fixture_problem();
    let all_fixed = FixedReferenceFsiBoundary2d::from_fixed_zero_velocity_vertices(
        (0..problem.mesh.vertices().len())
            .map(VertexId::new)
            .collect(),
    );
    let error = finalize_fixed_reference_fsi_step_2d(
        &problem.mesh,
        &problem.partition,
        &all_fixed,
        &problem.previous,
        problem.config,
        &problem.quadrature,
    )
    .unwrap_err();
    assert_eq!(error.code(), codes::INVALID_DISCRETIZATION);
    assert!(error.message().contains("constant pressure unclosed"));
}

#[test]
fn finalization_rejects_a_backend_result_outside_the_prepared_target_shape() {
    let problem = fixture_problem();
    let error = finalize_fixed_reference_fsi_step_2d_with_assembly(
        &problem.mesh,
        &problem.partition,
        &problem.boundary,
        &problem.previous,
        problem.config,
        &problem.quadrature,
        &WrongShapeAssemblyBackend,
    )
    .expect_err("an assembly backend cannot replace the prepared target shapes");
    assert_eq!(error.code(), codes::INVALID_DISCRETIZATION);
    assert!(error.message().contains("prepared target shape"));
}

#[derive(Debug)]
struct WrongShapeAssemblyBackend;

impl AssemblyBackend for WrongShapeAssemblyBackend {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        original_work: &dyn crate::AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        let plan = AssemblyPlan::new(vec![AssemblyTarget::new(1)?, AssemblyTarget::new(1)?])?;
        let reduced = plan
            .target_id(0)
            .expect("two-target malformed test plan owns reduced");
        let full = plan
            .target_id(1)
            .expect("two-target malformed test plan owns full");
        let dof = DofId::new(0);
        let work = IndexedAssemblyWork::new(original_work.packet_count(), move |_| {
            let local = LocalContribution::new(1, 1, vec![1.0], vec![0.0])?;
            let map = || AssemblyMap::new(vec![Some(dof)], vec![LocalUnknown::Free(dof)]);
            AssemblyPacket::new(
                local,
                vec![
                    TargetAssemblyMap::new(reduced, map()?),
                    TargetAssemblyMap::new(full, map()?),
                ],
            )
        });
        REFERENCE_ASSEMBLY_BACKEND.assemble(&plan, &work)
    }
}

struct Fixture {
    mesh: SimplicialMesh,
    partition: FixedReferenceFsiPartition2d,
    boundary: FixedReferenceFsiBoundary2d,
    previous: FixedReferenceFsiState2d,
    config: FixedReferenceFsiStepConfig2d,
    quadrature: QuadratureRule,
}

struct Fixture3d {
    mesh: SimplicialMesh,
    partition: FixedReferenceFsiPartition3d,
    boundary: FixedReferenceFsiBoundary3d,
    previous: FixedReferenceFsiState3d,
    config: FixedReferenceFsiStepConfig3d,
    quadrature: QuadratureRule,
}

fn fixture_problem() -> Fixture {
    let mesh = two_domain_mesh();
    let (fluid, solid, interface) = inventories(&mesh);
    let partition = FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
    let boundary = FixedReferenceFsiBoundary2d::homogeneous_exterior(&mesh).unwrap();
    let mut displacement = vec![[0.0; 2]; mesh.vertices().len()];
    let interface_midpoint = find_vertex(&mesh, [1.0, 0.5]);
    displacement[interface_midpoint][0] = 0.02;
    let previous = FixedReferenceFsiState2d::new(
        &mesh,
        &partition,
        vec![[0.0; 2]; mesh.vertices().len()],
        vec![[0.0; 2]; partition.fluid_cells().len()],
        displacement,
    )
    .unwrap();
    let material = FixedReferenceFsiMaterial2d::new(1.0, 0.05, 1.5, 2.0, 3.0).unwrap();
    let scale = FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap();
    let config =
        FixedReferenceFsiStepConfig2d::new(0.05, material, scale, FixedReferenceFsiLoad2d::Zero)
            .unwrap();
    Fixture {
        mesh,
        partition,
        boundary,
        previous,
        config,
        quadrature: triangle_duffy_gauss_legendre(4).unwrap(),
    }
}

fn fixture_problem_3d() -> Fixture3d {
    let mesh = interface_bipyramid_mesh();
    let fluid = (0..4).map(CellId::new).collect::<Vec<_>>();
    let solid = (4..8).map(CellId::new).collect::<Vec<_>>();
    let interface = (0..mesh.entity_count(2).unwrap())
        .filter(|&facet| {
            mesh.entity_vertices(MeshEntity::new(2, facet))
                .unwrap()
                .iter()
                .all(|vertex| mesh.vertices()[vertex.index()][0] == 0.0)
        })
        .map(FacetId::new)
        .collect::<Vec<_>>();
    let partition = FixedReferenceFsiPartition3d::new(&mesh, fluid, solid, interface).unwrap();
    let boundary = FixedReferenceFsiBoundary3d::homogeneous_exterior(&mesh).unwrap();
    let mut displacement = vec![[0.0; 3]; mesh.vertices().len()];
    displacement[2] = [0.015, -0.01, 0.005];
    let previous = FixedReferenceFsiState3d::new(
        &mesh,
        &partition,
        vec![[0.0; 3]; mesh.vertices().len()],
        vec![[0.0; 3]; partition.fluid_cells().len()],
        displacement,
    )
    .unwrap();
    let material = FixedReferenceFsiMaterial3d::new(1.0, 0.05, 1.5, 2.0, 3.0).unwrap();
    let scale = FixedReferenceFsiScale3d::new(2.0, 1.0, 1.0).unwrap();
    let config =
        FixedReferenceFsiStepConfig3d::new(0.05, material, scale, FixedReferenceFsiLoad3d::Zero)
            .unwrap();
    Fixture3d {
        mesh,
        partition,
        boundary,
        previous,
        config,
        quadrature: simplex_duffy_gauss_legendre(3, 6).unwrap(),
    }
}

fn two_domain_mesh() -> SimplicialMesh {
    let mut vertices = Vec::new();
    for y in [0.0, 0.5, 1.0] {
        for x in [0.0, 1.0, 2.0] {
            vertices.push(vec![x, y]);
        }
    }
    let width = 3;
    let mut cells = Vec::new();
    for row in 0..2 {
        for column in 0..2 {
            let lower_left = row * width + column;
            let lower_right = lower_left + 1;
            let upper_left = lower_left + width;
            let upper_right = upper_left + 1;
            cells.push(vec![lower_left, lower_right, upper_right]);
            cells.push(vec![lower_left, upper_right, upper_left]);
        }
    }
    SimplicialMesh::new(2, vertices, cells, MeshQualityGate::new(0.3).unwrap()).unwrap()
}

fn two_tetrahedron_mesh() -> SimplicialMesh {
    SimplicialMesh::new(
        3,
        vec![
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![0.0, 0.0, -1.0],
        ],
        vec![vec![0, 1, 2, 3], vec![0, 2, 1, 4]],
        MeshQualityGate::new(0.05).unwrap(),
    )
    .unwrap()
}

fn interface_bipyramid_mesh() -> SimplicialMesh {
    let vertices = vec![
        vec![-1.0, 0.0, 0.0],
        vec![1.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![0.0, 0.0, 1.0],
        vec![0.0, -1.0, 0.0],
        vec![0.0, 0.0, -1.0],
    ];
    let ring = [3, 4, 5, 6];
    let mut cells = Vec::new();
    for apex in [0, 1] {
        for edge in 0..ring.len() {
            let mut cell = vec![apex, 2, ring[edge], ring[(edge + 1) % ring.len()]];
            if signed_tetrahedron_jacobian(&vertices, &cell) < 0.0 {
                cell.swap(1, 2);
            }
            cells.push(cell);
        }
    }
    SimplicialMesh::new(3, vertices, cells, MeshQualityGate::new(0.05).unwrap()).unwrap()
}

fn signed_tetrahedron_jacobian(vertices: &[Vec<f64>], cell: &[usize]) -> f64 {
    let origin = &vertices[cell[0]];
    let column = |vertex: usize, axis: usize| vertices[cell[vertex]][axis] - origin[axis];
    column(1, 0) * (column(2, 1) * column(3, 2) - column(2, 2) * column(3, 1))
        - column(2, 0) * (column(1, 1) * column(3, 2) - column(1, 2) * column(3, 1))
        + column(3, 0) * (column(1, 1) * column(2, 2) - column(1, 2) * column(2, 1))
}

fn shared_tetrahedron_interface(mesh: &SimplicialMesh) -> FacetId {
    (0..mesh.entity_count(2).unwrap())
        .find(|&facet| {
            mesh.entity_vertices(MeshEntity::new(2, facet))
                .unwrap()
                .iter()
                .map(|vertex| vertex.index())
                .collect::<Vec<_>>()
                == [0, 1, 2]
        })
        .map(FacetId::new)
        .unwrap()
}

fn inventories(mesh: &SimplicialMesh) -> (Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
    let mut fluid = Vec::new();
    let mut solid = Vec::new();
    for (index, cell) in mesh.cells().iter().enumerate() {
        let centroid_x = cell
            .iter()
            .map(|vertex| mesh.vertices()[*vertex][0])
            .sum::<f64>()
            / 3.0;
        if centroid_x < 1.0 {
            fluid.push(CellId::new(index));
        } else {
            solid.push(CellId::new(index));
        }
    }
    let interface = (0..mesh.entity_count(1).unwrap())
        .filter(|&facet| {
            mesh.entity_vertices(MeshEntity::new(1, facet))
                .unwrap()
                .iter()
                .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
        })
        .map(FacetId::new)
        .collect();
    (fluid, solid, interface)
}

fn find_vertex(mesh: &SimplicialMesh, target: [f64; 2]) -> usize {
    mesh.vertices()
        .iter()
        .position(|coordinates| coordinates.as_slice() == target)
        .unwrap()
}

fn reference_solver() -> LinearSolveRequest<'static> {
    let plan = SolverPlan::new(
        LinearSolver::MinimumResidual,
        1.0e-11,
        1.0e-13,
        NonZeroUsize::new(20_000).unwrap(),
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Reproducible);
    LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan)
}

fn canonical_entry(system: &CanonicalCsrSystemView, row: usize, column: usize) -> f64 {
    let start = system.row_offsets()[row];
    let end = system.row_offsets()[row + 1];
    match system.column_indices()[start..end].binary_search(&column) {
        Ok(position) => system.values()[start + position],
        Err(_) => 0.0,
    }
}

fn assert_close(left: impl IntoIterator<Item = f64>, right: impl IntoIterator<Item = f64>) {
    let mut left = left.into_iter();
    let mut right = right.into_iter();
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) => {
                let tolerance = 2.0e-9 + 2.0e-9 * left.abs().max(right.abs());
                assert!((left - right).abs() <= tolerance, "{left:e} != {right:e}");
            }
            (None, None) => break,
            _ => panic!("compared physical fields have different coefficient counts"),
        }
    }
}
