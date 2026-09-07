//! Client-neutral, immutable declarations for native model construction.
//!
//! Drafts are an ergonomic control-plane input. They are neither accepted
//! Semantic Models nor a durable wire format. The compiler lowers their
//! synthetic AST through exactly the same path as parsed source.

use std::collections::{HashMap, HashSet};
use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::ast::{BinaryOp, Expr, ExprKind, ModelDecl, NamePath, TextRange, UnaryOp};
use crate::draft_spatial::{DraftSpatialDomain, DraftSpatialDomainKind};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, GraphPath, ValueLiteral, ValueType};

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
                    if !value.across_type.shape().is_scalar()
                        || !value.through_type.shape().is_scalar()
                    {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            value.name(),
                            "scalar physical quantities require scalar mathematical types",
                        ));
                    }
                }
                DraftDeclaration::SpatialDomain(value) => {
                    spatial_domain_symbols.insert(value.symbol().clone());
                }
                DraftDeclaration::ConservingPort(value) => {
                    ports.insert(value.symbol.clone(), value);
                }
                DraftDeclaration::Relation(_)
                | DraftDeclaration::Initial(_)
                | DraftDeclaration::ConservingConnection(_) => {}
            }
            if let Some((path, residuals)) = declaration.equations() {
                if residuals.is_empty() {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        path,
                        "equation group requires at least one residual",
                    ));
                }
                if residuals
                    .iter()
                    .any(DraftExpression::contains_non_finite_constant)
                {
                    diagnostics.push(native_diagnostic(
                        &self.name,
                        path,
                        "equation group contains a non-finite numeric literal",
                    ));
                }
            }
            match declaration {
                DraftDeclaration::Field(field) => {
                    if let Err(message) = value_type::validate(&field.value_type) {
                        diagnostics.push(native_diagnostic(&self.name, field.name(), message));
                    }
                }
                DraftDeclaration::Parameter(parameter) => {
                    if let Err(error) = crate::SourceAstFactory::value_literal(
                        parameter.value(),
                        TextRange::new(0, 1),
                    ) {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            parameter.name(),
                            error.to_string(),
                        ));
                    }
                    if let Err(message) = value_type::validate(parameter.value_type()) {
                        diagnostics.push(native_diagnostic(&self.name, parameter.name(), message));
                    }
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
            let Some((path, residuals)) = declaration.equations() else {
                continue;
            };
            let mut referenced = Vec::new();
            for residual in residuals {
                residual.references(&mut referenced);
            }
            for reference in referenced {
                match reference {
                    DraftExpressionReference::Value(reference)
                        if !value_symbols.contains(&reference.symbol) =>
                    {
                        diagnostics.push(native_diagnostic(
                            &self.name,
                            path,
                            format!(
                                "equation group `{path}` references foreign or omitted {} `{}`",
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
                            path,
                            format!(
                                "equation group `{path}` references foreign or omitted conserving Port `{}`",
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
}

/// One declaration admitted by the first native-construction slice.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DraftDeclaration {
    /// Cartesian volume or one oriented boundary Domain.
    SpatialDomain(DraftSpatialDomain),
    /// Nominal scalar physical Domain.
    PhysicalDomain(DraftPhysicalDomain),
    /// Mutable scalar state.
    Field(DraftField),
    /// Revision-local scalar design value.
    Parameter(DraftParameter),
    /// Scalar conserving Port on one nominal physical Domain.
    ConservingPort(DraftConservingPort),
    /// Continuous implicit residual group.
    Relation(DraftRelation),
    /// Simultaneous fresh-initialization residuals, each equal to zero.
    ///
    /// These are mathematical conditions, separate from numerical guesses.
    /// Empty groups, non-finite constants, and foreign references are rejected
    /// when closing the draft. Types are checked by the common compiler.
    Initial(Vec<DraftExpression>),
    /// Anonymous N-ary conserving connection net.
    ConservingConnection(DraftConservingConnection),
}

impl DraftDeclaration {
    fn name(&self) -> Option<&str> {
        match self {
            Self::SpatialDomain(value) => Some(value.name()),
            Self::PhysicalDomain(value) => Some(value.name()),
            Self::Field(value) => Some(value.name()),
            Self::Parameter(value) => Some(value.name()),
            Self::ConservingPort(value) => Some(value.name()),
            Self::Relation(value) => Some(value.name()),
            Self::Initial(_) | Self::ConservingConnection(_) => None,
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            Self::SpatialDomain(_) => "SpatialDomain",
            Self::PhysicalDomain(_) => "PhysicalDomain",
            Self::Field(_) => "Field",
            Self::Parameter(_) => "Parameter",
            Self::ConservingPort(_) => "ConservingPort",
            Self::Relation(_) => "Relation",
            Self::Initial(_) => "Initial",
            Self::ConservingConnection(_) => "ConservingConnection",
        }
    }

    fn equations(&self) -> Option<(&str, &[DraftExpression])> {
        match self {
            Self::Relation(relation) => Some((relation.name(), &relation.residuals)),
            Self::Initial(residuals) => Some(("initial", residuals)),
            _ => None,
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
    across_type: eqiora_core::ValueType,
    through_type: eqiora_core::ValueType,
}

impl DraftPhysicalDomain {
    /// Declare one nominal scalar physical Domain.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        across_type: eqiora_core::ValueType,
        through_type: eqiora_core::ValueType,
    ) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            across_type,
            through_type,
        }
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Complete mathematical type of the across variable.
    #[must_use]
    pub const fn across_type(&self) -> &eqiora_core::ValueType {
        &self.across_type
    }

    /// Complete mathematical type of the through variable.
    #[must_use]
    pub const fn through_type(&self) -> &eqiora_core::ValueType {
        &self.through_type
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

/// Immutable typed Field declaration, either local or spatially supported.
#[derive(Debug, Clone)]
pub struct DraftField {
    symbol: DraftSymbol,
    name: String,
    value_type: ValueType,
    role: crate::ast::FieldRoleSyntax,
    spatial_scope: Option<DraftSpatialScope>,
}

#[derive(Debug, Clone)]
struct DraftSpatialScope {
    domain: DraftSpatialDomain,
}

impl DraftField {
    /// Declare one unknown with its complete type and explicit evolution role.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        value_type: ValueType,
        role: crate::ast::FieldRoleSyntax,
    ) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            value_type,
            role,
            spatial_scope: None,
        }
    }

    /// Declare one typed unknown over an exact draft-local Domain.
    #[must_use]
    pub fn spatial(
        name: impl Into<String>,
        domain: &DraftSpatialDomain,
        value_type: ValueType,
        role: crate::ast::FieldRoleSyntax,
    ) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            value_type,
            role,
            spatial_scope: Some(DraftSpatialScope {
                domain: domain.clone(),
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
        self.value_type.dimension()
    }

    /// Complete mathematical type, independent of storage and execution.
    #[must_use]
    pub const fn value_type(&self) -> &ValueType {
        &self.value_type
    }

    /// Declared evolution role, independent of numerical initialization guesses.
    #[must_use]
    pub const fn role(&self) -> crate::ast::FieldRoleSyntax {
        self.role
    }

    /// Exact draft-local spatial Domain, when distributed.
    #[must_use]
    pub fn domain(&self) -> Option<&DraftSpatialDomain> {
        self.spatial_scope.as_ref().map(|scope| &scope.domain)
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

/// Immutable Parameter declaration owning one complete validated value.
#[derive(Debug, Clone)]
pub struct DraftParameter {
    symbol: DraftSymbol,
    name: String,
    value: ValueLiteral,
}

impl DraftParameter {
    /// Declare one complete Parameter value in coherent SI units.
    #[must_use]
    pub fn new(name: impl Into<String>, value: ValueLiteral) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
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
        self.value.value_type().dimension()
    }

    /// Complete declared mathematical type.
    #[must_use]
    pub const fn value_type(&self) -> &ValueType {
        self.value.value_type()
    }

    /// Complete value in coherent SI units, without scalar projection.
    #[must_use]
    pub const fn value(&self) -> &ValueLiteral {
        &self.value
    }

    /// Use this Parameter as a typed expression.
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

    /// Construct an ordered channel-array expression.
    #[must_use]
    pub fn array(values: impl IntoIterator<Item = Self>) -> Self {
        Self {
            kind: DraftExpressionKind::Array(values.into_iter().collect()),
        }
    }

    /// Select a static channel index. The compiler checks type and bounds.
    #[must_use]
    pub fn index(self, index: u32) -> Self {
        Self {
            kind: DraftExpressionKind::Index {
                value: Box::new(self),
                index,
            },
        }
    }

    /// Construct a complex scalar without discarding either component.
    #[must_use]
    pub fn complex(real: f64, imaginary: f64) -> Self {
        Self {
            kind: DraftExpressionKind::Complex(real, imaginary),
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
            DraftExpressionKind::Constant(_) | DraftExpressionKind::Complex(_, _) => {}
            DraftExpressionKind::Array(values) => {
                for value in values {
                    value.references(output);
                }
            }
            DraftExpressionKind::Index { value, .. } => value.references(output),
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
            DraftExpressionKind::Complex(real, imaginary) => {
                !real.is_finite() || !imaginary.is_finite()
            }
            DraftExpressionKind::Array(values) => {
                values.iter().any(Self::contains_non_finite_constant)
            }
            DraftExpressionKind::Index { value, .. } => value.contains_non_finite_constant(),
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

    /// Project an authored expression into the shared AST with synthetic ranges.
    #[doc(hidden)]
    #[must_use]
    pub fn source_ast(&self) -> Expr {
        self.ast(
            &GraphPath::new(["argument".to_owned()]),
            &mut RangeAllocator::default(),
            &mut HashMap::new(),
        )
    }

    fn ast(
        &self,
        path: &GraphPath,
        ranges: &mut RangeAllocator,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> Expr {
        let kind = match &self.kind {
            DraftExpressionKind::Constant(value) => ExprKind::Number(*value),
            DraftExpressionKind::Complex(real, imaginary) => ExprKind::Call {
                callee: NamePath::from_parsed_segments(
                    vec!["math".to_owned(), "complex".to_owned()],
                    ranges.allocate(path, paths),
                ),
                arguments: [*real, *imaginary]
                    .into_iter()
                    .map(|number| Expr {
                        kind: ExprKind::Number(number),
                        range: ranges.allocate(path, paths),
                    })
                    .collect(),
            },
            DraftExpressionKind::Array(values) => ExprKind::Array(
                values
                    .iter()
                    .map(|value| value.ast(path, ranges, paths))
                    .collect(),
            ),
            DraftExpressionKind::Index { value, index } => ExprKind::Index {
                value: Box::new(value.ast(path, ranges, paths)),
                index: Box::new(Expr {
                    kind: ExprKind::Number(f64::from(*index)),
                    range: ranges.allocate(path, paths),
                }),
            },
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
    Complex(f64, f64),
    Array(Vec<DraftExpression>),
    Index {
        value: Box<DraftExpression>,
        index: u32,
    },
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

/// Synthetic AST plus paths that recover native declaration context.
#[doc(hidden)]
#[derive(Debug)]
pub struct NativeModelAst {
    model: ModelDecl,
    paths: HashMap<TextRange, GraphPath>,
}

mod ast_bridge;
mod dimension;
mod symbol;
mod value_type;
use ast_bridge::{RangeAllocator, physical_accessor_ast};
use dimension::dimension_expression;
pub(crate) use symbol::DraftSymbol;

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
mod tests;
