use eqiora::api::ModelDocument;

const SOURCE: &str = r#"
model Decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 2;
  relation law continuous {
    derivative(x) + rate * x = 0;
    x - 1 = 0;
  }
}
"#;

#[test]
fn source_mutations_separate_structure_from_exact_occurrence_identity() {
    let baseline = ModelDocument::compile("source.eqi", SOURCE).unwrap();
    let mutations = [
        ("fresh occurrences", "source.eqi", SOURCE.to_owned(), true),
        ("source move", "moved/source.eqi", SOURCE.to_owned(), true),
        (
            "comment",
            "source.eqi",
            format!("// A display-only description.\n{SOURCE}"),
            true,
        ),
        (
            "identifier rename",
            "source.eqi",
            SOURCE
                .replace("Decay", "Renamed")
                .replace("rate", "decay_rate"),
            true,
        ),
        (
            "coherent input-unit notation",
            "source.eqi",
            SOURCE.replace("rate: 1 / s = 2", "rate: Hz = 2 [Hz]"),
            true,
        ),
        (
            "initial state",
            "source.eqi",
            SOURCE.replace("field x: 1 = 1", "field x: 1 = 3"),
            false,
        ),
        (
            "law",
            "source.eqi",
            SOURCE.replace("rate * x", "rate / x"),
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
        ),
    ];
    for (mutation, filename, source, equivalent) in mutations {
        let candidate = ModelDocument::compile(filename, &source).unwrap();
        assert_eq!(
            baseline.structurally_equivalent(&candidate).unwrap(),
            equivalent,
            "{mutation}",
        );
        assert_ne!(
            baseline.artifact_reference().unwrap(),
            candidate.artifact_reference().unwrap(),
            "independent occurrences: {mutation}",
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
