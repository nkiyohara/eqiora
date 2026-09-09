use eqiora_lang::{ConnectorSyntax, format, parse};

#[test]
fn named_scalar_and_support_specialized_field_contracts_roundtrip() {
    let source = r#"connector Pin {
  across voltage: V;
  through current: A;
}
connector Mechanical {
  trace velocity: m/s;
  flux traction: Pa;
  shape spatial_vector;
  frame spatial;
  pairing euclidean_boundary_duality;
  orientation parent_outward;
}
component Resistor(port positive: Pin, port negative: Pin) {
  relation law { positive.voltage = negative.voltage; }
  connect positive, negative;
}
model Circuit() {
  domain electrical = scalar_physical(across voltage: V, through current: A);
  port positive: electrical;
  port negative: electrical;
  connect positive, negative;
}
"#;
    let document = parse("named.eqi", source)
        .into_document()
        .expect("named quantities");
    let ConnectorSyntax::ScalarPhysical {
        across_name,
        through_name,
        ..
    } = document.connectors()[0].syntax()
    else {
        panic!("scalar connector")
    };
    assert_eq!(
        (across_name.as_str(), through_name.as_str()),
        ("voltage", "current")
    );
    let canonical = format(&document);
    assert!(canonical.contains("connect positive, negative;"));
    assert!(!canonical.contains("conserving"));
    assert!(!canonical.contains("field_physical"));
    assert_eq!(
        format(&parse("canonical.eqi", &canonical).into_document().unwrap()),
        canonical
    );
}

#[test]
fn displaced_or_ambiguous_connector_contracts_are_rejected() {
    for source in [
        "connector Pin = scalar_physical(across = V, through = A); model M() {}",
        "connector Pin { across value: V; through value: A; } model M() {}",
        "connector Pin { across voltage: V; } model M() {}",
        "connector Pin { across voltage: V; through current: A; flux extra: Pa; } model M() {}",
        "connector M { trace v: m/s; flux t: Pa; shape spatial_vector; frame spatial; pairing euclidean_boundary_duality; } model A() {}",
        "connector M { trace v: m/s; flux t: Pa; shape spatial_vector; frame spatial; pairing euclidean_boundary_duality; orientation inward; } model A() {}",
        "model M() { connect conserving a, b; }",
        "model M() { port p: conserving on electrical; }",
    ] {
        assert!(
            parse("invalid.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
}
