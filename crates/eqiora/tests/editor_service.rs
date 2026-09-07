use eqiora::api::{EditorPosition, EditorService, EditorSymbolKind};

#[test]
fn public_facade_analyzes_one_versioned_source_snapshot() {
    let source = "// μ\nmodel Demo { field state: 1 = 0; relation balance { state = 0; } }\n";
    let service = EditorService::new("demo.eqi", 11, source);
    let snapshot = service.snapshot(11).expect("current source version");

    assert!(snapshot.diagnostics().is_empty());
    let formatted = concat!(
        "// μ\n",
        "model Demo {\n",
        "  field state: 1 = 0;\n",
        "  relation balance {\n",
        "    state = 0;\n",
        "  }\n",
        "}\n",
    );
    assert_eq!(snapshot.formatted(), Some(formatted));
    assert_eq!(snapshot.symbols()[0].kind(), EditorSymbolKind::Model);
    assert_eq!(snapshot.symbols()[0].name(), "Demo");
    assert_eq!(snapshot.symbols()[0].children().len(), 2);
    assert_eq!(snapshot.position(5), Some(EditorPosition::new(0, 4)));
    assert_eq!(snapshot.byte_offset(EditorPosition::new(1, 0)), Some(6));
}
