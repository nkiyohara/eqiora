use std::num::NonZeroUsize;

use eqiora_assembly::{AssemblyMap, CooAssembler, DofId, LocalContribution, LocalUnknown};
use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology, QuadratureRule};
use eqiora_numerics::solid::{
    lower_cartesian_q1_linear_elasticity_local_action_2d, lower_isotropic_elasticity_cartesian_2d,
    solve_cartesian_q1_linear_elasticity_2d,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearSolveRequest, LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

const SOURCE: &str =
    include_str!("../../../verify/solid/isotropic-elasticity-2d/models/linear-load.eqi");
const MATRIX: [f64; 64] = [
    11.0 / 3.0,
    5.0 / 4.0,
    -13.0 / 6.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    1.0 / 4.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    5.0 / 4.0,
    11.0 / 3.0,
    1.0 / 4.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -13.0 / 6.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    11.0 / 3.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    5.0 / 4.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    -5.0 / 4.0,
    11.0 / 3.0,
    5.0 / 4.0,
    -11.0 / 6.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 3.0,
    -1.0 / 4.0,
    -11.0 / 6.0,
    5.0 / 4.0,
    11.0 / 3.0,
    -5.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    5.0 / 4.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    11.0 / 3.0,
    -1.0 / 4.0,
    1.0 / 3.0,
    -11.0 / 6.0,
    -5.0 / 4.0,
    1.0 / 3.0,
    1.0 / 4.0,
    -13.0 / 6.0,
    -1.0 / 4.0,
    11.0 / 3.0,
    5.0 / 4.0,
    -5.0 / 4.0,
    -11.0 / 6.0,
    -1.0 / 4.0,
    -13.0 / 6.0,
    1.0 / 4.0,
    1.0 / 3.0,
    5.0 / 4.0,
    11.0 / 3.0,
];
const ABSOLUTE_TOLERANCE: f64 = 1.0e-9;

#[test]
fn ordinary_local_path_matches_the_frozen_cell_and_affine_patch() {
    let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 2]).unwrap();
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let action =
        lower_cartesian_q1_linear_elasticity_local_action_2d(&mesh, 3.0, 2.0, &quadrature).unwrap();
    assert_absolute_slice(&action.coefficients()[..64], &MATRIX);

    let vertex_count = mesh.entity_count(0).unwrap();
    let mut assembler = CooAssembler::new(2 * vertex_count).unwrap();
    for cell_index in 0..mesh.entity_count(2).unwrap() {
        let offset = cell_index * 64;
        let local = LocalContribution::new(
            8,
            8,
            action.coefficients()[offset..offset + 64].to_vec(),
            vec![0.0; 8],
        )
        .unwrap();
        let vertices = mesh
            .entity_vertices(MeshEntity::new(2, cell_index))
            .unwrap();
        let global = vertices
            .iter()
            .flat_map(|vertex| [2 * vertex.index(), 2 * vertex.index() + 1])
            .collect::<Vec<_>>();
        let map = AssemblyMap::new(
            global
                .iter()
                .map(|index| Some(DofId::new(*index)))
                .collect(),
            global
                .iter()
                .map(|index| LocalUnknown::Free(DofId::new(*index)))
                .collect(),
        )
        .unwrap();
        assembler.scatter(&map, &local).unwrap();
    }
    let system = assembler.finish().unwrap();
    let center = (0..vertex_count)
        .find(|vertex| {
            mesh.vertex_coordinates(MeshEntity::new(0, *vertex))
                .unwrap()
                == [0.5, 0.5]
        })
        .unwrap();
    let mut center_x = vec![0.0; 2 * vertex_count];
    center_x[2 * center] = 1.0;
    let mut center_y = vec![0.0; 2 * vertex_count];
    center_y[2 * center + 1] = 1.0;
    let column_x = system.matrix().multiply(&center_x).unwrap();
    let column_y = system.matrix().multiply(&center_y).unwrap();
    assert_absolute_slice(
        &[
            column_x[2 * center],
            column_y[2 * center],
            column_x[2 * center + 1],
            column_y[2 * center + 1],
        ],
        &[44.0 / 3.0, 0.0, 0.0, 44.0 / 3.0],
    );

    let displacement = (0..vertex_count)
        .flat_map(|vertex| {
            let point = mesh.vertex_coordinates(MeshEntity::new(0, vertex)).unwrap();
            [
                2.0 * point[0] + 3.0 * point[1] + 1.0,
                5.0 * point[0] + 7.0 * point[1] - 2.0,
            ]
        })
        .collect::<Vec<_>>();
    let elastic_force = system.matrix().multiply(&displacement).unwrap();
    let mut resultant = [0.0; 2];
    for vertex in 0..vertex_count {
        let force = &elastic_force[2 * vertex..2 * vertex + 2];
        if vertex == center {
            assert_absolute_slice(force, &[0.0, 0.0]);
        } else {
            resultant[0] += force[0];
            resultant[1] += force[1];
        }
    }
    assert_absolute_slice(&resultant, &[0.0, 0.0]);
}

#[test]
fn loaded_homogeneous_patch_has_the_separate_exact_reaction_balance() {
    let program = compile_program();
    let model = lower_isotropic_elasticity_cartesian_2d(&program).unwrap();
    let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[2, 2]).unwrap();
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();
    let plan = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-15,
        1.0e-15,
        NonZeroUsize::new(64).unwrap(),
    )
    .unwrap();
    let solution = solve_cartesian_q1_linear_elasticity_2d(
        &mesh,
        model.shear_modulus(),
        model.first_lame_parameter(),
        model.load_potential_expression(),
        &quadrature,
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan),
    )
    .unwrap();

    let center = (0..mesh.entity_count(0).unwrap())
        .find(|vertex| {
            mesh.vertex_coordinates(MeshEntity::new(0, *vertex))
                .unwrap()
                == [0.5, 0.5]
        })
        .unwrap();
    assert_absolute_slice(
        solution.displacement().vertex_values(center).unwrap(),
        &[3.0 / 176.0, 0.0],
    );
    assert_absolute_slice(&solution.boundary_reaction(), &[-1.0, 0.0]);
    assert_absolute_slice(&solution.integrated_body_force(), &[1.0, 0.0]);
    assert_absolute_slice(
        &[
            solution.boundary_reaction()[0] + solution.integrated_body_force()[0],
            solution.boundary_reaction()[1] + solution.integrated_body_force()[1],
        ],
        &[0.0, 0.0],
    );
}

fn compile_program() -> KernelProgram {
    let mut compiled = compile("compiled-cartesian-elasticity.eqi", SOURCE).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn assert_absolute_slice(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= ABSOLUTE_TOLERANCE,
            "actual={actual:e}, expected={expected:e}, error={:e}, tolerance={ABSOLUTE_TOLERANCE:e}",
            (actual - expected).abs(),
        );
    }
}
