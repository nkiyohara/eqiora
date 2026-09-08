//! Static case values reuse exact enum exhaustiveness and branch typing.
use super::*;
use eqiora_schema::kernel::typing::ExpressionType;

pub(super) fn evaluate(
    file: &str,
    expression: &Expr,
    expected: Option<ScalarDomain>,
    evaluate_values: bool,
    mut operand: impl FnMut(&Expr, Option<ScalarDomain>, bool) -> Result<EvaluatedParameter, Diagnostic>,
) -> Result<EvaluatedParameter, Diagnostic> {
    if let Some(literal) = expression.resolved_enum() {
        return Ok(EvaluatedParameter {
            value_type: EvaluatedType::Known(literal.value_type().clone()),
            value: evaluate_values.then(|| literal.clone()),
            expression: Some(LoweringExpression::literal(
                literal.clone(),
                expression.range(),
            )),
            bare_literal: false,
            lineage: Some(ParameterLineage::Constant),
        });
    }
    let ExprKind::Case { value, arms } = expression.kind() else {
        unreachable!("checked case")
    };
    let selector = operand(value, None, evaluate_values)?;
    let patterns = crate::enumeration::case_patterns(
        file,
        expression.range(),
        selector.value_type.value_type(),
        arms,
    )?;
    let selected = selector.value.as_ref().and_then(ValueLiteral::enum_tag);
    let branches = arms
        .iter()
        .zip(&patterns)
        .map(|(arm, pattern)| {
            operand(
                arm.value(),
                expected,
                evaluate_values && selected.is_some() && selected == pattern.enum_tag(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    finish(file, expression.range(), selector, patterns, branches)
}

pub(super) fn finish(
    file: &str,
    range: TextRange,
    selector: EvaluatedParameter,
    patterns: Vec<ValueLiteral>,
    branches: Vec<EvaluatedParameter>,
) -> Result<EvaluatedParameter, Diagnostic> {
    let inferred = crate::enumeration::result_type(
        ExpressionType::<()>::new(selector.value_type.value_type().clone(), None),
        &branches
            .iter()
            .map(|branch| ExpressionType::new(branch.value_type.value_type().clone(), None))
            .collect::<Vec<_>>(),
    )
    .map_err(|error| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string()))?;
    let selected = selector.value.as_ref().and_then(ValueLiteral::enum_tag);
    let value = patterns
        .iter()
        .zip(&branches)
        .find_map(|(pattern, branch)| {
            (selected.is_some() && selected == pattern.enum_tag())
                .then(|| branch.value.clone())
                .flatten()
        });
    let lineage = branches
        .iter()
        .fold(selector.lineage.clone(), |lineage, branch| {
            combine_lineages(lineage, branch.lineage.clone())
        });
    let expression = selector
        .expression
        .zip(
            patterns
                .into_iter()
                .zip(&branches)
                .map(|(pattern, branch)| branch.expression.clone().map(|value| (pattern, value)))
                .collect::<Option<Vec<_>>>(),
        )
        .map(|(selector, arms)| LoweringExpression::case(selector, arms, range));
    Ok(EvaluatedParameter {
        value,
        expression,
        lineage,
        bare_literal: false,
        value_type: EvaluatedType::Known(inferred.value_type),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expression(value: &str) -> Expr {
        let source = format!(
            "enum Mode {{ A, B }} enum Other {{ A, B }} enum Single {{ Only }} model M() {{ let value = {value}; }}"
        );
        let mut document = eqiora_lang::parse("case.eqi", &source)
            .into_document()
            .unwrap();
        let namespace = crate::identity::IdentityNamespace::new(["static-case-test"]).unwrap();
        let enums =
            crate::enumeration::declarations("case.eqi", &document, &namespace, |_| None).unwrap();
        crate::enumeration::bind_document("case.eqi", &mut document, &enums).unwrap();
        let eqiora_lang::Item::Let(value) = &document.models()[0].items()[0] else {
            panic!("let")
        };
        value.value().clone()
    }
    fn evaluate(value: &str) -> Result<EvaluatedParameter, Diagnostic> {
        super::super::evaluate_mode(
            "case.eqi",
            &expression(value),
            ExpressionContext::Let,
            &mut |_, range| {
                Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    "case.eqi",
                    range,
                    "unknown value",
                ))
            },
            (&mut |_| None, &mut |_| None),
            None,
            true,
        )
    }
    #[test]
    fn checked_enum_literals_and_lazy_exhaustive_case_preserve_nominal_values() {
        let literal = evaluate("Mode.A").unwrap().value.unwrap();
        assert_eq!(literal.enum_tag(), Some(0));
        let value = evaluate("case Mode.A { Mode.B => math.sqrt(-1), Mode.A => 3[m] }");
        assert!(
            value.is_err(),
            "inactive branch still needs the same dimension"
        );
        let value = evaluate("case Mode.A { Mode.B => math.sqrt(-1), Mode.A => 3 }")
            .unwrap()
            .value
            .unwrap();
        assert_eq!(value.real_scalar_value().unwrap().value(), 3.0);
        let tag = evaluate("case Mode.B { Mode.A => Other.A, Mode.B => Other.B }")
            .unwrap()
            .value
            .unwrap();
        assert_eq!(tag.enum_tag(), Some(1));
        assert_ne!(tag.value_type(), literal.value_type());
        assert!(evaluate("case Mode.B { Mode.A => 3, Mode.B => math.sqrt(-1) }").is_err());
    }
    #[test]
    fn missing_foreign_and_mismatched_case_arms_fail_without_coercion() {
        for value in [
            "case Mode.A { Mode.A => 1 }",
            "case Mode.A { Mode.A => 1, Other.B => 2 }",
            "case 1 { Mode.A => 1, Mode.B => 2 }",
            "case Mode.A { Mode.A => 1, Mode.B => false }",
            "case Mode.A { Mode.A => 1, Mode.B => missing }",
            "case Mode.A { Mode.A => Mode.A, Mode.B => Other.B }",
        ] {
            assert!(evaluate(value).is_err(), "{value}");
        }
    }
    #[test]
    fn singleton_case_demands_a_partial_selector_and_keeps_all_dependencies() {
        assert!(
            evaluate(
                "case (if math.sqrt(-1) > 0 then Single.Only else Single.Only) { Single.Only => 1 }"
            )
            .is_err()
        );
        let (deps, errors) = super::super::super::dependencies::collect_expression_dependencies(
            "case.eqi",
            &expression("case Mode.A { Mode.A => a, Mode.B => b }"),
            |name| matches!(name, "a" | "b"),
            ExpressionContext::Default,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            deps.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }
}
