use std::cell::Cell;

use eqiora_api::editor::{
    EditorPosition, EditorService, EditorSymbolKind, EditorWorkspaceService,
    EditorWorkspaceSnapshot,
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
    );
    assert!(cancelled.is_none());
    assert_eq!(polls.get(), 6);
}

#[test]
fn workspace_uses_compiler_resolved_module_identities_and_locations() {
    let owner = CompilationNamespaceId::new(["editor-test"]).expect("namespace");
    let main =
        "// 🧪\nimport library.parts as lib;\nmodel Main { instance load: lib.Resistor(); }\n";
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

    let workspace = EditorWorkspaceSnapshot::analyze_modules(11, input.clone());
    assert!(workspace.diagnostics().is_empty());
    assert_eq!(workspace.version(), 11);
    let mut service = EditorWorkspaceService::new(workspace.clone());
    assert_eq!(service.snapshot(11).expect("current workspace"), &workspace);
    assert_eq!(
        service
            .snapshot(10)
            .expect_err("older workspace request is stale")
            .code(),
        codes::PRECONDITION_FAILED
    );
    assert_eq!(
        service
            .snapshot(12)
            .expect_err("future workspace request is unknown")
            .code(),
        codes::PRECONDITION_FAILED
    );
    assert_eq!(
        service
            .replace(workspace.clone())
            .expect_err("published version cannot be replaced")
            .code(),
        codes::PRECONDITION_FAILED
    );
    service.begin(12).expect("newer workspace request begins");
    assert!(service.current().is_none());
    assert!(service.snapshot(11).is_err());
    assert!(service.snapshot(12).is_err());
    let stale = EditorWorkspaceSnapshot::analyze_modules(10, input.clone());
    assert_eq!(
        service
            .replace(stale)
            .expect_err("completed stale analysis cannot publish")
            .code(),
        codes::PRECONDITION_FAILED
    );
    assert!(service.current().is_none());
    let newer = EditorWorkspaceSnapshot::analyze_modules(12, input.clone());
    assert_eq!(
        service
            .replace(newer)
            .expect("newer analysis publishes")
            .version(),
        12
    );
    assert_eq!(
        service.current().map(EditorWorkspaceSnapshot::version),
        Some(12)
    );
    service.begin(13).expect("third workspace request begins");
    service
        .begin(14)
        .expect("newest workspace request supersedes it");
    let completed_stale = EditorWorkspaceSnapshot::analyze_modules(13, input.clone());
    assert_eq!(
        service
            .replace(completed_stale)
            .expect_err("superseded analysis cannot publish")
            .code(),
        codes::PRECONDITION_FAILED
    );
    assert!(service.current().is_none());
    let latest = EditorWorkspaceSnapshot::analyze_modules(14, input);
    assert_eq!(
        service
            .replace(latest)
            .expect("current analysis publishes")
            .version(),
        14
    );
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
            .definition_for_reference_at_position(reference.file(), EditorPosition::new(2, 32),)
            .expect("UTF-16 go-to-definition target"),
        resistor
    );
    let (position_hovered, position_detail) = workspace
        .hover_at_position(reference.file(), EditorPosition::new(2, 32))
        .expect("UTF-16 reference hover");
    assert_eq!(position_hovered, resistor);
    assert_eq!(position_detail, "public component Resistor {}");
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
    let definition_name_range = resistor.name_range().expect("definition name range");
    assert_eq!(
        &library[usize::try_from(definition_name_range.start()).unwrap()
            ..usize::try_from(definition_name_range.end()).unwrap()],
        "Resistor"
    );
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

#[test]
fn invalid_workspace_retains_recovered_documents_and_diagnostics() {
    let owner = CompilationNamespaceId::new(["editor-recovery"]).expect("namespace");
    let main = "import library.parts as lib;\nmodel Main { instance load: lib.Resistor(); }\n";
    let broken = "module library.parts;\npublic component Resistor { nonsense; }\n";
    let input = ResolvedHierarchyInput::new(
        owner.clone(),
        vec![
            ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", main),
            ResolvedSourceUnit::new(owner, "src/library.eqi", broken),
        ],
        vec![],
    );
    let workspace = EditorWorkspaceSnapshot::analyze_modules(12, input.clone());
    assert_eq!(
        workspace,
        EditorWorkspaceSnapshot::analyze_modules(12, input)
    );

    assert_eq!(workspace.version(), 12);
    assert_eq!(workspace.files().len(), 2);
    assert!(!workspace.diagnostics().is_empty());
    assert!(workspace.definitions().is_empty());
    assert!(workspace.references().is_empty());

    let main_file = workspace
        .files()
        .find(|file| file.ends_with(":src/main.eqi"))
        .expect("root file")
        .to_owned();
    let broken_file = workspace
        .files()
        .find(|file| file.ends_with(":src/library.eqi"))
        .expect("broken module file")
        .to_owned();
    let main_document = workspace.document(&main_file).expect("root document");
    assert!(main_document.formatted().is_some());
    assert_eq!(main_document.symbols()[1].name(), "Main");

    let broken_document = workspace.document(&broken_file).expect("broken document");
    assert!(!broken_document.diagnostics().is_empty());
    assert!(broken_document.formatted().is_none());
    assert!(
        broken_document
            .symbols()
            .iter()
            .any(|symbol| symbol.name() == "Resistor")
    );
    assert!(workspace.diagnostics().iter().all(|diagnostic| {
        diagnostic
            .source_span()
            .is_none_or(|span| workspace.document(&span.file).is_some())
    }));
}
