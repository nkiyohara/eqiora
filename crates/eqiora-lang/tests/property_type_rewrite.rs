use eqiora_lang::{ExprKind, SourceAstFactory, TextRange};

#[test]
fn property_dimension_rewrite_preserves_complete_result_type() {
    let source =
        "property contract Value(): array<complex<Voltage>, 2> { derivatives value_only; }";
    let mut document = eqiora_lang::parse("property.eqi", source)
        .into_document()
        .unwrap();
    SourceAstFactory::rewrite_dimension_expressions(&mut document, |_| {
        SourceAstFactory::expression(ExprKind::Name("V".into()), TextRange::new(0, 0)).unwrap()
    });
    let formatted = eqiora_lang::format(&document);
    assert!(formatted.contains("array<complex<V>, 2>"));
    assert_eq!(
        eqiora_lang::format(
            &eqiora_lang::parse("again.eqi", &formatted)
                .into_document()
                .unwrap()
        ),
        formatted
    );
}
