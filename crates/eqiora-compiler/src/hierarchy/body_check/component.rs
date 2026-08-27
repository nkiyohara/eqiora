use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{ComponentItem, RepresentationSyntax, SupportSlotSyntax};
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

use crate::diagnostics::source_error;

use super::expression::{validate_relation_expression, validate_relation_family_expression};
use super::scope::{
    DefinitionScope, SymbolContract, component_port_contract, component_port_family_contract,
    field_expression_type, validate_boundary_connection, validate_connection,
};
use super::{
    ChildInstanceProof, DefinitionBodyProof, LocalPhysicalPortProof, SymbolicParameterMap,
    validate_clock,
};
use crate::hierarchy::field_slots::FieldInterface;
use crate::hierarchy::preflight::{ComponentDefinition, DefinitionKey, Elaborator};
use crate::hierarchy::supports::SupportInterface;

pub(super) fn validate(
    elaborator: &Elaborator<'_>,
    definition: &ComponentDefinition<'_>,
    parameters: &SymbolicParameterMap,
    supports: &SupportInterface,
    fields: &FieldInterface,
) -> Result<DefinitionBodyProof, Vec<Diagnostic>> {
    let mut checker =
        ComponentBodyChecker::new(elaborator, definition, parameters, supports, fields);
    checker.validate();
    if checker.diagnostics.is_empty() {
        Ok(checker.proof)
    } else {
        Err(checker.diagnostics)
    }
}

struct ComponentBodyChecker<'e, 'd> {
    definition: &'e ComponentDefinition<'d>,
    parameters: &'e SymbolicParameterMap,
    supports: &'e SupportInterface,
    fields: &'e FieldInterface,
    scope: DefinitionScope<'e, 'd>,
    connected_ports: BTreeSet<Vec<String>>,
    proof: DefinitionBodyProof,
    diagnostics: Vec<Diagnostic>,
}

impl<'e, 'd> ComponentBodyChecker<'e, 'd> {
    fn new(
        elaborator: &'e Elaborator<'d>,
        definition: &'e ComponentDefinition<'d>,
        parameters: &'e SymbolicParameterMap,
        supports: &'e SupportInterface,
        fields: &'e FieldInterface,
    ) -> Self {
        Self {
            definition,
            parameters,
            supports,
            fields,
            scope: DefinitionScope::new(elaborator, definition.namespace.clone(), definition.file),
            connected_ports: BTreeSet::new(),
            proof: DefinitionBodyProof::new(
                definition.file,
                definition.declaration.range(),
                elaborator.limits.connection_sets,
            ),
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.bind_complete_exteriors();
        self.bind_interfaces();
        self.validate_declarations();
    }

    fn bind_complete_exteriors(&mut self) {
        for item in self.definition.declaration.items() {
            let ComponentItem::Support(declaration) = item else {
                continue;
            };
            let SupportSlotSyntax::CompleteExterior { parent } = declaration.syntax() else {
                continue;
            };
            let Some(parent) = self.supports.get(parent) else {
                self.diagnostics.push(self.scope.wrong_local_kind(
                    declaration.range(),
                    parent,
                    "complete-exterior volume parent",
                ));
                continue;
            };
            let SpatialSupport::Volume { .. } = parent.support() else {
                self.diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.definition.file,
                    declaration.range(),
                    "complete-exterior support requires a volume parent",
                ));
                continue;
            };
            self.scope.symbols.insert(
                declaration.name().to_owned(),
                SymbolContract::CompleteExterior {
                    parent: parent.support().clone(),
                },
            );
        }
    }

    fn bind_interfaces(&mut self) {
        for item in self.definition.declaration.items() {
            match item {
                ComponentItem::Parameter(declaration) => {
                    let Some(parameter) = self.parameters.get(declaration.name()) else {
                        self.diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            self.definition.file,
                            declaration.range(),
                            format!(
                                "symbolic interface is missing component Parameter `{}`",
                                declaration.name()
                            ),
                        ));
                        continue;
                    };
                    self.scope.symbols.insert(
                        declaration.name().to_owned(),
                        SymbolContract::Parameter(ExpressionType::scalar(
                            parameter.dimension,
                            None,
                        )),
                    );
                }
                ComponentItem::Port(declaration) => match component_port_contract(
                    self.scope.elaborator,
                    self.definition,
                    declaration,
                ) {
                    Ok(contract) => {
                        if contract.is_physical() {
                            self.proof.local_physical_ports.insert(
                                declaration.name().to_owned(),
                                LocalPhysicalPortProof {
                                    public: declaration.visibility()
                                        == eqiora_lang::VisibilitySyntax::Public,
                                    range: declaration.range(),
                                },
                            );
                        }
                        self.scope.symbols.insert(
                            declaration.name().to_owned(),
                            SymbolContract::Port(contract),
                        );
                    }
                    Err(mut errors) => self.diagnostics.append(&mut errors),
                },
                ComponentItem::PortFamily(declaration) => {
                    let active = match self.scope.boundary_family_scope(declaration.binder()) {
                        Ok(active) => active,
                        Err(error) => {
                            self.diagnostics.push(error);
                            continue;
                        }
                    };
                    match component_port_family_contract(
                        self.scope.elaborator,
                        self.definition,
                        declaration,
                        active.support(),
                    ) {
                        Ok(contract) => {
                            self.scope.symbols.insert(
                                declaration.port().name().to_owned(),
                                SymbolContract::PortFamily(contract),
                            );
                        }
                        Err(mut errors) => self.diagnostics.append(&mut errors),
                    }
                }
                ComponentItem::Support(declaration) => {
                    if matches!(
                        declaration.syntax(),
                        SupportSlotSyntax::CompleteExterior { .. }
                    ) {
                        continue;
                    }
                    let Some(contract) = self.supports.get(declaration.name()) else {
                        self.diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            self.definition.file,
                            declaration.range(),
                            format!(
                                "typed support interface is missing slot `{}`",
                                declaration.name()
                            ),
                        ));
                        continue;
                    };
                    self.scope.symbols.insert(
                        declaration.name().to_owned(),
                        SymbolContract::Support(contract.support().clone()),
                    );
                }
                ComponentItem::FieldSlot(declaration) => {
                    let Some(contract) = self.fields.field(declaration.name()) else {
                        self.diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            self.definition.file,
                            declaration.range(),
                            format!(
                                "typed Field interface is missing slot `{}`",
                                declaration.name()
                            ),
                        ));
                        continue;
                    };
                    self.scope.symbols.insert(
                        declaration.name().to_owned(),
                        SymbolContract::Field(contract.value().clone()),
                    );
                }
                ComponentItem::Representation(declaration) => {
                    self.scope.symbols.insert(
                        declaration.name().to_owned(),
                        SymbolContract::Representation,
                    );
                }
                ComponentItem::Field(declaration) => {
                    let support = match (declaration.domain(), declaration.representation()) {
                        (Some(domain), Some(representation))
                            if matches!(
                                self.scope.symbols.get(representation),
                                Some(SymbolContract::Representation)
                            ) && matches!(
                                self.scope.spatial_support(domain),
                                Some(SpatialSupport::Volume { .. })
                            ) =>
                        {
                            self.scope.spatial_support(domain)
                        }
                        _ => None,
                    };
                    match field_expression_type(self.definition.file, declaration, support) {
                        Ok(inferred) => {
                            self.scope.symbols.insert(
                                declaration.name().to_owned(),
                                SymbolContract::Field(inferred),
                            );
                        }
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                ComponentItem::Clock(declaration) => {
                    self.scope
                        .symbols
                        .insert(declaration.name().to_owned(), SymbolContract::Clock);
                }
                ComponentItem::Relation(declaration) => {
                    self.scope
                        .symbols
                        .insert(declaration.name().to_owned(), SymbolContract::Relation);
                }
                ComponentItem::RelationFamily(declaration) => {
                    self.scope.symbols.insert(
                        declaration.relation().name().to_owned(),
                        SymbolContract::Relation,
                    );
                }
                ComponentItem::Instance(instance) => {
                    if let Ok(child) = self.scope.elaborator.resolve_component(
                        &self.definition.namespace,
                        instance.definition(),
                        self.definition.file,
                        instance.range(),
                    ) {
                        self.proof.children.insert(
                            instance.name().to_owned(),
                            ChildInstanceProof {
                                definition: DefinitionKey {
                                    namespace: child.namespace.clone(),
                                    name: child.declaration.name().to_owned(),
                                },
                                range: instance.range(),
                            },
                        );
                        self.scope
                            .children
                            .insert(instance.name().to_owned(), child);
                        self.scope
                            .child_instances
                            .insert(instance.name().to_owned(), instance);
                    }
                }
                ComponentItem::Connection(_) | ComponentItem::BoundaryConnection(_) => {}
                _ => self.diagnostics.push(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    self.definition.file,
                    self.definition.declaration.range(),
                    "component item is newer than definition-body validation",
                )),
            }
        }
    }

    fn validate_declarations(&mut self) {
        for item in self.definition.declaration.items() {
            match item {
                ComponentItem::Parameter(_)
                | ComponentItem::Port(_)
                | ComponentItem::PortFamily(_)
                | ComponentItem::Support(_)
                | ComponentItem::FieldSlot(_) => {}
                ComponentItem::Representation(declaration) => {
                    if !matches!(declaration.syntax(), RepresentationSyntax::Continuum) {
                        self.diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            self.definition.file,
                            declaration.range(),
                            "Representation syntax is newer than definition-body validation",
                        ));
                    }
                }
                ComponentItem::Field(declaration) => {
                    match (declaration.domain(), declaration.representation()) {
                        (None, None) => {}
                        (Some(domain), Some(representation)) => {
                            match self.scope.spatial_support(domain) {
                                Some(SpatialSupport::Volume { .. }) => {}
                                Some(SpatialSupport::Boundary { .. }) => {
                                    self.diagnostics.push(source_error(
                                        codes::LANGUAGE_TYPE_ERROR,
                                        self.definition.file,
                                        declaration.range(),
                                        "spatial Field cannot be defined on a boundary support",
                                    ));
                                }
                                Some(SpatialSupport::Interface { .. }) => {
                                    self.diagnostics.push(source_error(
                                        codes::LANGUAGE_LOWERING_ERROR,
                                        self.definition.file,
                                        declaration.range(),
                                        "derived interface support cannot define a source Field",
                                    ));
                                }
                                None => self.diagnostics.push(self.scope.wrong_local_kind(
                                    declaration.range(),
                                    domain,
                                    "Field support",
                                )),
                            }
                            if !matches!(
                                self.scope.symbols.get(representation),
                                Some(SymbolContract::Representation)
                            ) {
                                self.diagnostics.push(self.scope.wrong_local_kind(
                                    declaration.range(),
                                    representation,
                                    "Field Representation",
                                ));
                            }
                        }
                        _ => self.diagnostics.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.definition.file,
                            declaration.range(),
                            "spatial Field requires both Domain and Representation",
                        )),
                    }
                }
                ComponentItem::Clock(declaration) => {
                    if let Err(error) = validate_clock(
                        self.definition.file,
                        declaration.range(),
                        declaration.period(),
                        declaration.phase(),
                    ) {
                        self.diagnostics.push(error);
                    }
                }
                ComponentItem::Relation(declaration) => {
                    let support = declaration.domain().and_then(|domain| {
                        let support = self.scope.spatial_support(domain);
                        if support.is_none() {
                            self.diagnostics.push(self.scope.wrong_local_kind(
                                declaration.range(),
                                domain,
                                "Relation support",
                            ));
                        }
                        support
                    });
                    match validate_relation_expression(&self.scope, declaration, support) {
                        Ok(endpoints) => self.proof.relation_endpoints.push(endpoints),
                        Err(errors) => self.diagnostics.extend(errors),
                    }
                }
                ComponentItem::RelationFamily(declaration) => {
                    let active = match self.scope.boundary_family_scope(declaration.binder()) {
                        Ok(active) => active,
                        Err(error) => {
                            self.diagnostics.push(error);
                            continue;
                        }
                    };
                    match validate_relation_family_expression(&self.scope, declaration, &active) {
                        Ok(_) => {}
                        Err(errors) => self.diagnostics.extend(errors),
                    }
                }
                ComponentItem::Connection(declaration) => {
                    match validate_connection(
                        &self.scope,
                        declaration,
                        &mut self.connected_ports,
                        self.proof.connection_limits,
                    ) {
                        Ok(Some(fragment)) => {
                            self.proof.physical_connection_fragments.push(fragment);
                        }
                        Ok(None) => {}
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                ComponentItem::BoundaryConnection(declaration) => {
                    match validate_boundary_connection(&self.scope, declaration) {
                        Ok(Some(memberships)) if memberships.len() >= 2 => {
                            match crate::connection_sets::ConnectionFragment::try_new(
                                memberships,
                                self.proof.connection_limits,
                            ) {
                                Ok(fragment) => {
                                    self.proof.physical_connection_fragments.push(fragment);
                                }
                                Err(error) => self.diagnostics.push(source_error(
                                    codes::LANGUAGE_LOWERING_ERROR,
                                    self.scope.file,
                                    declaration.range(),
                                    format!(
                                        "cannot retain exact boundary Connection class: {error}"
                                    ),
                                )),
                            }
                        }
                        Ok(Some(memberships)) => self
                            .proof
                            .deferred_connection_memberships
                            .extend(memberships),
                        Ok(None) => {}
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                ComponentItem::Instance(_) => {}
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::field_slots::component_field_interface;
    use crate::hierarchy::parameters::resolve_component_parameters_symbolically;
    use crate::hierarchy::supports::component_support_interface;
    use crate::source_identity::LocalSourceIdentity;

    fn validate_component(
        source: &str,
        name: &str,
    ) -> Result<DefinitionBodyProof, Vec<Diagnostic>> {
        let document = eqiora_lang::parse("family.eqi", source)
            .into_document()
            .expect("test source parses");
        let identity = LocalSourceIdentity::from_document(&document).expect("source identity");
        let elaborator = Elaborator::new(
            "family.eqi",
            source.len(),
            &document,
            identity,
            crate::hierarchy::HierarchyLimits::default(),
        )?;
        let definition = elaborator
            .components()
            .find_map(|(_, definition)| {
                (definition.declaration.name() == name).then(|| definition.clone())
            })
            .expect("selected Component exists");
        let parameters =
            resolve_component_parameters_symbolically(definition.file, definition.declaration)?;
        let supports = component_support_interface(definition.file, definition.declaration)?;
        let fields = component_field_interface(definition.file, definition.declaration, &supports)?;
        validate(&elaborator, &definition, &parameters, &supports, &fields)
    }

    const SCALAR_CONNECTOR: &str = r#"
public connector BoundaryScalar = field_physical(
  trace = value: 1,
  flux = flux: 1,
  shape = [],
  frame = invariant,
  pairing = euclidean_boundary_duality
);
"#;

    #[test]
    fn complete_exterior_family_is_checked_once_with_a_synthetic_member_identity() {
        let source = format!(
            r#"{SCALAR_CONNECTOR}
component BoundaryLaw {{
  public port natural[boundary in exterior]: conserving BoundaryScalar over boundary;
  public port coupled[boundary in exterior]: conserving BoundaryScalar over boundary;
  public support exterior: complete_exterior(parent = body);
  public support body: volume(ambient_dimension = 2);
  relation natural_law[boundary in exterior] continuous on boundary {{
    flux(natural[boundary = boundary]) = 0;
  }}
  connect conserving [boundary in exterior]
    natural[boundary = boundary], coupled[boundary = boundary];
}}
"#
        );
        let proof = validate_component(&source, "BoundaryLaw")
            .expect("closed family is valid before occurrence expansion");

        assert!(proof.relation_endpoints.is_empty());
        assert!(proof.physical_connection_fragments.is_empty());
        assert!(proof.local_physical_ports.is_empty());
    }

    #[test]
    fn binderless_exact_boundary_connection_retains_its_component_class() {
        let source = format!(
            r#"{SCALAR_CONNECTOR}
component Coupler {{
  public support left_body: volume(ambient_dimension = 2);
  public support left_face: boundary(parent = left_body);
  public support right_body: volume(ambient_dimension = 2);
  public support right_face: boundary(parent = right_body);
  public port left: conserving BoundaryScalar over left_face;
  public port right: conserving BoundaryScalar over right_face;
  relation left_law continuous on left_face {{ trace(left) = 0; flux(left) = 0; }}
  relation right_law continuous on right_face {{ trace(right) = 0; flux(right) = 0; }}
  connect conserving left, right;
}}
"#
        );
        let proof = validate_component(&source, "Coupler")
            .expect("binderless exact boundary Connection is typed");

        assert_eq!(proof.physical_connection_fragments.len(), 1);
        assert_eq!(proof.physical_connection_fragments[0].members().len(), 2);
        assert!(proof.deferred_connection_memberships.is_empty());
    }

    #[test]
    fn family_binders_and_selectors_fail_closed_outside_one_exact_scope() {
        let cases = [
            (
                "unknown exterior",
                "missing",
                "complete-exterior support set",
            ),
            (
                "wrong selector target",
                "exterior",
                "does not name active family member",
            ),
        ];
        for (case, relation_set, expected) in cases {
            let target = if case == "wrong selector target" {
                "other"
            } else {
                "boundary"
            };
            let source = format!(
                r#"{SCALAR_CONNECTOR}
component BoundaryLaw {{
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port natural[boundary in exterior]: conserving BoundaryScalar over boundary;
  relation law[boundary in {relation_set}] continuous on boundary {{
    flux(natural[boundary = {target}]) = 0;
  }}
}}
"#
            );
            let diagnostics = validate_component(&source, "BoundaryLaw").expect_err(case);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "{case}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn boundary_connection_requires_exactly_selected_matching_port_families() {
        let source = r#"
public connector A = field_physical(
  trace = value: 1,
  flux = flux: 1,
  shape = [],
  frame = invariant,
  pairing = euclidean_boundary_duality
);
public connector B = field_physical(
  trace = value: 1,
  flux = flux: 1,
  shape = [],
  frame = invariant,
  pairing = euclidean_boundary_duality
);
component InvalidConnection {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port left[boundary in exterior]: conserving A over boundary;
  public port right[boundary in exterior]: conserving B over boundary;
  connect conserving [boundary in exterior]
    left[boundary = boundary], right[boundary = boundary];
}
"#;
        let diagnostics =
            validate_component(source, "InvalidConnection").expect_err("nominal mismatch");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("exact same specialized Connector")
        }));
    }

    #[test]
    fn child_port_family_requires_explicit_complete_exterior_forwarding() {
        let prefix = format!(
            r#"{SCALAR_CONNECTOR}
component Leaf {{
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[side in exterior]: conserving BoundaryScalar over side;
}}
"#
        );
        let parent = |forwarding: &str| {
            format!(
                r#"{prefix}
component Parent {{
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[boundary in exterior]: conserving BoundaryScalar over boundary;
  instance child: Leaf(support body = body{forwarding});
  connect conserving [boundary in exterior]
    child.mechanical[side = boundary], mechanical[boundary = boundary];
}}
"#
            )
        };

        validate_component(&parent(", support exterior = exterior"), "Parent")
            .expect("child family is mapped through one explicit set forwarding");
        let diagnostics = validate_component(&parent(""), "Parent")
            .expect_err("a child family cannot capture an unrelated active binder");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("is not forwarded from active complete exterior `exterior`")
        }));
    }
}
