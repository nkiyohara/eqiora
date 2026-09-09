//! Resolve exact scalar symbols and retained record member paths.
use super::*;

impl DefinitionScope<'_, '_> {
    pub(in crate::hierarchy::body_check) fn resolve_symbol(
        &self,
        path: &NamePath,
    ) -> Result<SymbolContract, Diagnostic> {
        self.resolve_symbol_at(path, None)
    }

    pub(in crate::hierarchy::body_check) fn resolve_symbol_at(
        &self,
        path: &NamePath,
        ordinal: Option<u32>,
    ) -> Result<SymbolContract, Diagnostic> {
        if let Some(symbol) = self.symbols.get(path.as_str()) {
            return Ok(symbol.clone());
        }
        let segments = path.segments().collect::<Vec<_>>();
        if let Some(symbol) = self.symbols.get(path.as_str()) {
            return Ok(symbol.clone());
        }
        match segments.as_slice() {
            [name] => self
                .symbols
                .get(*name)
                .cloned()
                .ok_or_else(|| unresolved(self.file, path.range(), name, "expression symbol")),
            [instance, member] => {
                let Some(child) = self.children.get(*instance) else {
                    return Err(self.invalid_public_port_selection(path));
                };
                let port = child
                    .owned_items()
                    .find_map(|item| match item {
                        ComponentItem::Port(port)
                            if port.name() == *member
                                && port.visibility() == VisibilitySyntax::Public =>
                        {
                            Some(port)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| self.invalid_public_port_selection(path))?;
                let occurrence = self
                    .child_instances
                    .get(*instance)
                    .expect("bound child instance");
                let specialized;
                let occurrence = if let Some(ordinal) = ordinal {
                    specialized = crate::hierarchy::reductions::instantiate_instance(
                        self.file, occurrence, ordinal,
                    )?;
                    &specialized
                } else {
                    occurrence
                };
                let values =
                    crate::hierarchy::parameters::resolve_instance_parameters_symbolically(
                        (child.file, self.file),
                        child.declaration,
                        occurrence,
                        &self.static_values,
                        &mut |name| {
                            crate::hierarchy::clocks::component(child.file, child.declaration, name)
                        },
                        &mut |name| self.spatial_support(name),
                        (
                            &crate::hierarchy::parameters::RecordContext::component(
                                self.elaborator,
                                child,
                            ),
                            &self.record_context,
                        ),
                    )
                    .map_err(|errors| {
                        errors
                            .into_iter()
                            .next()
                            .expect("failed parameter admission")
                    })?;
                component_port_contract(self.elaborator, child, port, &values)
                    .map(|contract| {
                        SymbolContract::Port(self.specialize_child_port(instance, contract))
                    })
                    .map_err(|mut errors| {
                        errors.pop().unwrap_or_else(|| {
                            source_error(
                                codes::LANGUAGE_LOWERING_ERROR,
                                self.file,
                                path.range(),
                                "child Port contract validation failed without a diagnostic",
                            )
                        })
                    })
            }
            _ => Err(self.invalid_public_port_selection(path)),
        }
    }
}
