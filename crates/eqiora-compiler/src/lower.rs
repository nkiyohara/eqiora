use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

mod binding;
mod connection;
mod domain;
mod domain_contract;
mod expression;
mod external;
#[cfg(test)]
mod tests;
use binding::{
    Binding, DomainContract, FieldContract, PortContract, ResolvedPortContract, bind_domain,
    bind_port, insert_binding, resolve_field_contract, resolve_port_contract,
};
use connection::{lower_connection, prepare_flat_physical_connections};
pub(crate) use domain_contract::{LoweringDomainContract, LoweringPortContract};
use expression::{TypedExpression, lower_relation};

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id, OntologyId, RawId, ValueShape};
use eqiora_graph::{EdgeKind, Op, Transaction};
use eqiora_lang::{
    ActivationSyntax, BinaryOp, BoundarySideSyntax, ConnectionSyntax, DomainSyntax, Expr, ExprKind,
    Item, ModelDecl, ModelDraft, PortSyntax, RepresentationSyntax, SignalDirectionSyntax,
    TextRange, UnaryOp, ValueShapeSyntax,
};
use eqiora_schema::kernel::pure_operator::PureOperatorDefinition;
use eqiora_schema::kernel::scalar_connection::{
    ScalarConnectionKind, ScalarConnectionViolation, ScalarPortContract, validate_scalar_connection,
};
use eqiora_schema::kernel::{
    ActivationDef, BoundaryPhysicalConnector, BoundarySide, ClockDomainDef, ConnectionDef,
    ConnectionSemantics, DomainDef, ExprDag, ExprDagBuilder, ExprId, FieldDef, KernelNode,
    ParameterDef, PortDef, RationalTime, RelationDef, RepresentationDef, SignalDirection,
    SymbolRef, UnaryMathFunction, ValueFrame,
};
use eqiora_schema::{Model, ModelView};

use crate::connection_sets::{ConnectionFragment, ConnectionSetLimits, normalize_connection_sets};
use crate::diagnostics::{native_diagnostic, source_error};
use crate::dimensions::{dimension_overflow, length_dimension, lower_dimension, time_dimension};
use crate::formulation::CompiledAuthoredFormulation;
use crate::projection::PhysicalExposureProjectionMap;
use crate::provenance::ProvenanceMap;

/// Source-name to Semantic Kernel ID map produced with one compiled model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSymbols {
    symbols: BTreeMap<String, RawId>,
}

impl ModelSymbols {
    /// Resolve one source declaration name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<RawId> {
        self.symbols.get(name).copied()
    }

    /// Names and IDs in deterministic lexical order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, RawId)> {
        self.symbols.iter().map(|(name, id)| (name.as_str(), *id))
    }

    pub(crate) fn from_map(symbols: BTreeMap<String, RawId>) -> Self {
        Self { symbols }
    }
}

/// One typed model transaction ready for atomic Graph Federation commit.
#[derive(Debug)]
pub struct CompiledModel {
    model: OntologyId<Model>,
    transaction: Transaction,
    symbols: ModelSymbols,
    provenance: Option<ProvenanceMap>,
    physical_exposures: PhysicalExposureProjectionMap,
    authored_formulations: Vec<CompiledAuthoredFormulation>,
}

impl CompiledModel {
    /// Typed Standard Ontology ModelView identifier.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Resolved declaration IDs.
    #[must_use]
    pub const fn symbols(&self) -> &ModelSymbols {
        &self.symbols
    }

    /// Borrow the typed transaction before commit.
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    /// Source provenance for deterministic hierarchy elaboration.
    ///
    /// Legacy flat lowering has no elaboration sidecar and returns `None`.
    #[must_use]
    pub const fn provenance(&self) -> Option<&ProvenanceMap> {
        self.provenance.as_ref()
    }

    /// Query projections for public physical Ports eliminated by hierarchy
    /// normalization.
    ///
    /// The catalog is a compiler sidecar. It adds no Kernel node, equation,
    /// alias, or source symbol to the canonical model.
    #[must_use]
    pub const fn physical_exposures(&self) -> &PhysicalExposureProjectionMap {
        &self.physical_exposures
    }

    /// Typed authored mathematics retained only by fresh source compilation.
    #[must_use]
    pub fn authored_formulations(
        &self,
    ) -> impl ExactSizeIterator<Item = &CompiledAuthoredFormulation> {
        self.authored_formulations.iter()
    }

    /// Consume into the transaction, model ID, and source symbol map.
    #[must_use]
    pub fn into_parts(self) -> (Transaction, OntologyId<Model>, ModelSymbols) {
        (self.transaction, self.model, self.symbols)
    }

    pub(crate) fn with_elaboration_metadata(
        mut self,
        symbols: ModelSymbols,
        provenance: ProvenanceMap,
        physical_exposures: PhysicalExposureProjectionMap,
    ) -> Self {
        self.symbols = symbols;
        self.provenance = Some(provenance);
        self.physical_exposures = physical_exposures;
        self
    }

    pub(crate) fn with_authored_formulations(
        mut self,
        formulations: Vec<CompiledAuthoredFormulation>,
    ) -> Self {
        self.authored_formulations = formulations;
        self
    }
}

/// Type-lower one client-neutral native model draft.
///
/// Native declarations bypass parsing, but intentionally do not bypass name,
/// dimension, activation, expression-DAG, or transaction lowering. Synthetic
/// ranges are removed from diagnostics and replaced with stable declaration
/// paths; they never pretend to be source locations.
///
/// # Errors
/// Returns graph-path diagnostics for invalid native declarations. No partial
/// transaction is returned.
pub fn lower_draft(draft: &ModelDraft) -> Result<CompiledModel, Vec<Diagnostic>> {
    let native = draft.native_ast();
    lower_model("<native>", native.model()).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| native_diagnostic(draft, &native, diagnostic))
            .collect()
    })
}

/// Resolve and lower one parsed model declaration.
///
/// Graph IDs are fresh in v0. Persistent source-anchor identity is deliberately
/// a later incremental-compiler contract rather than a hash hidden here.
///
/// # Errors
/// Returns source-spanned name, dimension, clock, connection, or DAG
/// diagnostics. No partial transaction is returned.
pub fn lower_model(file: &str, model: &ModelDecl) -> Result<CompiledModel, Vec<Diagnostic>> {
    lower_model_with_identities(file, model, &mut FreshLoweringIdentities)
}

/// Compiler-owned declaration form consumed by Kernel lowering.
///
/// It is the sole entry shape for the typed transaction lowerer.
/// Parsed source and hierarchy elaboration both enter lowering through this
/// exact form. In particular, hierarchy-only boundary-physical contracts are
/// represented directly rather than disguised as source-language scalar
/// physical declarations.
#[derive(Debug, Clone)]
pub(crate) struct LoweringModel {
    pub(crate) name: String,
    pub(crate) range: TextRange,
    pub(crate) items: Vec<LoweringItem>,
}

/// Compiler-owned, typed scalar expression consumed by Kernel lowering.
///
/// Source expressions enter through [`Self::from_source`]. Hierarchy
/// elaboration may additionally substitute dimensioned constants and shared
/// Parameter-expression DAGs without fabricating source declarations.
#[derive(Debug, Clone)]
pub(crate) struct LoweringExpression {
    node: Arc<LoweringExpressionNode>,
    range: TextRange,
}

impl PartialEq for LoweringExpression {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
    }
}

#[derive(Debug, PartialEq)]
enum LoweringExpressionNode {
    Quantity(DynQuantity),
    Name(String),
    Neg(LoweringExpression),
    Binary {
        operator: BinaryOp,
        left: LoweringExpression,
        right: LoweringExpression,
    },
    Call {
        callee: String,
        argument: LoweringExpression,
    },
    PureOperator {
        definition: PureOperatorDefinition,
        arguments: Vec<LoweringExpression>,
    },
    UnknownMath(String),
    InvalidUnit(&'static str),
    Unsupported,
}

impl LoweringExpression {
    pub(crate) fn from_source(expression: &Expr) -> Self {
        expression::from_source(expression)
    }

    pub(crate) fn quantity(value: DynQuantity, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Quantity(DynQuantity::new(
                normalize_zero(value.value()),
                value.dim(),
            ))),
            range,
        }
    }

    pub(crate) fn name(name: String, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Name(name)),
            range,
        }
    }

    pub(crate) fn neg(value: Self, range: TextRange) -> Self {
        if let LoweringExpressionNode::Quantity(quantity) = value.node.as_ref()
            && quantity.value() == 0.0
        {
            return Self::quantity(DynQuantity::new(0.0, quantity.dim()), range);
        }
        Self {
            node: Arc::new(LoweringExpressionNode::Neg(value)),
            range,
        }
    }

    pub(crate) fn binary(operator: BinaryOp, left: Self, right: Self, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            }),
            range,
        }
    }

    pub(crate) fn call(callee: String, argument: Self, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Call { callee, argument }),
            range,
        }
    }

    pub(crate) fn pure_operator(
        definition: PureOperatorDefinition,
        arguments: Vec<Self>,
        range: TextRange,
    ) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            }),
            range,
        }
    }

    pub(crate) fn with_quantity_dimension(&self, dimension: DimExponents) -> Self {
        match self.node.as_ref() {
            LoweringExpressionNode::Quantity(value) => {
                Self::quantity(DynQuantity::new(value.value(), dimension), self.range)
            }
            _ => self.clone(),
        }
    }

    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }

    #[cfg(test)]
    pub(crate) fn name_value(&self) -> Option<&str> {
        match self.node.as_ref() {
            LoweringExpressionNode::Name(name) => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum LoweringItem {
    Domain {
        name: String,
        contract: LoweringDomainContract,
        range: TextRange,
    },
    Representation {
        name: String,
        syntax: RepresentationSyntax,
        range: TextRange,
    },
    Field {
        name: String,
        domain: Option<String>,
        representation: Option<String>,
        shape: Option<ValueShapeSyntax>,
        dimension: Expr,
        initial: Option<f64>,
        range: TextRange,
    },
    Parameter {
        name: String,
        dimension: Expr,
        value: f64,
        range: TextRange,
    },
    Port {
        name: String,
        contract: LoweringPortContract,
        range: TextRange,
    },
    Clock {
        name: String,
        period: eqiora_lang::RationalSyntax,
        phase: eqiora_lang::RationalSyntax,
        range: TextRange,
    },
    Relation {
        name: String,
        activation: ActivationSyntax,
        domain: Option<String>,
        residuals: Vec<LoweringExpression>,
        range: TextRange,
    },
    Connection {
        syntax: ConnectionSyntax,
        ports: Vec<String>,
        range: TextRange,
    },
    Boundary {
        ports: Vec<String>,
        range: TextRange,
    },
    Unsupported {
        range: TextRange,
    },
}

mod source;
/// Identity source for one completely staged lowering.
///
/// Supplies collision-checked hierarchical identities or fresh flat identities.
pub(crate) trait LoweringIdentities {
    fn model(&mut self, name: &str) -> OntologyId<Model>;

    fn domain(&mut self, name: &str) -> Id<kinds::Domain>;

    fn representation(&mut self, name: &str) -> Id<kinds::Representation>;

    fn field(&mut self, name: &str) -> Id<kinds::Field>;

    fn parameter(&mut self, name: &str) -> Id<kinds::Parameter>;

    fn port(&mut self, name: &str) -> Id<kinds::Port>;

    fn clock(&mut self, name: &str) -> Id<kinds::ClockDomain>;

    fn relation(&mut self, name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>);

    fn connection(&mut self) -> Id<kinds::Connection>;
}

struct FreshLoweringIdentities;

impl LoweringIdentities for FreshLoweringIdentities {
    fn model(&mut self, _name: &str) -> OntologyId<Model> {
        OntologyId::new()
    }

    fn domain(&mut self, _name: &str) -> Id<kinds::Domain> {
        Id::new()
    }

    fn representation(&mut self, _name: &str) -> Id<kinds::Representation> {
        Id::new()
    }

    fn field(&mut self, _name: &str) -> Id<kinds::Field> {
        Id::new()
    }

    fn parameter(&mut self, _name: &str) -> Id<kinds::Parameter> {
        Id::new()
    }

    fn port(&mut self, _name: &str) -> Id<kinds::Port> {
        Id::new()
    }

    fn clock(&mut self, _name: &str) -> Id<kinds::ClockDomain> {
        Id::new()
    }

    fn relation(&mut self, _name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>) {
        (Id::new(), Id::new())
    }

    fn connection(&mut self) -> Id<kinds::Connection> {
        Id::new()
    }
}

pub(crate) fn lower_model_with_identities(
    file: &str,
    model: &ModelDecl,
    identities: &mut impl LoweringIdentities,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    lower_typed_model(
        file,
        &LoweringModel::from_source(file, model).map_err(|error| vec![error])?,
        identities,
    )
}

pub(crate) fn lower_typed_model(
    file: &str,
    model: &LoweringModel,
    identities: &mut impl LoweringIdentities,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let mut bindings = BTreeMap::new();
    let mut diagnostics = crate::math::model_name_diagnostics(file, &model.name, model.range);

    for item in &model.items {
        match item {
            LoweringItem::Domain {
                name,
                contract,
                range,
            } => {
                let contract = match contract {
                    LoweringDomainContract::Source(syntax) => bind_domain(file, *range, syntax),
                    LoweringDomainContract::ExternalGeometryRegion { dimensions, .. } => {
                        Ok(DomainContract::Spatial {
                            dimensions: Some(*dimensions),
                        })
                    }
                    LoweringDomainContract::ExternalGeometryBoundary { .. } => {
                        Ok(DomainContract::Spatial { dimensions: None })
                    }
                    LoweringDomainContract::BoundaryPhysical(contract) => {
                        Ok(DomainContract::BoundaryPhysical(contract.clone()))
                    }
                };
                match contract {
                    Ok(contract) => insert_binding(
                        file,
                        &mut bindings,
                        name,
                        Binding::Domain(identities.domain(name), contract),
                        *range,
                        &mut diagnostics,
                    ),
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            LoweringItem::Representation { name, range, .. } => insert_binding(
                file,
                &mut bindings,
                name,
                Binding::Representation(identities.representation(name)),
                *range,
                &mut diagnostics,
            ),
            LoweringItem::Field {
                name,
                domain,
                shape,
                dimension,
                range,
                ..
            } => match lower_dimension(file, dimension) {
                Ok(dimension) => insert_binding(
                    file,
                    &mut bindings,
                    name,
                    Binding::Field(
                        identities.field(name),
                        FieldContract {
                            dimension,
                            shape: shape.clone(),
                            domain: domain.clone(),
                        },
                    ),
                    *range,
                    &mut diagnostics,
                ),
                Err(diagnostic) => diagnostics.push(diagnostic),
            },
            LoweringItem::Parameter {
                name,
                dimension,
                range,
                ..
            } => match lower_dimension(file, dimension) {
                Ok(dimension) => insert_binding(
                    file,
                    &mut bindings,
                    name,
                    Binding::Parameter(identities.parameter(name), dimension),
                    *range,
                    &mut diagnostics,
                ),
                Err(diagnostic) => diagnostics.push(diagnostic),
            },
            LoweringItem::Port {
                name,
                contract,
                range,
            } => {
                let contract = match contract {
                    LoweringPortContract::Source(syntax) => bind_port(file, *range, syntax),
                    LoweringPortContract::BoundaryPhysical {
                        connector,
                        boundary,
                    } => Ok(PortContract::BoundaryPhysical {
                        connector: connector.clone(),
                        boundary: boundary.clone(),
                    }),
                };
                match contract {
                    Ok(contract) => insert_binding(
                        file,
                        &mut bindings,
                        name,
                        Binding::Port(identities.port(name), contract),
                        *range,
                        &mut diagnostics,
                    ),
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
            LoweringItem::Clock { name, range, .. } => insert_binding(
                file,
                &mut bindings,
                name,
                Binding::Clock(identities.clock(name)),
                *range,
                &mut diagnostics,
            ),
            LoweringItem::Relation { name, range, .. } => {
                let (relation, activation) = identities.relation(name);
                insert_binding(
                    file,
                    &mut bindings,
                    name,
                    Binding::Relation {
                        relation,
                        activation,
                    },
                    *range,
                    &mut diagnostics,
                );
            }
            LoweringItem::Connection { .. } | LoweringItem::Boundary { .. } => {}
            LoweringItem::Unsupported { range } => diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                *range,
                "model item is newer than this compiler",
            )),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let model_id = identities.model(&model.name);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut boundary = BTreeSet::new();
    let mut connected_ports = BTreeSet::new();
    let physical_connections = prepare_flat_physical_connections(file, model, &bindings)?;

    for (item_index, item) in model.items.iter().enumerate() {
        let result = match item {
            LoweringItem::Domain {
                name,
                contract: lowering_contract,
                range,
            } => {
                let Binding::Domain(id, contract) = bindings[name].clone() else {
                    unreachable!("first pass assigns Domain bindings");
                };
                domain::lower_domain(file, *range, id, contract, lowering_contract, &bindings).map(
                    |(definition, parent, dependencies)| {
                        nodes.push(definition.into());
                        if let Some(parent) = parent {
                            edges.push((id.erase(), parent, EdgeKind::BoundaryOf));
                        }
                        for dependency in dependencies {
                            edges.push((id.erase(), dependency, EdgeKind::DependsOn));
                        }
                    },
                )
            }
            LoweringItem::Representation {
                name,
                syntax,
                range,
            } => {
                let Binding::Representation(id) = bindings[name].clone() else {
                    unreachable!("first pass assigns Representation bindings");
                };
                match syntax {
                    RepresentationSyntax::Continuum => {
                        nodes.push(RepresentationDef::continuum(id).into());
                        Ok(())
                    }
                    _ => Err(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        file,
                        *range,
                        "Representation syntax is newer than this compiler",
                    )),
                }
            }
            LoweringItem::Field {
                name,
                domain,
                representation,
                initial,
                range,
                ..
            } => {
                let Binding::Field(id, contract) = bindings[name].clone() else {
                    unreachable!("first pass assigns Field bindings");
                };
                resolve_field_contract(file, *range, &contract, &bindings)
                    .and_then(|resolved| {
                        let definition = match (resolved.shape.is_scalar(), *initial) {
                            (true, Some(initial)) => FieldDef::new(id, resolved.dimension)
                                .with_initial(DynQuantity::new(
                                    normalize_zero(initial),
                                    resolved.dimension,
                                )),
                            (true, None) => Ok(FieldDef::new(id, resolved.dimension)),
                            (false, None) => FieldDef::shaped(
                                id,
                                resolved.dimension,
                                resolved.shape,
                                resolved.frame,
                            ),
                            (false, Some(_)) => Err(source_error(
                                codes::LANGUAGE_TYPE_ERROR,
                                file,
                                *range,
                                "non-scalar Field cannot receive a scalar initial value",
                            )),
                        }?;
                        Ok(definition)
                    })
                    .and_then(|definition| {
                        nodes.push(definition.into());
                        match (domain.as_deref(), representation.as_deref()) {
                            (None, None) => Ok(()),
                            (Some(domain), Some(representation)) => {
                                let Some(domain_binding) = bindings.get(domain) else {
                                    return Err(unresolved(file, *range, domain, "Field Domain"));
                                };
                                let domain = match domain_binding {
                                    Binding::Domain(id, DomainContract::Spatial { .. }) => *id,
                                    Binding::Domain(
                                        _,
                                        DomainContract::ScalarPhysical { .. },
                                    ) => {
                                        return Err(source_error(
                                            codes::LANGUAGE_TYPE_ERROR,
                                            file,
                                            *range,
                                            "spatial Field cannot be defined on a scalar physical Domain",
                                        ));
                                    }
                                    _ => {
                                        return Err(unresolved(
                                            file,
                                            *range,
                                            domain,
                                            "Field Domain",
                                        ));
                                    }
                                };
                                let Some(Binding::Representation(representation)) =
                                    bindings.get(representation).cloned()
                                else {
                                    return Err(unresolved(
                                        file,
                                        *range,
                                        representation,
                                        "Field Representation",
                                    ));
                                };
                                edges.push((id.erase(), domain.erase(), EdgeKind::DefinedOn));
                                edges.push((
                                    id.erase(),
                                    representation.erase(),
                                    EdgeKind::DefinedOn,
                                ));
                                Ok(())
                            }
                            _ => Err(source_error(
                                codes::LANGUAGE_TYPE_ERROR,
                                file,
                                *range,
                                "spatial Field requires both `on Domain` and `as Representation`",
                            )),
                        }
                    })
            }
            LoweringItem::Parameter { name, value, .. } => {
                let Binding::Parameter(id, dimension) = bindings[name].clone() else {
                    unreachable!("first pass assigns Parameter bindings");
                };
                nodes.push(
                    ParameterDef::new(id, DynQuantity::new(normalize_zero(*value), dimension))
                        .into(),
                );
                Ok(())
            }
            LoweringItem::Port { name, range, .. } => {
                let Binding::Port(id, contract) = bindings[name].clone() else {
                    unreachable!("first pass assigns Port bindings");
                };
                match lower_port(file, *range, id, &contract, &bindings) {
                    Ok(port) => {
                        nodes.push(port.into());
                        Ok(())
                    }
                    Err(diagnostic) => Err(diagnostic),
                }
            }
            LoweringItem::Clock {
                name,
                period,
                phase,
                range,
            } => {
                let Binding::Clock(id) = bindings[name].clone() else {
                    unreachable!("first pass assigns Clock bindings");
                };
                lower_clock(*period, *phase)
                    .and_then(|(period, phase)| ClockDomainDef::periodic(id, period, phase))
                    .map(|definition| nodes.push(definition.into()))
                    .map_err(|diagnostic| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            *range,
                            diagnostic.message(),
                        )
                    })
            }
            LoweringItem::Relation {
                name,
                activation,
                domain,
                residuals,
                range,
            } => lower_relation(
                file,
                *range,
                activation,
                domain.as_deref(),
                residuals,
                &bindings,
            )
            .map(|lowered| {
                let Binding::Relation {
                    relation,
                    activation: activation_id,
                } = bindings[name].clone()
                else {
                    unreachable!("first pass assigns Relation bindings");
                };
                nodes.push(RelationDef::new(relation, lowered.residuals).into());
                let activation_definition = match activation {
                    ActivationSyntax::Continuous => ActivationDef::continuous(activation_id),
                    ActivationSyntax::Periodic(_) => ActivationDef::periodic(activation_id),
                    _ => unreachable!("unsupported Activation was diagnosed"),
                };
                nodes.push(activation_definition.into());
                for dependency in lowered.dependencies {
                    edges.push((relation.erase(), dependency, EdgeKind::DependsOn));
                }
                for port in lowered.ports {
                    edges.push((relation.erase(), port, EdgeKind::HasPort));
                }
                edges.push((activation_id.erase(), relation.erase(), EdgeKind::Activates));
                if let Some(domain_name) = domain {
                    let Binding::Domain(domain, _) = bindings[domain_name].clone() else {
                        unreachable!("Relation Domain was resolved while lowering");
                    };
                    edges.push((relation.erase(), domain.erase(), EdgeKind::AppliesOn));
                }
                if let ActivationSyntax::Periodic(clock_name) = activation {
                    let Binding::Clock(clock) = bindings[clock_name].clone() else {
                        unreachable!("periodic clock was resolved while lowering");
                    };
                    edges.push((activation_id.erase(), clock.erase(), EdgeKind::ClockedBy));
                }
            }),
            LoweringItem::Connection {
                syntax,
                ports,
                range,
            } => {
                if let Some(ports) = physical_connections.emissions.get(&item_index) {
                    let definition = ConnectionDef::new(
                        identities.connection(),
                        ConnectionSemantics::Conserving,
                    );
                    let connection = definition.id().erase();
                    nodes.push(definition.into());
                    for port in ports {
                        edges.push((connection, *port, EdgeKind::Connects));
                    }
                    Ok(())
                } else if physical_connections.consumed.contains(&item_index) {
                    Ok(())
                } else {
                    let connection = identities.connection();
                    lower_connection(
                        file,
                        *range,
                        *syntax,
                        ports,
                        connection,
                        &bindings,
                        &mut connected_ports,
                    )
                    .map(|(definition, ports)| {
                        let connection = definition.id().erase();
                        nodes.push(definition.into());
                        for port in ports {
                            edges.push((connection, port, EdgeKind::Connects));
                        }
                    })
                }
            }
            LoweringItem::Boundary { ports, range } => {
                for name in ports {
                    match bindings.get(name).cloned() {
                        Some(Binding::Port(id, _)) => {
                            boundary.insert(id.erase());
                        }
                        Some(_) => diagnostics.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            *range,
                            format!("boundary name `{name}` is not a Port"),
                        )),
                        None => diagnostics.push(unresolved(file, *range, name, "boundary Port")),
                    }
                }
                Ok(())
            }
            LoweringItem::Unsupported { range } => Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                *range,
                "model item is newer than this compiler",
            )),
        };
        if let Err(diagnostic) = result {
            diagnostics.push(diagnostic);
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let members = nodes.iter().map(KernelNode::id).collect::<BTreeSet<_>>();
    let view = ModelView::new(model_id, members, boundary).map_err(|diagnostic| {
        vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            model.range,
            diagnostic.message(),
        )]
    })?;
    let mut transaction = Transaction::new(format!("compile model {}", model.name));
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in edges {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });

    let symbols = bindings
        .into_iter()
        .map(|(name, binding)| (name, binding.primary_id()))
        .collect();
    Ok(CompiledModel {
        model: model_id,
        transaction,
        symbols: ModelSymbols::from_map(symbols),
        provenance: None,
        physical_exposures: PhysicalExposureProjectionMap::default(),
        authored_formulations: Vec::new(),
    })
}

fn lower_port(
    file: &str,
    range: TextRange,
    id: Id<kinds::Port>,
    contract: &PortContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<PortDef, Diagnostic> {
    match resolve_port_contract(file, range, contract, bindings)? {
        ResolvedPortContract::Signal {
            direction: SignalDirectionSyntax::Input,
            dimension,
        } => Ok(PortDef::signal(id, SignalDirection::Input, dimension)),
        ResolvedPortContract::Signal {
            direction: SignalDirectionSyntax::Output,
            dimension,
        } => Ok(PortDef::signal(id, SignalDirection::Output, dimension)),
        ResolvedPortContract::ConservingMarker { dimension } => {
            Ok(PortDef::conserving_marker(id, dimension))
        }
        ResolvedPortContract::ScalarPhysical { domain, .. } => {
            Ok(PortDef::scalar_physical(id, domain))
        }
        ResolvedPortContract::BoundaryPhysical {
            connector,
            boundary,
            ..
        } => Ok(PortDef::boundary_physical(id, connector, boundary)),
    }
}

fn lower_clock(
    period: eqiora_lang::RationalSyntax,
    phase: eqiora_lang::RationalSyntax,
) -> Result<(RationalTime, RationalTime), Diagnostic> {
    Ok((
        RationalTime::new(period.numerator(), period.denominator())?,
        RationalTime::new(phase.numerator(), phase.denominator())?,
    ))
}

fn lowering_integer_literal(expression: &LoweringExpression) -> Option<i32> {
    let value = match expression.node.as_ref() {
        LoweringExpressionNode::Quantity(value) if value.dim() == DimExponents::DIMENSIONLESS => {
            value.value()
        }
        LoweringExpressionNode::Neg(value) => match value.node.as_ref() {
            LoweringExpressionNode::Quantity(value)
                if value.dim() == DimExponents::DIMENSIONLESS =>
            {
                -value.value()
            }
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn instantiate_pure_dimension(
    definition: &PureOperatorDefinition,
    arguments: &[TypedExpression],
) -> Option<DimExponents> {
    if arguments.len() != definition.formals().len() {
        return None;
    }
    arguments
        .iter()
        .zip(definition.dimension_monomial().exponents())
        .try_fold(
            DimExponents::DIMENSIONLESS,
            |result, (argument, exponent)| {
                let term = argument.dimension.pow(i32::from(*exponent), 1)?;
                result.mul(term)
            },
        )
}

fn unresolved(file: &str, range: TextRange, name: &str, expected: &str) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        format!("unresolved {expected} `{name}`"),
    )
}
