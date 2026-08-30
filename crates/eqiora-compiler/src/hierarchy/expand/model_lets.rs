use eqiora_core::Diagnostic;
use eqiora_lang::{Item, ModelDecl};

use super::RootExpansion;
use crate::hierarchy::hierarchy_error;
use crate::hierarchy::scope::Scope;

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_model_lets(
        &self,
        scope: &mut Scope,
        model: &ModelDecl,
    ) -> Result<(), Diagnostic> {
        let mut values = scope.symbolic_parameters();
        crate::hierarchy::parameters::resolve_model_lets(self.model.file, model, &mut values)
            .map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| hierarchy_error("let alias resolution failed"))
            })?;
        for item in model.items() {
            let Item::Let(declaration) = item else {
                continue;
            };
            let value = values
                .remove(declaration.name())
                .ok_or_else(|| hierarchy_error("resolved let alias is missing"))?;
            scope
                .insert_let(declaration.name().to_owned(), value)
                .map_err(hierarchy_error)?;
        }
        Ok(())
    }
}
