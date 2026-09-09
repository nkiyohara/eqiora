//! Resolve occurrence Parameters only after exact support bindings are validated.
use super::*;

pub(super) fn resolve(
    component: &ComponentDefinition<'_>,
    instance: &InstanceDecl,
    instance_file: &str,
    instance_path: &InstancePath,
    parent_scope: &Scope,
) -> Result<BTreeMap<String, super::super::parameters::ResolvedParameter>, Vec<Diagnostic>> {
    ParameterResolver::new(
        component.file,
        instance_file,
        component,
        instance,
        |name| parent_scope.parameter(name).cloned(),
        |name| super::super::clocks::occurrence(parent_scope, name),
        |name| {
            parent_scope
                .spatial_support(name)
                .map(super::super::parameters::frames::occurrence)
        },
    )
    .and_then(|resolver| {
        resolver.resolve_all(|name| {
            super::super::clocks::component_occurrence(
                component.file,
                component.declaration,
                instance,
                parent_scope,
                name,
            )
        })
    })
    .map_err(|errors| contextualize_diagnostics(errors, instance_path))
}

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_parameter(
        &mut self,
        occurrence: ComponentOccurrence<'_, '_>,
        declaration: &eqiora_lang::ComponentParameterDecl,
        parameters: &BTreeMap<String, ResolvedParameter>,
        bindings: &[SourceLocation],
        scope: &mut Scope,
    ) -> Result<(), Vec<Diagnostic>> {
        let ComponentOccurrence {
            definition: component,
            instance,
            instance_file,
            instance_path,
            display_prefix,
        } = occurrence;
        let resolved = parameters[declaration.name()].clone();
        if let ParameterLineage::Parameter(full) = resolved.lineage {
            self.record_structural(
                &internal_name(full),
                resolved.expression.structural_parameters(),
            )
            .map_err(one_diagnostic)?;
        }
        let occurrence_identity = self
            .entity_identity(
                instance_path,
                definition_path(
                    &component.namespace,
                    "component",
                    component.name(),
                    declaration.name(),
                ),
                EntityKind::Parameter,
                SourceLocation::new(component.file, declaration.range()),
                SourceLocation::new(instance_file, instance.range()),
                bindings.to_vec(),
            )
            .map_err(one_diagnostic)?;
        self.record_notation(
            &display_child(display_prefix, declaration.name()),
            &occurrence_identity,
            &SymbolKind::Parameter,
        );
        let spec = self
            .notation_specs
            .last_mut()
            .expect("registered Parameter label");
        spec.graph_identity = if let ParameterLineage::Parameter(full) = resolved.lineage {
            Some(full)
        } else {
            None
        };
        if scope
            .insert_parameter(declaration.name().to_owned(), resolved.clone())
            .is_some()
        {
            return Err(vec![contextualize_diagnostic(
                hierarchy_error(format!(
                    "duplicate flattened Parameter term `{}`",
                    declaration.name()
                )),
                instance_path,
            )]);
        }
        if let ParameterLineage::Parameter(full) = resolved.lineage {
            let display = display_child(display_prefix, declaration.name());
            if self
                .display_symbols
                .insert(
                    display.clone(),
                    DisplayIdentity {
                        full,
                        kind: EntityKind::Parameter,
                    },
                )
                .is_some()
            {
                return Err(vec![contextualize_diagnostic(
                    hierarchy_error(format!("duplicate flattened display symbol `{display}`")),
                    instance_path,
                )]);
            }
        }
        Ok(())
    }
}
