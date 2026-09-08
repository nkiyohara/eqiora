//! Exact ordered formal derivatives, independently checked in physical units.
use eqiora_core::{DimExponents, DynQuantity, ScalarDomain, ValueLiteral, ValueType};
use eqiora_ir::{
    CalculusBuilder, CalculusNode, ExactRational, OperatorExpansionExt, PureValueClass,
    ScalarOperatorIr,
};
use eqiora_schema::kernel::typing::{ExpressionType, RootContract, TypedResidual};
use eqiora_schema::kernel::{ExprDagBuilder, ExprNode};

fn temperature() -> DimExponents {
    DimExponents::from_integers([0, 0, 0, 0, 1, 0, 0]).unwrap()
}

#[test]
fn dimensioned_linear_operator_has_typed_zero_second_derivative_and_bounded_orders() {
    let t = temperature();
    let mut definition = CalculusBuilder::new(
        [PureValueClass::invariant_scalar().with_dimension(t)],
        PureValueClass::invariant_scalar().with_dimension(t),
    )
    .unwrap();
    let x = definition
        .push(CalculusNode::FormalComponent {
            formal: 0,
            axes: Box::new([]),
        })
        .unwrap();
    let two = definition
        .push(CalculusNode::Rational(ExactRational::integer(2)))
        .unwrap();
    let root = definition.push(CalculusNode::Mul(two, x)).unwrap();
    let definition = definition.finish(root).unwrap();
    let types = [ExpressionType::<()>::scalar(t, None)];
    let scalar = definition
        .instantiate(&types)
        .unwrap()
        .component(&[])
        .unwrap();
    let mut builder = ExprDagBuilder::new();
    let x = builder.constant(DynQuantity::new(20., t)).unwrap();
    assert!(scalar.partial(&mut builder, &[x], 0, 0).is_err());
    assert!(scalar.partial(&mut builder, &[x], 0, 3).is_err());
    assert!(scalar.partial(&mut builder, &[x], 1, 1).is_err());
    let before = builder.finish([x]).unwrap();
    assert_eq!(
        before.nodes().len(),
        1,
        "rejected orders do not append nodes"
    );
    let mut builder = ExprDagBuilder::new();
    let x = builder.constant(DynQuantity::new(20., t)).unwrap();
    let (first, first_type) = scalar.partial(&mut builder, &[x], 0, 1).unwrap();
    let (second, second_type) = scalar.partial(&mut builder, &[x], 0, 2).unwrap();
    assert_eq!(first_type.dimension(), DimExponents::DIMENSIONLESS);
    assert_eq!(second_type.dimension(), t.pow(-1, 1).unwrap());
    let dag = builder.finish([first, second]).unwrap();
    assert!(
        matches!(dag.node(second),Some(ExprNode::Constant(value)) if value.is_zero() && value.value_type().dimension()==t.pow(-1,1).unwrap())
    );
    TypedResidual::<()>::infer(dag.clone(), None, RootContract::InitialResiduals, |_| {
        Err::<ExpressionType<()>, _>(())
    })
    .unwrap();
    assert_eq!(
        ScalarOperatorIr::lower(&dag)
            .unwrap()
            .evaluate(&[])
            .unwrap(),
        [2., 0.]
    );
}

#[test]
fn retained_scalar_application_uses_typed_execution_and_rejects_unprojected_ad_or_complex() {
    let mut definition = CalculusBuilder::new(
        [PureValueClass::invariant_scalar()],
        PureValueClass::invariant_scalar(),
    )
    .unwrap();
    let x = definition
        .push(CalculusNode::FormalComponent {
            formal: 0,
            axes: Box::new([]),
        })
        .unwrap();
    let square = definition.push(CalculusNode::Mul(x, x)).unwrap();
    let definition = definition.finish(square).unwrap();
    let mut builder = ExprDagBuilder::new();
    let x = builder
        .constant(DynQuantity::new(3., temperature()))
        .unwrap();
    let root = builder.pure_operator(&definition, [x]).unwrap();
    let dag = builder.finish([root]).unwrap();
    let ir = ScalarOperatorIr::lower(&dag).unwrap();
    let value = ir.evaluate_typed(&[root], &mut |_| None).unwrap().remove(0);
    assert_eq!(value.real_scalar_value().unwrap().value(), 9.);
    assert_eq!(
        value.value_type().dimension(),
        temperature().pow(2, 1).unwrap()
    );
    assert!(ir.evaluate(&[]).is_err());
    assert!(ir.linearize(&[], &[]).is_err());
    let complex = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
    let types = [ExpressionType::<()>::new(complex.clone(), None)];
    let scalar = definition
        .instantiate(&types)
        .unwrap()
        .component(&[])
        .unwrap();
    let mut builder = ExprDagBuilder::new();
    let x = builder
        .constant(ValueLiteral::new(complex, [(1., 2.)]).unwrap())
        .unwrap();
    assert!(scalar.partial(&mut builder, &[x], 0, 1).is_err());
    let root = builder.pure_operator(&definition, [x]).unwrap();
    let dag = builder.finish([root]).unwrap();
    assert!(
        ScalarOperatorIr::lower(&dag)
            .unwrap()
            .evaluate_typed(&[root], &mut |_| None)
            .is_err()
    );
}

#[test]
fn finite_partial_does_not_evaluate_an_unused_overflowing_primal() {
    let scalar = PureValueClass::invariant_scalar();
    let mut definition = CalculusBuilder::new([scalar], scalar).unwrap();
    let x = definition
        .push(CalculusNode::FormalComponent {
            formal: 0,
            axes: Box::new([]),
        })
        .unwrap();
    let square = definition.push(CalculusNode::Mul(x, x)).unwrap();
    let definition = definition.finish(square).unwrap();
    let types = [ExpressionType::<()>::scalar(
        DimExponents::DIMENSIONLESS,
        None,
    )];
    let scalar = definition
        .instantiate(&types)
        .unwrap()
        .component(&[])
        .unwrap();
    let mut primal = ExprDagBuilder::new();
    let x = primal
        .constant(DynQuantity::new(1e200, DimExponents::DIMENSIONLESS))
        .unwrap();
    let root = primal.pure_operator(&definition, [x]).unwrap();
    let dag = primal.finish([root]).unwrap();
    assert!(
        ScalarOperatorIr::lower(&dag)
            .unwrap()
            .evaluate_typed(&[root], &mut |_| None)
            .is_err()
    );
    let mut derivative = ExprDagBuilder::new();
    let x = derivative
        .constant(DynQuantity::new(1e200, DimExponents::DIMENSIONLESS))
        .unwrap();
    let (first, _) = scalar.partial(&mut derivative, &[x], 0, 1).unwrap();
    let (second, _) = scalar.partial(&mut derivative, &[x], 0, 2).unwrap();
    let dag = derivative.finish([first, second]).unwrap();
    assert!(
        !dag.nodes()
            .iter()
            .any(|node| matches!(node,ExprNode::Mul(a,b) if *a==x && *b==x))
    );
    assert_eq!(
        ScalarOperatorIr::lower(&dag)
            .unwrap()
            .evaluate(&[])
            .unwrap(),
        [2e200, 2.]
    );
}

#[test]
fn partial_keeps_live_product_rule_order_instead_of_floating_reassociation() {
    let scalar = PureValueClass::invariant_scalar();
    let mut definition = CalculusBuilder::new([scalar], scalar).unwrap();
    let x = definition
        .push(CalculusNode::FormalComponent {
            formal: 0,
            axes: Box::new([]),
        })
        .unwrap();
    let big = definition
        .push(CalculusNode::Rational(ExactRational::integer(
            10_000_000_000_000_000,
        )))
        .unwrap();
    let product = definition.push(CalculusNode::Mul(big, x)).unwrap();
    let sum = definition.push(CalculusNode::Add(product, x)).unwrap();
    let neg = definition.push(CalculusNode::Neg(product)).unwrap();
    let root = definition.push(CalculusNode::Add(sum, neg)).unwrap();
    let definition = definition.finish(root).unwrap();
    let types = [ExpressionType::<()>::scalar(
        DimExponents::DIMENSIONLESS,
        None,
    )];
    let scalar = definition
        .instantiate(&types)
        .unwrap()
        .component(&[])
        .unwrap();
    let mut builder = ExprDagBuilder::new();
    let x = builder
        .constant(DynQuantity::new(1., DimExponents::DIMENSIONLESS))
        .unwrap();
    let (first, _) = scalar.partial(&mut builder, &[x], 0, 1).unwrap();
    let dag = builder.finish([first]).unwrap();
    // Binary64 evaluates (1e16 + 1) - 1e16 as 0. Exact normalization to 1
    // would change executable floating semantics and belongs only to proof.
    assert_eq!(
        ScalarOperatorIr::lower(&dag)
            .unwrap()
            .evaluate(&[])
            .unwrap(),
        [0.]
    );
}

#[test]
fn tensor_applications_still_require_existing_component_expansion() {
    use eqiora_schema::kernel::{SymbolRef, pure_operator::PureOperatorDefinition};
    let mut builder = ExprDagBuilder::new();
    let tensor = builder
        .symbol(SymbolRef::Field(eqiora_core::Id::new()))
        .unwrap();
    let root = builder
        .pure_operator(&PureOperatorDefinition::symmetric_part().unwrap(), [tensor])
        .unwrap();
    let dag = builder.finish([root]).unwrap();
    let error = ScalarOperatorIr::lower(&dag).unwrap_err();
    assert!(error.to_string().contains("component expansion"));
}
