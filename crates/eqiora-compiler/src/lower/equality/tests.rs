use super::*;
use crate::lower::{
    FreshLoweringIdentities, LoweringExpression, LoweringItem, LoweringModel, lower_typed_model,
};
use eqiora_core::{DimExponents, ScalarDomain, ValueLiteral, ValueType};
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

#[test]
fn contextual_zero_adopts_complete_type_but_explicit_zero_never_does() {
    let complex = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
    let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    let array = complex.clone().array(2).unwrap();
    let support = Some(typing::SpatialSupport::Volume {
        domain: "body",
        dimensions: 2,
    });
    for value_type in [complex.clone(), array.clone()] {
        let left = ExpressionType::new(value_type.clone(), support.clone());
        let checked = check(
            left.clone(),
            ExpressionType::new(real.clone(), None),
            false,
            true,
        )
        .unwrap();
        assert_eq!(checked.left, left);
        assert_eq!(checked.right.value_type, value_type);
        assert_eq!(checked.right.support, None);
        assert_eq!(checked.residual, left);
    }
    assert!(
        check(
            ExpressionType::<()>::new(real.clone(), None),
            ExpressionType::new(array, None),
            false,
            false
        )
        .is_err()
    );
    let promoted = check(
        ExpressionType::<()>::new(real, None),
        ExpressionType::new(complex.clone(), None),
        false,
        false,
    )
    .unwrap();
    assert_eq!(promoted.residual.value_type, complex);
    assert_ne!(promoted.residual, promoted.left);
}

#[test]
fn explicit_complex_rhs_zero_keeps_promotion_in_the_actual_residual() {
    let document = eqiora_lang::parse(
        "typed.eqi",
        "model M { field x: 1 = 1; relation r { x = 0; } }",
    )
    .into_document()
    .unwrap();
    let mut model = LoweringModel::from_source("typed.eqi", &document.models()[0]).unwrap();
    let LoweringItem::Relation { equations, .. } = &mut model.items[1] else {
        panic!("relation")
    };
    let equation = &mut equations[0];
    equation.right = LoweringExpression::literal(
        ValueLiteral::new(
            ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS),
            -0.0,
        )
        .unwrap(),
        equation.right.range(),
    );
    equation.contextual_right_zero = false;
    let compiled = lower_typed_model("typed.eqi", &model, &mut FreshLoweringIdentities).unwrap();
    let relation = compiled
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation),
            _ => None,
        })
        .unwrap();
    let root = relation.residuals().roots()[0];
    let ExprNode::Sub(_, right) = relation.residuals().nodes()[root.index() as usize] else {
        panic!("real lhs cannot absorb a complex-zero promotion")
    };
    let ExprNode::Constant(value) = &relation.residuals().nodes()[right.index() as usize] else {
        panic!("typed zero")
    };
    assert_eq!(value.value_type().scalar_domain(), ScalarDomain::Complex);
    assert_eq!(value.literal().to_bits(), 0.0_f64.to_bits());
}

#[test]
fn substituted_named_zero_is_not_a_literal_neutral_rule() {
    let source = "component C { public parameter zero: 1 = 0; field x: 1 = 1; relation r { x = zero; } } model M { instance c: C; }";
    let compiled = crate::compile("named.eqi", source).unwrap();
    let dag = compiled[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation.residuals()),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        dag.nodes()[dag.roots()[0].index() as usize],
        ExprNode::Sub(..)
    ));
}

#[test]
fn full_type_support_and_activation_fail_in_the_source_owner() {
    for (source, message) in [
        (
            "model M { field x: m = 0; relation r { x = 0[s]; } }",
            "incompatible types",
        ),
        (
            "model M { field x: array<1,2>; field y: array<1,3>; relation r { x = y; } }",
            "incompatible types",
        ),
        (
            "model M { domain d = box(0,1,0,1); representation c = continuum; field x on d as c: vector<1,2>; field y on d as c: array<1,2>; relation r on d { x = y; } }",
            "incompatible types",
        ),
        (
            "model M { domain a = box(0,1); domain b = box(0,1); representation c = continuum; field x on a as c:1; field y on b as c:1; relation r on a { x = y; } }",
            "incompatible supports",
        ),
        (
            "model M { domain a = box(0,1); domain b = box(0,1); representation c = continuum; field x on a as c:1; relation r on b { x = 0; } }",
            "Relation scope",
        ),
        (
            "model M { field x:1=0; relation r { next(x) = pre(x); } }",
            "continuous Relation cannot use",
        ),
        (
            "model M { clock tick = periodic(period=1/1,phase=0/1); field x:1=0; relation r at tick { derivative(x) = 0; } }",
            "clocked Relation cannot use",
        ),
        (
            "component C { field x:1=0; clock tick = periodic(period=1/1,phase=0/1); relation r at tick { derivative(x) = 0; } } model M {}",
            "clocked Relation cannot use",
        ),
    ] {
        let diagnostics = crate::compile("denied.eqi", source).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_span().is_some()
                    && diagnostic.message().contains(message)),
            "{source}: {diagnostics:?}"
        );
    }
    for source in [
        "model M { field x:complex<1>=0; relation r { x = -(-0); } }",
        "model M { domain d = box(0,1,0,1); representation c = continuum; field x on d as c:vector<m,2>; relation r on d { x = 0; } }",
        "model M { clock tick = periodic(period=1/1,phase=0/1); field x:1=0; relation r at tick { next(x) = pre(x); } }",
    ] {
        crate::compile("positive.eqi", source).unwrap();
    }
}

#[test]
fn unit_normalization_retains_representable_subnormal_and_rejects_nonzero_underflow() {
    assert_eq!(
        crate::units::normalize_value(f64::from_bits(1), 1.0)
            .unwrap()
            .to_bits(),
        1
    );
    assert!(crate::units::normalize_value(f64::from_bits(1), 0.001).is_err());
    assert_eq!(
        crate::units::normalize_value(-0.0, 0.001)
            .unwrap()
            .to_bits(),
        0
    );
}

#[test]
fn checked_residuals_preserve_negative_base_and_signed_power_meaning() {
    for (expression, expected) in [
        ("-x^2", -4.0),
        ("(-x)^2", 4.0),
        ("x^-2", 0.25),
        ("-x^-2", -0.25),
    ] {
        let compiled = crate::compile(
            "precedence.eqi",
            &format!("model M {{ field x:1=2; relation r {{ {expression} = 0; }} }}"),
        )
        .unwrap();
        let dag = compiled[0]
            .transaction()
            .ops()
            .iter()
            .find_map(|operation| match operation {
                Op::DefineKernelNode {
                    node: KernelNode::Relation(relation),
                } => Some(relation.residuals()),
                _ => None,
            })
            .unwrap();
        let mut values: Vec<f64> = Vec::new();
        for node in dag.nodes() {
            values.push(match node {
                ExprNode::Symbol(_) => 2.0,
                ExprNode::Neg(value) => -values[value.index() as usize],
                ExprNode::PowI(base, exponent) => values[base.index() as usize].powi(*exponent),
                other => panic!("outside fixed power corpus: {other:?}"),
            });
        }
        assert_eq!(
            values[dag.roots()[0].index() as usize],
            expected,
            "{expression}"
        );
    }
}
