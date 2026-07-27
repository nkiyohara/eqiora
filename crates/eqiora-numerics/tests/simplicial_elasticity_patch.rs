use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora_meshing::{
    MeshEntity, MeshGeometry, MeshQualityGate, MeshTopology, SimplicialMesh, simplex_centroid_rule,
};
use eqiora_numerics::common::SimplicialP1Field;
use eqiora_solver::{LinearSolveRequest, LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

#[path = "../../../verify/solid/simplicial-elasticity-patch-2d/fixtures/distorted_patch.rs"]
mod distorted_patch;

const MU: f64 = 3.0;
const LAMBDA: f64 = 2.0;
const MACHINE_PRECISION_BOUND: f64 = 256.0 * f64::EPSILON;
const COMPLETE_BOUNDARY: &str = "complete-boundary";

type PatchSolution = ([SimplicialP1Field; 2], BTreeMap<String, [f64; 2]>, [f64; 2]);

#[test]
fn distorted_patch_reproduces_three_linear_displacement_fields() {
    let fields = [
        ([[0.25, 0.0], [0.0, 0.0]], [0.0, 0.0], [31.0 / 400.0, 0.0]),
        ([[0.0, 0.0], [0.0, 0.25]], [0.0, 0.0], [0.0, 63.0 / 400.0]),
        (
            [[0.0, 0.125], [0.125, 0.0]],
            [0.0, 0.0],
            [63.0 / 800.0, 31.0 / 800.0],
        ),
    ];

    for (gradient, offset, expected_interior) in fields {
        let mesh = distorted_patch_mesh();
        let solution = solve(&mesh, [0.0, 0.0], &|point| {
            affine_value(gradient, offset, point)
        });
        assert_vector_close(
            displacement_at(&solution.0, distorted_patch::INTERIOR_VERTEX),
            expected_interior,
        );
        for (vertex, point) in mesh.vertices().iter().enumerate() {
            assert_vector_close(
                displacement_at(&solution.0, vertex),
                affine_value(gradient, offset, [point[0], point[1]]),
            );
        }
    }
}

#[test]
fn rigid_body_motions_have_roundoff_strain_energy() {
    let mesh = distorted_patch_mesh();
    let translation = solve(&mesh, [0.0, 0.0], &|_| [0.75, -0.5]);
    let rotation = solve(&mesh, [0.0, 0.0], &|point| {
        [-0.25 * point[1], 0.25 * point[0]]
    });

    for solution in [&translation, &rotation] {
        let (strains, _, energy) =
            SimplicialP1Field::linear_elasticity_cell_states_2d(&solution.0, MU, LAMBDA).unwrap();
        assert!(
            energy.abs() <= MACHINE_PRECISION_BOUND,
            "rigid energy {energy:e} exceeded {:e}",
            MACHINE_PRECISION_BOUND
        );
        for strain in strains {
            for value in strain.into_iter().flatten() {
                assert_close(value, 0.0);
            }
        }
    }
}

#[test]
fn constant_strain_produces_exact_constant_stress_in_every_triangle() {
    let gradient = [[3.0 / 16.0, -1.0 / 8.0], [1.0 / 16.0, 1.0 / 4.0]];
    let offset = [1.0 / 16.0, -3.0 / 32.0];
    let expected_strain = [[3.0 / 16.0, -1.0 / 32.0], [-1.0 / 32.0, 1.0 / 4.0]];
    let expected_stress = [[2.0, -3.0 / 16.0], [-3.0 / 16.0, 19.0 / 8.0]];
    let mesh = distorted_patch_mesh();
    let solution = solve(&mesh, [0.0, 0.0], &|point| {
        affine_value(gradient, offset, point)
    });
    let (strains, stresses, _) =
        SimplicialP1Field::linear_elasticity_cell_states_2d(&solution.0, MU, LAMBDA).unwrap();

    assert_vector_close(
        displacement_at(&solution.0, distorted_patch::INTERIOR_VERTEX),
        [67.0 / 1600.0, 133.0 / 1600.0],
    );
    for (strain, stress) in strains.into_iter().zip(stresses) {
        assert_tensor_close(strain, expected_strain);
        assert_tensor_close(stress, expected_stress);
    }
}

#[test]
fn named_complete_boundary_reaction_balances_nonzero_body_force() {
    let mesh = distorted_patch_mesh();
    let body_force = [0.75, -0.625];
    let solution = solve(&mesh, body_force, &|_| [0.0, 0.0]);
    let reaction = solution
        .1
        .get(COMPLETE_BOUNDARY)
        .copied()
        .expect("the fixture names its complete constrained boundary");

    assert_vector_close(
        displacement_at(&solution.0, distorted_patch::INTERIOR_VERTEX),
        [554001.0 / 55700000.0, -554001.0 / 64280000.0],
    );
    assert_vector_close(solution.2, body_force);
    assert_vector_close(reaction, [-0.75, 0.625]);
    assert_vector_close(
        [reaction[0] + solution.2[0], reaction[1] + solution.2[1]],
        [0.0, 0.0],
    );
}

#[test]
fn repeated_solve_displacements_are_bitwise_identical() {
    let gradient = [[3.0 / 16.0, -1.0 / 8.0], [1.0 / 16.0, 1.0 / 4.0]];
    let offset = [1.0 / 16.0, -3.0 / 32.0];
    let first_mesh = distorted_patch_mesh();
    let first = solve(&first_mesh, [0.0, 0.0], &|point| {
        affine_value(gradient, offset, point)
    });
    let second_mesh = distorted_patch_mesh();
    let second = solve(&second_mesh, [0.0, 0.0], &|point| {
        affine_value(gradient, offset, point)
    });
    let first_bits = displacement_bits(&first.0);
    let second_bits = displacement_bits(&second.0);

    assert_eq!(first_bits, second_bits);
    assert_eq!(
        &first_bits[2 * distorted_patch::INTERIOR_VERTEX..2 * distorted_patch::INTERIOR_VERTEX + 2],
        &second_bits
            [2 * distorted_patch::INTERIOR_VERTEX..2 * distorted_patch::INTERIOR_VERTEX + 2],
    );
}

#[test]
fn non_box_boundary_is_rejected_before_assembly() {
    let mesh = SimplicialMesh::new(
        2,
        [[0.0, 0.0], [1.0, 0.0], [0.8, 1.0], [0.0, 1.0]]
            .into_iter()
            .map(Vec::from)
            .collect(),
        [[0, 1, 2], [0, 2, 3]].into_iter().map(Vec::from).collect(),
        MeshQualityGate::new(0.3).unwrap(),
    )
    .unwrap();
    let plan = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-15,
        1.0e-15,
        NonZeroUsize::new(64).unwrap(),
    )
    .unwrap();
    let error = SimplicialP1Field::solve_linear_elasticity_simplicial_2d(
        &distorted_patch::BOUNDS,
        &mesh,
        MU,
        LAMBDA,
        [0.0, 0.0],
        &BTreeMap::new(),
        &|_| Ok([0.0, 0.0]),
        &simplex_centroid_rule(2).unwrap(),
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan),
    )
    .unwrap_err();

    assert!(
        error
            .message()
            .contains("boundary facet does not lie on one box side")
    );
}

fn solve(
    mesh: &SimplicialMesh,
    body_force: [f64; 2],
    displacement: &(impl Fn([f64; 2]) -> [f64; 2] + Sync),
) -> PatchSolution {
    let named_surfaces =
        BTreeMap::from([(COMPLETE_BOUNDARY.to_owned(), complete_boundary_facets(mesh))]);
    let plan = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-15,
        1.0e-15,
        NonZeroUsize::new(64).unwrap(),
    )
    .unwrap();
    let (fields, reactions, force, assembly, solved) =
        SimplicialP1Field::solve_linear_elasticity_simplicial_2d(
            &distorted_patch::BOUNDS,
            mesh,
            MU,
            LAMBDA,
            body_force,
            &named_surfaces,
            &|point| Ok(displacement(point)),
            &simplex_centroid_rule(2).unwrap(),
            LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan),
        )
        .unwrap();
    assert_eq!(assembly.target_count(), 2);
    assert!(solved.true_residual_norm() <= solved.residual_target());
    (fields, reactions, force)
}

fn distorted_patch_mesh() -> SimplicialMesh {
    let mesh = SimplicialMesh::new(
        2,
        distorted_patch::VERTICES
            .into_iter()
            .map(Vec::from)
            .collect(),
        distorted_patch::CELLS.into_iter().map(Vec::from).collect(),
        MeshQualityGate::new(0.3).unwrap(),
    )
    .unwrap();
    assert_eq!(
        mesh.vertices()[distorted_patch::INTERIOR_VERTEX],
        [0.31, 0.63]
    );
    for (cell_index, expected_area) in distorted_patch::CELL_AREAS.into_iter().enumerate() {
        let geometry = mesh.geometry_map(MeshEntity::new(2, cell_index)).unwrap();
        assert_close(0.5 * geometry.measure_scale(), expected_area);
    }
    mesh
}

fn complete_boundary_facets(mesh: &SimplicialMesh) -> Vec<MeshEntity> {
    (0..mesh.entity_count(1).unwrap())
        .filter_map(|index| {
            let facet = MeshEntity::new(1, index);
            mesh.is_boundary_entity(facet).unwrap().then_some(facet)
        })
        .collect()
}

fn affine_value(gradient: [[f64; 2]; 2], offset: [f64; 2], point: [f64; 2]) -> [f64; 2] {
    [
        gradient[0][0] * point[0] + gradient[0][1] * point[1] + offset[0],
        gradient[1][0] * point[0] + gradient[1][1] * point[1] + offset[1],
    ]
}

fn displacement_at(fields: &[SimplicialP1Field; 2], vertex: usize) -> [f64; 2] {
    [
        fields[0].vertex_values()[vertex],
        fields[1].vertex_values()[vertex],
    ]
}

fn displacement_bits(fields: &[SimplicialP1Field; 2]) -> Vec<u64> {
    (0..fields[0].vertex_values().len())
        .flat_map(|vertex| displacement_at(fields, vertex).map(f64::to_bits))
        .collect()
}

fn assert_tensor_close(actual: [[f64; 2]; 2], expected: [[f64; 2]; 2]) {
    for (actual, expected) in actual
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert_close(actual, expected);
    }
}

fn assert_vector_close(actual: [f64; 2], expected: [f64; 2]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert_close(actual, expected);
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= MACHINE_PRECISION_BOUND,
        "actual {actual:e}, expected {expected:e}, error {:e}, bound {:e}",
        (actual - expected).abs(),
        MACHINE_PRECISION_BOUND
    );
}
