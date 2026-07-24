use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{BoundaryDecl, DomainSyntax, Item, RepresentationSyntax};
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;

use super::expression::validate_relation_expression;
use super::scope::{
    DefinitionScope, DomainContract, SymbolContract, field_expression_type, model_port_contract,
    unresolved, validate_connection, validate_model_boundary_connection,
};
use super::{ChildInstanceProof, DefinitionBodyProof, LocalPhysicalPortProof, validate_clock};
use crate::hierarchy::preflight::{DefinitionKey, Elaborator, ModelDefinition};

pub(super) fn validate(
    elaborator: &Elaborator<'_>,
    definition: &ModelDefinition<'_>,
) -> Result<DefinitionBodyProof, Vec<Diagnostic>> {
    let mut checker = ModelBodyChecker::new(elaborator, definition);
    checker.validate();
    if checker.diagnostics.is_empty() {
        Ok(checker.proof)
    } else {
        Err(checker.diagnostics)
    }
}

struct ModelBodyChecker<'e, 'd> {
    definition: &'e ModelDefinition<'d>,
    scope: DefinitionScope<'e, 'd>,
    connected_ports: BTreeSet<Vec<String>>,
    proof: DefinitionBodyProof,
    diagnostics: Vec<Diagnostic>,
}

impl<'e, 'd> ModelBodyChecker<'e, 'd> {
    fn new(elaborator: &'e Elaborator<'d>, definition: &'e ModelDefinition<'d>) -> Self {
        Self {
            definition,
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
        self.bind_non_boundary_interfaces();
        self.bind_boundaries();
        self.bind_fields_and_ports();
        self.validate_declarations();
    }

    fn bind_non_boundary_interfaces(&mut self) {
        for item in self.definition.declaration.items() {
            let binding = match item {
                Item::Domain(declaration) => match declaration.syntax() {
                    DomainSyntax::CartesianBox(bounds) => Ok(Some((
                        declaration.name(),
                        SymbolContract::Domain(DomainContract::Spatial(SpatialSupport::Volume {
                            domain: declaration.name().to_owned(),
                            dimensions: bounds.len(),
                        })),
                    ))),
                    DomainSyntax::Boundary { .. } => Ok(None),
                    DomainSyntax::ScalarPhysical {
                        across_dimension,
                        through_dimension,
                    } => lower_dimension(self.scope.file, across_dimension).and_then(|across| {
                        lower_dimension(self.scope.file, through_dimension).map(|through| {
                            Some((
                                declaration.name(),
                                SymbolContract::Domain(DomainContract::Physical {
                                    across_dimension: across,
                                    through_dimension: through,
                                }),
                            ))
                        })
                    }),
                    _ => Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.scope.file,
                        declaration.range(),
                        "Domain syntax is newer than definition-body validation",
                    )),
                },
                Item::Representation(declaration) => {
                    Ok(Some((declaration.name(), SymbolContract::Representation)))
                }
                Item::Parameter(declaration) => {
                    lower_dimension(self.scope.file, declaration.dimension()).map(|dimension| {
                        Some((
                            declaration.name(),
                            SymbolContract::Parameter(ExpressionType::scalar(dimension, None)),
                        ))
                    })
                }
                Item::Clock(declaration) => Ok(Some((declaration.name(), SymbolContract::Clock))),
                Item::Relation(declaration) => {
                    Ok(Some((declaration.name(), SymbolContract::Relation)))
                }
                Item::Instance(instance) => {
                    if let Ok(child) = self.scope.elaborator.resolve_component(
                        &self.scope.namespace,
                        instance.definition(),
                        self.scope.file,
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
                    Ok(None)
                }
                Item::Field(_)
                | Item::Port(_)
                | Item::Connection(_)
                | Item::BoundaryConnection(_)
                | Item::Boundary(_) => Ok(None),
                _ => Err(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    self.scope.file,
                    self.definition.declaration.range(),
                    "model item is newer than definition-body validation",
                )),
            };
            match binding {
                Ok(Some((name, contract))) => {
                    self.scope.symbols.insert(name.to_owned(), contract);
                }
                Ok(None) => {}
                Err(error) => self.diagnostics.push(error),
            }
        }
    }

    fn bind_boundaries(&mut self) {
        for item in self.definition.declaration.items() {
            let Item::Domain(declaration) = item else {
                continue;
            };
            let DomainSyntax::Boundary { parent, .. } = declaration.syntax() else {
                continue;
            };
            match self.scope.symbols.get(parent) {
                Some(SymbolContract::Domain(DomainContract::Spatial(SpatialSupport::Volume {
                    dimensions,
                    ..
                }))) => {
                    self.scope.symbols.insert(
                        declaration.name().to_owned(),
                        SymbolContract::Domain(DomainContract::Spatial(SpatialSupport::Boundary {
                            domain: declaration.name().to_owned(),
                            parent: parent.clone(),
                            dimensions: *dimensions,
                        })),
                    );
                }
                Some(SymbolContract::Domain(DomainContract::Spatial(
                    SpatialSupport::Boundary { .. },
                ))) => {
                    self.diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        declaration.range(),
                        "Cartesian boundary parent must be a Cartesian box Domain",
                    ));
                }
                Some(_) => self.diagnostics.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.scope.file,
                    declaration.range(),
                    format!("boundary parent `{parent}` is not a spatial Domain"),
                )),
                None => self.diagnostics.push(unresolved(
                    self.scope.file,
                    declaration.range(),
                    parent,
                    "boundary parent Domain",
                )),
            }
        }
    }

    fn bind_fields_and_ports(&mut self) {
        for item in self.definition.declaration.items() {
            match item {
                Item::Field(declaration) => {
                    let support = match (declaration.domain(), declaration.representation()) {
                        (None, None) => None,
                        (Some(domain), Some(representation))
                            if matches!(
                                self.scope.symbols.get(representation),
                                Some(SymbolContract::Representation)
                            ) =>
                        {
                            match self.scope.symbols.get(domain) {
                                Some(SymbolContract::Domain(DomainContract::Spatial(
                                    SpatialSupport::Volume { .. },
                                ))) => self.scope.spatial_support(domain),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    match field_expression_type(self.scope.file, declaration, support) {
                        Ok(inferred) => {
                            self.scope.symbols.insert(
                                declaration.name().to_owned(),
                                SymbolContract::Field(inferred),
                            );
                        }
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                Item::Port(declaration) => match model_port_contract(&self.scope, declaration) {
                    Ok(contract) => {
                        if contract.is_physical() {
                            self.proof.local_physical_ports.insert(
                                declaration.name().to_owned(),
                                LocalPhysicalPortProof {
                                    public: false,
                                    range: declaration.range(),
                                },
                            );
                        }
                        self.scope.symbols.insert(
                            declaration.name().to_owned(),
                            SymbolContract::Port(contract),
                        );
                    }
                    Err(error) => self.diagnostics.push(error),
                },
                _ => {}
            }
        }
    }

    fn validate_declarations(&mut self) {
        for item in self.definition.declaration.items() {
            match item {
                Item::Domain(declaration) => self.validate_domain(declaration),
                Item::Representation(declaration) => {
                    if !matches!(declaration.syntax(), RepresentationSyntax::Continuum) {
                        self.diagnostics.push(source_error(
                            codes::LANGUAGE_LOWERING_ERROR,
                            self.scope.file,
                            declaration.range(),
                            "Representation syntax is newer than definition-body validation",
                        ));
                    }
                }
                Item::Field(declaration) => self.validate_field(declaration),
                Item::Parameter(_) | Item::Port(_) | Item::Instance(_) => {}
                Item::Clock(declaration) => {
                    if let Err(error) = validate_clock(
                        self.scope.file,
                        declaration.range(),
                        declaration.period(),
                        declaration.phase(),
                    ) {
                        self.diagnostics.push(error);
                    }
                }
                Item::Relation(declaration) => self.validate_relation(declaration),
                Item::Connection(declaration) => {
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
                Item::BoundaryConnection(declaration) => {
                    match validate_model_boundary_connection(&self.scope, declaration) {
                        Ok(memberships) => self
                            .proof
                            .deferred_connection_memberships
                            .extend(memberships),
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                Item::Boundary(declaration) => self.validate_boundary(declaration),
                _ => {}
            }
        }
    }

    fn validate_domain(&mut self, declaration: &eqiora_lang::DomainDecl) {
        match declaration.syntax() {
            DomainSyntax::CartesianBox(bounds) => {
                if bounds.is_empty() {
                    self.diagnostics.push(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        self.scope.file,
                        declaration.range(),
                        "Cartesian Domain requires at least one coordinate axis",
                    ));
                }
                if bounds.iter().any(|(lower, upper)| {
                    !lower.is_finite() || !upper.is_finite() || upper <= lower
                }) {
                    self.diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        declaration.range(),
                        "Cartesian axis bounds must be finite and strictly increasing",
                    ));
                }
            }
            DomainSyntax::Boundary { parent, axis, .. } => {
                if let Some(SymbolContract::Domain(DomainContract::Spatial(
                    SpatialSupport::Volume { dimensions, .. },
                ))) = self.scope.symbols.get(parent)
                    && *axis >= *dimensions
                {
                    self.diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        declaration.range(),
                        format!("boundary axis {axis} is outside parent dimension {dimensions}"),
                    ));
                }
            }
            DomainSyntax::ScalarPhysical { .. } => {}
            _ => {}
        }
    }

    fn validate_field(&mut self, declaration: &eqiora_lang::FieldDecl) {
        match (declaration.domain(), declaration.representation()) {
            (None, None) => {}
            (Some(domain), Some(representation)) => {
                match self.scope.symbols.get(domain) {
                    Some(SymbolContract::Domain(DomainContract::Spatial(
                        SpatialSupport::Volume { .. },
                    ))) => {}
                    Some(SymbolContract::Domain(_)) => self.diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        declaration.range(),
                        "spatial Field cannot be defined on a non-volume Domain",
                    )),
                    Some(_) | None => self.diagnostics.push(unresolved(
                        self.scope.file,
                        declaration.range(),
                        domain,
                        "Field Domain",
                    )),
                }
                if !matches!(
                    self.scope.symbols.get(representation),
                    Some(SymbolContract::Representation)
                ) {
                    self.diagnostics.push(unresolved(
                        self.scope.file,
                        declaration.range(),
                        representation,
                        "Field Representation",
                    ));
                }
            }
            _ => self.diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                declaration.range(),
                "spatial Field requires both `on Domain` and `as Representation`",
            )),
        }
    }

    fn validate_relation(&mut self, declaration: &eqiora_lang::RelationDecl) {
        let support = declaration.domain().and_then(|domain| {
            let support = self.scope.spatial_support(domain);
            if support.is_none() {
                self.diagnostics.push(unresolved(
                    self.scope.file,
                    declaration.range(),
                    domain,
                    "Relation Domain",
                ));
            }
            support
        });
        match validate_relation_expression(&self.scope, declaration, support) {
            Ok(endpoints) => self.proof.relation_endpoints.push(endpoints),
            Err(errors) => self.diagnostics.extend(errors),
        }
    }

    fn validate_boundary(&mut self, declaration: &BoundaryDecl) {
        for path in declaration.port_paths() {
            if let Err(error) = self.scope.resolve_port(path) {
                self.diagnostics.push(error);
            }
        }
    }
}
