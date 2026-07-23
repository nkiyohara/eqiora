//! Client-neutral, immutable declarations for native model construction.
//!
//! Drafts are an ergonomic control-plane input. They are neither accepted
//! Semantic Models nor a durable wire format. The compiler lowers their
//! synthetic AST through exactly the same path as parsed source.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::sync::Arc;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, GraphPath};

use crate::ast::{
    ActivationSyntax, BinaryOp, ConnectionDecl, ConnectionSyntax, DomainDecl, DomainSyntax, Expr,
    ExprKind, FieldDecl, Item, ModelDecl, NamePath, ParameterDecl, PortDecl, PortSyntax,
    RelationDecl, RepresentationDecl, RepresentationSyntax, TextRange, UnaryOp,
};
use crate::draft_spatial::{DraftRepresentation, DraftSpatialDomain, DraftSpatialDomainKind};

/// One immutable native model definition request.
#[derive(Debug, Clone)]
pub struct ModelDraft {
    name: String,
    declarations: Vec<DraftDeclaration>,
}

impl ModelDraft {
    /// Close a set of declarations into one atomic model draft.
    ///
    /// # Errors
    /// Returns structured diagnostics for invalid or duplicate names, invalid
    /// scalar values, empty Relations, omitted declaration identities, and
    /// invalid conserving-connection membership.
    pub fn new(
        name: impl Into<String>,
        declarations: impl IntoIterator<Item = DraftDeclaration>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let value = Self {
            name: name.into(),
            declarations: declarations.into_iter().collect(),
        };
        value.validate()?;
        Ok(value)
    }

    /// Native model name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declarations in request order.
    #[must_use]
    pub fn declarations(&self) -> &[DraftDeclaration] {
        &self.declarations
    }

    fn validate(&self) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut names = HashMap::new();
        let mut value_symbols = HashSet::new();
        let mut domain_symbols = HashSet::new();
        let mut spatial_domain_symbols = HashSet::new();
        let mut representation_symbols = HashSet::new();
        let mut ports = HashMap::new();

        if !is_language_identifier(&self.name) {
            diagnostics.push(
                Diagnostic::error(
                    codes::LANGUAGE_TYPE_ERROR,
                    format!(
                        "native model name `{}` is not an Eqiora Language identifier",
                        self.name
                    ),
                )
                .with_graph_path(GraphPath::new([self.name.clone()])),
            );
        }

        for declaration in &self.declarations {
            if let Some(name) = declaration.name() {
                if !is_language_identifier(name) {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        name,
                        format!("declaration name `{name}` is not an Eqiora Language identifier"),
                    ));
                }
                if let Some(previous) = names.insert(name, declaration.kind_name()) {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        name,
                        format!(
                            "duplicate declaration `{name}` conflicts with an earlier {previous}"
                        ),
                    ));
                }
            }
            match declaration {
                DraftDeclaration::Field(value) => {
                    value_symbols.insert(value.symbol.clone());
                }
                DraftDeclaration::Parameter(value) => {
                    value_symbols.insert(value.symbol.clone());
                }
                DraftDeclaration::PhysicalDomain(value) => {
                    domain_symbols.insert(value.symbol.clone());
                }
                DraftDeclaration::SpatialDomain(value) => {
                    spatial_domain_symbols.insert(value.symbol().clone());
                }
                DraftDeclaration::Representation(value) => {
                    representation_symbols.insert(value.symbol.clone());
                }
                DraftDeclaration::ConservingPort(value) => {
                    ports.insert(value.symbol.clone(), value);
                }
                DraftDeclaration::Relation(_) | DraftDeclaration::ConservingConnection(_) => {}
            }
            if let DraftDeclaration::Relation(relation) = declaration {
                if relation.residuals.is_empty() {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        relation.name(),
                        "Relation requires at least one residual",
                    ));
                }
                if relation
                    .residuals
                    .iter()
                    .any(DraftExpression::contains_non_finite_constant)
                {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        relation.name(),
                        "Relation contains a non-finite numeric literal",
                    ));
                }
            }
            match declaration {
                DraftDeclaration::Field(field) if !field.initial.is_finite() => {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        field.name(),
                        "Field initial value must be finite",
                    ));
                }
                DraftDeclaration::Parameter(parameter) if !parameter.value.is_finite() => {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        parameter.name(),
                        "Parameter value must be finite",
                    ));
                }
                _ => {}
            }
        }

        for declaration in &self.declarations {
            let DraftDeclaration::SpatialDomain(domain) = declaration else {
                continue;
            };
            if let DraftSpatialDomainKind::Boundary { parent, .. } = domain.kind()
                && !spatial_domain_symbols.contains(parent.symbol())
            {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    domain.name(),
                    format!(
                        "boundary Domain `{}` references foreign or omitted parent Domain `{}`",
                        domain.name(),
                        parent.name()
                    ),
                ));
            }
        }

        for declaration in &self.declarations {
            let DraftDeclaration::Field(field) = declaration else {
                continue;
            };
            let Some(scope) = &field.spatial_scope else {
                continue;
            };
            if !spatial_domain_symbols.contains(scope.domain.symbol()) {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    field.name(),
                    format!(
                        "spatial Field `{}` references foreign or omitted Domain `{}`",
                        field.name(),
                        scope.domain.name()
                    ),
                ));
            }
            if !representation_symbols.contains(&scope.representation.symbol) {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    field.name(),
                    format!(
                        "spatial Field `{}` references foreign or omitted Representation `{}`",
                        field.name(),
                        scope.representation.name()
                    ),
                ));
            }
        }

        for declaration in &self.declarations {
            let DraftDeclaration::Relation(relation) = declaration else {
                continue;
            };
            if let Some(domain) = &relation.domain
                && !spatial_domain_symbols.contains(domain.symbol())
            {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    relation.name(),
                    format!(
                        "spatial Relation `{}` references foreign or omitted Domain `{}`",
                        relation.name(),
                        domain.name()
                    ),
                ));
            }
        }

        for declaration in &self.declarations {
            let DraftDeclaration::ConservingPort(port) = declaration else {
                continue;
            };
            if !domain_symbols.contains(&port.domain.symbol) {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    port.name(),
                    format!(
                        "conserving Port `{}` references foreign or omitted scalar physical Domain `{}`",
                        port.name(),
                        port.domain.name()
                    ),
                ));
            }
        }

        for declaration in &self.declarations {
            let DraftDeclaration::Relation(relation) = declaration else {
                continue;
            };
            let mut referenced = Vec::new();
            for residual in &relation.residuals {
                residual.references(&mut referenced);
            }
            for reference in referenced {
                match reference {
                    DraftExpressionReference::Value(reference)
                        if !value_symbols.contains(&reference.symbol) =>
                    {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            relation.name(),
                            format!(
                                "Relation `{}` references foreign or omitted {} `{}`",
                                relation.name(),
                                reference.kind.label(),
                                reference.name
                            ),
                        ));
                    }
                    DraftExpressionReference::Port(reference)
                        if !ports.contains_key(&reference.symbol) =>
                    {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            relation.name(),
                            format!(
                                "Relation `{}` references foreign or omitted conserving Port `{}`",
                                relation.name(),
                                reference.name
                            ),
                        ));
                    }
                    DraftExpressionReference::Value(_) | DraftExpressionReference::Port(_) => {}
                }
            }
        }

        let mut connected_ports = HashSet::new();
        for declaration in &self.declarations {
            let DraftDeclaration::ConservingConnection(connection) = declaration else {
                continue;
            };
            let path = connection_path(connection);
            if connection.ports.len() < 2 {
                diagnostics.push(native_diagnostic(
                    &self.name,
                    &path,
                    "conserving Connection requires at least two Ports",
                ));
            }
            let mut members = HashSet::new();
            let mut expected_domain = None;
            for port in &connection.ports {
                if !members.insert(port.symbol.clone()) {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        &path,
                        format!("conserving Connection repeats Port `{}`", port.name()),
                    ));
                    continue;
                }
                let Some(declared) = ports.get(&port.symbol) else {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        &path,
                        format!(
                            "conserving Connection references foreign or omitted Port `{}`",
                            port.name()
                        ),
                    ));
                    continue;
                };
                if !connected_ports.insert(port.symbol.clone()) {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        &path,
                        format!(
                            "conserving Port `{}` already belongs to another Connection",
                            port.name()
                        ),
                    ));
                }
                match &expected_domain {
                    None => expected_domain = Some(declared.domain.symbol.clone()),
                    Some(domain) if *domain != declared.domain.symbol => {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            &path,
                            "conserving Connection requires Ports on the exact same draft-local scalar physical Domain",
                        ));
                    }
                    Some(_) => {}
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    /// Build the private compiler bridge without formatting or parsing source.
    #[doc(hidden)]
    #[must_use]
    pub fn native_ast(&self) -> NativeModelAst {
        let mut ranges = RangeAllocator::default();
        let mut paths = HashMap::new();
        let mut items = Vec::with_capacity(self.declarations.len());

        for declaration in &self.declarations {
            let declaration_path =
                declaration
                    .name()
                    .map(str::to_owned)
                    .unwrap_or_else(|| match declaration {
                        DraftDeclaration::ConservingConnection(connection) => {
                            connection_path(connection)
                        }
                        _ => unreachable!("only connections are anonymous"),
                    });
            let path = GraphPath::new([self.name.clone(), declaration_path]);
            let range = ranges.allocate(&path, &mut paths);
            let item = match declaration {
                DraftDeclaration::SpatialDomain(domain) => Item::Domain(DomainDecl {
                    name: domain.name().to_owned(),
                    syntax: match domain.kind() {
                        DraftSpatialDomainKind::CartesianBox { bounds } => {
                            DomainSyntax::CartesianBox(bounds.clone())
                        }
                        DraftSpatialDomainKind::Boundary { parent, axis, side } => {
                            DomainSyntax::Boundary {
                                parent: parent.name().to_owned(),
                                axis: *axis,
                                side: (*side).into(),
                            }
                        }
                    },
                    range,
                }),
                DraftDeclaration::PhysicalDomain(domain) => Item::Domain(DomainDecl {
                    name: domain.name.clone(),
                    syntax: DomainSyntax::ScalarPhysical {
                        across_dimension: dimension_expression(
                            domain.across_dimension,
                            &path,
                            &mut ranges,
                            &mut paths,
                        ),
                        through_dimension: dimension_expression(
                            domain.through_dimension,
                            &path,
                            &mut ranges,
                            &mut paths,
                        ),
                    },
                    range,
                }),
                DraftDeclaration::Representation(representation) => {
                    Item::Representation(RepresentationDecl {
                        name: representation.name.clone(),
                        syntax: RepresentationSyntax::Continuum,
                        range,
                    })
                }
                DraftDeclaration::Field(field) => Item::Field(FieldDecl {
                    name: field.name.clone(),
                    domain: field
                        .spatial_scope
                        .as_ref()
                        .map(|scope| scope.domain.name().to_owned()),
                    representation: field
                        .spatial_scope
                        .as_ref()
                        .map(|scope| scope.representation.name.clone()),
                    shape: None,
                    dimension: dimension_expression(
                        field.dimension,
                        &path,
                        &mut ranges,
                        &mut paths,
                    ),
                    initial: Some(field.initial),
                    range,
                }),
                DraftDeclaration::Parameter(parameter) => Item::Parameter(ParameterDecl {
                    name: parameter.name.clone(),
                    dimension: dimension_expression(
                        parameter.dimension,
                        &path,
                        &mut ranges,
                        &mut paths,
                    ),
                    initial: parameter.value,
                    range,
                }),
                DraftDeclaration::ConservingPort(port) => Item::Port(PortDecl {
                    name: port.name.clone(),
                    syntax: PortSyntax::ScalarPhysical {
                        domain: port.domain.name.clone(),
                    },
                    range,
                }),
                DraftDeclaration::Relation(relation) => Item::Relation(RelationDecl {
                    name: relation.name.clone(),
                    activation: ActivationSyntax::Continuous,
                    domain: relation
                        .domain
                        .as_ref()
                        .map(|domain| domain.name().to_owned()),
                    residuals: relation
                        .residuals
                        .iter()
                        .map(|expression| expression.ast(&path, &mut ranges, &mut paths))
                        .collect(),
                    range,
                }),
                DraftDeclaration::ConservingConnection(connection) => {
                    Item::Connection(ConnectionDecl {
                        syntax: ConnectionSyntax::Conserving,
                        ports: connection
                            .ports
                            .iter()
                            .map(|port| NamePath::single(port.name.clone(), range))
                            .collect(),
                        range,
                    })
                }
            };
            items.push(item);
        }

        let model_path = GraphPath::new([self.name.clone()]);
        let range = ranges.allocate(&model_path, &mut paths);
        NativeModelAst {
            model: ModelDecl {
                name: self.name.clone(),
                items,
                range,
            },
            paths,
        }
    }
}

/// One declaration admitted by the first native-construction slice.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DraftDeclaration {
    /// Cartesian volume or one oriented boundary Domain.
    SpatialDomain(DraftSpatialDomain),
    /// Nominal scalar physical Domain.
    PhysicalDomain(DraftPhysicalDomain),
    /// Continuous pre-discretization Representation.
    Representation(DraftRepresentation),
    /// Mutable scalar state.
    Field(DraftField),
    /// Revision-local scalar design value.
    Parameter(DraftParameter),
    /// Scalar conserving Port on one nominal physical Domain.
    ConservingPort(DraftConservingPort),
    /// Continuous implicit residual group.
    Relation(DraftRelation),
    /// Anonymous N-ary conserving connection net.
    ConservingConnection(DraftConservingConnection),
}

impl DraftDeclaration {
    fn name(&self) -> Option<&str> {
        match self {
            Self::SpatialDomain(value) => Some(value.name()),
            Self::PhysicalDomain(value) => Some(value.name()),
            Self::Representation(value) => Some(value.name()),
            Self::Field(value) => Some(value.name()),
            Self::Parameter(value) => Some(value.name()),
            Self::ConservingPort(value) => Some(value.name()),
            Self::Relation(value) => Some(value.name()),
            Self::ConservingConnection(_) => None,
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::SpatialDomain(_) => "SpatialDomain",
            Self::PhysicalDomain(_) => "PhysicalDomain",
            Self::Representation(_) => "Representation",
            Self::Field(_) => "Field",
            Self::Parameter(_) => "Parameter",
            Self::ConservingPort(_) => "ConservingPort",
            Self::Relation(_) => "Relation",
            Self::ConservingConnection(_) => "ConservingConnection",
        }
    }
}

impl From<DraftSpatialDomain> for DraftDeclaration {
    fn from(value: DraftSpatialDomain) -> Self {
        Self::SpatialDomain(value)
    }
}

impl From<DraftPhysicalDomain> for DraftDeclaration {
    fn from(value: DraftPhysicalDomain) -> Self {
        Self::PhysicalDomain(value)
    }
}

impl From<DraftRepresentation> for DraftDeclaration {
    fn from(value: DraftRepresentation) -> Self {
        Self::Representation(value)
    }
}

impl From<DraftField> for DraftDeclaration {
    fn from(value: DraftField) -> Self {
        Self::Field(value)
    }
}

impl From<DraftParameter> for DraftDeclaration {
    fn from(value: DraftParameter) -> Self {
        Self::Parameter(value)
    }
}

impl From<DraftConservingPort> for DraftDeclaration {
    fn from(value: DraftConservingPort) -> Self {
        Self::ConservingPort(value)
    }
}

impl From<DraftRelation> for DraftDeclaration {
    fn from(value: DraftRelation) -> Self {
        Self::Relation(value)
    }
}

impl From<DraftConservingConnection> for DraftDeclaration {
    fn from(value: DraftConservingConnection) -> Self {
        Self::ConservingConnection(value)
    }
}

/// Immutable nominal Domain for one scalar across/through pair.
///
/// Domain compatibility follows this handle's identity. Equal names and
/// equal dimensions never make separately constructed Domains compatible.
/// Closing a draft containing this declaration requires an explicitly
/// selected v2 model wire at the application boundary; legacy convenience
/// entry points intentionally remain exact v1 defaults.
#[derive(Debug, Clone)]
pub struct DraftPhysicalDomain {
    symbol: DraftSymbol,
    name: String,
    across_dimension: DimExponents,
    through_dimension: DimExponents,
}

impl DraftPhysicalDomain {
    /// Declare one nominal scalar physical Domain.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    ) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            across_dimension,
            through_dimension,
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension of the across variable.
    #[must_use]
    pub const fn across_dimension(&self) -> DimExponents {
        self.across_dimension
    }

    /// Static SI dimension of the through variable.
    #[must_use]
    pub const fn through_dimension(&self) -> DimExponents {
        self.through_dimension
    }
}

/// Immutable scalar conserving Port bound to one draft-local Domain identity.
#[derive(Debug, Clone)]
pub struct DraftConservingPort {
    symbol: DraftSymbol,
    name: String,
    domain: DraftPhysicalDomain,
}

impl DraftConservingPort {
    /// Declare one scalar conserving Port on `domain`.
    #[must_use]
    pub fn new(name: impl Into<String>, domain: &DraftPhysicalDomain) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            domain: domain.clone(),
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Nominal scalar physical Domain handle.
    #[must_use]
    pub const fn domain(&self) -> &DraftPhysicalDomain {
        &self.domain
    }
}

/// Immutable anonymous conserving connection net.
#[derive(Debug, Clone)]
pub struct DraftConservingConnection {
    ports: Vec<DraftConservingPort>,
}

impl DraftConservingConnection {
    /// Request one N-ary conserving connection.
    ///
    /// Membership is checked atomically by [`ModelDraft::new`]. In
    /// particular, the closed draft requires at least two distinct declared
    /// Ports on the exact same nominal Domain, and each Port may belong to at
    /// most one Connection.
    #[must_use]
    pub fn new<'a>(ports: impl IntoIterator<Item = &'a DraftConservingPort>) -> Self {
        Self {
            ports: ports.into_iter().cloned().collect(),
        }
    }

    /// Member Ports in request order.
    #[must_use]
    pub fn ports(&self) -> &[DraftConservingPort] {
        &self.ports
    }
}

/// Immutable scalar Field declaration, either local or spatially supported.
#[derive(Debug, Clone)]
pub struct DraftField {
    symbol: DraftSymbol,
    name: String,
    dimension: DimExponents,
    initial: f64,
    spatial_scope: Option<DraftSpatialScope>,
}

#[derive(Debug, Clone)]
struct DraftSpatialScope {
    domain: DraftSpatialDomain,
    representation: DraftRepresentation,
}

impl DraftField {
    /// Declare one scalar Field in coherent SI units.
    #[must_use]
    pub fn new(name: impl Into<String>, dimension: DimExponents, initial: f64) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            dimension,
            initial,
            spatial_scope: None,
        }
    }

    /// Declare one scalar Field over an exact draft-local Domain and
    /// continuum Representation.
    #[must_use]
    pub fn spatial_scalar(
        name: impl Into<String>,
        domain: &DraftSpatialDomain,
        representation: &DraftRepresentation,
        dimension: DimExponents,
        initial: f64,
    ) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            dimension,
            initial,
            spatial_scope: Some(DraftSpatialScope {
                domain: domain.clone(),
                representation: representation.clone(),
            }),
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Initial value in coherent SI units.
    #[must_use]
    pub const fn initial(&self) -> f64 {
        self.initial
    }

    /// Exact draft-local spatial Domain, when distributed.
    #[must_use]
    pub fn domain(&self) -> Option<&DraftSpatialDomain> {
        self.spatial_scope.as_ref().map(|scope| &scope.domain)
    }

    /// Exact draft-local continuum Representation, when distributed.
    #[must_use]
    pub fn representation(&self) -> Option<&DraftRepresentation> {
        self.spatial_scope
            .as_ref()
            .map(|scope| &scope.representation)
    }

    /// Use this Field as a scalar expression.
    #[must_use]
    pub fn expression(&self) -> DraftExpression {
        DraftExpression::reference(
            self.symbol.clone(),
            self.name.clone(),
            DraftSymbolKind::Field,
        )
    }
}

/// Immutable scalar Parameter declaration.
#[derive(Debug, Clone)]
pub struct DraftParameter {
    symbol: DraftSymbol,
    name: String,
    dimension: DimExponents,
    value: f64,
}

impl DraftParameter {
    /// Declare one scalar Parameter in coherent SI units.
    #[must_use]
    pub fn new(name: impl Into<String>, dimension: DimExponents, value: f64) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            dimension,
            value,
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Value in coherent SI units.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Use this Parameter as a scalar expression.
    #[must_use]
    pub fn expression(&self) -> DraftExpression {
        DraftExpression::reference(
            self.symbol.clone(),
            self.name.clone(),
            DraftSymbolKind::Parameter,
        )
    }
}

/// Immutable continuous implicit Relation declaration.
#[derive(Debug, Clone)]
pub struct DraftRelation {
    name: String,
    domain: Option<DraftSpatialDomain>,
    residuals: Vec<DraftExpression>,
}

impl DraftRelation {
    /// Declare residual expressions whose canonical meaning is zero.
    #[must_use]
    pub fn continuous(
        name: impl Into<String>,
        residuals: impl IntoIterator<Item = DraftExpression>,
    ) -> Self {
        Self {
            name: name.into(),
            domain: None,
            residuals: residuals.into_iter().collect(),
        }
    }

    /// Declare continuous residuals on one exact draft-local spatial Domain.
    #[must_use]
    pub fn continuous_on(
        name: impl Into<String>,
        domain: &DraftSpatialDomain,
        residuals: impl IntoIterator<Item = DraftExpression>,
    ) -> Self {
        Self {
            name: name.into(),
            domain: Some(domain.clone()),
            residuals: residuals.into_iter().collect(),
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Residuals in declared order.
    #[must_use]
    pub fn residuals(&self) -> &[DraftExpression] {
        &self.residuals
    }

    /// Exact draft-local support Domain, when spatially scoped.
    #[must_use]
    pub const fn domain(&self) -> Option<&DraftSpatialDomain> {
        self.domain.as_ref()
    }
}

/// Immutable symbolic expression used only while defining a native model.
///
/// Shape and spatial support remain opaque here. The shared semantic
/// validator infers them and requires every finalized Relation residual to be
/// scalar.
#[derive(Debug, Clone)]
pub struct DraftExpression {
    kind: DraftExpressionKind,
}

impl DraftExpression {
    /// Dimensionless numeric literal.
    #[must_use]
    pub const fn constant(value: f64) -> Self {
        Self {
            kind: DraftExpressionKind::Constant(value),
        }
    }

    fn reference(symbol: DraftSymbol, name: String, kind: DraftSymbolKind) -> Self {
        Self {
            kind: DraftExpressionKind::Reference(DraftReference { symbol, name, kind }),
        }
    }

    /// Time derivative of one Field.
    #[must_use]
    pub fn derivative(field: &DraftField) -> Self {
        Self {
            kind: DraftExpressionKind::Derivative(DraftReference {
                symbol: field.symbol.clone(),
                name: field.name.clone(),
                kind: DraftSymbolKind::Field,
            }),
        }
    }

    /// Read the across variable of one scalar conserving Port.
    #[must_use]
    pub fn across(port: &DraftConservingPort) -> Self {
        Self {
            kind: DraftExpressionKind::Across(DraftPortReference::from(port)),
        }
    }

    /// Read the through variable of one scalar conserving Port.
    #[must_use]
    pub fn through(port: &DraftConservingPort) -> Self {
        Self {
            kind: DraftExpressionKind::Through(DraftPortReference::from(port)),
        }
    }

    /// Spatial gradient of one expression.
    #[must_use]
    pub fn gradient(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Gradient, value)
    }

    /// Spatial divergence of one expression.
    #[must_use]
    pub fn divergence(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Divergence, value)
    }

    /// Boundary trace of one expression.
    #[must_use]
    pub fn trace(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Trace, value)
    }

    fn spatial_call(operator: DraftSpatialOperator, value: Self) -> Self {
        Self {
            kind: DraftExpressionKind::SpatialCall {
                operator,
                value: Box::new(value),
            },
        }
    }

    fn binary(self, operator: BinaryOp, right: Self) -> Self {
        Self {
            kind: DraftExpressionKind::Binary {
                operator,
                left: Box::new(self),
                right: Box::new(right),
            },
        }
    }

    fn references<'a>(&'a self, output: &mut Vec<DraftExpressionReference<'a>>) {
        match &self.kind {
            DraftExpressionKind::Constant(_) => {}
            DraftExpressionKind::Reference(reference)
            | DraftExpressionKind::Derivative(reference) => {
                output.push(DraftExpressionReference::Value(reference));
            }
            DraftExpressionKind::Across(reference) | DraftExpressionKind::Through(reference) => {
                output.push(DraftExpressionReference::Port(reference));
            }
            DraftExpressionKind::Neg(value) | DraftExpressionKind::SpatialCall { value, .. } => {
                value.references(output);
            }
            DraftExpressionKind::Binary { left, right, .. } => {
                left.references(output);
                right.references(output);
            }
        }
    }

    fn contains_non_finite_constant(&self) -> bool {
        match &self.kind {
            DraftExpressionKind::Constant(value) => !value.is_finite(),
            DraftExpressionKind::Reference(_)
            | DraftExpressionKind::Derivative(_)
            | DraftExpressionKind::Across(_)
            | DraftExpressionKind::Through(_) => false,
            DraftExpressionKind::Neg(value) | DraftExpressionKind::SpatialCall { value, .. } => {
                value.contains_non_finite_constant()
            }
            DraftExpressionKind::Binary { left, right, .. } => {
                left.contains_non_finite_constant() || right.contains_non_finite_constant()
            }
        }
    }

    fn ast(
        &self,
        path: &GraphPath,
        ranges: &mut RangeAllocator,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> Expr {
        let kind = match &self.kind {
            DraftExpressionKind::Constant(value) => ExprKind::Number(*value),
            DraftExpressionKind::Reference(reference) => ExprKind::Name(reference.name.clone()),
            DraftExpressionKind::Derivative(reference) => ExprKind::Call {
                callee: NamePath::single("derivative".to_owned(), ranges.allocate(path, paths)),
                arguments: vec![Expr {
                    kind: ExprKind::Name(reference.name.clone()),
                    range: ranges.allocate(path, paths),
                }],
            },
            DraftExpressionKind::Across(reference) => {
                physical_accessor_ast("across", reference, path, ranges, paths)
            }
            DraftExpressionKind::Through(reference) => {
                physical_accessor_ast("through", reference, path, ranges, paths)
            }
            DraftExpressionKind::SpatialCall { operator, value } => ExprKind::Call {
                callee: NamePath::single(
                    operator.source_name().to_owned(),
                    ranges.allocate(path, paths),
                ),
                arguments: vec![value.ast(path, ranges, paths)],
            },
            DraftExpressionKind::Neg(value) => ExprKind::Unary {
                op: UnaryOp::Neg,
                value: Box::new(value.ast(path, ranges, paths)),
            },
            DraftExpressionKind::Binary {
                operator,
                left,
                right,
            } => ExprKind::Binary {
                op: *operator,
                left: Box::new(left.ast(path, ranges, paths)),
                right: Box::new(right.ast(path, ranges, paths)),
            },
        };
        Expr {
            kind,
            range: ranges.allocate(path, paths),
        }
    }
}

impl Neg for DraftExpression {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            kind: DraftExpressionKind::Neg(Box::new(self)),
        }
    }
}

macro_rules! impl_binary_expression_operator {
    ($trait:ident, $method:ident, $operator:expr) => {
        impl $trait for DraftExpression {
            type Output = Self;

            fn $method(self, right: Self) -> Self::Output {
                self.binary($operator, right)
            }
        }
    };
}

impl_binary_expression_operator!(Add, add, BinaryOp::Add);
impl_binary_expression_operator!(Sub, sub, BinaryOp::Sub);
impl_binary_expression_operator!(Mul, mul, BinaryOp::Mul);
impl_binary_expression_operator!(Div, div, BinaryOp::Div);

#[derive(Debug, Clone)]
enum DraftExpressionKind {
    Constant(f64),
    Reference(DraftReference),
    Derivative(DraftReference),
    Across(DraftPortReference),
    Through(DraftPortReference),
    SpatialCall {
        operator: DraftSpatialOperator,
        value: Box<DraftExpression>,
    },
    Neg(Box<DraftExpression>),
    Binary {
        operator: BinaryOp,
        left: Box<DraftExpression>,
        right: Box<DraftExpression>,
    },
}

#[derive(Debug, Clone, Copy)]
enum DraftSpatialOperator {
    Gradient,
    Divergence,
    Trace,
}

impl DraftSpatialOperator {
    const fn source_name(self) -> &'static str {
        match self {
            Self::Gradient => "grad",
            Self::Divergence => "div",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone)]
struct DraftReference {
    symbol: DraftSymbol,
    name: String,
    kind: DraftSymbolKind,
}

#[derive(Debug, Clone)]
struct DraftPortReference {
    symbol: DraftSymbol,
    name: String,
}

impl From<&DraftConservingPort> for DraftPortReference {
    fn from(port: &DraftConservingPort) -> Self {
        Self {
            symbol: port.symbol.clone(),
            name: port.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DraftExpressionReference<'a> {
    Value(&'a DraftReference),
    Port(&'a DraftPortReference),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftSymbolKind {
    Field,
    Parameter,
}

impl DraftSymbolKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Field => "Field",
            Self::Parameter => "Parameter",
        }
    }
}

#[derive(Clone)]
pub(crate) struct DraftSymbol(Arc<()>);

impl DraftSymbol {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl fmt::Debug for DraftSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DraftSymbol(<local>)")
    }
}

impl PartialEq for DraftSymbol {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for DraftSymbol {}

impl Hash for DraftSymbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

fn physical_accessor_ast(
    callee: &str,
    reference: &DraftPortReference,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> ExprKind {
    ExprKind::Call {
        callee: NamePath::single(callee.to_owned(), ranges.allocate(path, paths)),
        arguments: vec![Expr {
            kind: ExprKind::Name(reference.name.clone()),
            range: ranges.allocate(path, paths),
        }],
    }
}

/// Synthetic AST plus paths that recover native declaration context.
#[doc(hidden)]
#[derive(Debug)]
pub struct NativeModelAst {
    model: ModelDecl,
    paths: HashMap<TextRange, GraphPath>,
}

impl NativeModelAst {
    /// Source-shaped model consumed by the shared compiler lowerer.
    #[must_use]
    pub const fn model(&self) -> &ModelDecl {
        &self.model
    }

    /// Native declaration path associated with one synthetic range.
    #[must_use]
    pub fn graph_path(&self, range: TextRange) -> Option<&GraphPath> {
        self.paths.get(&range)
    }
}

#[derive(Debug, Default)]
struct RangeAllocator {
    next: u32,
}

impl RangeAllocator {
    fn allocate(
        &mut self,
        path: &GraphPath,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> TextRange {
        let start = self.next;
        self.next = self.next.saturating_add(1);
        let range = TextRange::new(start, self.next);
        paths.insert(range, path.clone());
        range
    }
}

fn dimension_expression(
    dimension: DimExponents,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> Expr {
    let exponents = [
        ("kg", dimension.mass),
        ("m", dimension.length),
        ("s", dimension.time),
        ("A", dimension.current),
        ("K", dimension.temperature),
        ("mol", dimension.amount),
        ("cd", dimension.luminous_intensity),
    ];
    let mut factors = exponents
        .into_iter()
        .filter(|(_, exponent)| *exponent != 0)
        .map(|(name, exponent)| {
            let base = Expr {
                kind: ExprKind::Name(name.to_owned()),
                range: ranges.allocate(path, paths),
            };
            if exponent == 1 {
                base
            } else {
                Expr {
                    kind: ExprKind::Binary {
                        op: BinaryOp::Pow,
                        left: Box::new(base),
                        right: Box::new(Expr {
                            kind: ExprKind::Number(f64::from(exponent)),
                            range: ranges.allocate(path, paths),
                        }),
                    },
                    range: ranges.allocate(path, paths),
                }
            }
        })
        .collect::<Vec<_>>()
        .into_iter();

    let Some(first) = factors.next() else {
        return Expr {
            kind: ExprKind::Number(1.0),
            range: ranges.allocate(path, paths),
        };
    };
    factors.fold(first, |left, right| Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: Box::new(left),
            right: Box::new(right),
        },
        range: ranges.allocate(path, paths),
    })
}

fn native_diagnostic(model: &str, declaration: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_TYPE_ERROR, message)
        .with_graph_path(GraphPath::new([model.to_owned(), declaration.to_owned()]))
}

fn connection_path(connection: &DraftConservingConnection) -> String {
    let mut members = connection
        .ports()
        .iter()
        .map(DraftConservingPort::name)
        .collect::<Vec<_>>();
    members.sort_unstable();
    format!("connection[{}]", members.join(","))
}

fn is_language_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft_spatial::DraftBoundarySide;

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

    #[test]
    fn native_draft_rejects_foreign_symbol_even_when_name_matches() {
        let included = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
        let foreign = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
        let relation = DraftRelation::continuous("flow", [foreign.expression()]);

        let diagnostic = ModelDraft::new("decay", [included.into(), relation.into()]).unwrap_err();
        assert_eq!(diagnostic[0].code(), codes::LANGUAGE_TYPE_ERROR);
        assert_eq!(
            diagnostic[0].graph_path().unwrap().to_string(),
            "decay.flow"
        );
    }

    #[test]
    fn typed_dimensions_and_expression_references_become_source_ast() {
        let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
        let rate = DraftParameter::new(
            "rate",
            DimExponents {
                time: -1,
                ..DimExponents::DIMENSIONLESS
            },
            1.0,
        );
        let residual = DraftExpression::derivative(&state) + rate.expression() * state.expression();
        let draft = ModelDraft::new(
            "decay",
            [
                state.into(),
                rate.into(),
                DraftRelation::continuous("flow", [residual]).into(),
            ],
        )
        .unwrap();

        let native = draft.native_ast();
        assert_eq!(native.model().name(), "decay");
        assert_eq!(native.model().items().len(), 3);
        assert!(native.graph_path(native.model().range()).is_some());
    }

    #[test]
    fn native_draft_rejects_names_and_numbers_source_could_not_express() {
        let field = DraftField::new("not valid", DimExponents::DIMENSIONLESS, f64::INFINITY);
        let relation = DraftRelation::continuous(
            "flow",
            [field.expression() + DraftExpression::constant(f64::NAN)],
        );

        let diagnostics = ModelDraft::new("", [field.into(), relation.into()]).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("model name"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("declaration name"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("must be finite"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("numeric literal"))
        );
    }

    #[test]
    fn physical_vocabulary_projects_only_to_existing_source_ast_forms() {
        let electrical =
            DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
        let positive = DraftConservingPort::new("positive", &electrical);
        let negative = DraftConservingPort::new("negative", &electrical);
        let resistance = DraftParameter::new(
            "resistance",
            DimExponents {
                mass: 1,
                length: 2,
                time: -3,
                current: -2,
                ..DimExponents::DIMENSIONLESS
            },
            2.0,
        );
        let relation = DraftRelation::continuous(
            "resistor",
            [
                DraftExpression::across(&positive)
                    - DraftExpression::across(&negative)
                    - resistance.expression() * DraftExpression::through(&positive),
                DraftExpression::through(&positive) + DraftExpression::through(&negative),
            ],
        );
        let connection = DraftConservingConnection::new([&positive, &negative]);
        let draft = ModelDraft::new(
            "resistor",
            [
                electrical.into(),
                positive.into(),
                negative.into(),
                resistance.into(),
                relation.into(),
                connection.into(),
            ],
        )
        .unwrap();

        let native = draft.native_ast();
        let items = native.model().items();
        assert!(matches!(
            items[0],
            Item::Domain(DomainDecl {
                syntax: DomainSyntax::ScalarPhysical { .. },
                ..
            })
        ));
        assert!(matches!(
            items[1],
            Item::Port(PortDecl {
                syntax: PortSyntax::ScalarPhysical { .. },
                ..
            })
        ));
        let Item::Relation(relation) = &items[4] else {
            panic!("fifth item must be a Relation");
        };
        assert_eq!(relation.residuals().len(), 2);
        assert!(relation.residuals().iter().any(|residual| {
            expression_contains_call(residual, "across")
                && expression_contains_call(residual, "through")
        }));
        let Item::Connection(connection) = &items[5] else {
            panic!("sixth item must be a Connection");
        };
        assert_eq!(connection.syntax(), ConnectionSyntax::Conserving);
        assert_eq!(
            connection.ports().collect::<Vec<_>>(),
            ["positive", "negative"]
        );
        assert!(native.graph_path(connection.range()).is_some());
    }

    #[test]
    fn draft_closure_rejects_foreign_domain_and_port_identity_before_rebinding_names() {
        let declared_domain =
            DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
        let foreign_domain =
            DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
        let declared_port = DraftConservingPort::new("terminal", &declared_domain);
        let foreign_domain_port = DraftConservingPort::new("foreign_domain", &foreign_domain);
        let foreign_port = DraftConservingPort::new("terminal", &declared_domain);
        let relation = DraftRelation::continuous("owner", [DraftExpression::across(&foreign_port)]);

        let diagnostics = ModelDraft::new(
            "identity",
            [
                declared_domain.into(),
                declared_port.into(),
                foreign_domain_port.into(),
                relation.into(),
            ],
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("foreign or omitted scalar physical Domain")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("foreign or omitted conserving Port `terminal`")
        }));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.source_span().is_none())
        );
    }

    #[test]
    fn draft_closure_rejects_invalid_connection_membership_atomically() {
        let electrical =
            DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
        let other = DraftPhysicalDomain::new("other", voltage_dimension(), current_dimension());
        let a = DraftConservingPort::new("a", &electrical);
        let b = DraftConservingPort::new("b", &electrical);
        let incompatible = DraftConservingPort::new("incompatible", &other);
        let foreign = DraftConservingPort::new("a", &electrical);
        let diagnostics = ModelDraft::new(
            "invalid_connections",
            [
                electrical.into(),
                other.into(),
                a.clone().into(),
                b.clone().into(),
                incompatible.clone().into(),
                DraftConservingConnection::new([&a]).into(),
                DraftConservingConnection::new([&a, &b, &b]).into(),
                DraftConservingConnection::new([&incompatible, &b, &foreign]).into(),
            ],
        )
        .unwrap_err();

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("at least two Ports"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("repeats Port `b`"))
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("already belongs to another Connection")
        }));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains("foreign or omitted Port `a`"))
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("exact same draft-local scalar physical Domain")
        }));
    }

    #[test]
    fn duplicate_names_are_rejected_across_physical_and_scalar_declarations() {
        let domain = DraftPhysicalDomain::new("shared", voltage_dimension(), current_dimension());
        let field = DraftField::new("shared", DimExponents::DIMENSIONLESS, 0.0);
        let diagnostics = ModelDraft::new("duplicates", [domain.into(), field.into()]).unwrap_err();
        assert!(
            diagnostics[0]
                .message()
                .contains("duplicate declaration `shared`")
        );
    }

    #[test]
    fn anonymous_connection_diagnostic_paths_follow_membership_not_declaration_position() {
        let domain =
            DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
        let terminal = DraftConservingPort::new("terminal", &domain);
        let unrelated = DraftField::new("x", DimExponents::DIMENSIONLESS, 0.0);
        let connection = DraftConservingConnection::new([&terminal]);
        let forward = ModelDraft::new(
            "stable_path",
            [
                domain.clone().into(),
                terminal.clone().into(),
                connection.clone().into(),
                unrelated.clone().into(),
            ],
        )
        .unwrap_err();
        let reordered = ModelDraft::new(
            "stable_path",
            [
                unrelated.into(),
                connection.into(),
                terminal.into(),
                domain.into(),
            ],
        )
        .unwrap_err();

        assert_eq!(
            forward[0].graph_path().unwrap(),
            reordered[0].graph_path().unwrap()
        );
        assert_eq!(
            forward[0].graph_path().unwrap().to_string(),
            "stable_path.connection[terminal]"
        );
    }

    #[test]
    fn spatial_draft_retains_exact_scope_identity_before_ast_projection() {
        let included = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
        let foreign = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
        let included_space = DraftRepresentation::continuum("space");
        let foreign_space = DraftRepresentation::continuum("space");
        let field = DraftField::spatial_scalar(
            "u",
            &foreign,
            &foreign_space,
            DimExponents::DIMENSIONLESS,
            0.0,
        );
        let diagnostics = ModelDraft::new(
            "foreign_scope",
            [included.into(), included_space.into(), field.into()],
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("foreign or omitted Domain `interval`")
        }));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("foreign or omitted Representation `space`")
        }));
    }

    #[test]
    fn spatial_draft_projects_only_to_existing_source_ast_forms() {
        let interval = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
        let lower = DraftSpatialDomain::boundary("lower", &interval, 0, DraftBoundarySide::Lower);
        let space = DraftRepresentation::continuum("space");
        let field =
            DraftField::spatial_scalar("u", &interval, &space, DimExponents::DIMENSIONLESS, 0.0);
        let balance = DraftRelation::continuous_on(
            "balance",
            &interval,
            [-DraftExpression::divergence(DraftExpression::gradient(
                field.expression(),
            ))],
        );
        let boundary = DraftRelation::continuous_on(
            "lower_value",
            &lower,
            [DraftExpression::trace(field.expression())],
        );
        let draft = ModelDraft::new(
            "poisson",
            [
                interval.into(),
                lower.into(),
                space.into(),
                field.into(),
                balance.into(),
                boundary.into(),
            ],
        )
        .unwrap();

        let native = draft.native_ast();
        assert!(matches!(
            native.model().items()[0],
            Item::Domain(DomainDecl {
                syntax: DomainSyntax::CartesianBox(_),
                ..
            })
        ));
        assert!(matches!(
            native.model().items()[1],
            Item::Domain(DomainDecl {
                syntax: DomainSyntax::Boundary { .. },
                ..
            })
        ));
        assert!(matches!(
            native.model().items()[2],
            Item::Representation(RepresentationDecl {
                syntax: RepresentationSyntax::Continuum,
                ..
            })
        ));
        let Item::Relation(relation) = &native.model().items()[4] else {
            panic!("fifth item must be a Relation");
        };
        assert_eq!(relation.domain(), Some("interval"));
        assert!(expression_contains_call(&relation.residuals()[0], "grad"));
        assert!(expression_contains_call(&relation.residuals()[0], "div"));
    }

    fn expression_contains_call(expression: &Expr, expected: &str) -> bool {
        match expression.kind() {
            ExprKind::Call { callee, arguments } => {
                callee.as_str() == expected
                    || arguments
                        .iter()
                        .any(|argument| expression_contains_call(argument, expected))
            }
            ExprKind::Unary { value, .. } => expression_contains_call(value, expected),
            ExprKind::Binary { left, right, .. } => {
                expression_contains_call(left, expected)
                    || expression_contains_call(right, expected)
            }
            ExprKind::Number(_)
            | ExprKind::Name(_)
            | ExprKind::Path(_)
            | ExprKind::BoundaryPortSelection { .. } => false,
        }
    }
}
