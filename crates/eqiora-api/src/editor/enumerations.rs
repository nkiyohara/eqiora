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
