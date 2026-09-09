//! Static product occurrences retain ordinary Parameter terms and their lineage.
use super::*;
use crate::lower::LoweringExpression;

impl RootExpansion<'_, '_> {
    pub(super) fn allocate_model_record_parameter(
        &mut self,
        scope: &mut Scope,
        declaration: &eqiora_lang::ParameterDecl,
        values: &BTreeMap<String, ResolvedParameter>,
    ) -> Result<bool, Diagnostic> {
        let Some(record) = scope
            .record_context
            .record_for_type(declaration.value_type())
            .cloned()
        else {
            return Ok(false);
        };
        let mut roots = Vec::new();
        let path = definition_path(
            &self.model.namespace,
            "record",
            self.model.name(),
            declaration.name(),
        );
        for (member, _) in record.definition.members() {
            let local = format!("{}.{member}", declaration.name());
            let mut member_path = path.clone();
            member_path.push(member.clone());
            let identity = self.entity_identity(
                &self.root_path,
                member_path,
                EntityKind::Parameter,
                SourceLocation::new(self.model.file, declaration.range()),
                SourceLocation::new(self.model.file, self.model.range()),
                Vec::new(),
            )?;
            let name = internal_name(identity.full);
            self.register_symbol(
                local.clone(),
                &local,
                &identity,
                SymbolKind::Parameter,
                scope,
            )?;
            let value = values[&local].value.clone();
            let resolved = ResolvedParameter::model_parameter(
                value.clone(),
                identity.full,
                name.clone(),
                declaration.range(),
            );
            roots.push(resolved.expression.clone());
            scope.insert_parameter(local, resolved);
            self.items.push(FlatItemBlueprint::Parameter {
                name,
                value,
                range: declaration.range(),
                identity,
            });
        }
        let model = self.model.clone();
        self.allocate_static_record_owner(
            declaration.name(),
            &record,
            roots,
            records::RecordFieldOccurrence {
                namespace: &model.namespace,
                definition_name: model.name(),
                instance_path: &self.root_path.clone(),
                display_prefix: "",
                file: self.model.file,
                instance: SourceLocation::new(self.model.file, self.model.range()),
                bindings: Vec::new(),
            },
            declaration.range(),
        )?;
        Ok(true)
    }

    pub(super) fn allocate_component_record_parameter(
        &mut self,
        occurrence: ComponentOccurrence<'_, '_>,
        declaration: &eqiora_lang::ComponentParameterDecl,
        values: &BTreeMap<String, ResolvedParameter>,
        bindings: &[SourceLocation],
        scope: &mut Scope,
    ) -> Result<bool, Vec<Diagnostic>> {
        let Some(record) = scope
            .record_context
            .record_for_type(declaration.value_type())
            .cloned()
        else {
            return Ok(false);
        };
        let mut roots = Vec::new();
        for (member, _) in record.definition.members() {
            let local = format!("{}.{member}", declaration.name());
            let resolved = values[&local].clone();
            roots.push(resolved.expression.clone());
            scope.insert_parameter(local.clone(), resolved.clone());
            if let ParameterLineage::Parameter(full) = resolved.lineage {
                self.display_symbols.insert(
                    display_child(occurrence.display_prefix, &local),
                    super::super::flat::DisplayIdentity {
                        full,
                        kind: EntityKind::Parameter,
                    },
                );
                self.record_structural(
                    &internal_name(full),
                    resolved.expression.structural_parameters(),
                )
                .map_err(one_diagnostic)?;
            }
        }
        self.allocate_static_record_owner(
            declaration.name(),
            &record,
            roots,
            records::RecordFieldOccurrence {
                namespace: &occurrence.definition.namespace,
                definition_name: occurrence.definition.name(),
                instance_path: occurrence.instance_path,
                display_prefix: occurrence.display_prefix,
                file: occurrence.definition.file,
                instance: SourceLocation::new(
                    occurrence.instance_file,
                    occurrence.instance.range(),
                ),
                bindings: bindings.to_vec(),
            },
            declaration.range(),
        )
        .map_err(one_diagnostic)?;
        Ok(true)
    }

    fn allocate_static_record_owner(
        &mut self,
        name: &str,
        record: &crate::record::BoundRecord,
        members: Vec<LoweringExpression>,
        occurrence: records::RecordFieldOccurrence<'_>,
        range: eqiora_lang::TextRange,
    ) -> Result<(), Diagnostic> {
        let identity = self.entity_identity(
            occurrence.instance_path,
            definition_path(
                occurrence.namespace,
                "record",
                occurrence.definition_name,
                name,
            ),
            EntityKind::RecordInstance,
            SourceLocation::new(occurrence.file, range),
            occurrence.instance,
            occurrence.bindings,
        )?;
        self.display_symbols.insert(
            display_child(occurrence.display_prefix, name),
            super::super::flat::DisplayIdentity {
                full: identity.full,
                kind: EntityKind::RecordInstance,
            },
        );
        self.items.push(FlatItemBlueprint::RecordInstance {
            name: internal_name(identity.full),
            definition: record.definition.id(),
            members,
            identity,
        });
        Ok(())
    }
}
