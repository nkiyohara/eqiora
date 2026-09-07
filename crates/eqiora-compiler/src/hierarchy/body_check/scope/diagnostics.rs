//! Source-local failures for unresolved or mismatched scoped names.

use super::*;

impl DefinitionScope<'_, '_> {
    pub(in crate::hierarchy::body_check) fn wrong_local_kind(
        &self,
        range: TextRange,
        name: &str,
        expected: &str,
    ) -> Diagnostic {
        if self.symbols.contains_key(name) || self.children.contains_key(name) {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                range,
                format!("`{name}` is not a {expected}"),
            )
        } else {
            unresolved(self.file, range, name, expected)
        }
    }
}

pub(in crate::hierarchy::body_check) fn unresolved(
    file: &str,
    range: TextRange,
    name: &str,
    expected: &str,
) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        format!("unresolved {expected} `{name}`"),
    )
}
