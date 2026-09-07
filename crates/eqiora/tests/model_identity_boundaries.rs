use eqiora::api::ModelDocument;

const SOURCE: &str = r#"
model Decay() {
  state x: 1; initial { x = 1; }
  parameter rate: 1 / s = 2;
  relation law {
    derivative(x) + rate * x = 0;
    x - 1 = 0;
  }
}
"#;

#[test]
fn source_mutations_separate_structure_from_exact_occurrence_identity() {
    let baseline = ModelDocument::compile("source.eqi", SOURCE).unwrap();
    let mutations = [
        ("repeat source", "source.eqi", SOURCE.to_owned(), true, true),
        (
            "source move",
            "moved/source.eqi",
            SOURCE.to_owned(),
            true,
            true,
        ),
        (
            "comment",
            "source.eqi",
            format!("// A display-only description.\n{SOURCE}"),
            true,
            true,
        ),
        (
            "identifier rename",
            "source.eqi",
            SOURCE
                .replace("Decay", "Renamed")
                .replace("rate", "decay_rate"),
            true,
            false,
        ),
        (
            "coherent input-unit notation",
            "source.eqi",
            SOURCE.replace("rate: 1 / s = 2", "rate: Hz = 2 [Hz]"),
            true,
            false,
        ),
        (
            "initial state",
            "source.eqi",
            SOURCE.replace("initial { x = 1; }", "initial { x = 3; }"),
            false,
            false,
        ),
        (
            "law",
            "source.eqi",
            SOURCE.replace("rate * x", "rate / x"),
            false,
            false,
        ),
        (
            "ordered equation roots",
            "source.eqi",
            SOURCE.replace(
                "derivative(x) + rate * x = 0;\n    x - 1 = 0;",
                "x - 1 = 0;\n    derivative(x) + rate * x = 0;",
            ),
            false,
            false,
        ),
    ];
    for (mutation, filename, source, equivalent, exact_same) in mutations {
        let candidate = ModelDocument::compile(filename, &source).unwrap();
        assert_eq!(
            baseline.structurally_equivalent(&candidate).unwrap(),
            equivalent,
            "{mutation}",
        );
        assert_eq!(
            baseline.artifact_reference().unwrap() == candidate.artifact_reference().unwrap(),
            exact_same,
            "canonical source identity: {mutation}",
        );
        let bytes = candidate.canonical_json().unwrap();
        let replayed = ModelDocument::replay(&bytes).unwrap();
        assert_eq!(replayed.canonical_json().unwrap(), bytes, "{mutation}");
        assert_eq!(
            replayed.artifact_reference().unwrap(),
            candidate.artifact_reference().unwrap(),
            "exact replay: {mutation}",
        );
        assert!(candidate.structurally_equivalent(&replayed).unwrap());
    }
}
