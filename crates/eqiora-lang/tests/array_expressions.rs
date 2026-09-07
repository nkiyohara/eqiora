use eqiora_lang::{Expr, ExprKind, Item, SourceAstFactory, TextRange};

fn expression(source: &str) -> Expr {
    let source = format!("model M {{ let a = {source}; }}");
    let document = eqiora_lang::parse("array.eqi", &source)
        .into_document()
        .unwrap();
    let Item::Let(alias) = &document.models()[0].items()[0] else {
        panic!("let")
    };
    alias.value().clone()
}

#[test]
fn arrays_and_postfix_indices_preserve_structure_and_precedence() {
    for source in [
        "[1, 2]",
        "[[1, 2], [3, 4]][1][0]",
        "(a + b)[2] ^ 2",
        "-a[2]",
        "math.complex(a, b)[0]",
        "(2)[0]",
        "(a ^ b)[0]",
        "a[b[0]]",
    ] {
        let document = eqiora_lang::parse("array.eqi", &format!("model M {{ let a = {source}; }}"))
            .into_document()
            .unwrap();
        let formatted = eqiora_lang::format(&document);
        assert_eq!(
            eqiora_lang::format(
                &eqiora_lang::parse("again.eqi", &formatted)
                    .into_document()
                    .unwrap()
            ),
            formatted
        );
    }
    let ExprKind::Index { value, index } = expression("[[1, 2], [3, 4]][1][0]").kind().clone()
    else {
        panic!("index")
    };
    assert!(matches!(value.kind(), ExprKind::Index { .. }));
    assert!(matches!(index.kind(), ExprKind::Number(0.0)));
    assert!(
        matches!(expression("-a[2]").kind(), ExprKind::Unary { value, .. } if matches!(value.kind(), ExprKind::Index { .. }))
    );
}

#[test]
fn quantities_and_explicit_boundary_selectors_keep_their_discriminators() {
    assert!(matches!(
        expression("10 [ms]").kind(),
        ExprKind::Quantity { .. }
    ));
    assert!(matches!(
        expression("port[side = wall]").kind(),
        ExprKind::BoundaryPortSelection { .. }
    ));
    assert!(matches!(
        expression("samples[position]").kind(),
        ExprKind::Index { .. }
    ));
    assert!(
        eqiora_lang::parse("unit.eqi", "model M { let a = 10 [2]; }")
            .into_document()
            .is_err()
    );
    for invalid in ["[]", "a[]", "[1,]", "a[side = 2]", "a[1, 2]"] {
        assert!(
            eqiora_lang::parse("bad.eqi", &format!("model M {{ let a = {invalid}; }}"))
                .into_document()
                .is_err()
        );
    }
    // Rectangularity and static index legality belong to the shared compiler.
    assert!(matches!(
        expression("[[1], [2, 3]]").kind(),
        ExprKind::Array(_)
    ));
}

#[test]
fn factory_and_reference_rewrite_preserve_index_and_array_structure() {
    let range = TextRange::new(0, 1);
    let value = SourceAstFactory::expression(ExprKind::Name("x".into()), range).unwrap();
    let array = SourceAstFactory::expression(ExprKind::Array(vec![value]), range).unwrap();
    let index = SourceAstFactory::expression(ExprKind::Number(0.0), range).unwrap();
    let value = SourceAstFactory::expression(
        ExprKind::Index {
            value: Box::new(array),
            index: Box::new(index),
        },
        range,
    )
    .unwrap();
    let mut names = vec![];
    let rewritten = value.rewrite_name_paths(|name| {
        names.push(name.as_str().to_owned());
        None
    });
    assert_eq!(names, ["x"]);
    assert_eq!(rewritten, value);
    assert!(SourceAstFactory::expression(ExprKind::Array(vec![]), range).is_err());
    let mut nested = SourceAstFactory::expression(ExprKind::Number(1.0), range).unwrap();
    for _ in 1..256 {
        nested = SourceAstFactory::expression(ExprKind::Array(vec![nested]), range).unwrap();
    }
    assert!(SourceAstFactory::expression(ExprKind::Array(vec![nested]), range).is_err());
}

#[test]
fn parameter_initializers_and_nested_comments_round_trip() {
    let source = "model M {\n/// channels\nparameter p: array<V, 2> = [1[V], // first\n 2[V]];\nlet z = math.complex(p[0], p[1]);\n}";
    let document = eqiora_lang::parse("values.eqi", source)
        .into_document()
        .unwrap();
    let formatted = eqiora_lang::format(&document);
    assert!(formatted.contains("channels"));
    assert!(formatted.contains("first"));
    assert_eq!(
        eqiora_lang::format(
            &eqiora_lang::parse("again.eqi", &formatted)
                .into_document()
                .unwrap()
        ),
        formatted
    );
}

#[test]
fn parser_rejects_excessive_array_and_index_depth() {
    for source in [
        format!("{}1{}", "[".repeat(256), "]".repeat(256)),
        format!("a{}", "[0]".repeat(256)),
    ] {
        assert!(
            eqiora_lang::parse("deep.eqi", &format!("model M {{ let a = {source}; }}"))
                .into_document()
                .is_err()
        );
    }
}
