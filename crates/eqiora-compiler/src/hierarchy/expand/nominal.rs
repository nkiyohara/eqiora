//! Nominal declaration nodes and structural dependencies in each occurrence.
use super::*;

impl<'a, 'd> RootExpansion<'a, 'd> {
    pub(super) fn allocate_finite_spaces(&mut self) -> Result<(), Diagnostic> {
        for (namespace, spaces) in &self.elaborator.finite_spaces {
            for (name, space) in spaces {
                let full = space.key.full_identity()?;
                let identity = EntityIdentity {
                    key: space.key.clone(), full,
                    definition: SourceLocation::new(self.model.file, space.range),
                    instance: SourceLocation::new(self.model.file, self.model.range()),
                    bindings: Vec::new(),
                };
                let display = if matches!(namespace, DefinitionNamespace::Local) { name.clone() } else { format!("{namespace}.{name}") };
                self.display_symbols.insert(display, DisplayIdentity { full, kind: EntityKind::FiniteSpace });
                self.items.push(FlatItemBlueprint::Nominal {
                    name: internal_name(full), definition: space.definition.clone().into(), dependencies: Vec::new(), range: space.range, identity,
                });
            }
        }
        Ok(())
    }

    pub(super) fn allocate_nominals(
        &mut self,
        scope: &mut Scope,
        namespace: &DefinitionNamespace,
        definition_name: &str,
        instance_path: &InstancePath,
        declarations: &[&eqiora_lang::NamedDefinitionDecl],
        file: &str,
    ) -> Result<(), Diagnostic> {
        for declaration in declarations {
            let invalid = |message: &str| source_error(codes::LANGUAGE_TYPE_ERROR, file, declaration.range(), message);
            let eqiora_lang::ExprKind::Call { callee, arguments } = declaration.value().kind() else { return Err(invalid("index set requires range(extent)")); };
            let [extent] = arguments.as_slice() else { return Err(invalid("range requires exactly one positive integer extent")); };
            if callee.as_str() != "range" || declaration.value_type().is_some() || declaration.domain().is_some() || declaration.activation().is_some() { return Err(invalid("index set requires only range(extent), without alias assertions")); }
            let (extent, dependencies) = super::super::parameters::structural_extent(file, extent, &scope.symbolic_parameters())?
                .ok_or_else(|| invalid("index set extent remains unresolved at this occurrence"))?;
            let identity = self.entity_identity(instance_path, definition_path(namespace, "indexset", definition_name, declaration.name()), EntityKind::IndexSet, SourceLocation::new(file, declaration.range()), SourceLocation::new(file, declaration.range()), Vec::new())?;
            let mut staging = crate::identity::StagingIdAllocator::new();
            let full = staging.stage(&identity.key)?;
            let id = staging.finish().resolve::<eqiora_core::entity::kinds::IndexSet>(full)?.id();
            let definition = eqiora_schema::kernel::IndexSetDef::new(id, extent).map_err(|error| invalid(error.message()))?;
            scope.insert_index_set(declaration.name().to_owned(), definition.clone())?;
            let display = if instance_path == &self.root_path { declaration.name().to_owned() } else { format!("{}.{}", instance_path.segments()[1..].join("."), declaration.name()) };
            self.display_symbols.insert(display, DisplayIdentity { full, kind: EntityKind::IndexSet });
            self.items.push(FlatItemBlueprint::Nominal { name: internal_name(full), definition: definition.into(), dependencies, range: declaration.range(), identity });
        }
        Ok(())
    }
}
