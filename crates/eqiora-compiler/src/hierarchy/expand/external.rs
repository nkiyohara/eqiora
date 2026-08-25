//! External Geometry root allocation kept outside the expansion ceiling.

use super::*;
use crate::external::ExternalGeometrySupportBinding;

impl<'a, 'd> RootExpansion<'a, 'd> {
    pub(in crate::hierarchy) fn expand_external(
        mut self,
        component: ComponentDefinition<'d>,
        supports: &[ExternalGeometrySupportBinding],
    ) -> Result<ExpandedBlueprint, Vec<Diagnostic>> {
        let model = self.model.clone();
        let mut root_scope = Scope::external_root();
        root_scope.set_pure_operators(self.elaborator.visible_pure_operators(&model.namespace));
        let identities = self
            .allocate_model_scope(&mut root_scope)
            .map_err(one_diagnostic)?;
        self.allocate_external_supports(&mut root_scope, supports)
            .map_err(one_diagnostic)?;
        let instance = model
            .items()
            .iter()
            .find_map(|item| match item {
                Item::Instance(instance) => Some(instance),
                _ => None,
            })
            .ok_or_else(|| vec![hierarchy_error("external root has no Component occurrence")])?;
        let instance_path = child_instance_path(
            &self.root_path,
            instance.name(),
            self.elaborator.limits.identity,
        )
        .map_err(one_diagnostic)?;
        self.expand_component(
            component,
            instance,
            model.file,
            instance_path,
            instance.name().to_owned(),
            &root_scope,
        )?;
        self.materialize_model_items(&root_scope, &identities)
            .map_err(one_diagnostic)?;
        self.finalize_physical_connections()
            .map_err(one_diagnostic)?;
        self.items.sort_by_key(FlatItemBlueprint::sort_key);
        Ok(ExpandedBlueprint::new(
            self.model.name().to_owned(),
            SourceLocation::new(self.model.file, self.model.range()),
            self.model_key,
            self.model_full,
            self.items,
            self.display_symbols,
            self.physical_exposures,
        ))
    }

    fn allocate_external_supports(
        &mut self,
        scope: &mut Scope,
        supports: &[ExternalGeometrySupportBinding],
    ) -> Result<(), Diagnostic> {
        for support in supports {
            let ExternalGeometrySupportBinding::Region {
                slot,
                geometry,
                entity_set,
                ambient_dimension,
            } = support
            else {
                continue;
            };
            let identity = self.external_support_identity(slot)?;
            let internal_name = internal_name(identity.full);
            self.register_symbol(slot.clone(), slot, &identity, SymbolKind::Domain, scope)?;
            scope.insert_spatial_support(
                slot.clone(),
                SpatialSupport::Volume {
                    domain: identity.full,
                    dimensions: *ambient_dimension,
                },
            );
            self.items.push(FlatItemBlueprint::Domain {
                name: internal_name,
                contract: LoweringDomainContract::ExternalGeometryRegion {
                    geometry: *geometry,
                    entity_set: entity_set.clone(),
                    dimensions: *ambient_dimension,
                },
                range: self.model.range(),
                identity,
            });
        }
        for support in supports {
            let ExternalGeometrySupportBinding::Boundary {
                slot,
                entity_set,
                parent_slot,
                ..
            } = support
            else {
                continue;
            };
            let Some(SpatialSupport::Volume {
                domain: parent,
                dimensions,
            }) = scope.spatial_support(parent_slot).cloned()
            else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.model.file,
                    self.model.range(),
                    format!(
                        "external boundary support `{slot}` has no volume parent `{parent_slot}`"
                    ),
                ));
            };
            let parent_name = scope
                .symbol(parent_slot)
                .ok_or_else(|| hierarchy_error("external volume support has no symbol"))?
                .internal_name
                .clone();
            let identity = self.external_support_identity(slot)?;
            let internal_name = internal_name(identity.full);
            self.register_symbol(slot.clone(), slot, &identity, SymbolKind::Domain, scope)?;
            scope.insert_spatial_support(
                slot.clone(),
                SpatialSupport::Boundary {
                    domain: identity.full,
                    parent,
                    dimensions,
                },
            );
            self.boundary_parents.insert(identity.full, parent);
            self.items.push(FlatItemBlueprint::Domain {
                name: internal_name,
                contract: LoweringDomainContract::ExternalGeometryBoundary {
                    entity_set: entity_set.clone(),
                    parent: parent_name,
                },
                range: self.model.range(),
                identity,
            });
        }
        Ok(())
    }

    fn external_support_identity(&self, slot: &str) -> Result<EntityIdentity, Diagnostic> {
        self.entity_identity(
            &self.root_path,
            definition_path(&self.model.namespace, "model", self.model.name(), slot),
            EntityKind::Domain,
            SourceLocation::new(self.model.file, self.model.range()),
            SourceLocation::new(self.model.file, self.model.range()),
            Vec::new(),
        )
    }
}
