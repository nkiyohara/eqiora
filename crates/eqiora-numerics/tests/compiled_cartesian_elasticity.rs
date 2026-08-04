use std::num::NonZeroUsize;

use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{CartesianMesh, MeshTopology, QuadratureRule};
use eqiora_numerics::solid::{
    lower_cartesian_q1_linear_elasticity_local_action_2d, lower_isotropic_elasticity_cartesian_2d,
    solve_cartesian_q1_linear_elasticity_2d,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearSolveRequest, LinearSolver, REFERENCE_LINEAR_SOLVER, SolverPlan};

const SOURCE: &str =
    include_str!("../../../verify/solid/isotropic-elasticity-2d/models/linear-load.eqi");

#[test]
fn public_elasticity_construction_and_scatter_smoke() {
    let program = compile_program();
    let model = lower_isotropic_elasticity_cartesian_2d(&program).unwrap();
    let mesh = CartesianMesh::uniform(model.bounds(), &[2, 2]).unwrap();
    let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2).unwrap();

    let local_action = lower_cartesian_q1_linear_elasticity_local_action_2d(
        &mesh,
        model.shear_modulus(),
        model.first_lame_parameter(),
        &quadrature,
    )
    .unwrap();
    assert_eq!(local_action.entity_count(), mesh.entity_count(2).unwrap());
    assert!(
        local_action
            .coefficients()
            .iter()
            .all(|value| value.is_finite())
    );

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
    assert_eq!(solution.displacement().mesh(), &mesh);
    assert!(
        solution
            .displacement()
            .values()
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(
        solution
            .boundary_reaction()
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(
        solution
            .integrated_body_force()
            .iter()
            .all(|value| value.is_finite())
    );
}

fn compile_program() -> KernelProgram {
    let mut compiled = compile("compiled-cartesian-elasticity.eqi", SOURCE).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
