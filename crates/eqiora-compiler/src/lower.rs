use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

mod domain;
mod domain_contract;
mod external;
pub(crate) use domain_contract::{LoweringDomainContract, LoweringPortContract};

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id, OntologyId, RawId, ValueShape};
use eqiora_graph::{EdgeKind, Op, Transaction};
use eqiora_lang::{
    ActivationSyntax, BinaryOp, BoundarySideSyntax, ConnectionSyntax, DomainSyntax, Expr, ExprKind,
    Item, ModelDecl, ModelDraft, PortSyntax, RepresentationSyntax, SignalDirectionSyntax,
    TextRange, UnaryOp, ValueShapeSyntax, parse,
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
use crate::dimensions::{
    checked_dimensions, checked_scale_dimension, dimension_overflow, length_dimension,
    lower_dimension, time_dimension,
};
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
}

/// Parse and type-lower every model in one source file.
///
/// # Errors
/// Returns accumulated parser diagnostics, or type/lowering diagnostics from
/// every model that could be independently checked.
pub fn compile(file: &str, source: &str) -> Result<Vec<CompiledModel>, Vec<Diagnostic>> {
    let document = parse(file, source).into_compilation_document()?;
    let has_hierarchy = !document.connectors().is_empty()
        || !document.components().is_empty()
        || !document.pure_operators().is_empty()
        || document.models().iter().any(|model| {
            model
                .items()
                .iter()
                .any(|item| matches!(item, Item::Instance(_)))
        });
    if has_hierarchy {
        return crate::hierarchy::compile_hierarchy(file, source.len(), &document);
    }
    let mut compiled = Vec::new();
    let mut diagnostics = Vec::new();
    for model in document.models() {
        match lower_model(file, model) {
            Ok(value) => compiled.push(value),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(compiled)
    } else {
        Err(diagnostics)
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
    Unsupported,
}

impl LoweringExpression {
    pub(crate) fn from_source(expression: &Expr) -> Self {
        let kind = match expression.kind() {
            ExprKind::Number(value) => LoweringExpressionNode::Quantity(DynQuantity::new(
                normalize_zero(*value),
                DimExponents::DIMENSIONLESS,
            )),
            ExprKind::Name(name) => LoweringExpressionNode::Name(name.clone()),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => return Self::neg(Self::from_source(value), expression.range()),
            ExprKind::Binary { op, left, right } => LoweringExpressionNode::Binary {
                operator: *op,
                left: Self::from_source(left),
                right: Self::from_source(right),
            },
            ExprKind::Call { callee, arguments }
                if !callee.is_qualified() && arguments.len() == 1 =>
            {
                LoweringExpressionNode::Call {
                    callee: callee.as_str().to_owned(),
                    argument: Self::from_source(&arguments[0]),
                }
            }
            _ => LoweringExpressionNode::Unsupported,
        };
        Self {
            node: Arc::new(kind),
            range: expression.range(),
        }
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

    pub(crate) fn collect_physical_port_names(&self, names: &mut BTreeSet<String>) -> bool {
        match self.node.as_ref() {
            LoweringExpressionNode::Call { callee, argument }
                if matches!(callee.as_str(), "across" | "through" | "trace" | "flux") =>
            {
                if let LoweringExpressionNode::Name(name) = argument.node.as_ref() {
                    names.insert(name.clone());
                }
                argument.collect_physical_port_names(names)
            }
            LoweringExpressionNode::Neg(value) => value.collect_physical_port_names(names),
            LoweringExpressionNode::Binary { left, right, .. } => {
                left.collect_physical_port_names(names) && right.collect_physical_port_names(names)
            }
            LoweringExpressionNode::Call { argument, .. } => {
                argument.collect_physical_port_names(names)
            }
            LoweringExpressionNode::PureOperator { arguments, .. } => arguments
                .iter()
                .all(|argument| argument.collect_physical_port_names(names)),
            LoweringExpressionNode::Quantity(_) | LoweringExpressionNode::Name(_) => true,
            LoweringExpressionNode::Unsupported => false,
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

impl LoweringModel {
    fn from_source(model: &ModelDecl) -> Self {
        let items = model
            .items()
            .iter()
            .map(|item| match item {
                Item::Domain(declaration) => LoweringItem::Domain {
                    name: declaration.name().to_owned(),
                    contract: LoweringDomainContract::Source(declaration.syntax().clone()),
                    range: declaration.range(),
                },
                Item::Representation(declaration) => LoweringItem::Representation {
                    name: declaration.name().to_owned(),
                    syntax: declaration.syntax(),
                    range: declaration.range(),
                },
                Item::Field(declaration) => LoweringItem::Field {
                    name: declaration.name().to_owned(),
                    domain: declaration.domain().map(str::to_owned),
                    representation: declaration.representation().map(str::to_owned),
                    shape: declaration.shape().cloned(),
                    dimension: declaration.dimension().clone(),
                    initial: declaration.initial(),
                    range: declaration.range(),
                },
                Item::Parameter(declaration) => LoweringItem::Parameter {
                    name: declaration.name().to_owned(),
                    dimension: declaration.dimension().clone(),
                    value: declaration.initial(),
                    range: declaration.range(),
                },
                Item::Port(declaration) => LoweringItem::Port {
                    name: declaration.name().to_owned(),
                    contract: LoweringPortContract::Source(declaration.syntax().clone()),
                    range: declaration.range(),
                },
                Item::Clock(declaration) => LoweringItem::Clock {
                    name: declaration.name().to_owned(),
                    period: declaration.period(),
                    phase: declaration.phase(),
                    range: declaration.range(),
                },
                Item::Relation(declaration) => LoweringItem::Relation {
                    name: declaration.name().to_owned(),
                    activation: declaration.activation().clone(),
                    domain: declaration.domain().map(str::to_owned),
                    residuals: declaration
                        .residuals()
                        .iter()
                        .map(LoweringExpression::from_source)
                        .collect(),
                    range: declaration.range(),
                },
                Item::Connection(declaration) => LoweringItem::Connection {
                    syntax: declaration.syntax(),
                    ports: declaration.ports().map(str::to_owned).collect(),
                    range: declaration.range(),
                },
                Item::Boundary(declaration) => LoweringItem::Boundary {
                    ports: declaration.ports().map(str::to_owned).collect(),
                    range: declaration.range(),
                },
                _ => LoweringItem::Unsupported {
                    range: model.range(),
                },
            })
            .collect();
        Self {
            name: model.name().to_owned(),
            range: model.range(),
            items,
        }
    }
}

/// Identity source for one completely staged lowering.
///
/// Hierarchical elaboration implements this seam only after every full
/// identity and its Kernel projection has been checked for collision. The
/// legacy flat path deliberately remains fresh and non-persistent.
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
    lower_typed_model(file, &LoweringModel::from_source(model), identities)
}

pub(crate) fn lower_typed_model(
    file: &str,
    model: &LoweringModel,
    identities: &mut impl LoweringIdentities,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let mut bindings = BTreeMap::new();
    let mut diagnostics = Vec::new();

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
                            (false, None) => FieldDef::shaped(
                                id,
                                resolved.dimension,
                                resolved.shape,
                                resolved.frame,
                            ),
                            (true, None) => Err(source_error(
                                codes::LANGUAGE_TYPE_ERROR,
                                file,
                                *range,
                                "scalar Field requires one scalar initial value",
                            )),
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
    })
}

#[derive(Debug, Clone)]
enum Binding {
    Domain(Id<kinds::Domain>, DomainContract),
    Representation(Id<kinds::Representation>),
    Field(Id<kinds::Field>, FieldContract),
    Parameter(Id<kinds::Parameter>, DimExponents),
    Port(Id<kinds::Port>, PortContract),
    Clock(Id<kinds::ClockDomain>),
    Relation {
        relation: Id<kinds::Relation>,
        activation: Id<kinds::Activation>,
    },
}

impl Binding {
    fn primary_id(&self) -> RawId {
        match self {
            Self::Domain(id, _) => id.erase(),
            Self::Representation(id) => id.erase(),
            Self::Field(id, _) => id.erase(),
            Self::Parameter(id, _) => id.erase(),
            Self::Port(id, _) => id.erase(),
            Self::Clock(id) => id.erase(),
            Self::Relation { relation, .. } => relation.erase(),
        }
    }
}

#[derive(Debug, Clone)]
enum DomainContract {
    Spatial {
        dimensions: Option<usize>,
    },
    ScalarPhysical {
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    BoundaryPhysical(BoundaryPhysicalConnector),
}

#[derive(Debug, Clone)]
struct FieldContract {
    dimension: DimExponents,
    shape: Option<ValueShapeSyntax>,
    domain: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedFieldContract {
    dimension: DimExponents,
    shape: ValueShape,
    frame: ValueFrame,
}

#[derive(Debug, Clone)]
enum PortContract {
    Signal {
        direction: SignalDirectionSyntax,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    ScalarPhysical {
        domain: String,
    },
    BoundaryPhysical {
        connector: String,
        boundary: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedPortContract {
    Signal {
        direction: SignalDirectionSyntax,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    ScalarPhysical {
        domain: Id<kinds::Domain>,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    BoundaryPhysical {
        connector: Id<kinds::Domain>,
        boundary: Id<kinds::Domain>,
        trace_dimension: DimExponents,
        flux_dimension: DimExponents,
    },
}

fn bind_domain(
    file: &str,
    range: TextRange,
    syntax: &DomainSyntax,
) -> Result<DomainContract, Diagnostic> {
    match syntax {
        DomainSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => Ok(DomainContract::ScalarPhysical {
            across_dimension: lower_dimension(file, across_dimension)?,
            through_dimension: lower_dimension(file, through_dimension)?,
        }),
        DomainSyntax::CartesianBox(bounds) => Ok(DomainContract::Spatial {
            dimensions: Some(bounds.len()),
        }),
        DomainSyntax::Boundary { .. } => Ok(DomainContract::Spatial { dimensions: None }),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Domain syntax is newer than this compiler",
        )),
    }
}

fn resolve_field_contract(
    file: &str,
    range: TextRange,
    contract: &FieldContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<ResolvedFieldContract, Diagnostic> {
    let (shape, frame) = match contract.shape.as_ref() {
        None | Some(ValueShapeSyntax::Scalar) => (ValueShape::scalar(), ValueFrame::Invariant),
        Some(ValueShapeSyntax::Exact(extents)) => (
            ValueShape::new(extents.iter().copied()).map_err(|error| {
                source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
            })?,
            ValueFrame::Invariant,
        ),
        Some(ValueShapeSyntax::SpatialVector) => {
            let Some(domain) = contract.domain.as_deref() else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "`spatial_vector` Field shape requires an exact spatial Domain",
                ));
            };
            let Some(Binding::Domain(
                _,
                DomainContract::Spatial {
                    dimensions: Some(dimensions),
                },
            )) = bindings.get(domain)
            else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!(
                        "`spatial_vector` Field support `{domain}` has no exact ambient dimension"
                    ),
                ));
            };
            let extent = u32::try_from(*dimensions).map_err(|_| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "spatial ambient dimension exceeds the portable u32 shape range",
                )
            })?;
            (
                ValueShape::new([extent]).map_err(|error| {
                    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
                })?,
                ValueFrame::SpatialCartesian,
            )
        }
        Some(_) => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "Field value shape is newer than this compiler",
            ));
        }
    };
    Ok(ResolvedFieldContract {
        dimension: contract.dimension,
        shape,
        frame,
    })
}

fn bind_port(
    file: &str,
    range: TextRange,
    syntax: &PortSyntax,
) -> Result<PortContract, Diagnostic> {
    match syntax {
        PortSyntax::Signal {
            direction,
            dimension,
        } => Ok(PortContract::Signal {
            direction: *direction,
            dimension: lower_dimension(file, dimension)?,
        }),
        PortSyntax::ConservingMarker { dimension } => Ok(PortContract::ConservingMarker {
            dimension: lower_dimension(file, dimension)?,
        }),
        PortSyntax::ScalarPhysical { domain } => Ok(PortContract::ScalarPhysical {
            domain: domain.clone(),
        }),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Port syntax is newer than this compiler",
        )),
    }
}

fn resolve_port_contract(
    file: &str,
    range: TextRange,
    contract: &PortContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<ResolvedPortContract, Diagnostic> {
    match contract {
        PortContract::Signal {
            direction,
            dimension,
        } => Ok(ResolvedPortContract::Signal {
            direction: *direction,
            dimension: *dimension,
        }),
        PortContract::ConservingMarker { dimension } => {
            Ok(ResolvedPortContract::ConservingMarker {
                dimension: *dimension,
            })
        }
        PortContract::ScalarPhysical { domain } => {
            let Some(Binding::Domain(
                domain_id,
                DomainContract::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                },
            )) = bindings.get(domain)
            else {
                return match bindings.get(domain) {
                    Some(_) => Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        format!("physical Port Domain `{domain}` is not scalar physical"),
                    )),
                    None => Err(unresolved(file, range, domain, "scalar physical Domain")),
                };
            };
            Ok(ResolvedPortContract::ScalarPhysical {
                domain: *domain_id,
                across_dimension: *across_dimension,
                through_dimension: *through_dimension,
            })
        }
        PortContract::BoundaryPhysical {
            connector,
            boundary,
        } => {
            let Some(Binding::Domain(connector_id, DomainContract::BoundaryPhysical(contract))) =
                bindings.get(connector)
            else {
                return Err(unresolved(
                    file,
                    range,
                    connector,
                    "field-physical Connector Domain",
                ));
            };
            let Some(Binding::Domain(boundary_id, DomainContract::Spatial { .. })) =
                bindings.get(boundary)
            else {
                return Err(unresolved(
                    file,
                    range,
                    boundary,
                    "field-physical boundary Domain",
                ));
            };
            Ok(ResolvedPortContract::BoundaryPhysical {
                connector: *connector_id,
                boundary: *boundary_id,
                trace_dimension: contract.trace_dimension(),
                flux_dimension: contract.flux_dimension(),
            })
        }
    }
}

fn insert_binding(
    file: &str,
    bindings: &mut BTreeMap<String, Binding>,
    name: &str,
    binding: Binding,
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_reserved(name) {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("`{name}` is reserved by Eqiora Language v0"),
        ));
    } else if bindings.insert(name.to_owned(), binding).is_some() {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("duplicate declaration name `{name}`"),
        ));
    }
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "model"
            | "domain"
            | "representation"
            | "field"
            | "parameter"
            | "port"
            | "clock"
            | "relation"
            | "connect"
            | "boundary"
            | "box"
            | "axis"
            | "side"
            | "lower"
            | "upper"
            | "continuum"
            | "on"
            | "as"
            | "continuous"
            | "periodic"
            | "signal"
            | "conserving"
            | "scalar_physical"
            | "input"
            | "output"
            | "period"
            | "phase"
            | "time"
            | "derivative"
            | "pre"
            | "next"
            | "grad"
            | "div"
            | "symmetric_part"
            | "isotropic_lift"
            | "trace"
            | "normal"
            | "across"
            | "through"
            | "kg"
            | "m"
            | "s"
            | "A"
            | "K"
            | "mol"
            | "cd"
    )
}

struct LoweredRelation {
    residuals: ExprDag,
    dependencies: BTreeSet<RawId>,
    ports: BTreeSet<RawId>,
}

fn lower_relation(
    file: &str,
    range: TextRange,
    activation: &ActivationSyntax,
    domain: Option<&str>,
    residuals: &[LoweringExpression],
    bindings: &BTreeMap<String, Binding>,
) -> Result<LoweredRelation, Diagnostic> {
    if let Some(domain) = domain {
        match bindings.get(domain) {
            Some(Binding::Domain(_, DomainContract::Spatial { .. })) => {}
            Some(Binding::Domain(_, DomainContract::ScalarPhysical { .. })) => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "spatial Relation scope cannot be a scalar physical Domain",
                ));
            }
            Some(_) | None => {
                return Err(unresolved(file, range, domain, "Relation Domain"));
            }
        }
    }
    if let ActivationSyntax::Periodic(clock) = activation {
        if !matches!(bindings.get(clock), Some(Binding::Clock(_))) {
            return Err(unresolved(file, range, clock, "periodic ClockDomain"));
        }
    } else if !matches!(activation, ActivationSyntax::Continuous) {
        return Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Activation syntax is newer than this compiler",
        ));
    }

    let mut lowerer = ExpressionLowerer {
        file,
        bindings,
        builder: ExprDagBuilder::new(),
        dependencies: BTreeSet::new(),
        ports: BTreeSet::new(),
        cache: HashMap::new(),
        allow_discrete_symbols: matches!(activation, ActivationSyntax::Periodic(_)),
    };
    let mut roots = Vec::new();
    for residual in residuals {
        roots.push(lowerer.lower(residual)?.id);
    }
    let residuals = lowerer.builder.finish(roots).map_err(|diagnostic| {
        source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            diagnostic.message(),
        )
    })?;
    Ok(LoweredRelation {
        residuals,
        dependencies: lowerer.dependencies,
        ports: lowerer.ports,
    })
}

struct ExpressionLowerer<'a> {
    file: &'a str,
    bindings: &'a BTreeMap<String, Binding>,
    builder: ExprDagBuilder,
    dependencies: BTreeSet<RawId>,
    ports: BTreeSet<RawId>,
    cache: HashMap<usize, TypedExpression>,
    allow_discrete_symbols: bool,
}

#[derive(Debug, Clone, Copy)]
struct TypedExpression {
    id: ExprId,
    dimension: DimExponents,
}

impl ExpressionLowerer<'_> {
    fn lower(&mut self, expression: &LoweringExpression) -> Result<TypedExpression, Diagnostic> {
        let key = Arc::as_ptr(&expression.node) as usize;
        if let Some(lowered) = self.cache.get(&key) {
            return Ok(*lowered);
        }
        let lowered = match expression.node.as_ref() {
            LoweringExpressionNode::Quantity(value) => self
                .builder
                .constant(*value)
                .map(|id| TypedExpression {
                    id,
                    dimension: value.dim(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic)),
            LoweringExpressionNode::Name(name) if name == "time" => self
                .builder
                .symbol(SymbolRef::Time)
                .map(|id| TypedExpression {
                    id,
                    dimension: time_dimension(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic)),
            LoweringExpressionNode::Name(name) => self.lower_name(expression, name),
            LoweringExpressionNode::Neg(value) => {
                let value = self.lower(value)?;
                self.builder
                    .neg(value.id)
                    .map(|id| TypedExpression {
                        id,
                        dimension: value.dimension,
                    })
                    .map_err(|diagnostic| self.builder_error(expression, diagnostic))
            }
            LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            } => self.lower_binary(expression, *operator, left, right),
            LoweringExpressionNode::Call { callee, argument } => {
                self.lower_call(expression, callee, argument)
            }
            LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            } => self.lower_pure_operator(expression, definition, arguments),
            LoweringExpressionNode::Unsupported => Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                self.file,
                expression.range(),
                "expression syntax is newer than this compiler",
            )),
        }?;
        self.cache.insert(key, lowered);
        Ok(lowered)
    }

    fn lower_name(
        &mut self,
        expression: &LoweringExpression,
        name: &str,
    ) -> Result<TypedExpression, Diagnostic> {
        let Some(binding) = self.bindings.get(name).cloned() else {
            return Err(unresolved(
                self.file,
                expression.range(),
                name,
                "expression symbol",
            ));
        };
        let (symbol, id, dimension) = match binding {
            Binding::Field(id, contract) => (SymbolRef::Field(id), id.erase(), contract.dimension),
            Binding::Parameter(id, dimension) => (SymbolRef::Parameter(id), id.erase(), dimension),
            Binding::Port(id, contract) => match resolve_port_contract(
                self.file,
                expression.range(),
                &contract,
                self.bindings,
            )? {
                ResolvedPortContract::Signal { dimension, .. }
                | ResolvedPortContract::ConservingMarker { dimension } => {
                    self.ports.insert(id.erase());
                    (SymbolRef::Port(id), id.erase(), dimension)
                }
                ResolvedPortContract::ScalarPhysical { .. } => {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        format!(
                            "scalar physical Port `{name}` must be read as `across({name})` or `through({name})`"
                        ),
                    ));
                }
                ResolvedPortContract::BoundaryPhysical { .. } => {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        format!(
                            "field-physical Port `{name}` must be read as `trace({name})` or `flux({name})`"
                        ),
                    ));
                }
            },
            Binding::Domain(_, _)
            | Binding::Representation(_)
            | Binding::Clock(_)
            | Binding::Relation { .. } => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!("`{name}` is not a scalar Field, Parameter, or Port"),
                ));
            }
        };
        self.dependencies.insert(id);
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_call(
        &mut self,
        expression: &LoweringExpression,
        callee: &str,
        argument: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        let boundary_trace = callee == "trace"
            && matches!(
                argument.node.as_ref(),
                LoweringExpressionNode::Name(name)
                    if matches!(
                        self.bindings.get(name),
                        Some(Binding::Port(_, PortContract::BoundaryPhysical { .. }))
                    )
            );
        if matches!(callee, "across" | "through" | "flux") || boundary_trace {
            return self.lower_physical_accessor(expression, callee, argument);
        }
        if callee == "coordinate" {
            let axis = lowering_integer_literal(argument)
                .and_then(|axis| usize::try_from(axis).ok())
                .ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        argument.range(),
                        "coordinate(...) requires a non-negative integer literal axis",
                    )
                })?;
            return self
                .builder
                .spatial_coordinate(axis)
                .map(|id| TypedExpression {
                    id,
                    dimension: length_dimension(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        if callee == "sin" {
            let operand = self.lower(argument)?;
            if operand.dimension != DimExponents::DIMENSIONLESS {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    format!(
                        "sin(...) requires a dimensionless scalar, received [{}]",
                        operand.dimension
                    ),
                ));
            }
            return self
                .builder
                .unary_math(UnaryMathFunction::Sin, operand.id)
                .map(|id| TypedExpression {
                    id,
                    dimension: DimExponents::DIMENSIONLESS,
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        if matches!(
            callee,
            "grad" | "div" | "symmetric_part" | "isotropic_lift" | "trace" | "normal"
        ) {
            let operand = self.lower(argument)?;
            let (result, dimension) = match callee {
                "grad" => (
                    self.builder.gradient(operand.id),
                    checked_dimensions(operand.dimension, length_dimension(), i8::checked_sub)
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                ),
                "div" => (
                    self.builder.divergence(operand.id),
                    checked_dimensions(operand.dimension, length_dimension(), i8::checked_sub)
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                ),
                "symmetric_part" => (self.builder.symmetric_part(operand.id), operand.dimension),
                "isotropic_lift" => (self.builder.isotropic_lift(operand.id), operand.dimension),
                "trace" => (self.builder.trace(operand.id), operand.dimension),
                "normal" => (self.builder.normal_component(operand.id), operand.dimension),
                _ => unreachable!("spatial operator was matched"),
            };
            return result
                .map(|id| TypedExpression { id, dimension })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        let LoweringExpressionNode::Name(name) = argument.node.as_ref() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                format!("{callee}(...) requires one Field name"),
            ));
        };
        let Some(Binding::Field(field, contract)) = self.bindings.get(name).cloned() else {
            return Err(unresolved(
                self.file,
                argument.range(),
                name,
                "Field operator argument",
            ));
        };
        if matches!(callee, "pre" | "next") && !self.allow_discrete_symbols {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                format!("continuous Relation cannot use `{callee}`"),
            ));
        }
        let (symbol, dimension) = match callee {
            "derivative" => (
                SymbolRef::Derivative(field),
                checked_dimensions(contract.dimension, time_dimension(), i8::checked_sub)
                    .ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.file,
                            expression.range(),
                            "derivative dimension exponent overflows i8",
                        )
                    })?,
            ),
            "pre" => (SymbolRef::Pre(field), contract.dimension),
            "next" => (SymbolRef::Next(field), contract.dimension),
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!("unknown scalar operator `{callee}`"),
                ));
            }
        };
        self.dependencies.insert(field.erase());
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_pure_operator(
        &mut self,
        expression: &LoweringExpression,
        definition: &PureOperatorDefinition,
        arguments: &[LoweringExpression],
    ) -> Result<TypedExpression, Diagnostic> {
        let arguments = arguments
            .iter()
            .map(|argument| self.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let dimension = instantiate_pure_dimension(definition, &arguments).ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "pure-operator result dimension overflows the portable SI exponent range",
            )
        })?;
        self.builder
            .pure_operator(definition, arguments.iter().map(|argument| argument.id))
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_physical_accessor(
        &mut self,
        expression: &LoweringExpression,
        callee: &str,
        argument: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        let LoweringExpressionNode::Name(name) = argument.node.as_ref() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                if matches!(callee, "across" | "through") {
                    format!("`{callee}(...)` requires one bare scalar physical Port name")
                } else {
                    format!("`{callee}(...)` requires one bare field-physical Port name")
                },
            ));
        };
        let Some(binding) = self.bindings.get(name) else {
            return Err(unresolved(
                self.file,
                argument.range(),
                name,
                "scalar physical Port",
            ));
        };
        let Binding::Port(port, contract) = binding else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                format!("`{name}` is not a scalar physical Port"),
            ));
        };
        let contract = resolve_port_contract(self.file, argument.range(), contract, self.bindings)?;
        self.dependencies.insert(port.erase());
        self.ports.insert(port.erase());
        let (symbol, dimension) = match (callee, contract) {
            (
                "across",
                ResolvedPortContract::ScalarPhysical {
                    across_dimension, ..
                },
            ) => (SymbolRef::Across(*port), across_dimension),
            (
                "through",
                ResolvedPortContract::ScalarPhysical {
                    through_dimension, ..
                },
            ) => (SymbolRef::Through(*port), through_dimension),
            (
                "trace",
                ResolvedPortContract::BoundaryPhysical {
                    trace_dimension, ..
                },
            ) => (SymbolRef::PortTrace(*port), trace_dimension),
            ("flux", ResolvedPortContract::BoundaryPhysical { flux_dimension, .. }) => {
                (SymbolRef::PortFlux(*port), flux_dimension)
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    if matches!(callee, "across" | "through") {
                        format!("`{name}` is not a scalar physical Port")
                    } else {
                        format!("`{name}` is not a field-physical Port")
                    },
                ));
            }
        };
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_binary(
        &mut self,
        expression: &LoweringExpression,
        operator: BinaryOp,
        left: &LoweringExpression,
        right: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        if operator == BinaryOp::Pow {
            let base = self.lower(left)?;
            let exponent = lowering_integer_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    right.range(),
                    "power exponent must be an i32 integer literal",
                )
            })?;
            let dimension = checked_scale_dimension(base.dimension, exponent).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    "power dimension exponent overflows i8",
                )
            })?;
            return self
                .builder
                .powi(base.id, exponent)
                .map(|id| TypedExpression { id, dimension })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }

        let left = self.lower(left)?;
        let right = self.lower(right)?;
        let dimension = match operator {
            BinaryOp::Add | BinaryOp::Sub if left.dimension == right.dimension => left.dimension,
            BinaryOp::Add | BinaryOp::Sub => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!(
                        "addition/subtraction combines dimensions [{}] and [{}]",
                        left.dimension, right.dimension
                    ),
                ));
            }
            BinaryOp::Mul => checked_dimensions(left.dimension, right.dimension, i8::checked_add)
                .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
            BinaryOp::Div => checked_dimensions(left.dimension, right.dimension, i8::checked_sub)
                .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
            BinaryOp::Pow => unreachable!("power handled above"),
        };
        let result = match operator {
            BinaryOp::Add => self.builder.add(left.id, right.id),
            BinaryOp::Sub => self.builder.sub(left.id, right.id),
            BinaryOp::Mul => self.builder.mul(left.id, right.id),
            BinaryOp::Div => self.builder.div(left.id, right.id),
            BinaryOp::Pow => unreachable!("power handled above"),
        };
        result
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn builder_error(&self, expression: &LoweringExpression, diagnostic: Diagnostic) -> Diagnostic {
        source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            self.file,
            expression.range(),
            diagnostic.message(),
        )
    }
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

#[derive(Debug, Default)]
struct FlatPhysicalConnectionPlan {
    emissions: BTreeMap<usize, Box<[RawId]>>,
    consumed: BTreeSet<usize>,
}

#[derive(Debug)]
struct FlatPhysicalFragment {
    item_index: usize,
    topology: ConnectionFragment<RawId>,
}

fn prepare_flat_physical_connections(
    file: &str,
    model: &LoweringModel,
    bindings: &BTreeMap<String, Binding>,
) -> Result<FlatPhysicalConnectionPlan, Vec<Diagnostic>> {
    let limits = ConnectionSetLimits::default();
    let mut fragments = Vec::new();
    for (item_index, item) in model.items.iter().enumerate() {
        let LoweringItem::Connection {
            syntax,
            ports,
            range,
        } = item
        else {
            continue;
        };
        if *syntax != ConnectionSyntax::Conserving {
            continue;
        }

        let mut resolved = Vec::new();
        let mut scalar_physical = true;
        for name in ports {
            let Some(Binding::Port(_, contract)) = bindings.get(name) else {
                scalar_physical = false;
                break;
            };
            let contract = resolve_port_contract(file, *range, contract, bindings)
                .map_err(|diagnostic| vec![diagnostic])?;
            scalar_physical &= matches!(contract, ResolvedPortContract::ScalarPhysical { .. });
            resolved.push(contract);
        }
        if !scalar_physical || resolved.is_empty() {
            continue;
        }

        let mut isolated_membership = BTreeSet::new();
        let (_, ports) = lower_connection(
            file,
            *range,
            *syntax,
            ports,
            Id::new(),
            bindings,
            &mut isolated_membership,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let topology = ConnectionFragment::try_new(ports, limits).map_err(|error| {
            vec![source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                *range,
                format!("invalid scalar physical Connection fragment: {error}"),
            )]
        })?;
        fragments.push(FlatPhysicalFragment {
            item_index,
            topology,
        });
    }

    let topologies = fragments
        .iter()
        .map(|fragment| fragment.topology.clone())
        .collect::<Vec<_>>();
    let normalized = normalize_connection_sets(&topologies, limits).map_err(|error| {
        vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            model.range,
            format!("cannot normalize scalar physical Connection fragments: {error}"),
        )]
    })?;
    let mut representatives = vec![usize::MAX; normalized.sets().len()];
    for (fragment, &set_index) in fragments.iter().zip(normalized.fragment_sets()) {
        representatives[set_index] = representatives[set_index].min(fragment.item_index);
    }

    let mut plan = FlatPhysicalConnectionPlan::default();
    for fragment in &fragments {
        plan.consumed.insert(fragment.item_index);
    }
    for (set_index, set) in normalized.sets().iter().enumerate() {
        let representative = representatives[set_index];
        debug_assert_ne!(representative, usize::MAX);
        plan.emissions
            .insert(representative, set.members().to_vec().into_boxed_slice());
    }
    Ok(plan)
}

fn lower_connection(
    file: &str,
    range: TextRange,
    syntax: ConnectionSyntax,
    names: &[String],
    id: Id<kinds::Connection>,
    bindings: &BTreeMap<String, Binding>,
    connected_ports: &mut BTreeSet<RawId>,
) -> Result<(ConnectionDef, Vec<RawId>), Diagnostic> {
    let mut ports = Vec::new();
    let mut definitions = Vec::new();
    for name in names {
        match bindings.get(name) {
            Some(Binding::Port(id, contract)) => {
                ports.push(id.erase());
                definitions.push(resolve_port_contract(file, range, contract, bindings)?);
            }
            Some(_) => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!("Connection name `{name}` is not a Port"),
                ));
            }
            None => {
                return Err(unresolved(file, range, name, "Connection Port"));
            }
        }
    }
    let unique_ports = ports.iter().copied().collect::<BTreeSet<_>>();
    if unique_ports.len() != ports.len() {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "Connection repeats the same Port",
        ));
    }
    if let Some(port) = ports.iter().find(|port| connected_ports.contains(port)) {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("Port `{port}` already belongs to another Connection"),
        ));
    }
    if syntax == ConnectionSyntax::SpatialPeriodic {
        if definitions.len() != 2
            || definitions.iter().any(|definition| {
                !matches!(definition, ResolvedPortContract::BoundaryPhysical { .. })
            })
        {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "spatial-periodic Connection requires exactly two field-physical Ports",
            ));
        }
        connected_ports.extend(&ports);
        return Ok((
            ConnectionDef::new(id, ConnectionSemantics::SpatialPeriodic),
            ports,
        ));
    }
    let (kind, semantics) = match syntax {
        ConnectionSyntax::Signal => (ScalarConnectionKind::Signal, ConnectionSemantics::Signal),
        ConnectionSyntax::Conserving => (
            ScalarConnectionKind::Conserving,
            ConnectionSemantics::Conserving,
        ),
        ConnectionSyntax::SpatialPeriodic => unreachable!("handled above"),
    };
    let contracts = definitions
        .iter()
        .map(resolved_scalar_port_contract)
        .collect::<Vec<_>>();
    validate_scalar_connection(kind, &contracts).map_err(|violation| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            lower_connection_violation_message(violation),
        )
    })?;
    if kind == ScalarConnectionKind::Signal
        && !matches!(
            definitions.first(),
            Some(ResolvedPortContract::Signal {
                direction: SignalDirectionSyntax::Output,
                ..
            })
        )
    {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "signal Connection source before `->` must be its output Port",
        ));
    }
    connected_ports.extend(&ports);
    Ok((ConnectionDef::new(id, semantics), ports))
}

fn resolved_scalar_port_contract(
    contract: &ResolvedPortContract,
) -> ScalarPortContract<Id<kinds::Domain>> {
    match contract {
        ResolvedPortContract::Signal {
            direction,
            dimension,
        } => ScalarPortContract::Signal {
            direction: match direction {
                SignalDirectionSyntax::Input => SignalDirection::Input,
                SignalDirectionSyntax::Output => SignalDirection::Output,
            },
            dimension: *dimension,
        },
        ResolvedPortContract::ConservingMarker { dimension } => {
            ScalarPortContract::ConservingMarker {
                dimension: *dimension,
            }
        }
        ResolvedPortContract::ScalarPhysical { domain, .. } => {
            ScalarPortContract::ScalarPhysical { nominal: *domain }
        }
        ResolvedPortContract::BoundaryPhysical { connector, .. } => {
            ScalarPortContract::ScalarPhysical {
                nominal: *connector,
            }
        }
    }
}

fn lower_connection_violation_message(violation: ScalarConnectionViolation) -> &'static str {
    match violation {
        ScalarConnectionViolation::TooFewPorts { .. } => "Connection requires at least two Ports",
        ScalarConnectionViolation::SignalDirections { .. } => {
            "signal Connection requires exactly one output and one or more inputs"
        }
        ScalarConnectionViolation::SignalDimensionMismatch => {
            "signal Connection requires dimension-matched inputs"
        }
        ScalarConnectionViolation::MixedConservingFamilies => {
            "conserving Connection cannot mix signal, legacy marker, and scalar physical Ports"
        }
        ScalarConnectionViolation::MarkerDimensionMismatch => {
            "conserving Connection requires dimension-matched legacy markers"
        }
        ScalarConnectionViolation::PhysicalNominalMismatch => {
            "conserving Connection requires scalar physical Ports on the exact same nominal Domain"
        }
    }
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
                let term = checked_scale_dimension(argument.dimension, i32::from(*exponent))?;
                checked_dimensions(result, term, i8::checked_add)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_symbols_are_ordered_and_exact() {
        let alpha = Id::<kinds::Field>::new().erase();
        let zeta = Id::<kinds::Parameter>::new().erase();
        let symbols = ModelSymbols::from_map(BTreeMap::from([
            ("zeta".to_owned(), zeta),
            ("alpha".to_owned(), alpha),
        ]));

        assert_eq!(symbols.get("alpha"), Some(alpha));
        assert_eq!(symbols.get("zeta"), Some(zeta));
        assert_eq!(symbols.get("missing"), None);
        assert_eq!(
            symbols.iter().collect::<Vec<_>>(),
            vec![("alpha", alpha), ("zeta", zeta)]
        );
    }

    #[test]
    fn executable_compile_still_requires_a_model_entry() {
        let source = "public component Resistor {} public connector Pin = scalar_physical(across = 1, through = A);";
        let diagnostics = compile("library.eqi", source)
            .expect_err("a declarations-only library is not an executable model");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code(), codes::SYNTAX_ERROR);
        assert_eq!(
            diagnostics[0].message(),
            "source requires at least one `model` declaration"
        );
        let span = diagnostics[0]
            .source_span()
            .expect("missing Model remains a source diagnostic");
        assert_eq!(span.file, "library.eqi");
        assert_eq!(span.start, u32::try_from(source.len()).unwrap());
        assert_eq!(span.end, span.start);
    }

    struct AssignedTestIdentities {
        model: OntologyId<Model>,
        domain: Id<kinds::Domain>,
        ports: BTreeMap<String, Id<kinds::Port>>,
        relation: Id<kinds::Relation>,
        activation: Id<kinds::Activation>,
        connection: Id<kinds::Connection>,
    }

    impl LoweringIdentities for AssignedTestIdentities {
        fn model(&mut self, _name: &str) -> OntologyId<Model> {
            self.model
        }

        fn domain(&mut self, _name: &str) -> Id<kinds::Domain> {
            self.domain
        }

        fn representation(&mut self, _name: &str) -> Id<kinds::Representation> {
            panic!("fixture has no Representation")
        }

        fn field(&mut self, _name: &str) -> Id<kinds::Field> {
            panic!("fixture has no Field")
        }

        fn parameter(&mut self, _name: &str) -> Id<kinds::Parameter> {
            panic!("fixture has no Parameter")
        }

        fn port(&mut self, name: &str) -> Id<kinds::Port> {
            self.ports[name]
        }

        fn clock(&mut self, _name: &str) -> Id<kinds::ClockDomain> {
            panic!("fixture has no ClockDomain")
        }

        fn relation(&mut self, _name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>) {
            (self.relation, self.activation)
        }

        fn connection(&mut self) -> Id<kinds::Connection> {
            self.connection
        }
    }

    fn voltage_dimension() -> DimExponents {
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -1,
            ..DimExponents::DIMENSIONLESS
        }
    }

    fn current_dimension() -> DimExponents {
        DimExponents {
            current: 1,
            ..DimExponents::DIMENSIONLESS
        }
    }

    fn resistance_dimension() -> DimExponents {
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -2,
            ..DimExponents::DIMENSIONLESS
        }
    }

    #[test]
    fn staged_identity_source_controls_every_lowered_identity() {
        let source = r#"
model assigned {
  domain electrical = scalar_physical(across = 1, through = 1);
  port positive: conserving on electrical;
  port negative: conserving on electrical;
  relation equal continuous { across(positive) - across(negative) = 0; }
  connect conserving positive, negative;
}
"#;
        let document = parse("assigned.eqi", source)
            .into_document()
            .expect("fixture parses");
        let positive = Id::new();
        let negative = Id::new();
        let mut identities = AssignedTestIdentities {
            model: OntologyId::new(),
            domain: Id::new(),
            ports: BTreeMap::from([
                ("negative".to_owned(), negative),
                ("positive".to_owned(), positive),
            ]),
            relation: Id::new(),
            activation: Id::new(),
            connection: Id::new(),
        };

        let compiled =
            lower_model_with_identities("assigned.eqi", &document.models()[0], &mut identities)
                .expect("assigned identities lower");

        assert_eq!(compiled.model(), identities.model);
        assert_eq!(
            compiled.symbols().get("electrical"),
            Some(identities.domain.erase())
        );
        assert_eq!(compiled.symbols().get("positive"), Some(positive.erase()));
        assert_eq!(compiled.symbols().get("negative"), Some(negative.erase()));
        assert_eq!(
            compiled.symbols().get("equal"),
            Some(identities.relation.erase())
        );
        assert!(compiled.transaction().ops().iter().any(|operation| {
            matches!(
                operation,
                Op::DefineKernelNode {
                    node: KernelNode::Activation(definition),
                } if definition.id() == identities.activation
            )
        }));
        assert!(compiled.transaction().ops().iter().any(|operation| {
            matches!(
                operation,
                Op::DefineKernelNode {
                    node: KernelNode::Connection(definition),
                } if definition.id() == identities.connection
            )
        }));
    }

    #[test]
    fn compiler_rejects_dimensionally_invalid_residual_at_source_span() {
        let source = r#"
model invalid {
  field temperature: K = 293;
  parameter tau: s = 10;
  relation bad continuous {
    temperature + tau = 0;
  }
}
"#;
        let diagnostics = compile("invalid.eqi", source).expect_err("K + s is invalid");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == codes::LANGUAGE_TYPE_ERROR && diagnostic.source_span().is_some()
        }));
    }

    #[test]
    fn compiler_rejects_unresolved_periodic_clock() {
        let source = "model m { field x: 1 = 0; relation r periodic(missing) { next(x) = 0; } }";
        let diagnostics = compile("missing.eqi", source).expect_err("clock is unresolved");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("periodic ClockDomain"))
        );
    }

    #[test]
    fn compiler_rejects_discrete_symbols_in_continuous_relations() {
        let source = "model m { field x: 1 = 0; relation r continuous { next(x) = 0; } }";
        let diagnostics = compile("activation.eqi", source).expect_err("Next needs a tick");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("continuous Relation"))
        );
    }

    #[test]
    fn compiler_rejects_spatial_boundary_unit_mismatch() {
        let source = r#"
model bar {
  domain body = box(0, 1);
  domain loaded = boundary(body, axis = 0, side = upper);
  representation space = continuum;
  field u on body as space: m = 0;
  parameter stiffness: kg * m / s ^ 2 = 10;
  parameter wrong_load: m = 1;
  relation load continuous on loaded {
    normal(stiffness * grad(u)) - wrong_load = 0;
  }
}
"#;
        let diagnostics = compile("bar.eqi", source).expect_err("force and length conflict");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
                && diagnostic.message().contains("addition/subtraction")
        }));
    }

    #[test]
    fn compiler_requires_dimensionless_trigonometric_arguments() {
        let source = r#"
model invalid {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: 1 = 0;
  relation balance continuous on interval {
    -div(grad(u)) - sin(coordinate(0)) = 0;
  }
}
"#;
        let diagnostics = compile("invalid-sin.eqi", source)
            .expect_err("a physical coordinate is not an angle by itself");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
                && diagnostic.message().contains("dimensionless scalar")
        }));
    }

    #[test]
    fn compiler_lowers_canonical_tensor_structure_without_a_physics_node() {
        let source = r#"
model elastic_relation {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  parameter mu: kg / (m * s ^ 2) = 2;
  parameter lambda: kg / (m * s ^ 2) = 3;
  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) = 0;
  }
}
"#;
        let compiled = compile("elastic-relation.eqi", source).expect("typed tensor relation");
        let relation = compiled[0]
            .transaction()
            .ops()
            .iter()
            .find_map(|operation| match operation {
                Op::DefineKernelNode {
                    node: KernelNode::Relation(relation),
                } => Some(relation),
                _ => None,
            })
            .expect("canonical Relation");
        assert!(
            relation
                .residuals()
                .nodes()
                .iter()
                .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::SymmetricPart(_)))
        );
        assert!(
            relation
                .residuals()
                .nodes()
                .iter()
                .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::IsotropicLift(_)))
        );
    }

    #[test]
    fn compiler_lowers_source_declared_pure_operator_as_one_generic_application() {
        let source = r#"
public pure operator dyadic(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);

model generic_operator {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field left on body as space: 1 shape spatial_vector;
  field right on body as space: 1 shape spatial_vector;
  relation balance continuous on body {
    div(div(dyadic(left, right))) = 0;
  }
}
"#;
        let compiled = compile("generic-operator.eqi", source).expect("typed pure operator");
        let relation = compiled[0]
            .transaction()
            .ops()
            .iter()
            .find_map(|operation| match operation {
                Op::DefineKernelNode {
                    node: KernelNode::Relation(relation),
                } => Some(relation),
                _ => None,
            })
            .expect("canonical Relation");
        let dag = relation.residuals();
        let applications = dag
            .nodes()
            .iter()
            .filter_map(|node| match node {
                eqiora_schema::kernel::ExprNode::PureOperatorApplication(application) => {
                    Some(application)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].arguments().len(), 2);
        assert_eq!(dag.definitions().len(), 1);
        assert_eq!(
            applications[0].definition(),
            eqiora_schema::kernel::pure_operator::PureOperatorDefinition::dyadic_product()
                .expect("standard definition")
                .digest()
        );
        let (transaction, _, _) = compiled
            .into_iter()
            .next()
            .expect("compiled Model")
            .into_parts();
        eqiora_graph::GraphStore::commit(&mut eqiora_graph::InMemoryGraphStore::new(), transaction)
            .expect("generic application passes whole-model admission");
    }

    #[test]
    fn pure_operator_arity_and_exact_value_class_fail_before_lowering() {
        let prefix = r#"
public pure operator dyadic(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
model invalid {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field scalar on body as space: 1 = 0;
  field vector on body as space: 1 shape spatial_vector;
  relation balance continuous on body {
"#;
        for (residual, expected) in [
            ("dyadic(vector) = 0;", "argument count"),
            ("dyadic(scalar, vector) = 0;", "exact type rule"),
        ] {
            let source = format!("{prefix}{residual}\n  }}\n}}\n");
            let diagnostics = compile("invalid-pure-operator.eqi", &source)
                .expect_err("invalid application must fail closed");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(expected)),
                "expected `{expected}`, got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn compiler_requires_a_literal_coordinate_axis() {
        let source = r#"
model invalid {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: m = 0;
  relation identity continuous on interval { u - coordinate(u) = 0; }
}
"#;
        let diagnostics = compile("invalid-coordinate.eqi", source)
            .expect_err("coordinate axis selection must remain structural");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
                && diagnostic.message().contains("integer literal axis")
        }));
    }

    #[test]
    fn native_lowering_replaces_synthetic_ranges_with_declaration_paths() {
        let temperature = eqiora_lang::DraftField::new(
            "temperature",
            DimExponents {
                temperature: 1,
                ..DimExponents::DIMENSIONLESS
            },
            293.0,
        );
        let duration = eqiora_lang::DraftParameter::new(
            "duration",
            DimExponents {
                time: 1,
                ..DimExponents::DIMENSIONLESS
            },
            1.0,
        );
        let relation = eqiora_lang::DraftRelation::continuous(
            "invalid",
            [temperature.expression() + duration.expression()],
        );
        let draft = ModelDraft::new(
            "thermal",
            [temperature.into(), duration.into(), relation.into()],
        )
        .unwrap();

        let diagnostics = lower_draft(&draft).unwrap_err();
        assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
        assert_eq!(
            diagnostics[0].graph_path().unwrap().to_string(),
            "thermal.invalid"
        );
        assert!(diagnostics[0].source_span().is_none());
    }

    #[test]
    fn source_and_native_physical_models_lower_to_the_same_normalized_semantics() {
        let source = r#"
model resistor {
  domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port positive: conserving on electrical;
  port negative: conserving on electrical;
  port tap: conserving on electrical;
  parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2) = 2;
  relation law continuous {
    across(positive) - across(negative) - resistance * through(positive) = 0;
    through(positive) + through(negative) + through(tap) = 0;
  }
  connect conserving positive, negative, tap;
}
"#;
        let source_model = compile("resistor.eqi", source).unwrap().remove(0);

        let electrical = eqiora_lang::DraftPhysicalDomain::new(
            "electrical",
            voltage_dimension(),
            current_dimension(),
        );
        let positive = eqiora_lang::DraftConservingPort::new("positive", &electrical);
        let negative = eqiora_lang::DraftConservingPort::new("negative", &electrical);
        let tap = eqiora_lang::DraftConservingPort::new("tap", &electrical);
        let resistance =
            eqiora_lang::DraftParameter::new("resistance", resistance_dimension(), 2.0);
        let law = eqiora_lang::DraftRelation::continuous(
            "law",
            [
                eqiora_lang::DraftExpression::across(&positive)
                    - eqiora_lang::DraftExpression::across(&negative)
                    - resistance.expression() * eqiora_lang::DraftExpression::through(&positive),
                eqiora_lang::DraftExpression::through(&positive)
                    + eqiora_lang::DraftExpression::through(&negative)
                    + eqiora_lang::DraftExpression::through(&tap),
            ],
        );
        let draft = ModelDraft::new(
            "resistor",
            [
                electrical.into(),
                positive.clone().into(),
                negative.clone().into(),
                tap.clone().into(),
                resistance.into(),
                law.into(),
                eqiora_lang::DraftConservingConnection::new([&positive, &negative, &tap]).into(),
            ],
        )
        .unwrap();
        let native_model = lower_draft(&draft).unwrap();

        assert_eq!(
            normalized_physical_semantics(&source_model),
            normalized_physical_semantics(&native_model)
        );

        let (transaction, _, _) = native_model.into_parts();
        let mut store = eqiora_graph::InMemoryGraphStore::new();
        eqiora_graph::GraphStore::commit(&mut store, transaction)
            .expect("the shared compiler transaction must pass full graph admission");
    }

    #[test]
    fn native_physical_projection_is_insensitive_to_declaration_and_net_permutation() {
        let electrical = eqiora_lang::DraftPhysicalDomain::new(
            "electrical",
            voltage_dimension(),
            current_dimension(),
        );
        let positive = eqiora_lang::DraftConservingPort::new("positive", &electrical);
        let negative = eqiora_lang::DraftConservingPort::new("negative", &electrical);
        let relation = eqiora_lang::DraftRelation::continuous(
            "balance",
            [
                eqiora_lang::DraftExpression::across(&positive)
                    - eqiora_lang::DraftExpression::across(&negative),
                eqiora_lang::DraftExpression::through(&positive)
                    + eqiora_lang::DraftExpression::through(&negative),
            ],
        );
        let forward = ModelDraft::new(
            "permuted",
            [
                electrical.clone().into(),
                positive.clone().into(),
                negative.clone().into(),
                relation.clone().into(),
                eqiora_lang::DraftConservingConnection::new([&positive, &negative]).into(),
            ],
        )
        .unwrap();
        let reversed = ModelDraft::new(
            "permuted",
            [
                eqiora_lang::DraftConservingConnection::new([&negative, &positive]).into(),
                relation.into(),
                negative.into(),
                positive.into(),
                electrical.into(),
            ],
        )
        .unwrap();

        assert_eq!(
            normalized_physical_semantics(&lower_draft(&forward).unwrap()),
            normalized_physical_semantics(&lower_draft(&reversed).unwrap())
        );
    }

    #[test]
    fn direct_flat_physical_fragments_normalize_before_kernel_lowering() {
        let nary = r#"
model network {
  domain physical = scalar_physical(across = 1, through = 1);
  port a: conserving on physical;
  port b: conserving on physical;
  port c: conserving on physical;
  relation owner continuous {
    across(a) - across(b) = 0;
    across(b) - across(c) = 0;
    through(a) + through(b) + through(c) = 0;
  }
  connect conserving a, b, c;
}
"#;
        let chain = r#"
model network {
  domain physical = scalar_physical(across = 1, through = 1);
  port a: conserving on physical;
  port b: conserving on physical;
  port c: conserving on physical;
  relation owner continuous {
    across(a) - across(b) = 0;
    across(b) - across(c) = 0;
    through(a) + through(b) + through(c) = 0;
  }
  connect conserving a, b;
  connect conserving b, c;
}
"#;
        let nary = compile("nary.eqi", nary).unwrap().remove(0);
        let chain = compile("chain.eqi", chain).unwrap().remove(0);

        assert_eq!(
            normalized_physical_semantics(&nary),
            normalized_physical_semantics(&chain)
        );
        assert_eq!(
            chain
                .transaction()
                .ops()
                .iter()
                .filter(|operation| matches!(
                    operation,
                    Op::DefineKernelNode {
                        node: KernelNode::Connection(_),
                    }
                ))
                .count(),
            1
        );

        let (transaction, _, _) = chain.into_parts();
        let mut store = eqiora_graph::InMemoryGraphStore::new();
        eqiora_graph::GraphStore::commit(&mut store, transaction).unwrap();
    }

    #[test]
    fn compiler_preserves_legacy_conserving_markers() {
        let source = r#"
model legacy {
  port p: conserving A;
  relation owner continuous { p = 0; }
}
"#;
        let models = compile("legacy.eqi", source).expect("legacy marker remains source-valid");
        let port = models[0].symbols().get("p").expect("Port ID");
        let relation = models[0].symbols().get("owner").expect("Relation ID");
        let mut saw_marker = false;
        let mut saw_legacy_symbol = false;
        for operation in models[0].transaction().ops() {
            let Op::DefineKernelNode { node } = operation else {
                continue;
            };
            match node {
                KernelNode::Port(definition) if definition.id().erase() == port => {
                    saw_marker = definition.marker_dimension().is_some();
                }
                KernelNode::Relation(definition) if definition.id().erase() == relation => {
                    saw_legacy_symbol = definition
                        .residuals()
                        .nodes()
                        .iter()
                        .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::Symbol(SymbolRef::Port(id)) if id.erase() == port));
                }
                _ => {}
            }
        }
        assert!(saw_marker);
        assert!(saw_legacy_symbol);
    }

    #[test]
    fn compiler_rejects_dimension_coincidence_across_nominal_domains() {
        let source = r#"
model crossed_types {
  domain electrical_a = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  domain electrical_b = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port a: conserving on electrical_a;
  port b: conserving on electrical_b;
  relation owner_a continuous { across(a) = 0; }
  relation owner_b continuous { across(b) = 0; }
  connect conserving a, b;
}
"#;
        let diagnostics = compile("crossed-types.eqi", source)
            .expect_err("equal dimensions never erase nominal Domain identity");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_span().is_some()
                && diagnostic.message().contains("exact same nominal Domain")
        }));
    }

    #[test]
    fn flat_lowering_consumes_the_shared_scalar_connection_contract() {
        let cases = [
            (
                "dimension mismatch",
                "model m { port out: signal output m; port sink: signal input s; connect signal out -> sink; }",
                "dimension-matched inputs",
            ),
            (
                "source direction",
                "model m { port out: signal output 1; port sink: signal input 1; connect signal sink -> out; }",
                "source before `->`",
            ),
            (
                "mixed conserving families",
                "model m { domain d = scalar_physical(across = 1, through = 1); port marker: conserving 1; port physical: conserving on d; connect conserving marker, physical; }",
                "cannot mix",
            ),
        ];
        for (name, source, message) in cases {
            let document = parse("connections.eqi", source)
                .into_document()
                .expect("fixture parses");
            let diagnostics =
                lower_model("connections.eqi", &document.models()[0]).expect_err(name);
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.source_span().is_some() && diagnostic.message().contains(message)
                }),
                "{name}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn compiler_rejects_non_physical_domains_and_unqualified_physical_ports() {
        let wrong_domain = r#"
model wrong_domain {
  domain space = box(0, 1);
  port p: conserving on space;
  relation owner continuous { across(p) = 0; }
}
"#;
        let diagnostics = compile("wrong-domain.eqi", wrong_domain)
            .expect_err("spatial Domains cannot type physical Ports");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_span().is_some()
                && diagnostic.message().contains("not scalar physical")
        }));

        let unqualified = r#"
model unqualified {
  domain electrical = scalar_physical(across = 1, through = 1);
  port p: conserving on electrical;
  relation owner continuous { p = 0; }
}
"#;
        let diagnostics = compile("unqualified.eqi", unqualified)
            .expect_err("physical variables require an explicit accessor");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_span().is_some()
                && diagnostic.message().contains("must be read as `across(p)`")
        }));
    }

    #[test]
    fn physical_accessors_require_one_bare_physical_port_name() {
        let malformed = r#"
model malformed {
  domain electrical = scalar_physical(across = 1, through = 1);
  port p: conserving on electrical;
  relation owner continuous { across(p + 1) = 0; }
}
"#;
        let diagnostics =
            compile("malformed.eqi", malformed).expect_err("accessor structure remains explicit");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_span().is_some()
                && diagnostic
                    .message()
                    .contains("one bare scalar physical Port name")
        }));

        let signal = r#"
model signal_accessor {
  port p: signal input 1;
  relation owner continuous { through(p) = 0; }
}
"#;
        let diagnostics = compile("signal-accessor.eqi", signal)
            .expect_err("signal Ports have no through variable");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.source_span().is_some()
                && diagnostic.message().contains("not a scalar physical Port")
        }));
    }

    fn normalized_physical_semantics(model: &CompiledModel) -> Vec<String> {
        use eqiora_schema::kernel::{ActivationKind, DomainKind, ExprNode, PortPayload, SymbolRef};

        let names = model
            .symbols()
            .iter()
            .map(|(name, id)| (id, name.to_owned()))
            .collect::<BTreeMap<_, _>>();
        let mut signatures = Vec::new();
        let mut connections = BTreeMap::<RawId, Vec<String>>::new();
        let mut activations = BTreeMap::new();

        for operation in model.transaction().ops() {
            match operation {
                Op::DefineKernelNode {
                    node: KernelNode::Domain(domain),
                } => {
                    if let DomainKind::ScalarPhysical {
                        across_dimension,
                        through_dimension,
                    } = domain.kind()
                    {
                        signatures.push(format!(
                            "domain:{}:{across_dimension:?}:{through_dimension:?}",
                            named(&names, domain.id().erase())
                        ));
                    }
                }
                Op::DefineKernelNode {
                    node: KernelNode::Parameter(parameter),
                } => signatures.push(format!(
                    "parameter:{}:{:016x}:{:?}",
                    named(&names, parameter.id().erase()),
                    parameter.value().value().to_bits(),
                    parameter.value().dim()
                )),
                Op::DefineKernelNode {
                    node: KernelNode::Port(port),
                } => {
                    if let PortPayload::ScalarPhysical { domain } = port.payload() {
                        signatures.push(format!(
                            "port:{}:{}",
                            named(&names, port.id().erase()),
                            named(&names, domain.erase())
                        ));
                    }
                }
                Op::DefineKernelNode {
                    node: KernelNode::Relation(relation),
                } => {
                    signatures.push(format!(
                        "relation:{}:{}",
                        named(&names, relation.id().erase()),
                        normalize_dag(relation.residuals(), &names)
                    ));
                }
                Op::DefineKernelNode {
                    node: KernelNode::Activation(activation),
                } => {
                    let kind = match activation.kind() {
                        ActivationKind::Continuous => "continuous",
                        ActivationKind::Periodic => "periodic",
                        ActivationKind::Event { .. } => "event",
                        ActivationKind::Guard { .. } => "guard",
                        _ => "newer",
                    };
                    activations.insert(activation.id().erase(), kind);
                }
                Op::DefineKernelNode {
                    node: KernelNode::Connection(connection),
                } => {
                    signatures.push(format!("connection-kind:{:?}", connection.semantics()));
                    connections.entry(connection.id().erase()).or_default();
                }
                Op::Connect {
                    from,
                    to,
                    edge: EdgeKind::Connects,
                } => connections
                    .entry(*from)
                    .or_default()
                    .push(named(&names, *to).to_owned()),
                Op::Connect {
                    from,
                    to,
                    edge: EdgeKind::DependsOn | EdgeKind::HasPort,
                } => signatures.push(format!(
                    "edge:{:?}:{}:{}",
                    operation_edge(operation),
                    named(&names, *from),
                    named(&names, *to)
                )),
                Op::Connect {
                    from,
                    to,
                    edge: EdgeKind::Activates,
                } => signatures.push(format!(
                    "activation:{}:{}",
                    activations.get(from).copied().unwrap_or("missing"),
                    named(&names, *to)
                )),
                _ => {}
            }
        }

        for mut members in connections.into_values() {
            members.sort();
            signatures.push(format!("connection-members:{members:?}"));
        }
        signatures.sort();

        fn normalize_dag(
            dag: &eqiora_schema::kernel::ExprDag,
            names: &BTreeMap<RawId, String>,
        ) -> String {
            let nodes = dag
                .nodes()
                .iter()
                .map(|node| match node {
                    ExprNode::Constant(value) => format!(
                        "constant({:016x},{:?})",
                        value.value().to_bits(),
                        value.dim()
                    ),
                    ExprNode::Symbol(symbol) => normalize_symbol(*symbol, names),
                    ExprNode::Neg(value) => format!("neg({})", value.index()),
                    ExprNode::Add(left, right) => {
                        format!("add({},{})", left.index(), right.index())
                    }
                    ExprNode::Sub(left, right) => {
                        format!("sub({},{})", left.index(), right.index())
                    }
                    ExprNode::Mul(left, right) => {
                        format!("mul({},{})", left.index(), right.index())
                    }
                    ExprNode::Div(left, right) => {
                        format!("div({},{})", left.index(), right.index())
                    }
                    other => format!("{other:?}"),
                })
                .collect::<Vec<_>>();
            let roots = dag
                .roots()
                .iter()
                .map(|root| root.index())
                .collect::<Vec<_>>();
            format!("{nodes:?}:{roots:?}")
        }

        fn normalize_symbol(symbol: SymbolRef, names: &BTreeMap<RawId, String>) -> String {
            match symbol {
                SymbolRef::Field(id) => format!("field({})", named(names, id.erase())),
                SymbolRef::Derivative(id) => {
                    format!("derivative({})", named(names, id.erase()))
                }
                SymbolRef::Pre(id) => format!("pre({})", named(names, id.erase())),
                SymbolRef::Next(id) => format!("next({})", named(names, id.erase())),
                SymbolRef::Parameter(id) => format!("parameter({})", named(names, id.erase())),
                SymbolRef::Port(id) => format!("port({})", named(names, id.erase())),
                SymbolRef::Across(id) => format!("across({})", named(names, id.erase())),
                SymbolRef::Through(id) => format!("through({})", named(names, id.erase())),
                SymbolRef::Time => "time".to_owned(),
                _ => "newer-symbol".to_owned(),
            }
        }

        fn operation_edge(operation: &Op) -> EdgeKind {
            let Op::Connect { edge, .. } = operation else {
                unreachable!("only Connect operations reach this helper");
            };
            *edge
        }

        fn named(names: &BTreeMap<RawId, String>, id: RawId) -> &str {
            names.get(&id).map_or("<anonymous>", String::as_str)
        }

        signatures
    }
}
