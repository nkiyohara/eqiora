//! The accepted table specimen has one closed source production.
const SOURCE: &str = r#"
public property contract Conductivity(input temperature: K): W / (m * K) {
  derivatives first_open_intervals;
}
public property release SyntheticConductivity: Conductivity {
  table {
    data conductivity_samples;
    axis temperature: K;
    value conductivity: W / (m * K);
    interpolation piecewise_affine;
    preprocessing identity;
    missing reject;
    knot_derivative reject;
    endpoint_derivative reject;
  }
  validity temperature in [310[K], 350[K]];
  outside reject;
  branch single;
  citation synthetic_definition;
  license repository_license;
}
"#;
#[test]
fn table_source_preserves_exact_asset_columns_and_declared_interval() {
    let document = eqiora_lang::parse("table.eqi", SOURCE)
        .into_document()
        .unwrap();
    let (_, _, _, value, ..) = document.property_release_syntax().next().unwrap();
    let eqiora_lang::PropertySourceSyntax::Table(table) = value else {
        panic!("table source")
    };
    assert_eq!(table.data().as_str(), "conductivity_samples");
    assert_eq!(table.axis(), "temperature");
    let formatted = eqiora_lang::format(&document);
    assert!(formatted.contains("validity temperature in ["));
    assert!(formatted.contains("derivatives first_open_intervals;"));
    assert_eq!(
        formatted,
        eqiora_lang::format(
            &eqiora_lang::parse("reopened.eqi", &formatted)
                .into_document()
                .unwrap()
        )
    );
}
#[test]
fn unspecified_or_substituted_table_policies_reject() {
    for invalid in [
        SOURCE.replace("piecewise_affine", "cubic"),
        SOURCE.replace("missing reject;", ""),
        SOURCE.replace("endpoint_derivative reject", "endpoint_derivative adjacent"),
        SOURCE.replace("validity temperature", "validity other"),
        SOURCE.replace("outside reject;", ""),
    ] {
        assert!(
            eqiora_lang::parse("invalid-table.eqi", &invalid)
                .into_document()
                .is_err()
        );
    }
}
