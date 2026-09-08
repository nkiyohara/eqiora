//! Exact native nominal declaration lookup in the existing draft scope.

use super::*;

impl ModelDraft {
    pub(super) fn nominal_name(&self, id: eqiora_core::RawId) -> Option<NamePath> {
        self.declarations
            .iter()
            .find_map(|declaration| match declaration {
                DraftDeclaration::FiniteSpace { name, definition }
                    if definition.id().erase() == id =>
                {
                    Some(NamePath::single(name.clone(), TextRange::new(0, 0)))
                }
                DraftDeclaration::IndexSet { name, definition }
                    if definition.id().erase() == id =>
                {
                    Some(NamePath::single(name.clone(), TextRange::new(0, 0)))
                }
                _ => None,
            })
    }
}
