use eqiora_lang::{Expr, ExprKind, Item, ReductionOp, SourceAstFactory, TextRange, format, parse};

fn expression(text: &str) -> Expr {
    let source = format!("model M() {{ let value = {text}; }}");
    let document = parse("reduce.eqi", &source).into_document().unwrap();
    let Item::Let(value) = &document.models()[0].items()[0] else {
        panic!("alias")
    };
    value.value().clone()
}

#[test]
fn reductions_retain_operation_binder_ranges_and_precedence() {
    for (text, operation) in [
        (
            "sum(cell[index(Stages, ordinal(i))].y + 1, over = (i in Stages))",
            ReductionOp::Sum,
        ),
        (
            "product(math.complex(2, 3) ^ ordinal(i), over = (i in catalog.Stages))",
            ReductionOp::Product,
        ),
        ("min(ordinal(i), over = (i in Stages))", ReductionOp::Min),
        ("max(2[m], over = (i in Stages))", ReductionOp::Max),
    ] {
        let source = format!("model M() {{ let value = {text}; }}");
        let document = parse("reduce.eqi", &source).into_document().unwrap();
        let Item::Let(alias) = &document.models()[0].items()[0] else {
            panic!("alias")
        };
        let ExprKind::Reduction {
            operation: actual,
            binder,
            value,
        } = alias.value().kind()
        else {
            panic!("reduction")
        };
        assert_eq!(*actual, operation);
        assert_eq!(binder.member(), "i");
        assert_eq!(
            &source[binder.range().start() as usize..binder.range().end() as usize],
            format!("(i in {})", binder.set())
        );
        assert_eq!(
            &source[alias.value().range().start() as usize..alias.value().range().end() as usize],
            text
        );
        assert_eq!(
            SourceAstFactory::reduction(
                operation,
                binder.clone(),
                value.as_ref().clone(),
                alias.value().range()
            )
            .unwrap(),
            *alias.value()
        );
        let rendered = format(&document);
        assert_eq!(
            format(&parse("again.eqi", &rendered).into_document().unwrap()),
            rendered
        );
    }
    for text in [
        "-sum(ordinal(i), over = (i in Stages)) ^ 2",
        "product(sum(ordinal(i) + ordinal(j), over = (j in Inner)), over = (i in Outer))",
    ] {
        let source = format!("component C() {{ let x = {text}; }}");
        let document = parse("nested.eqi", &source).into_document().unwrap();
        let rendered = format(&document);
        assert_eq!(
            format(&parse("again.eqi", &rendered).into_document().unwrap()),
            rendered
        );
    }
}

#[test]
fn reductions_require_one_explicit_nominal_binder() {
    for value in [
        "sum()",
        "sum(1)",
        "sum(1, Stages)",
        "sum(1, over = i in Stages)",
        "sum(1, over = (i Stages))",
        "sum(1, over = (i in 3))",
        "sum(1, over = (i in A + B))",
        "sum(1, over = (i in A), over = (j in B))",
        "sum(over = (i in A), 1)",
        "product(1, over = (i in A), 2)",
    ] {
        assert!(
            parse("bad.eqi", &format!("model M() {{ let x = {value}; }}"))
                .into_document()
                .is_err(),
            "{value}"
        );
    }
}

#[test]
fn lexical_rewrites_preserve_bound_names_and_visit_set_names() {
    let source = expression(
        "sum(ordinal(i) + free + product(ordinal(i) + ordinal(j), over = (j in Inner)), over = (i in Outer))",
    );
    let replacement = expression("catalog.Renamed");
    let ExprKind::Path(replacement) = replacement.kind() else {
        panic!("path")
    };
    let mut visited = Vec::new();
    let rewritten = source.rewrite_name_paths(|path| {
        visited.push(path.as_str().to_owned());
        matches!(path.as_str(), "Outer" | "Inner" | "free").then(|| replacement.clone())
    });
    assert!(!visited.iter().any(|name| name == "i" || name == "j"));
    for name in ["Outer", "Inner", "free"] {
        assert!(visited.iter().any(|actual| actual == name));
    }
    let ExprKind::Reduction { binder, .. } = rewritten.kind() else {
        panic!("reduction")
    };
    assert_eq!(binder.member(), "i");
    assert_eq!(binder.set().as_str(), "catalog.Renamed");
    assert_eq!(rewritten.range(), source.range());
}

#[test]
fn reduction_factory_and_parser_keep_existing_depth_limits() {
    let template = expression("sum(1, over = (i in Stages))");
    let ExprKind::Reduction { binder, .. } = template.kind() else {
        panic!("reduction")
    };
    let range = TextRange::new(0, 1);
    let mut value = expression("1");
    for _ in 0..255 {
        value =
            SourceAstFactory::reduction(ReductionOp::Sum, binder.clone(), value, range).unwrap();
    }
    assert!(
        SourceAstFactory::reduction(ReductionOp::Product, binder.clone(), value, range).is_err()
    );
    for (count, admitted) in [(255, true), (256, false)] {
        let value = format!(
            "{}1{}",
            "sum(".repeat(count),
            ", over = (i in Stages))".repeat(count)
        );
        assert_eq!(
            parse("depth.eqi", &format!("model M() {{ let x = {value}; }}"))
                .into_document()
                .is_ok(),
            admitted
        );
    }
}
