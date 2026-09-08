//! Exact entity and relation identity construction for occurrence expansion.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn entity_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: impl IntoIterator<Item = String>,
        kind: EntityKind,
        definition: SourceLocation,
        instance: SourceLocation,
        bindings: Vec<SourceLocation>,
    ) -> Result<EntityIdentity, Diagnostic> {
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let key = ElaborationKey::entity_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            kind,
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        Ok(EntityIdentity {
            key,
            full,
            definition,
            instance,
            bindings,
        })
    }

    pub(super) fn relation_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        definition: SourceLocation,
        instance: SourceLocation,
        bindings: Vec<SourceLocation>,
    ) -> Result<RelationIdentity, Diagnostic> {
        let entity = self.entity_identity(
            instance_path,
            declaration_path.clone(),
            EntityKind::Relation,
            definition,
            instance,
            bindings,
        )?;
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let activation_key = ElaborationKey::generated_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            GeneratedRole::RelationActivation,
            self.elaborator.limits.identity,
        )?;
        let activation_full = activation_key.full_identity()?;
        Ok(RelationIdentity {
            entity,
            activation_key,
            activation_full,
        })
    }

    pub(super) fn boundary_family_entity_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        kind: EntityKind,
        boundary: FullElaborationIdentity,
        source: EntitySourceOrigin,
    ) -> Result<EntityIdentity, Diagnostic> {
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let key = ElaborationKey::boundary_family_entity_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            kind,
            boundary,
            self.elaborator.limits.identity,
        )?;
        let full = key.full_identity()?;
        Ok(EntityIdentity {
            key,
            full,
            definition: source.definition,
            instance: source.instance,
            bindings: source.bindings,
        })
    }

    pub(super) fn boundary_family_relation_identity(
        &self,
        instance_path: &InstancePath,
        declaration_path: Vec<String>,
        boundary: FullElaborationIdentity,
        source: EntitySourceOrigin,
    ) -> Result<RelationIdentity, Diagnostic> {
        let entity = self.boundary_family_entity_identity(
            instance_path,
            declaration_path.clone(),
            EntityKind::Relation,
            boundary,
            source,
        )?;
        let declaration_path =
            DeclarationPath::with_limits(declaration_path, self.elaborator.limits.identity)?;
        let activation_key = ElaborationKey::boundary_family_generated_with_limits(
            self.namespace.clone(),
            instance_path.clone(),
            declaration_path,
            GeneratedRole::RelationActivation,
            boundary,
            self.elaborator.limits.identity,
        )?;
        let activation_full = activation_key.full_identity()?;
        Ok(RelationIdentity {
            entity,
            activation_key,
            activation_full,
        })
    }
}
