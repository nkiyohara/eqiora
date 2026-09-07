use eqiora_lang::{
    DecimalLiteral, DraftExpression, ExprKind, Item, SourceAstFactory, TextRange, format, parse,
};

#[test]
fn large_source_and_native_literals_retain_the_same_exact_decimal() {
    let text = "9007199254740993";
    let document = parse("integer.eqi", &format!("model M() {{ let n = {text}; }}"))
        .into_document()
        .unwrap();
    let Item::Let(alias) = &document.models()[0].items()[0] else {
        panic!("let")
    };
    let ExprKind::Number(value) = alias.value().kind() else {
        panic!("literal")
    };
    assert_eq!(value.to_i64().unwrap(), 9007199254740993);
    let native = DraftExpression::constant(DecimalLiteral::parse(text).unwrap()).source_ast();
    assert_eq!(native.kind(), alias.value().kind());
    let rebuilt =
        SourceAstFactory::expression(ExprKind::Number(value.clone()), TextRange::new(0, 0))
            .unwrap();
    assert_eq!(rebuilt.kind(), native.kind());
    let formatted = format(&document);
    assert!(formatted.contains(text));
    assert_eq!(
        format(&parse("round.eqi", &formatted).into_document().unwrap()),
        formatted
    );
}
