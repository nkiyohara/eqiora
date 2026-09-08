use eqiora_lang::{
    BinaryOp, DecimalLiteral, DraftExpression, Expr, ExprKind, Item, NamePath,
    SourceAstFactory as F, TextRange, format, parse,
};

fn value(source: &str) -> Expr {
    let document = parse(
        "select.eqi",
        &format!("model M() {{ let value = {source}; }}"),
    )
    .into_document()
    .unwrap();
    let Item::Let(value) = &document.models()[0].items()[0] else {
        panic!("value")
    };
    value.value().clone()
}

#[test]
fn nested_selections_preserve_branch_ownership_precedence_and_ranges() {
    let source = "if x < 0 then if p then -x else x else x + 2";
    let expression = value(source);
    let ExprKind::Select {
        condition,
        then_value,
        else_value,
    } = expression.kind()
    else {
        panic!("select")
    };
    assert!(matches!(
        condition.kind(),
        ExprKind::Binary {
            op: BinaryOp::Less,
            ..
        }
    ));
    assert!(matches!(then_value.kind(), ExprKind::Select { .. }));
    assert!(matches!(
        else_value.kind(),
        ExprKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
    let full = format!("model M() {{ let value = {source}; }}");
    for (expression, spelling) in [
        (condition.as_ref(), "x < 0"),
        (then_value.as_ref(), "if p then -x else x"),
        (else_value.as_ref(), "x + 2"),
    ] {
        assert_eq!(
            &full[expression.range().start() as usize..expression.range().end() as usize],
            spelling
        );
    }
    assert!(matches!(
        value("(if true then 1 else 2) + 3").kind(),
        ExprKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn canonical_format_preserves_nested_selects_and_comments() {
    for body in [
        "if true then 1 else if false then 2 else 3",
        "if (if true then false else true) then 1 else 2",
        "(if p then x else y)^2",
        "math.sqrt(if x > 0 then x else 0)",
        "if p then // true branch\n x else // false branch\n y",
    ] {
        let document = parse("round.eqi", &format!("operator f(input x: 1): 1 = {body};"))
            .into_document()
            .unwrap();
        let rendered = format(&document);
        assert_eq!(
            format(&parse("again.eqi", &rendered).into_document().unwrap()),
            rendered
        );
        if body.contains("//") {
            assert!(rendered.contains("// true branch") && rendered.contains("// false branch"));
        }
    }
}

#[test]
fn missing_branches_and_unparenthesized_tight_operands_reject_locally() {
    for body in [
        "if then 1 else 2",
        "if true 1 else 2",
        "if true then else 2",
        "if true then 1",
        "if true then 1 else",
        "if true then else else 2",
        "1 + if true then 2 else 3",
        "not if true then false else true",
    ] {
        let source = format!("model M() {{ let value = {body}; }}");
        let diagnostics = parse("missing.eqi", &source).into_document().unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("expected")
                    || diagnostic.message().contains("parentheses")),
            "{body}: {diagnostics:?}"
        );
    }
}

#[test]
fn source_factory_and_draft_share_select_and_rewrite_all_three_children() {
    let range = TextRange::new(0, 0);
    let name = |name: &str| F::expression(ExprKind::Name(name.to_owned()), range).unwrap();
    let native = F::expression(
        ExprKind::Select {
            condition: Box::new(name("p")),
            then_value: Box::new(name("x")),
            else_value: Box::new(name("y")),
        },
        range,
    )
    .unwrap();
    let rewritten = native.rewrite_name_paths(|path| {
        Some(NamePath::from_segments(["scope", path.as_str()], range).unwrap())
    });
    let ExprKind::Select {
        condition,
        then_value,
        else_value,
    } = rewritten.kind()
    else {
        panic!("select")
    };
    for (child, expected) in [
        (condition, "scope.p"),
        (then_value, "scope.x"),
        (else_value, "scope.y"),
    ] {
        assert!(matches!(child.kind(), ExprKind::Path(path) if path.as_str() == expected));
    }
    let constant = |value| DraftExpression::constant(DecimalLiteral::parse(value).unwrap());
    let draft =
        DraftExpression::select(DraftExpression::boolean(true), constant("1"), constant("2"))
            .source_ast(|_| None, |_| None)
            .unwrap();
    assert!(
        matches!(draft.kind(), ExprKind::Select { condition, then_value, else_value } if matches!(condition.kind(), ExprKind::Boolean(true)) && matches!(then_value.kind(), ExprKind::Number(value) if value.to_i64().unwrap() == 1) && matches!(else_value.kind(), ExprKind::Number(value) if value.to_i64().unwrap() == 2))
    );
}

#[test]
fn shared_scoped_visitor_visits_predicate_and_both_branches_before_select() {
    let mut document = parse(
        "visit.eqi",
        "model M() { let selected = if p then x else y; }",
    )
    .into_document()
    .unwrap();
    let mut names = Vec::new();
    F::visit_expressions(&mut document, |scope, expression| {
        assert_eq!(scope, Some("M"));
        match expression.kind() {
            ExprKind::Name(name) => names.push(name.clone()),
            ExprKind::Select { .. } => names.push("select".into()),
            _ => {}
        }
    });
    assert_eq!(names, ["p", "x", "y", "select"]);
}

#[test]
fn selection_depth_uses_existing_shared_limit() {
    for (depth, accepted) in [(255, true), (256, false)] {
        let body = format!("{}0", "if true then 1 else ".repeat(depth));
        assert_eq!(
            parse("depth.eqi", &format!("model M() {{ let x = {body}; }}"))
                .into_document()
                .is_ok(),
            accepted
        );
    }
    let range = TextRange::new(0, 0);
    let predicate = F::expression(ExprKind::Boolean(true), range).unwrap();
    let leaf = F::expression(ExprKind::Number(DecimalLiteral::parse("1").unwrap()), range).unwrap();
    let mut value = leaf.clone();
    for _ in 0..255 {
        value = F::expression(
            ExprKind::Select {
                condition: Box::new(predicate.clone()),
                then_value: Box::new(leaf.clone()),
                else_value: Box::new(value),
            },
            range,
        )
        .unwrap();
    }
    assert!(
        F::expression(
            ExprKind::Select {
                condition: Box::new(predicate),
                then_value: Box::new(leaf),
                else_value: Box::new(value)
            },
            range
        )
        .is_err()
    );
}

#[test]
fn native_selection_checks_unselected_branch_ownership_and_literals() {
    use eqiora_core::{DimExponents, ScalarDomain, ValueType};
    use eqiora_lang::{DraftField, DraftRelation, FieldRoleSyntax, ModelDraft};
    let kind = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap();
    let included = DraftField::new("x", kind.clone(), FieldRoleSyntax::Variable);
    let foreign = DraftField::new("x", kind, FieldRoleSyntax::Variable);
    let make = |branch| {
        let selected = DraftExpression::select(
            DraftExpression::boolean(true),
            included.expression(),
            branch,
        );
        let relation = DraftRelation::continuous("law", [(included.expression(), selected)]);
        ModelDraft::new("M", [included.clone().into(), relation.into()])
    };
    assert!(make(included.expression()).is_ok());
    assert!(make(foreign.expression()).is_err());
    assert!(make(DraftExpression::complex(f64::INFINITY, 0.0)).is_err());
}
