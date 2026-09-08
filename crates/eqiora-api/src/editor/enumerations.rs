use super::{EditorService, EditorSymbolKind};

#[test]
fn enum_symbols_preserve_member_order_and_exact_ranges() {
    let source = "public enum Mode { Heating, Cooling, Fault }\n";
    let editor = EditorService::new("modes.eqi", 1, source);
    let snapshot = editor.current();
    assert!(
        snapshot.diagnostics().is_empty(),
        "{:?}",
        snapshot.diagnostics()
    );
    let declaration = &snapshot.symbols()[0];
    assert_eq!(declaration.kind(), EditorSymbolKind::Enum);
    assert_eq!(declaration.name(), "Mode");
    assert_eq!(
        declaration
            .children()
            .iter()
            .map(|member| {
                assert_eq!(member.kind(), EditorSymbolKind::EnumMember);
                let range = member.range();
                assert_eq!(
                    &source[range.start() as usize..range.end() as usize],
                    member.name()
                );
                member.name()
            })
            .collect::<Vec<_>>(),
        ["Heating", "Cooling", "Fault"]
    );
}

#[test]
fn imported_enum_visibility_and_case_use_resolved_workspace() {
    use super::EditorWorkspaceSnapshot;
    use eqiora_compiler::{CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit};

    let owner = CompilationNamespaceId::new(["enum_editor"]).unwrap();
    let source = "import enum_editor.modes as controls;\nmodel Main() { let command = case controls.Mode.Heating { controls.Mode.Heating => 1, controls.Mode.Cooling => -1 }; }\n";
    let analyze = |visibility| {
        EditorWorkspaceSnapshot::analyze_modules(
            1,
            ResolvedHierarchyInput::new(
                owner.clone(),
                vec![
                    ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", source).unwrap(),
                    ResolvedSourceUnit::new(
                        owner.clone(),
                        "src/modes.eqi",
                        format!("{visibility}enum Mode {{ Heating, Cooling }}\n"),
                    )
                    .unwrap(),
                ],
                vec![],
            ),
        )
    };
    let public = analyze("public ");
    assert!(
        public.diagnostics().is_empty(),
        "{:?}",
        public.diagnostics()
    );
    let modes = public
        .definitions()
        .iter()
        .filter(|definition| definition.kind() == EditorSymbolKind::Enum)
        .collect::<Vec<_>>();
    assert_eq!(modes.len(), 1);
    assert!(modes[0].path().ends_with("Mode"));
    let private = analyze("");
    assert!(
        !private.diagnostics().is_empty(),
        "private imported enum must be rejected"
    );
}
