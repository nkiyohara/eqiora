//! Ordinary Relation emission from exact ordered IndexSet members.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn add_indexed_relations(
        &mut self,
        family: &eqiora_lang::RelationFamilyDecl,
        scope: &Scope,
        occurrence: (&InstancePath, &str),
        declaration_path: Vec<String>,
        origin: EntitySourceOrigin,
    ) -> Result<(), Diagnostic> {
        let set = scope
            .index_set(family.binder().set().as_str())
            .ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    &origin.definition.file,
                    family.range(),
                    "Relation family requires a resolved exact IndexSet",
                )
            })?;
        let file = &origin.definition.file;
        for ordinal in 0..set.extent() {
            let member = scope.with_index_member(family.binder().member(), set, ordinal)?;
            let mut path = declaration_path.clone();
            // Additional structured path segments cannot collide with an ordinary
            // declaration's path, even when its display name resembles a member.
            path.extend([
                "index_member".to_owned(),
                set.id().to_string(),
                ordinal.to_string(),
            ]);
            let identity = self.relation_identity(
                occurrence.0,
                path,
                origin.definition.clone(),
                origin.instance.clone(),
                origin.bindings.clone(),
            )?;
            self.register_family_relation_display(
                display_child(
                    occurrence.1,
                    &format!("{}[{ordinal}]", family.relation().name()),
                ),
                &identity,
            )?;
            let (activation, domain, equations) =
                rewrite_relation(file, family.relation(), &member)?;
            self.record_physical_relation_owners(
                file,
                family.range(),
                identity.entity.full,
                &equations,
            )?;
            self.items.push(FlatItemBlueprint::Relation {
                initial: false,
                name: internal_name(identity.entity.full),
                activation,
                domain,
                equations,
                range: family.range(),
                identity,
            });
        }
        Ok(())
    }
}
