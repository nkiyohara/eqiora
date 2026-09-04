use std::cell::Cell;

use eqiora_api::editor::{
    EditorPosition, EditorService, EditorSymbolKind, EditorWorkspaceSnapshot,
};
use eqiora_compiler::{CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit};
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

#[test]
fn workspace_cancellation_publishes_no_partial_snapshot() {
    let owner = CompilationNamespaceId::new(["editor-cancel"]).expect("namespace");
    let polls = Cell::new(0_u8);
    let cancelled = EditorWorkspaceSnapshot::analyze_modules_with_cancellation(
        9,
        ResolvedHierarchyInput::new(
            owner.clone(),
            vec![
                ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", "model Main {}"),
                ResolvedSourceUnit::in_module(
                    owner,
                    ["broken"],
                    "src/broken.eqi",
                    "not valid source",
                )
                .expect("module input"),
            ],
            vec![],
        ),
        || {
            let next = polls.get() + 1;
            polls.set(next);
            next == 6
        },
    )
    .expect("cancellation is not a diagnostic");
    assert!(cancelled.is_none());
    assert_eq!(polls.get(), 6);
}

#[test]
fn workspace_uses_compiler_resolved_module_identities_and_locations() {
    let owner = CompilationNamespaceId::new(["editor-test"]).expect("namespace");
    let main = "import library.parts as lib;\nmodel Main { instance load: lib.Resistor(); }\n";
    let library = r#"module library.parts;
public connector Pin = scalar_physical(across = 1, through = A);
public component Socket { public port terminal: conserving on Pin; }
public component Resistor {}
"#;
    let input = ResolvedHierarchyInput::new(
        owner.clone(),
        vec![
            ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", main),
            ResolvedSourceUnit::new(owner, "src/library.eqi", library),
        ],
        vec![],
    );

    let workspace =
        EditorWorkspaceSnapshot::analyze_modules(11, input).expect("valid resolved workspace");
    assert_eq!(workspace.version(), 11);
    assert_eq!(workspace.files().len(), 2);
    assert_eq!(workspace.definitions().len(), 4);

    let resistor = workspace
        .definitions()
        .iter()
        .find(|definition| definition.path() == "library.parts.Resistor")
        .expect("resolved component definition");
    assert_eq!(resistor.namespace(), &["editor-test"]);
    assert_eq!(resistor.kind(), EditorSymbolKind::Component);
    assert!(resistor.file().ends_with(":src/library.eqi"));
    let document = workspace
        .document(resistor.file())
        .expect("definition source document");
    assert_eq!(
        &library[usize::try_from(resistor.range().start()).unwrap()
            ..usize::try_from(resistor.range().end()).unwrap()],
        "public component Resistor {}"
    );
    assert!(
        document
            .symbols()
            .iter()
            .any(|symbol| symbol.name() == "Resistor")
    );

    let reference_start = u32::try_from(main.find("lib.Resistor").unwrap()).unwrap();
    let reference = workspace
        .references()
        .iter()
        .find(|reference| reference.range().start() == reference_start)
        .expect("resolved component reference");
    assert!(reference.file().ends_with(":src/main.eqi"));
    assert_eq!(reference.definition(), resistor);
    let (hovered, detail) = workspace
        .hover(reference.file(), reference_start + 4)
        .expect("reference hover");
    assert_eq!(hovered, resistor);
    assert_eq!(detail, "public component Resistor {}");
    assert_eq!(
        workspace
            .definition_for_reference(reference.file(), reference_start + 4)
            .expect("go-to-definition target"),
        resistor
    );
    assert!(
        workspace
            .definition_for_reference(reference.file(), reference.range().end())
            .is_none()
    );
    let definition_name = u32::try_from(library.find("Resistor").unwrap()).unwrap();
    let (hovered, detail) = workspace
        .hover(resistor.file(), definition_name + 2)
        .expect("definition hover");
    assert_eq!(hovered, resistor);
    assert_eq!(detail, "public component Resistor {}");
    assert!(
        workspace
            .hover(resistor.file(), resistor.range().start())
            .is_none()
    );

    let pin_reference = workspace
        .references()
        .iter()
        .find(|reference| reference.definition().path() == "library.parts.Pin")
        .expect("resolved Connector reference");
    assert_eq!(
        &library[usize::try_from(pin_reference.range().start()).unwrap()
            ..usize::try_from(pin_reference.range().end()).unwrap()],
        "Pin"
    );
    assert_eq!(
        pin_reference.definition().kind(),
        EditorSymbolKind::Connector
    );
}
