use super::*;
use crate::hierarchy::parameters::SymbolicParameterValue;
use crate::lower::LoweringExpression;
use eqiora_core::{DimExponents, ScalarDomain, ValueType};
use eqiora_lang::{BinaryOp, TextRange};

#[test]
fn forward_shared_aliases_preserve_the_explicit_parameter_expression() {
    let parameter = |name: &str, value| {
        (
            name.to_owned(),
            SymbolicParameterValue {
                value: Some(value),
                value_type: ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
                expression: Some(LoweringExpression::name(
                    name.to_owned(),
                    TextRange::new(0, 0),
                )),
                lineage: None,
            },
        )
    };
    for (x, y) in [(2.0, 3.0), (5.0, -2.0)] {
        for aliases in [
            "let f = z * y; let z = x * x; let shared = f + z;",
            "let z = x * x; let shared = f + z; let f = z * y;",
        ] {
            let source = format!("model M {{ {aliases} }}");
            let document = eqiora_lang::parse("alias.eqi", &source)
                .into_document()
                .unwrap();
            let mut values = BTreeMap::from([parameter("x", x), parameter("y", y)]);
            resolve_model_lets("alias.eqi", &document.models()[0], &mut values).unwrap();
            // The exact retained polynomial is (x*x)*y, not a literal value or a new symbol.
            // Existing differentiation therefore receives the original Parameter dependencies.
            let name = |name: &str| LoweringExpression::name(name.to_owned(), TextRange::new(0, 0));
            let mul = |left, right| {
                LoweringExpression::binary(BinaryOp::Mul, left, right, TextRange::new(0, 0))
            };
            assert_eq!(
                values["f"].expression,
                Some(mul(mul(name("x"), name("x")), name("y")))
            );
            assert_eq!(values["f"].value, Some(x * x * y));
            assert_eq!(values["shared"].value, Some(x * x * y + x * x));
        }
    }
}

#[test]
fn cycle_diagnostics_point_to_an_actual_cycle_reference() {
    for (declarations, cycle, reference) in [
        (
            "let self_ref = self_ref;",
            "self_ref -> self_ref",
            "self_ref;",
        ),
        ("let b = a; let a = b;", "a -> b -> a", "b;"),
        (
            "let dependent = a; let b = a; let a = b;",
            "a -> b -> a",
            "b;",
        ),
    ] {
        let source = format!("model M {{ {declarations} }}");
        let errors = crate::compile("cycle.eqi", &source).unwrap_err();
        let error = errors
            .iter()
            .find(|error| error.message().ends_with(cycle))
            .unwrap();
        let span = error.source_span().unwrap();
        let expected = source.rfind(reference).unwrap();
        assert_eq!(span.start as usize, expected);
        assert_eq!(span.end as usize, expected + reference.len() - 1);
        assert!(
            !errors
                .iter()
                .any(|error| error.message().contains("unknown"))
        );
    }
}

#[test]
fn forward_chain_uses_existing_symbolic_term_bound() {
    let mut source = String::from("model M {");
    for index in 0..255 {
        source.push_str(&format!("let a{index} = a{};", index + 1));
    }
    source.push_str("let a255: m = 2; relation r { a0 = 2[m]; } }");
    let document = eqiora_lang::parse("chain.eqi", &source)
        .into_document()
        .unwrap();
    let mut values = BTreeMap::new();
    resolve_model_lets("chain.eqi", &document.models()[0], &mut values).unwrap();
    assert_eq!(values["a0"].value, Some(2.0));
    assert_eq!(values["a0"].value_type, values["a255"].value_type);
    crate::compile("chain.eqi", &source).unwrap();
    let errors = crate::hierarchy::compile_hierarchy_with_limits(
        "chain.eqi",
        source.len(),
        &document,
        crate::hierarchy::HierarchyLimits {
            max_parameter_terms: 255,
            ..crate::hierarchy::HierarchyLimits::default()
        },
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("255 symbolic Parameter term limit")
    }));
}
