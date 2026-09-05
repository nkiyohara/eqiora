use super::*;
use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_schema::kernel::KernelNode;

#[test]
fn evaluates_distinct_axes_and_requires_exact_coordinate_shape() {
    let expression = ScalarSpatialExpression {
        coordinate_dimension: 2,
        instructions: vec![
            Instruction::Coordinate(0),
            Instruction::Coordinate(1),
            Instruction::Mul(1, 1),
            Instruction::Add(0, 2),
        ],
        root: 3,
        coordinate_dependent: true,
        parameter_fields: Vec::new(),
        parameter_values: Vec::new(),
    };

    assert_eq!(expression.coordinate_dimension(), 2);
    assert_eq!(expression.evaluate(&[2.0, 3.0]).unwrap(), 11.0);
    assert_eq!(
        expression.evaluate(&[2.0]).unwrap_err().code(),
        codes::OPERATOR_INPUT_MISMATCH
    );
    assert_eq!(
        expression.evaluate(&[2.0, f64::NAN]).unwrap_err().code(),
        codes::NONFINITE_EVALUATION
    );
}

#[test]
fn constant_tape_retains_coordinate_dimension() {
    let expression = ScalarSpatialExpression::constant(3, 4.5);
    assert_eq!(expression.coordinate_dimension(), 3);
    assert_eq!(expression.constant_value(), Some(4.5));
    assert_eq!(expression.evaluate(&[1.0, 2.0, 3.0]).unwrap(), 4.5);
}

#[test]
fn lowers_both_axes_from_one_canonical_plane_relation() {
    let source = r#"
model plane_source {
  domain plane = box(0, 2, 0, 3);
  representation space = continuum;
  field u on plane as space: m = 0;
  relation identity continuous on plane {
u - (coordinate(0) + coordinate(1)) = 0;
  }
}
"#;
    let mut compiled = compile("plane-source.eqi", source).unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let relation = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Relation(relation) => Some(relation),
            _ => None,
        })
        .unwrap();
    let root = relation.residuals().roots()[0];
    let source_root = match relation.residuals().node(root) {
        Some(ExprNode::Sub(_, source)) => *source,
        _ => panic!("fixture has one field-minus-source residual"),
    };
    let lowered = lower(
        &program,
        relation.residuals(),
        source_root,
        relation.id().erase(),
        2,
    )
    .unwrap();

    assert_eq!(lowered.coordinate_dimension(), 2);
    assert_eq!(lowered.evaluate(&[2.0, 3.0]).unwrap(), 5.0);
}

#[test]
fn retains_parameter_identity_and_evaluates_analytic_jvp() {
    let source = r#"
model parameterized_source {
  domain interval = box(0, 2);
  representation space = continuum;
  field u on interval as space: m ^ 2 = 0;
  parameter amplitude: m = 3;
  relation identity continuous on interval {
u - amplitude ^ 2 * math.sin(coordinate(0) / amplitude) = 0;
  }
}
"#;
    let mut compiled = compile("parameterized-source.eqi", source).unwrap();
    let amplitude = compiled[0]
        .symbols()
        .get("amplitude")
        .unwrap()
        .downcast::<kinds::Parameter>()
        .unwrap();
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let relation = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Relation(relation) => Some(relation),
            _ => None,
        })
        .unwrap();
    let root = relation.residuals().roots()[0];
    let source_root = match relation.residuals().node(root) {
        Some(ExprNode::Sub(_, source)) => *source,
        _ => panic!("fixture has one field-minus-source residual"),
    };
    let lowered = lower(
        &program,
        relation.residuals(),
        source_root,
        relation.id().erase(),
        1,
    )
    .unwrap();

    let coordinate = 0.4_f64;
    let (value, tangent) = lowered
        .evaluate_parameter_jvp(&[coordinate], &[2.0])
        .unwrap();
    let (_, combined_tangent) = lowered
        .evaluate_jvp(&[coordinate], &[0.25], &[2.0])
        .unwrap();
    let output_cotangent = 1.75;
    let (vjp_value, coordinate_cotangent, parameter_cotangent) = lowered
        .evaluate_vjp(&[coordinate], output_cotangent)
        .unwrap();
    let expected_value = 9.0 * (coordinate / 3.0).sin();
    let expected_tangent =
        2.0 * (6.0 * (coordinate / 3.0).sin() - coordinate * (coordinate / 3.0).cos());
    let expected_coordinate_derivative = 3.0 * (coordinate / 3.0).cos();
    let expected_parameter_derivative =
        6.0 * (coordinate / 3.0).sin() - coordinate * (coordinate / 3.0).cos();
    assert_eq!(lowered.parameter_fields(), &[amplitude]);
    assert_eq!(lowered.parameter_values(), &[3.0]);
    assert!((value - expected_value).abs() < 1.0e-14);
    assert!((vjp_value - expected_value).abs() < 1.0e-14);
    assert!((tangent - expected_tangent).abs() < 1.0e-14);
    assert!(
        (combined_tangent - (expected_tangent + 0.25 * expected_coordinate_derivative)).abs()
            < 1.0e-14
    );
    assert!(
        (coordinate_cotangent[0] - output_cotangent * expected_coordinate_derivative).abs()
            < 1.0e-14
    );
    assert!(
        (parameter_cotangent[0] - output_cotangent * expected_parameter_derivative).abs() < 1.0e-14
    );
    let jvp_pairing = output_cotangent * combined_tangent;
    let vjp_pairing = 0.25 * coordinate_cotangent[0] + 2.0 * parameter_cotangent[0];
    assert!((jvp_pairing - vjp_pairing).abs() < 1.0e-14);
}

#[test]
fn product_deduplicates_parameter_coordinates() {
    let parameter = Id::<kinds::Parameter>::new();
    let left = ScalarSpatialExpression {
        coordinate_dimension: 1,
        instructions: vec![Instruction::Parameter(0)],
        root: 0,
        coordinate_dependent: false,
        parameter_fields: vec![parameter],
        parameter_values: vec![2.0],
    };
    let right = ScalarSpatialExpression {
        coordinate_dimension: 1,
        instructions: vec![
            Instruction::Parameter(0),
            Instruction::Coordinate(0),
            Instruction::Add(0, 1),
        ],
        root: 2,
        coordinate_dependent: true,
        parameter_fields: vec![parameter],
        parameter_values: vec![2.0],
    };
    let product = left.multiply(right);

    assert_eq!(product.parameter_fields(), &[parameter]);
    assert_eq!(
        product.evaluate_parameter_jvp(&[3.0], &[1.0]).unwrap(),
        (10.0, 7.0)
    );
}

#[test]
fn coefficient_equality_is_identity_aware_not_value_inferred() {
    let first = Id::<kinds::Parameter>::new();
    let second = Id::<kinds::Parameter>::new();
    let coefficient = |parameter| ScalarSpatialExpression {
        coordinate_dimension: 2,
        instructions: vec![Instruction::Parameter(0)],
        root: 0,
        coordinate_dependent: false,
        parameter_fields: vec![parameter],
        parameter_values: vec![3.0],
    };

    let volume = coefficient(first);
    assert!(volume.is_same_coefficient_as(&volume.clone()));
    assert!(
        !volume.is_same_coefficient_as(&coefficient(second)),
        "equal revision-local values do not merge independent Parameters"
    );
    assert!(
        ScalarSpatialExpression::constant(2, 3.0)
            .is_same_coefficient_as(&ScalarSpatialExpression::constant(2, 3.0))
    );
}
