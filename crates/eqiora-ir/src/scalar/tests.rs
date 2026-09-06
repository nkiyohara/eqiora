use eqiora_core::{DimExponents, DynQuantity, Id, entity::kinds};
use eqiora_schema::kernel::ExprDagBuilder;

use super::*;

#[test]
fn scalar_ir_rejects_nonreal_or_shaped_constants() {
    use eqiora_core::{ScalarDomain, ValueLiteral, ValueType};
    let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    for value_type in [
        ValueType::scalar(ScalarDomain::Complex, real.dimension()),
        real.array(3).unwrap(),
    ] {
        let mut builder = ExprDagBuilder::new();
        let root = builder
            .constant(ValueLiteral::new(value_type, 0.0).unwrap())
            .unwrap();
        let error = ScalarOperatorIr::lower(&builder.finish([root]).unwrap()).unwrap_err();
        assert!(error.message().contains("real scalar constants"));
    }
}

#[test]
fn lowering_deduplicates_symbols_and_preserves_residual_value() {
    let field = Id::<kinds::Field>::new();
    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).expect("field");
    let two = expression
        .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
        .expect("constant");
    let square = expression.mul(value, value).expect("square");
    let residual = expression.sub(square, two).expect("residual");
    let dag = expression.finish([residual]).expect("DAG");

    let ir = ScalarOperatorIr::lower(&dag).expect("lowered");
    assert_eq!(ir.symbols(), &[SymbolRef::Field(field)]);
    assert_eq!(ir.evaluate(&[3.0]).expect("evaluate"), vec![7.0]);
}

#[test]
fn evaluation_rejects_wrong_or_nonfinite_inputs() {
    let field = Id::<kinds::Field>::new();
    let mut expression = ExprDagBuilder::new();
    let root = expression.symbol(SymbolRef::Field(field)).expect("field");
    let ir = ScalarOperatorIr::lower(&expression.finish([root]).expect("DAG")).expect("lowered");

    assert_eq!(
        ir.evaluate(&[]).expect_err("missing input").code(),
        codes::OPERATOR_INPUT_MISMATCH
    );
    assert_eq!(
        ir.evaluate(&[f64::INFINITY])
            .expect_err("nonfinite input")
            .code(),
        codes::NONFINITE_EVALUATION
    );
}

#[test]
fn scalar_ssa_jvp_and_vjp_are_paired_on_a_nonsymmetric_relation() {
    let first = Id::<kinds::Field>::new();
    let second = Id::<kinds::Field>::new();
    let first_parameter = Id::<kinds::Parameter>::new();
    let second_parameter = Id::<kinds::Parameter>::new();
    let mut expression = ExprDagBuilder::new();
    let w0 = expression.symbol(SymbolRef::Field(first)).unwrap();
    let w1 = expression.symbol(SymbolRef::Field(second)).unwrap();
    let p0 = expression
        .symbol(SymbolRef::Parameter(first_parameter))
        .unwrap();
    let p1 = expression
        .symbol(SymbolRef::Parameter(second_parameter))
        .unwrap();
    let two = expression
        .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let three = expression
        .constant(DynQuantity::new(3.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let square = expression.mul(w0, w0).unwrap();
    let first_sum = expression.add(square, w1).unwrap();
    let first_residual = expression.sub(first_sum, p0).unwrap();
    let twice_w0 = expression.mul(two, w0).unwrap();
    let thrice_w1 = expression.mul(three, w1).unwrap();
    let second_sum = expression.add(twice_w0, thrice_w1).unwrap();
    let second_residual = expression.sub(second_sum, p1).unwrap();
    let ir = ScalarOperatorIr::lower(
        &expression
            .finish([first_residual, second_residual])
            .unwrap(),
    )
    .unwrap();
    let point = [2.0, 1.0, 5.0, 7.0];
    let roles = [
        DifferentiationRole::Unknown,
        DifferentiationRole::Unknown,
        DifferentiationRole::Parameter,
        DifferentiationRole::Parameter,
    ];
    let linearization = ir.linearize(&point, &roles).unwrap();

    let mut primal = [f64::NAN; 2];
    linearization.primal(&mut primal).unwrap();
    assert_eq!(primal, [0.0, 0.0]);

    let unknown_tangent = [0.25, -0.5];
    let parameter_tangent = [0.75, -1.0];
    let mut jvp = [0.0; 2];
    linearization
        .jvp(
            RelationTangent::Both {
                unknown: &unknown_tangent,
                parameter: &parameter_tangent,
            },
            &mut jvp,
        )
        .unwrap();
    assert_eq!(jvp, [-0.25, 0.0]);

    let residual_cotangent = [1.5, -0.25];
    let mut unknown_cotangent = [0.0; 2];
    let mut parameter_cotangent = [0.0; 2];
    linearization
        .vjp(
            &residual_cotangent,
            RelationCotangent::Both {
                unknown: &mut unknown_cotangent,
                parameter: &mut parameter_cotangent,
            },
        )
        .unwrap();
    assert_eq!(unknown_cotangent, [5.5, 0.75]);
    assert_eq!(parameter_cotangent, [-1.5, 0.25]);

    let left = residual_cotangent
        .iter()
        .zip(jvp)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let right = unknown_tangent
        .iter()
        .zip(unknown_cotangent)
        .chain(parameter_tangent.iter().zip(parameter_cotangent))
        .map(|(left, right)| left * right)
        .sum::<f64>();
    assert!((left - right).abs() < 1.0e-14);

    let full_tangent = [0.25, -0.5, 0.75, -1.0];
    let step = 1.0e-6;
    let plus: [f64; 4] = std::array::from_fn(|index| point[index] + step * full_tangent[index]);
    let minus: [f64; 4] = std::array::from_fn(|index| point[index] - step * full_tangent[index]);
    let plus = ir.evaluate(&plus).unwrap();
    let minus = ir.evaluate(&minus).unwrap();
    for ((plus, minus), jvp) in plus.iter().zip(minus).zip(jvp) {
        let difference = (plus - minus) / (2.0 * step);
        assert!((difference - jvp).abs() < 2.0e-9);
    }
}

#[test]
fn linearization_fails_closed_on_shapes_and_handles_zero_power_at_zero() {
    let field = Id::<kinds::Field>::new();
    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).unwrap();
    let root = expression.powi(value, 0).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
    let linearization = ir
        .linearize(&[0.0], &[DifferentiationRole::Unknown])
        .unwrap();
    let mut tangent = [f64::NAN];
    linearization
        .jvp(RelationTangent::Unknown(&[2.0]), &mut tangent)
        .unwrap();
    assert_eq!(tangent, [0.0]);
    assert_eq!(
        linearization
            .jvp(RelationTangent::Unknown(&[]), &mut tangent)
            .expect_err("wrong unknown tangent")
            .code(),
        codes::INVALID_LINEARIZATION
    );
    assert_eq!(
        ir.linearize(&[0.0], &[]).expect_err("missing role").code(),
        codes::INVALID_LINEARIZATION
    );

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).unwrap();
    let root = expression.powi(value, i32::MIN).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
    let linearization = ir
        .linearize(&[1.0], &[DifferentiationRole::Unknown])
        .unwrap();
    linearization
        .jvp(RelationTangent::Unknown(&[1.0]), &mut tangent)
        .unwrap();
    assert_eq!(tangent, [f64::from(i32::MIN)]);
}

#[test]
fn constant_symbol_jacobian_proves_an_exact_derivative_identity() {
    let first = Id::<kinds::Field>::new();
    let second = Id::<kinds::Field>::new();
    let parameter = Id::<kinds::Parameter>::new();
    let mut expression = ExprDagBuilder::new();
    let d_first = expression.symbol(SymbolRef::Derivative(first)).unwrap();
    let d_second = expression.symbol(SymbolRef::Derivative(second)).unwrap();
    let first_value = expression.symbol(SymbolRef::Field(first)).unwrap();
    let parameter_value = expression.symbol(SymbolRef::Parameter(parameter)).unwrap();
    let first_residual = expression.sub(d_first, first_value).unwrap();
    let twice_second = expression.add(d_second, d_second).unwrap();
    let scaled_second = expression.sub(twice_second, parameter_value).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([first_residual, scaled_second]).unwrap())
        .unwrap();

    let jacobian = ir
        .constant_symbol_jacobian(&[SymbolRef::Derivative(first), SymbolRef::Derivative(second)])
        .unwrap();
    assert_eq!(jacobian.row_count(), 2);
    assert_eq!(jacobian.column_count(), 2);
    assert_eq!(jacobian.row(0), Some([1.0, 0.0].as_slice()));
    assert_eq!(jacobian.row(1), Some([0.0, 2.0].as_slice()));
    assert_eq!(jacobian.row(2), None);
}

#[test]
fn constant_symbol_jacobian_rejects_variable_and_nonlinear_coefficients() {
    let field = Id::<kinds::Field>::new();
    let derivative = SymbolRef::Derivative(field);

    let mut expression = ExprDagBuilder::new();
    let rate = expression.symbol(derivative).unwrap();
    let state = expression.symbol(SymbolRef::Field(field)).unwrap();
    let root = expression.mul(rate, state).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
    assert!(matches!(
        ir.constant_symbol_jacobian(&[derivative]),
        Err(SymbolicLinearityFailure::VariableCoefficient { .. })
    ));

    let mut expression = ExprDagBuilder::new();
    let rate = expression.symbol(derivative).unwrap();
    let root = expression.mul(rate, rate).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
    assert!(matches!(
        ir.constant_symbol_jacobian(&[derivative]),
        Err(SymbolicLinearityFailure::Nonlinear { .. })
    ));
    assert_eq!(
        ir.constant_symbol_jacobian(&[derivative, derivative]),
        Err(SymbolicLinearityFailure::RepeatedVariable(derivative))
    );
}

#[test]
fn bound_affine_admits_parameter_coefficients_and_retains_declared_orders() {
    let first = SymbolRef::Field(Id::<kinds::Field>::new());
    let second = SymbolRef::Field(Id::<kinds::Field>::new());
    let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());
    let mut expression = ExprDagBuilder::new();
    let first_value = expression.symbol(first).unwrap();
    let second_value = expression.symbol(second).unwrap();
    let parameter_value = expression.symbol(parameter).unwrap();
    let time = expression.symbol(SymbolRef::Time).unwrap();
    let scaled_first = expression.mul(parameter_value, first_value).unwrap();
    let first_sum = expression.add(scaled_first, second_value).unwrap();
    let first_residual = expression.sub(first_sum, time).unwrap();
    let divided_first = expression.div(first_value, parameter_value).unwrap();
    let second_residual = expression.add(divided_first, time).unwrap();
    let ir = ScalarOperatorIr::lower(
        &expression
            .finish([first_residual, second_residual])
            .unwrap(),
    )
    .unwrap();

    let selected = [second, first];
    let affine = ir
        .bind_affine(&selected, &[(parameter, 3.0), (SymbolRef::Time, 5.0)])
        .unwrap();

    assert_eq!(affine.selected_symbols(), selected.as_slice());
    assert_eq!(affine.residual_count(), 2);
    assert_eq!(affine.selected_symbol_count(), 2);
    assert_eq!(affine.coefficients(), &[1.0, 3.0, 0.0, 1.0 / 3.0]);
    assert_eq!(affine.coefficient_row(0), Some([1.0, 3.0].as_slice()));
    assert_eq!(affine.coefficient_row(1), Some([0.0, 1.0 / 3.0].as_slice()));
    assert_eq!(affine.coefficient_row(2), None);
    assert_eq!(affine.offsets(), &[-5.0, 5.0]);
}

#[test]
fn bound_affine_validates_the_complete_binding_boundary() {
    let unknown = SymbolRef::Field(Id::<kinds::Field>::new());
    let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());
    let mut expression = ExprDagBuilder::new();
    let unknown_value = expression.symbol(unknown).unwrap();
    let parameter_value = expression.symbol(parameter).unwrap();
    let residual = expression.add(unknown_value, parameter_value).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([residual]).unwrap()).unwrap();

    assert_eq!(
        ir.bind_affine(&[unknown, unknown], &[(parameter, 2.0)]),
        Err(BoundAffineFailure::RepeatedSelectedSymbol(unknown))
    );
    assert_eq!(
        ir.bind_affine(&[unknown], &[(parameter, 2.0), (parameter, 3.0)]),
        Err(BoundAffineFailure::DuplicateBinding(parameter))
    );
    assert_eq!(
        ir.bind_affine(&[unknown], &[(unknown, 1.0), (parameter, 2.0)]),
        Err(BoundAffineFailure::SelectedSymbolBound(unknown))
    );
    assert_eq!(
        ir.bind_affine(&[unknown], &[(parameter, f64::NAN)]),
        Err(BoundAffineFailure::NonFiniteBinding(parameter))
    );
    assert_eq!(
        ir.bind_affine(&[unknown], &[]),
        Err(BoundAffineFailure::UnboundSymbol(parameter))
    );
}

#[test]
fn bound_affine_rejects_nonlinear_dependence_and_nonfinite_arithmetic() {
    let unknown = SymbolRef::Field(Id::<kinds::Field>::new());
    let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(unknown).unwrap();
    let square = expression.mul(value, value).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([square]).unwrap()).unwrap();
    assert!(matches!(
        ir.bind_affine(&[unknown], &[]),
        Err(BoundAffineFailure::Nonlinear { .. })
    ));

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(unknown).unwrap();
    let one = expression
        .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
        .unwrap();
    let reciprocal = expression.div(one, value).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([reciprocal]).unwrap()).unwrap();
    assert!(matches!(
        ir.bind_affine(&[unknown], &[]),
        Err(BoundAffineFailure::Nonlinear { .. })
    ));

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(unknown).unwrap();
    let square = expression.powi(value, 2).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([square]).unwrap()).unwrap();
    assert!(matches!(
        ir.bind_affine(&[unknown], &[]),
        Err(BoundAffineFailure::Nonlinear { .. })
    ));

    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(unknown).unwrap();
    let divisor = expression.symbol(parameter).unwrap();
    let quotient = expression.div(value, divisor).unwrap();
    let ir = ScalarOperatorIr::lower(&expression.finish([quotient]).unwrap()).unwrap();
    assert!(matches!(
        ir.bind_affine(&[unknown], &[(parameter, 0.0)]),
        Err(BoundAffineFailure::NonFiniteArithmetic { .. })
    ));
}
