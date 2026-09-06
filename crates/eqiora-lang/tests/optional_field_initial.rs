use eqiora_lang::{ExprKind, Item, SourceAstFactory, TextRange, format, parse};

#[test]
fn field_initial_units_and_sign_roundtrip_without_erasure() {
    let source = "model M { field distance: complex<m> = -2500[mm]; }";
    let document = parse("field-unit.eqi", source).into_document().unwrap();
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("expected Field");
    };
    let initial = field.initial().unwrap();
    assert_eq!(
        &source[initial.range().start() as usize..initial.range().end() as usize],
        "-2500[mm]"
    );
    assert!(
        matches!(initial.kind(), ExprKind::Quantity { value: -2500.0, unit } if matches!(unit.kind(), ExprKind::Name(name) if name == "mm"))
    );
    let formatted = format(&document);
    assert!(formatted.contains("= -2500 [mm];"));
    let reparsed = parse("formatted.eqi", &formatted).into_document().unwrap();
    assert_eq!(format(&reparsed), formatted);
}

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
    let value_type = SourceAstFactory::value_type(
        eqiora_lang::ValueTypeSyntaxKind::Scalar {
            domain: eqiora_core::ScalarDomain::Real,
            dimension,
        },
        range,
    )
    .unwrap();
    let field = SourceAstFactory::field("pressure", None, None, value_type, None, range)
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
