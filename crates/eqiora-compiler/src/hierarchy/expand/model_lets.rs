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
        crate::hierarchy::parameters::resolve_model_lets(
            self.model.file,
            model,
            &mut values,
            |name| crate::hierarchy::clocks::occurrence(scope, name),
        )
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
            let Some(value) = values.remove(declaration.name()) else {
                continue;
            };
            scope
                .insert_let(declaration.name().to_owned(), value)
                .map_err(hierarchy_error)?;
        }
        Ok(())
    }
}

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_component_lets(
        &self,
        scope: &mut Scope,
        component: &super::ComponentDefinition<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut values = scope.symbolic_parameters();
        crate::hierarchy::parameters::resolve_component_lets(
            component.file,
            component.declaration,
            &mut values,
            |name| crate::hierarchy::clocks::occurrence(scope, name),
        )?;
        for item in component.items() {
            let eqiora_lang::ComponentItem::Let(declaration) = item else {
                continue;
            };
            let Some(value) = values.remove(declaration.name()) else {
                continue;
            };
            scope
                .insert_let(declaration.name().to_owned(), value)
                .map_err(|message| vec![hierarchy_error(message)])?;
        }
        Ok(())
    }
}

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_runtime_lets<'d>(
        &self,
        scope: &mut Scope,
        file: &str,
        declarations: impl Iterator<Item = &'d eqiora_lang::NamedDefinitionDecl>,
    ) -> Result<(), Vec<Diagnostic>> {
        let order = crate::hierarchy::parameters::alias_order(file, declarations)?;
        for declaration in order {
            if scope.parameter(declaration.name()).is_some() {
                continue;
            }
            let activation = scope.alias_activation(declaration.value());
            if let Some(clock) = declaration.activation() {
                let exact = scope.symbol(clock).map(|symbol| &symbol.internal_name);
                if !matches!((&activation, exact), (crate::hierarchy::body_check::DependencyActivation::Clock(actual), Some(expected)) if actual == expected)
                {
                    return Err(vec![crate::diagnostics::source_error(
                        eqiora_core::diagnostic::codes::LANGUAGE_TYPE_ERROR,
                        file,
                        declaration.range(),
                        "let alias activation assertion does not match its exact occurrence dependency clock",
                    )]);
                }
            }
            let expression = crate::hierarchy::scope::rewrite_expression_with_boundary_member(
                file,
                declaration.value(),
                scope,
                None,
            )
            .map_err(|error| vec![error])?;
            scope
                .insert_runtime_let(declaration.name().to_owned(), expression, activation)
                .map_err(|message| vec![hierarchy_error(message)])?;
        }
        Ok(())
    }
}
