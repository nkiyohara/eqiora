//! Bounded local declaration and complete-exterior footprint accounting.

use super::*;

pub(super) fn component_local_footprint(
    elaborator: &Elaborator<'_>,
    definition: &ComponentDefinition<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (LocalFootprint, BTreeSet<DefinitionKey>) {
    let mut declarations = 0_usize;
    let mut connections = 0_usize;
    let mut local_connectors = BTreeSet::new();
    for item in definition.owned_items() {
        match item {
            ComponentItem::Parameter(_)
            | ComponentItem::Port(_)
            | ComponentItem::Initial(_)
            | ComponentItem::Clock(_) => checked_local_add(
                &mut declarations,
                1,
                definition.file,
                definition.declaration.range(),
                "declaration",
                diagnostics,
            ),
            ComponentItem::Field(field) => checked_local_add(
                &mut declarations,
                if field.domain().is_some() { 2 } else { 1 },
                definition.file,
                field.range(),
                "Field and continuum representation",
                diagnostics,
            ),
            ComponentItem::Relation(_) => checked_local_add(
                &mut declarations,
                2,
                definition.file,
                definition.declaration.range(),
                "declaration",
                diagnostics,
            ),
            ComponentItem::Connection(_) => checked_local_add(
                &mut connections,
                1,
                definition.file,
                definition.declaration.range(),
                "Connection",
                diagnostics,
            ),
            ComponentItem::PortFamily(family) => {
                if let Some(members) = complete_exterior_cardinality(
                    definition,
                    family.binder().set().as_str(),
                    family.range(),
                    diagnostics,
                ) {
                    checked_local_add(
                        &mut declarations,
                        members,
                        definition.file,
                        family.range(),
                        "complete-exterior Port-family declaration",
                        diagnostics,
                    );
                }
            }
            ComponentItem::RelationFamily(family) => {
                if let Some(members) = complete_exterior_cardinality(
                    definition,
                    family.binder().set().as_str(),
                    family.range(),
                    diagnostics,
                ) {
                    match members.checked_mul(2) {
                        Some(declarations_per_family) => checked_local_add(
                            &mut declarations,
                            declarations_per_family,
                            definition.file,
                            family.range(),
                            "complete-exterior Relation and Activation declarations",
                            diagnostics,
                        ),
                        None => diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            definition.file,
                            family.range(),
                            "complete-exterior Relation-family declaration count overflows usize",
                        )),
                    }
                }
            }
            ComponentItem::BoundaryConnection(connection) => {
                let Some(binder) = connection.binder() else {
                    checked_local_add(
                        &mut connections,
                        1,
                        definition.file,
                        connection.range(),
                        "selected boundary Connection",
                        diagnostics,
                    );
                    continue;
                };
                if let Some(members) = complete_exterior_cardinality(
                    definition,
                    binder.set().as_str(),
                    connection.range(),
                    diagnostics,
                ) {
                    checked_local_add(
                        &mut connections,
                        members,
                        definition.file,
                        connection.range(),
                        "complete-exterior Connection family",
                        diagnostics,
                    );
                }
            }
            ComponentItem::Let(_) => {}
            ComponentItem::Instance(instance) => {
                let count = input_binding_count(
                    elaborator,
                    &definition.namespace,
                    definition.file,
                    instance,
                    diagnostics,
                );
                checked_local_add(
                    &mut connections,
                    count,
                    definition.file,
                    instance.range(),
                    "Input connections",
                    diagnostics,
                );
            }
            _ => {}
        }
        let port = match item {
            ComponentItem::Port(port) => port,
            ComponentItem::PortFamily(family) => family.port(),
            _ => continue,
        };
        let connector = match port.syntax() {
            PortSyntax::ScalarPhysicalConnector { connector }
            | PortSyntax::FieldPhysical { connector, .. } => connector,
            _ => continue,
        };
        match elaborator.resolve_connector(
            &definition.namespace,
            connector,
            definition.file,
            port.range(),
        ) {
            Ok(connector) => {
                local_connectors.insert(DefinitionKey {
                    namespace: connector.namespace,
                    name: connector.declaration.name().to_owned(),
                });
            }
            Err(error) => diagnostics.push(error),
        }
    }
    (
        LocalFootprint {
            declarations,
            connections,
        },
        local_connectors,
    )
}

fn complete_exterior_cardinality(
    definition: &ComponentDefinition<'_>,
    set: &str,
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<usize> {
    let support = definition
        .declaration
        .signature()
        .iter()
        .find_map(|item| match item {
            eqiora_lang::SignatureItem::Support(support) if support.name() == set => Some(support),
            _ => None,
        });
    let Some(support) = support else {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            definition.file,
            range,
            format!("boundary family refers to unknown complete-exterior support `{set}`"),
        ));
        return None;
    };
    let eqiora_lang::SupportSlotSyntax::CompleteExterior { parent } = support.syntax() else {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            definition.file,
            range,
            format!("boundary family support `{set}` is not a complete exterior"),
        ));
        return None;
    };
    let parent_name = parent;
    let parent = definition
        .declaration
        .signature()
        .iter()
        .find_map(|item| match item {
            eqiora_lang::SignatureItem::Support(parent_support)
                if parent_support.name() == parent_name =>
            {
                Some(parent_support)
            }
            _ => None,
        });
    let Some(parent) = parent else {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            definition.file,
            range,
            format!("complete exterior `{set}` refers to unknown parent support `{parent_name}`"),
        ));
        return None;
    };
    let eqiora_lang::SupportSlotSyntax::Volume { ambient_dimension } = parent.syntax() else {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            definition.file,
            range,
            format!("complete exterior `{set}` requires a volume parent support"),
        ));
        return None;
    };
    match ambient_dimension.checked_mul(2) {
        Some(members) => Some(members),
        None => {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                definition.file,
                range,
                format!("complete exterior `{set}` member count overflows usize"),
            ));
            None
        }
    }
}

pub(super) fn model_local_footprint(
    elaborator: &Elaborator<'_>,
    definition: &ModelDefinition<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> LocalFootprint {
    let mut footprint = LocalFootprint::default();
    for item in definition.owned_items() {
        match item {
            Item::Field(field) => checked_local_add(
                &mut footprint.declarations,
                if field.domain().is_some() { 2 } else { 1 },
                definition.file,
                field.range(),
                "Field and continuum representation",
                diagnostics,
            ),
            Item::Relation(_) => checked_local_add(
                &mut footprint.declarations,
                2,
                definition.file,
                definition.declaration.range(),
                "declaration",
                diagnostics,
            ),
            Item::Connection(_) | Item::BoundaryConnection(_) => checked_local_add(
                &mut footprint.connections,
                1,
                definition.file,
                definition.declaration.range(),
                "Connection",
                diagnostics,
            ),
            Item::Let(_) => {}
            Item::Instance(instance) => {
                let count = input_binding_count(
                    elaborator,
                    &definition.namespace,
                    definition.file,
                    instance,
                    diagnostics,
                );
                checked_local_add(
                    &mut footprint.connections,
                    count,
                    definition.file,
                    instance.range(),
                    "Input connections",
                    diagnostics,
                );
            }
            _ => checked_local_add(
                &mut footprint.declarations,
                1,
                definition.file,
                definition.declaration.range(),
                "declaration",
                diagnostics,
            ),
        }
    }
    footprint
}

fn checked_local_add(
    count: &mut usize,
    additional: usize,
    file: &str,
    range: TextRange,
    resource: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match count.checked_add(additional) {
        Some(next) => *count = next,
        None => diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            format!("local {resource} count overflows usize"),
        )),
    }
}

fn input_binding_count(
    elaborator: &Elaborator<'_>,
    namespace: &super::super::preflight::DefinitionNamespace,
    file: &str,
    instance: &eqiora_lang::InstanceDecl,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    match elaborator.resolve_component(namespace, instance.definition(), file, instance.range()) {
        Ok(child) => instance.bindings().iter().filter(|binding| child.signature().iter().any(|item| matches!(item, eqiora_lang::SignatureItem::Input(input) if input.name() == binding.name()))).count(),
        Err(error) => { diagnostics.push(error); 0 }
    }
}
