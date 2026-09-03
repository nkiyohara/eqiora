use eqiora_lang::{ExprKind, Item, SourceAstFactory, TextRange, format, parse};

#[test]
fn scalar_field_absence_roundtrips_through_source() {
    let document = parse("uninitialized-field.eqi", "model M { field pressure: Pa; }")
        .into_document()
        .expect("uninitialized scalar Field parses");
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("fixture contains one Field");
    };
    assert_eq!(field.initial(), None);

    let formatted = format(&document);
    assert_eq!(formatted, "model M {\n  field pressure: Pa;\n}\n");
    assert!(parse("formatted.eqi", &formatted).into_document().is_ok());
}

#[test]
fn factory_constructs_an_uninitialized_scalar_field() {
    let range = TextRange::new(0, 0);
    let dimension = SourceAstFactory::expression(ExprKind::Number(1.0), range).expect("dimension");
    let field =
        SourceAstFactory::field_with_shape("pressure", None, None, None, dimension, None, range)
            .expect("uninitialized scalar Field");
    let model = SourceAstFactory::model(
        eqiora_lang::VisibilitySyntax::Private,
        "flow",
        vec![Item::Field(field)],
        range,
    )
    .expect("model");
    let document =
        SourceAstFactory::document(Vec::new(), Vec::new(), vec![model]).expect("document");

    let source = format(&document);
    assert_eq!(source, "model flow {\n  field pressure: 1;\n}\n");
    assert!(parse("factory-field.eqi", &source).into_document().is_ok());
}
