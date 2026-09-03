use eqiora_api::editor::{EditorPosition, EditorService, EditorSymbolKind};
use eqiora_core::diagnostic::codes;

#[test]
fn snapshot_combines_recovery_formatting_symbols_and_semantic_diagnostics() {
    let source = r#"// authored note
module local.main;
dimension Scalar = 1;
component Source {
  public parameter gain: Scalar;
  relation law continuous { gain = 0; }
}
model Demo {
  parameter input: Scalar = 1;
  field state: Scalar = 0;
  instance source: Source(gain = input);
  relation balance continuous { state = 0; }
}
"#;
    let service = EditorService::new("demo.eqi", 7, source);
    let repeated = EditorService::new("demo.eqi", 7, source);
    let snapshot = service.snapshot(7).expect("current snapshot");

    assert_eq!(snapshot, repeated.current());
    assert!(snapshot.diagnostics().is_empty());
    assert_eq!(snapshot.formatted(), Some(source));
    assert_eq!(
        snapshot
            .symbols()
            .iter()
            .map(|symbol| (symbol.kind(), symbol.name()))
            .collect::<Vec<_>>(),
        vec![
            (EditorSymbolKind::Module, "local.main"),
            (EditorSymbolKind::Dimension, "Scalar"),
            (EditorSymbolKind::Component, "Source"),
            (EditorSymbolKind::Model, "Demo"),
        ]
    );
    assert_eq!(
        snapshot.symbols()[2]
            .children()
            .iter()
            .map(|symbol| (symbol.kind(), symbol.name()))
            .collect::<Vec<_>>(),
        vec![
            (EditorSymbolKind::Parameter, "gain"),
            (EditorSymbolKind::Relation, "law"),
        ]
    );
    assert_eq!(
        snapshot.symbols()[3]
            .children()
            .iter()
            .map(|symbol| (symbol.kind(), symbol.name()))
            .collect::<Vec<_>>(),
        vec![
            (EditorSymbolKind::Parameter, "input"),
            (EditorSymbolKind::Field, "state"),
            (EditorSymbolKind::Instance, "source"),
            (EditorSymbolKind::Relation, "balance"),
        ]
    );

    let invalid = EditorService::new(
        "invalid.eqi",
        1,
        "model M { field x: m = 0; relation r continuous { x + 1 = 0; } }",
    );
    assert!(
        invalid
            .current()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == codes::LANGUAGE_TYPE_ERROR)
    );
    assert!(invalid.current().formatted().is_some());

    let recovering = EditorService::new(
        "broken.eqi",
        1,
        "model M { field retained: 1 = 0; nonsense; }",
    );
    assert!(!recovering.current().diagnostics().is_empty());
    assert!(recovering.current().formatted().is_none());
    assert_eq!(recovering.current().symbols()[0].name(), "M");
}

#[test]
fn positions_round_trip_utf8_utf16_and_line_endings() {
    let source = "// 🧪\r\nmodel M {}\n";
    let service = EditorService::new("unicode.eqi", 3, source);
    let snapshot = service.current();

    assert_eq!(snapshot.position(7), Some(EditorPosition::new(0, 5)));
    assert_eq!(snapshot.byte_offset(EditorPosition::new(0, 5)), Some(7));
    assert_eq!(snapshot.byte_offset(EditorPosition::new(0, 4)), None);
    assert_eq!(snapshot.position(9), Some(EditorPosition::new(1, 0)));
    assert_eq!(snapshot.position(8), None);
    assert_eq!(snapshot.byte_offset(EditorPosition::new(0, 6)), None);
    assert_eq!(snapshot.position(4), None);
}

#[test]
fn service_rejects_stale_and_unknown_versions_without_mutation() {
    let mut service = EditorService::new("versioned.eqi", 4, "model Four {}");

    let stale = service
        .replace(4, "model Replaced {}")
        .expect_err("equal version is stale");
    assert_eq!(stale.code(), codes::PRECONDITION_FAILED);
    assert_eq!(service.current().version(), 4);
    assert_eq!(service.current().symbols()[0].name(), "Four");

    service
        .replace(5, "model Five {}")
        .expect("newer version is accepted");
    assert_eq!(service.current().version(), 5);
    assert!(service.snapshot(4).is_err());
    assert!(service.snapshot(6).is_err());
    assert_eq!(service.snapshot(5).unwrap().symbols()[0].name(), "Five");
}
