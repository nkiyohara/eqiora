//! Capture quantities while exact source occurrences still exist, before graph cuts.
use super::*;
use crate::notation::{NotationSpec, QuantityIdentity, QuantityRole};

impl RootExpansion<'_, '_> {
    pub(super) fn record_borrowed_fields(
        &mut self,
        occurrence: ComponentOccurrence<'_, '_>,
        scope: &Scope,
        bindings: &[SourceLocation],
    ) -> Result<(), Diagnostic> {
        let component = occurrence.definition;
        for item in component.signature() {
            let eqiora_lang::SignatureItem::Field(field) = item else {
                continue;
            };
            let target = scope
                .symbol(field.name())
                .expect("accepted exact borrowed Field target");
            let identity = self.entity_identity(
                occurrence.instance_path,
                definition_path(
                    &component.namespace,
                    "component",
                    component.name(),
                    field.name(),
                ),
                EntityKind::Field,
                SourceLocation::new(component.file, field.range()),
                SourceLocation::new(occurrence.instance_file, occurrence.instance.range()),
                bindings.to_vec(),
            )?;
            self.record_notation(
                &display_child(occurrence.display_prefix, field.name()),
                &identity,
                &SymbolKind::Field,
            );
            let spec = self
                .notation_specs
                .last_mut()
                .expect("registered borrowed Field label");
            spec.graph_identity = Some(target.full_identity);
        }
        Ok(())
    }

    pub(super) fn record_notation(
        &mut self,
        selector: &str,
        identity: &EntityIdentity,
        kind: &SymbolKind,
    ) {
        let roles: Vec<(QuantityRole, &str)> = match kind {
            SymbolKind::Field
            | SymbolKind::Parameter
            | SymbolKind::Port {
                quantities: None, ..
            } => vec![(QuantityRole::Value, "")],
            SymbolKind::Port {
                quantities: Some(PhysicalMemberNames::Scalar { across, through }),
                ..
            } => vec![
                (QuantityRole::Across, across),
                (QuantityRole::Through, through),
            ],
            SymbolKind::Port {
                quantities: Some(PhysicalMemberNames::Boundary { trace, flux }),
                ..
            } => vec![(QuantityRole::Trace, trace), (QuantityRole::Flux, flux)],
            _ => return,
        };
        let location = &identity.definition;
        let declared = self.elaborator.notations.get(&(
            location.file.clone(),
            location.range.start(),
            location.range.end(),
        ));
        let path = identity.key.instance_segments();
        let qualifiers = (1..=path.len())
            .filter_map(|length| self.instance_qualifiers.get(&path[..length]).cloned())
            .collect::<Vec<_>>();
        for (role, role_name) in roles {
            self.notation_specs.push(NotationSpec {
                graph_identity: Some(identity.full),
                identity: QuantityIdentity {
                    scope: self.model_full,
                    occurrence: identity.full,
                    role,
                    member: identity.key.family_member(),
                },
                selector: if role_name.is_empty() {
                    selector.to_owned()
                } else {
                    format!("{selector}.{role_name}")
                },
                graph_id: None,
                definition: Some(location.span()),
                instance: Some(identity.instance.span()),
                declared: declared.cloned(),
                qualifiers: qualifiers.clone(),
                // The root Model isn't a distinguishing occurrence qualifier.
                instance_path: path.iter().skip(1).cloned().collect(),
                declaration: identity
                    .key
                    .declaration_segments()
                    .last()
                    .cloned()
                    .unwrap_or_default(),
                role_name: if role_name.is_empty() {
                    role.name().to_owned()
                } else {
                    role_name.to_owned()
                },
            });
        }
    }
}
