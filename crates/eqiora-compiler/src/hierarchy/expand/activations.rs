//! Allocate occurrence-owned local clock and event activation declarations.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_local_activation(
        &mut self,
        item: &ComponentItem,
        occurrence: &ComponentOccurrence<'_, '_>,
        bindings: &[SourceLocation],
        scope: &mut Scope,
        identities: &mut ScopeIdentities,
    ) -> Result<(), Vec<Diagnostic>> {
        let component = occurrence.definition;
        let instance = occurrence.instance;
        let instance_file = occurrence.instance_file;
        let instance_path = occurrence.instance_path;
        let display_prefix = occurrence.display_prefix;
        match item {
            ComponentItem::Event(declaration) => {
                let identity = self
                    .entity_identity(
                        instance_path,
                        definition_path(
                            &component.namespace,
                            "component",
                            component.name(),
                            declaration.name(),
                        ),
                        EntityKind::Activation,
                        SourceLocation::new(component.file, declaration.range()),
                        SourceLocation::new(instance_file, instance.range()),
                        bindings.to_vec(),
                    )
                    .map_err(one_diagnostic)?;
                self.register_symbol(
                    display_child(display_prefix, declaration.name()),
                    declaration.name(),
                    &identity,
                    SymbolKind::Event,
                    scope,
                )
                .map_err(one_diagnostic)?;
                identities
                    .entities
                    .insert(declaration.name().to_owned(), identity);
            }
            ComponentItem::Clock(declaration) => {
                let identity = self
                    .entity_identity(
                        instance_path,
                        definition_path(
                            &component.namespace,
                            "component",
                            component.name(),
                            declaration.name(),
                        ),
                        EntityKind::ClockDomain,
                        SourceLocation::new(component.file, declaration.range()),
                        SourceLocation::new(instance_file, instance.range()),
                        bindings.to_vec(),
                    )
                    .map_err(one_diagnostic)?;
                self.register_symbol(
                    display_child(display_prefix, declaration.name()),
                    declaration.name(),
                    &identity,
                    SymbolKind::Clock(
                        crate::units::lower_clock(
                            component.file,
                            declaration.period(),
                            declaration.phase(),
                        )
                        .map_err(one_diagnostic)?
                        .0,
                    ),
                    scope,
                )
                .map_err(one_diagnostic)?;
                identities
                    .entities
                    .insert(declaration.name().to_owned(), identity);
            }
            _ => unreachable!("caller selects local clock or event declarations"),
        }
        Ok(())
    }
}
