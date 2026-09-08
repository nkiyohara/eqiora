use eqiora_lang::{ComponentItem, ExprKind, Item, SourceAstFactory, TextRange, VisibilitySyntax};

#[test]
fn private_aliases_round_trip_with_comments_and_type_assertions() {
    let source = "component C() {\n /// local distance\n let distance: m = 2; // retained\n let doubled = distance + distance;\n}\n";
    let document = eqiora_lang::parse("let.eqi", source)
        .into_document()
        .unwrap();
    let ComponentItem::Let(alias) = &document.components()[0].items()[0] else {
        panic!("let");
    };
    assert_eq!(alias.name(), "distance");
    assert_eq!(document.doc_comments().count(), 1);
    assert!(alias.value_type().is_some());
    let formatted = eqiora_lang::format(&document);
    assert!(formatted.contains("local distance"));
    assert!(formatted.contains("retained"));
    let reparsed = eqiora_lang::parse("let.eqi", &formatted)
        .into_document()
        .unwrap();
    assert_eq!(eqiora_lang::format(&reparsed), formatted);
    assert!(
        eqiora_lang::parse("public.eqi", "component C() { public let a = 1; }")
            .into_document()
            .is_err()
    );
}

#[test]
fn one_factory_alias_declaration_can_be_owned_by_either_container() {
    let range = TextRange::new(0, 1);
    let value = SourceAstFactory::expression(
        ExprKind::Number(eqiora_lang::DecimalLiteral::parse("2.0").expect("exact literal")),
        range,
    )
    .unwrap();
    let alias = SourceAstFactory::let_alias("a", None, None, None, value, range).unwrap();
    let component = SourceAstFactory::component(
        VisibilitySyntax::Private,
        "C",
        vec![],
        vec![ComponentItem::Let(alias.clone())],
        range,
    )
    .unwrap();
    let model = SourceAstFactory::model(
        VisibilitySyntax::Private,
        "M",
        vec![],
        vec![Item::Let(alias)],
        range,
    )
    .unwrap();
    let document = SourceAstFactory::document(vec![], vec![component], vec![model]).unwrap();
    let formatted = eqiora_lang::format(&document);
    assert_eq!(formatted.matches("let a = 2;").count(), 2);
    assert!(
        eqiora_lang::parse("factory.eqi", &formatted)
            .into_document()
            .is_ok()
    );
}
