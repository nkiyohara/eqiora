use eqiora_lang::{ComponentItem, ExprKind, Item, SourceAstFactory, TextRange};

#[test]
fn support_assertions_round_trip_in_both_containers_with_source_ownership() {
    for container in ["model M()", "component C()"] {
        for head in ["let q on body", "let q: vector<W / m^2, 2> on body"] {
            let source =
                format!("{container} {{\n/// heat flux\n{head} // support\n = flux; // value\n}}");
            let document = eqiora_lang::parse("support.eqi", &source)
                .into_document()
                .unwrap();
            let alias = if let Some(model) = document.models().first() {
                let Item::Let(alias) = &model.items()[0] else {
                    panic!("let")
                };
                alias
            } else {
                let ComponentItem::Let(alias) = &document.components()[0].items()[0] else {
                    panic!("let")
                };
                alias
            };
            assert_eq!(alias.domain(), Some("body"));
            assert_eq!(
                &source[alias.range().start() as usize..alias.range().end() as usize],
                format!("{head} // support\n = flux;")
            );
            let formatted = eqiora_lang::format(&document);
            for comment in ["heat flux", "support", "value"] {
                assert!(formatted.contains(comment));
            }
            let reparsed = eqiora_lang::parse("again.eqi", &formatted)
                .into_document()
                .unwrap();
            assert_eq!(eqiora_lang::format(&reparsed), formatted);
        }
    }
}

#[test]
fn support_assertions_reject_duplicate_reordered_or_missing_clauses() {
    for container in ["model M()", "component C()"] {
        for head in [
            "let q on body on other",
            "let q on body: W",
            "let q on",
            "let q at clock on body",
        ] {
            let source = format!("{container} {{ {head} = flux; }}");
            assert!(
                eqiora_lang::parse("invalid.eqi", &source)
                    .into_document()
                    .is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn factory_support_assertion_survives_dimension_rewrite_and_reparse() {
    let range = TextRange::new(0, 0);
    let value = SourceAstFactory::expression(ExprKind::Name("flux".into()), range).unwrap();
    let dimension = SourceAstFactory::expression(ExprKind::Name("Length".into()), range).unwrap();
    let alias = SourceAstFactory::let_alias(
        "q",
        Some(
            SourceAstFactory::value_type(
                eqiora_lang::ValueTypeSyntaxKind::Scalar {
                    domain: eqiora_core::ScalarDomain::Real,
                    dimension,
                },
                range,
            )
            .unwrap(),
        ),
        Some("body".into()),
        Some("sample".into()),
        value.clone(),
        range,
    )
    .unwrap();
    let mut document = eqiora_lang::parse(
        "base.eqi",
        "component C() { let q: Length on body at sample = flux; } model M() { let q: Length on body at sample = flux; }",
    )
    .into_document()
    .unwrap();
    let component = SourceAstFactory::component(
        eqiora_lang::VisibilitySyntax::Private,
        "C",
        vec![],
        vec![ComponentItem::Let(alias.clone())],
        range,
    )
    .unwrap();
    let model = SourceAstFactory::model(
        eqiora_lang::VisibilitySyntax::Private,
        "M",
        vec![],
        vec![Item::Let(alias)],
        range,
    )
    .unwrap();
    let native =
        SourceAstFactory::document(Vec::new(), vec![], vec![component], vec![model]).unwrap();
    assert_eq!(eqiora_lang::format(&native), eqiora_lang::format(&document));
    SourceAstFactory::rewrite_dimension_expressions(&mut document, |_| {
        SourceAstFactory::expression(ExprKind::Name("m".into()), range).unwrap()
    });
    let formatted = eqiora_lang::format(&document);
    assert_eq!(
        formatted
            .matches("let q: m on body at sample = flux;")
            .count(),
        2
    );
    assert_eq!(
        eqiora_lang::format(
            &eqiora_lang::parse("again.eqi", &formatted)
                .into_document()
                .unwrap()
        ),
        formatted
    );
    for invalid in ["", "body.other", "two supports"] {
        assert!(
            SourceAstFactory::let_alias(
                "q",
                None,
                None,
                Some(invalid.into()),
                value.clone(),
                range
            )
            .is_err()
        );
        assert!(
            SourceAstFactory::let_alias(
                "q",
                None,
                Some(invalid.into()),
                None,
                value.clone(),
                range
            )
            .is_err()
        );
    }
}
