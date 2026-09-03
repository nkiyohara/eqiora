use eqiora_lang::{format, parse};

const COMMENTED_EXAMPLES: &[(&str, &str)] = &[
    (
        "algebraic-split.eqi",
        include_str!("../../../examples/algebraic-split.eqi"),
    ),
    ("decay.eqi", include_str!("../../../examples/decay.eqi")),
    (
        "fixed-reference-fsi.eqi",
        include_str!("../../../examples/fixed-reference-fsi.eqi"),
    ),
    (
        "mixed-boundary-elasticity.eqi",
        include_str!("../../../examples/mixed-boundary-elasticity.eqi"),
    ),
    (
        "steady-flow-past-cylinder.eqi",
        include_str!("../../../examples/steady-flow-past-cylinder.eqi"),
    ),
];

#[test]
fn formatting_preserves_all_commented_examples_byte_for_byte() {
    for (filename, source) in COMMENTED_EXAMPLES {
        let document = parse(*filename, source)
            .into_document()
            .expect("commented example parses");
        let formatted = format(&document);
        assert_eq!(&formatted, source, "{filename}");

        let reparsed = parse(*filename, &formatted)
            .into_document()
            .expect("preserved example reparses");
        assert_eq!(format(&reparsed), formatted, "{filename}");
    }
}

#[test]
fn formatting_preserves_leading_trailing_and_body_comments() {
    let source = "// leading\nmodel M { // trailing\n  // body\n  field x: 1 = 0;\n}\n";
    let document = parse("comments.eqi", source)
        .into_document()
        .expect("comment positions parse");

    assert_eq!(format(&document), source);
}

#[test]
fn formatting_still_canonicalizes_comment_free_source() {
    let document = parse(
        "plain.eqi",
        "model plain{field x:1=0;relation r continuous{x=0;}}",
    )
    .into_document()
    .expect("plain source parses");
    let formatted = format(&document);

    assert_eq!(
        formatted,
        "model plain {\n  field x: 1 = 0;\n  relation r continuous {\n    x = 0;\n  }\n}\n"
    );
    let reparsed = parse("plain.eqi", &formatted)
        .into_document()
        .expect("formatted plain source reparses");
    assert_eq!(format(&reparsed), formatted);
}
