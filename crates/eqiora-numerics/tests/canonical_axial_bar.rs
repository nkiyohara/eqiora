use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_numerics::{
    DefaultScalarElliptic1dConfig, ScalarBoundaryCondition1d, solve_default_scalar_elliptic_1d,
};
use eqiora_sem::KernelProgram;

const SOURCE: &str = include_str!("../../../verify/solid/axial-bar/models/axial-bar.eqi");

#[test]
fn canonical_axial_bar_verifies_displacement_stress_and_reaction() {
    let mut compiled = compile("verify/solid/axial-bar/models/axial-bar.eqi", SOURCE)
        .expect("the canonical verification model compiles");
    assert_eq!(compiled.len(), 1);
    let compiled = compiled.remove(0);
    let youngs_modulus = compiled
        .symbols()
        .get("youngs_modulus")
        .expect("declared material Parameter");
    let (transaction, model_id, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("canonical graph transaction commits atomically");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model_id)
        .expect("whole spatial model validates");

    let (model, solution) = solve_default_scalar_elliptic_1d(
        &program,
        DefaultScalarElliptic1dConfig {
            cells: 16,
            ..Default::default()
        },
    )
    .expect("canonical model lowers through the default realization");

    let expected_displacement = 1.0e-5;
    let tip_displacement = *solution
        .field()
        .values()
        .last()
        .expect("line solution has an upper endpoint");
    assert!((tip_displacement - expected_displacement).abs() < 2.0e-17);

    let youngs_modulus = program
        .value(youngs_modulus)
        .expect("material value is captured by the KernelProgram")
        .value();
    let expected_stress = 1.0e6;
    assert!(
        solution
            .cell_gradients()
            .iter()
            .all(|gradient| { (youngs_modulus * gradient - expected_stress).abs() < 2.0e-6 })
    );

    let reaction = solution.endpoint_reactions()[0].expect("clamp has a reaction");
    assert!((reaction + 10000.0).abs() < 2.0e-7);
    assert_eq!(solution.endpoint_reactions()[1], None);
    assert_eq!(model.coefficient(), 2.0e9);
    assert_eq!(model.source().constant_value(), Some(0.0));
    assert_eq!(
        model.boundary().lower(),
        ScalarBoundaryCondition1d::Essential(0.0)
    );
    assert_eq!(
        model.boundary().upper(),
        ScalarBoundaryCondition1d::Natural(10000.0)
    );
}
