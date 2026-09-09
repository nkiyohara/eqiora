use eqiora_lang::{VisibilitySyntax, format, parse};

#[test]
fn closed_records_preserve_member_types_order_visibility_and_comments() {
    let source = "// sensor bus\npublic record Sensor { voltage: V, // health\n valid: boolean, samples: array<integer,3> } model M() {}";
    let document = parse("record.eqi", source).into_document().unwrap();
    let record = &document.records()[0];
    assert_eq!(record.name(), "Sensor");
    assert_eq!(record.visibility(), VisibilitySyntax::Public);
    assert_eq!(
        record
            .members()
            .iter()
            .map(|m| m.name())
            .collect::<Vec<_>>(),
        ["voltage", "valid", "samples"]
    );
    assert_eq!(record.members()[0].value_type().to_source(), "V");
    assert_eq!(record.members()[1].value_type().to_source(), "boolean");
    assert_eq!(
        record.members()[2].value_type().to_source(),
        "array<integer, 3>"
    );
    let emitted = format(&document);
    assert!(emitted.contains("// sensor bus") && emitted.contains("// health"));
    assert_eq!(
        format(&parse("again.eqi", &emitted).into_document().unwrap()),
        emitted
    );
}

#[test]
fn record_declarations_reject_empty_duplicate_untyped_and_temporal_members() {
    for record in [
        "record R {}",
        "record R { x: V, x: A }",
        "record R { x }",
        "record R { _: V }",
        "record R { x: V at tick }",
    ] {
        assert!(
            parse("invalid.eqi", record).into_document().is_err(),
            "{record}"
        );
    }
}
