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

#[test]
fn component_aliases_preserve_parameter_interface_and_symbolic_polynomial() {
    use crate::hierarchy::parameters::resolve_component_parameters_symbolically;
    let source = "component C() { public parameter x: 1; public parameter y: 1; let f = z * y; let z = x * x; }";
    let document = eqiora_lang::parse("component.eqi", source)
        .into_document()
        .unwrap();
    let component = &document.components()[0];
    let parameters = resolve_component_parameters_symbolically("component.eqi", component).unwrap();
    let mut symbolic = parameters.clone();
    resolve_component_lets("component.eqi", component, &mut symbolic).unwrap();
    assert_eq!(
        parameters.keys().map(String::as_str).collect::<Vec<_>>(),
        ["x", "y"]
    );
    assert!(symbolic["f"].value.is_none());
    assert_eq!(symbolic["f"].value_type, parameters["x"].value_type);
    for (x, y) in [(2.0, 3.0), (5.0, -2.0)] {
        let name = |name: &str| LoweringExpression::name(name.to_owned(), TextRange::new(0, 0));
        let mut values = parameters.clone();
        for (key, value) in [("x", x), ("y", y)] {
            let parameter = values.get_mut(key).unwrap();
            parameter.value = Some(value);
            parameter.expression = Some(name(key));
        }
        resolve_component_lets("component.eqi", component, &mut values).unwrap();
        let mul = |left, right| {
            LoweringExpression::binary(BinaryOp::Mul, left, right, TextRange::new(0, 0))
        };
        assert_eq!(
            values["f"].expression,
            Some(mul(mul(name("x"), name("x")), name("y")))
        );
        assert_eq!(values["f"].value, Some(x * x * y));
        assert!(matches!(
            values["f"].lineage,
            Some(ParameterLineage::Derived)
        ));
    }
}

#[test]
fn unused_component_aliases_consume_the_existing_symbolic_term_budget() {
    let source = "component C() { let a = b; let b = c; let c = 1; } model M {}";
    let document = eqiora_lang::parse("bounded.eqi", source)
        .into_document()
        .unwrap();
    let errors = crate::hierarchy::compile_hierarchy_with_limits(
        "bounded.eqi",
        source.len(),
        &document,
        crate::hierarchy::HierarchyLimits {
            max_parameter_terms: 2,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("2 symbolic Parameter term limit"))
    );
}
