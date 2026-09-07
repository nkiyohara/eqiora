//! Resolve nominal property declarations through the exact module import scope.

use super::*;

pub(super) fn resolve_path<T>(
    namespace: &CompilationModuleId,
    path: &NamePath,
    aliases: &[ResolvedAlias],
    values: &BTreeMap<Key, T>,
    visibility: impl Fn(&T) -> VisibilitySyntax,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Key> {
    let segments = path.segments().collect::<Vec<_>>();
    let key = if segments.len() == 1 {
        (namespace.clone(), segments[0].to_owned())
    } else if segments.len() == 2 {
        let Some(alias) = aliases
            .iter()
            .find(|value| value.declaring_module() == namespace && value.alias() == segments[0])
        else {
            diagnostics.push(error(
                file,
                path.range(),
                format!("unknown import alias `{}`", segments[0]),
            ));
            return None;
        };
        (alias.target_module().clone(), segments[1].to_owned())
    } else {
        diagnostics.push(error(
            file,
            path.range(),
            "property paths support one optional import alias",
        ));
        return None;
    };
    let Some(value) = values.get(&key) else {
        diagnostics.push(error(
            file,
            path.range(),
            format!("unresolved property declaration `{path}`"),
        ));
        return None;
    };
    if &key.0 != namespace && visibility(value) != VisibilitySyntax::Public {
        diagnostics.push(error(
            file,
            path.range(),
            format!("property declaration `{path}` is private"),
        ));
        return None;
    }
    Some(key)
}
