use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{DomainSyntax, Item};
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

use crate::diagnostics::source_error;

use super::expression::validate_relation_expression;
use super::scope::{
    DefinitionScope, DomainContract, SymbolContract, field_expression_type, model_port_contract,
    unresolved, validate_model_boundary_connection,
};
use super::{ChildInstanceProof, DefinitionBodyProof, LocalPhysicalPortProof, validate_clock};
use crate::hierarchy::parameters::SymbolicParameterMap;
use crate::hierarchy::preflight::{DefinitionKey, Elaborator, ModelDefinition};

pub(super) fn validate(
    elaborator: &Elaborator<'_>,
    definition: &ModelDefinition<'_>,
    compile_time_values: &SymbolicParameterMap,
) -> Result<DefinitionBodyProof, Vec<Diagnostic>> {
    let mut checker = ModelBodyChecker::new(elaborator, definition, compile_time_values);
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
    compile_time_values: &'e SymbolicParameterMap,
}

impl<'e, 'd> ModelBodyChecker<'e, 'd> {
    fn new(
        elaborator: &'e Elaborator<'d>,
        definition: &'e ModelDefinition<'d>,
        compile_time_values: &'e SymbolicParameterMap,
    ) -> Self {
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
            compile_time_values,
        }
    }

    fn validate(&mut self) {
        self.scope.static_values = self.compile_time_values.clone();
        for item in self.definition.declaration.items() {
            if let Item::IndexSet(declaration) = item
                && let Err(error) = self.scope.bind_index_set(declaration)
            {
                self.diagnostics.push(error);
            }
        }
        self.bind_non_boundary_interfaces();
        self.bind_boundaries();
        self.bind_fields_and_ports();
        if let Err(errors) = super::expression::validate_aliases(
            &mut self.scope,
            self.definition
                .declaration
                .items()
                .iter()
                .filter_map(|item| match item {
                    Item::Let(d) => Some(d),
                    _ => None,
                }),
            self.compile_time_values,
        ) {
            self.diagnostics.extend(errors);
        }
        self.validate_declarations();
    }

    fn bind_non_boundary_interfaces(&mut self) {
        let signature = self.definition.declaration.signature();
        let supports =
            super::super::supports::signature_support_interface(self.scope.file, signature);
        match supports {
            Ok(supports) => {
                for (name, contract) in supports.iter() {
                    self.scope.symbols.insert(
                        name.to_owned(),
                        SymbolContract::Support(contract.support().clone()),
                    );
                }
                match super::super::field_slots::signature_field_interface(
                    self.scope.file,
                    signature,
                    &supports,
                ) {
                    Ok(fields) => {
                        for item in signature {
                            if let eqiora_lang::SignatureItem::Field(field) = item
                                && let Some(contract) = fields.field(field.name())
                            {
                                self.scope.symbols.insert(
                                    field.name().to_owned(),
                                    SymbolContract::Field(
                                        contract.value().clone(),
                                        field.role(),
                                        field.activation().clone(),
                                    ),
                                );
                            }
                        }
                    }
                    Err(errors) => self.diagnostics.extend(errors),
                }
            }
            Err(errors) => self.diagnostics.extend(errors),
        }
        for item in signature {
            match item {
                eqiora_lang::SignatureItem::Input(value)
                | eqiora_lang::SignatureItem::Output(value) => {
                    self.scope.exposed_signals.insert(value.name().to_owned());
                }
                eqiora_lang::SignatureItem::Clock(value) => {
                    self.scope.borrowed_clocks.insert(value.name().to_owned());
                    self.scope
                        .symbols
                        .insert(value.name().to_owned(), SymbolContract::Clock);
                }
                eqiora_lang::SignatureItem::Parameter(value) => {
                    if let Some(value_type) = self.compile_time_values.get(value.name()) {
                        self.scope.symbols.insert(
                            value.name().to_owned(),
                            SymbolContract::Parameter(ExpressionType::new(
                                value_type.value_type.clone(),
                                None,
                            )),
                        );
                    }
                }
                _ => {}
            }
        }

        for item in self.definition.declaration.items() {
            if let Item::Instance(instance) = item
                && let Ok(child) = self.scope.elaborator.resolve_component(
                    &self.scope.namespace,
                    instance.definition(),
                    self.scope.file,
                    instance.range(),
                )
            {
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
        for item in self.definition.owned_items() {
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
                        across_type,
                        through_type,
                    } => crate::value_types::lower_scalar_type(self.scope.file, across_type)
                        .and_then(|across| {
                            crate::value_types::lower_scalar_type(self.scope.file, through_type)
                                .map(|through| {
                                    Some((
                                        declaration.name(),
                                        SymbolContract::Domain(DomainContract::Physical {
                                            across_type: across,
                                            through_type: through,
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
                Item::Parameter(declaration) => {
                    let value_type = self
                        .compile_time_values
                        .get(declaration.name())
                        .map(|value| Ok(value.value_type.clone()))
                        .unwrap_or_else(|| {
                            let frames = crate::hierarchy::supports::model_spatial_supports(
                                self.scope.file,
                                self.definition.declaration,
                            )
                            .map_err(|mut errors| errors.remove(0))?;
                            crate::hierarchy::parameters::frames::parameter_type(
                                self.scope.file,
                                declaration.value_type(),
                                Some(declaration.value()),
                                &frames,
                            )
                        });
                    value_type.map(|value_type| {
                        Some((
                            declaration.name(),
                            SymbolContract::Parameter(ExpressionType::new(value_type, None)),
                        ))
                    })
                }
                Item::Let(declaration) => {
                    Ok(self
                        .compile_time_values
                        .get(declaration.name())
                        .map(|value| {
                            (
                                declaration.name(),
                                SymbolContract::Parameter(ExpressionType::new(
                                    value.value_type.clone(),
                                    None,
                                )),
                            )
                        }))
                }
                Item::Clock(declaration) => Ok(Some((declaration.name(), SymbolContract::Clock))),
                Item::RelationFamily(family) => {
                    Ok(Some((family.relation().name(), SymbolContract::Relation)))
                }
                Item::Relation(declaration) => {
                    Ok(Some((declaration.name(), SymbolContract::Relation)))
                }
                Item::IndexSet(_) | Item::Instance(_) => Ok(None),
                Item::Initial(_)
                | Item::Field(_)
                | Item::Port(_)
                | Item::Connection(_)
                | Item::BoundaryConnection(_) => Ok(None),
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
        for item in self.definition.owned_items() {
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
        for item in self.definition.owned_items() {
            match item {
                Item::Field(declaration) => {
                    let support = declaration
                        .domain()
                        .and_then(|domain| self.scope.spatial_support(domain));
                    match field_expression_type(self.scope.file, declaration, support) {
                        Ok(inferred) => {
                            self.scope.symbols.insert(
                                declaration.name().to_owned(),
                                SymbolContract::Field(
                                    inferred,
                                    declaration.role(),
                                    declaration.activation().clone(),
                                ),
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
        for item in self.definition.owned_items() {
            match item {
                Item::Initial(declaration) => {
                    if let Err(errors) =
                        super::expression::validate_initial_expression(&self.scope, declaration)
                    {
                        self.diagnostics.extend(errors);
                    }
                }
                Item::Domain(declaration) => self.validate_domain(declaration),
                Item::Field(declaration) => self.validate_field(declaration),
                Item::IndexSet(_) | Item::Parameter(_) | Item::Let(_) | Item::Port(_) => {}
                Item::Instance(instance) => {
                    if let Err(error) = super::scope::validate_input_bindings(
                        &self.scope,
                        instance,
                        &mut self.connected_ports,
                        self.proof.connection_limits,
                    ) {
                        self.diagnostics.push(error);
                    }
                }
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
                Item::RelationFamily(family) => {
                    match super::indexed::extent(&self.scope, family.binder()) {
                        Ok(extent) => {
                            for ordinal in 0..extent {
                                match super::indexed::relation(self.scope.file, family, ordinal) {
                                    Ok(relation) => self.validate_relation(&relation),
                                    Err(error) => self.diagnostics.push(error),
                                }
                            }
                        }
                        Err(error) => self.diagnostics.push(error),
                    }
                }
                Item::Connection(declaration) => {
                    match super::indexed::connections(
                        &self.scope,
                        declaration,
                        &mut self.connected_ports,
                        self.proof.connection_limits,
                    ) {
                        Ok(fragments) => self.proof.physical_connection_fragments.extend(fragments),
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
                    lower
                        .fixed_value()
                        .zip(upper.fixed_value())
                        .is_some_and(|(lower, upper)| upper <= lower)
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
        if let Some(domain) = declaration.domain()
            && self.scope.spatial_support(domain).is_none()
        {
            self.diagnostics.push(unresolved(
                self.scope.file,
                declaration.range(),
                domain,
                "Field Domain",
            ));
        }
        if let eqiora_lang::ActivationSyntax::Periodic(clock) = declaration.activation()
            && !matches!(self.scope.symbols.get(clock), Some(SymbolContract::Clock))
        {
            self.diagnostics.push(unresolved(
                self.scope.file,
                declaration.range(),
                clock,
                "Field ClockDomain",
            ));
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
}
