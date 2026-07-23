use eqiora::language::{format, parse};

#[test]
fn public_facade_parses_and_formats_eqiora_source() {
    let source = "model decay{field x:1=1;relation ode continuous{derivative(x)+x=0;}}";
    let document = parse("decay.eqi", source)
        .into_document()
        .expect("valid source");
    let formatted = format(&document);

    assert!(formatted.starts_with("model decay {\n"));
    assert!(formatted.contains("derivative(x) + x = 0;"));
}
