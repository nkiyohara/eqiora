//! Checked owned-AST construction for compiler-side source transformations.

use core::fmt;

mod domain_validation;

use crate::ast::{
    ActivationSyntax, BoundaryConnectionDecl, BoundaryDecl, BoundaryFamilyBinderSyntax,
    BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax, BoundarySetBindingDecl,
    BoundarySetMemberSyntax, ClockDecl, ComponentDecl, ComponentItem, ComponentParameterDecl,
    ComponentPortDecl, ComponentPortFamilyDecl, ConnectionDecl, ConnectionSyntax, ConnectorDecl,
    ConnectorQuantitySyntax, ConnectorSyntax, Document, DomainDecl, DomainSyntax,
    ExactIntegerSyntax, Expr, ExprKind, FieldBindingDecl, FieldDecl, FieldSlotDecl, InstanceDecl,
    Item, ModelDecl, NamePath, ParameterBindingDecl, ParameterDecl, PortDecl, PortSyntax,
    PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind, PureOperatorFormal,
    PureValueClassSyntax, RationalSyntax, RelationDecl, RelationFamilyDecl, RepresentationDecl,
    RepresentationSyntax, SupportBindingDecl, SupportSlotDecl, SupportSlotSyntax, TextRange,
    ValueShapeSyntax, VisibilitySyntax,
};
use domain_validation::validate_domain_syntax;

/// A structural source-AST construction failure.
///
/// This error reports syntax-shape contradictions only. Name resolution,
/// dimensions, component visibility, and other semantic checks remain compiler
/// responsibilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstConstructionError {
    message: String,
}

impl AstConstructionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Stable human-readable explanation of the structural contradiction.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AstConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AstConstructionError {}

/// Checked factory for owned source AST values.
///
/// The factory is intentionally syntax-only. It gives parsers, elaborators,
/// and source transforms one construction boundary without exposing mutable
/// declaration fields or requiring a format-and-reparse cycle.
#[derive(Debug, Clone, Copy, Default)]
pub struct SourceAstFactory;

impl SourceAstFactory {
    /// Close one nonempty compilation unit into a formatter-compatible document.
    ///
    /// A declarations-only document is valid source syntax for a package
    /// library. Executable compilation still requires at least one Model.
    ///
    /// # Errors
    /// Returns an error when the compilation unit is empty.
    pub fn document(
        connectors: Vec<ConnectorDecl>,
        components: Vec<ComponentDecl>,
        models: Vec<ModelDecl>,
    ) -> Result<Document, AstConstructionError> {
        if connectors.is_empty() && components.is_empty() && models.is_empty() {
            return Err(AstConstructionError::new(
                "a source document requires at least one top-level declaration",
            ));
        }
        Ok(Document {
            connectors,
            components,
            pure_operators: Vec::new(),
            models,
        })
    }

    /// Close one nonempty compilation unit including pure operators.
    ///
    /// The three-argument [`Self::document`] constructor remains the compact
    /// compatibility entry point for clients that do not author operators.
    ///
    /// # Errors
    /// Returns an error when the compilation unit is empty.
    pub fn document_with_pure_operators(
        connectors: Vec<ConnectorDecl>,
        components: Vec<ComponentDecl>,
        pure_operators: Vec<PureOperatorDecl>,
        models: Vec<ModelDecl>,
    ) -> Result<Document, AstConstructionError> {
        if connectors.is_empty()
            && components.is_empty()
            && pure_operators.is_empty()
            && models.is_empty()
        {
            return Err(AstConstructionError::new(
                "a source document requires at least one top-level declaration",
            ));
        }
        Ok(Document {
            connectors,
            components,
            pure_operators,
            models,
        })
    }

    /// Close one or more flat models into a formatter-compatible document.
    ///
    /// # Errors
    /// Returns an error when `models` is empty, matching the source grammar.
    pub fn flat_document(models: Vec<ModelDecl>) -> Result<Document, AstConstructionError> {
        if models.is_empty() {
            return Err(AstConstructionError::new(
                "a source document requires at least one model",
            ));
        }
        Ok(Document {
            connectors: Vec::new(),
            components: Vec::new(),
            pure_operators: Vec::new(),
            models,
        })
    }

    /// Construct one exact integer token used by pure-operator syntax.
    ///
    /// # Errors
    /// Returns an error when `spelling` is not an unsigned decimal integer,
    /// does not fit in `u64`, or the source range is reversed.
    pub fn exact_integer(
        spelling: impl Into<String>,
        range: TextRange,
    ) -> Result<ExactIntegerSyntax, AstConstructionError> {
        let spelling = spelling.into();
        let value = spelling.parse::<u64>().map_err(|_| {
            AstConstructionError::new(format!(
                "exact integer `{spelling}` must be an unsigned decimal integer fitting in u64"
            ))
        })?;
        Ok(ExactIntegerSyntax {
            spelling,
            value,
            range: checked_range(range)?,
        })
    }

    /// Construct one ordered pure-operator formal.
    ///
    /// # Errors
    /// Returns an error for an invalid name, value class, or source range.
    pub fn pure_operator_formal(
        name: impl Into<String>,
        value_class: PureValueClassSyntax,
        range: TextRange,
    ) -> Result<PureOperatorFormal, AstConstructionError> {
        validate_pure_value_class(&value_class)?;
        Ok(PureOperatorFormal {
            name: checked_identifier(name, "pure operator formal")?,
            value_class,
            range: checked_range(range)?,
        })
    }

    /// Construct one bounded pure-operator expression.
    ///
    /// # Errors
    /// Returns an error for malformed exact syntax or a reversed source range.
    pub fn pure_operator_expression(
        kind: PureOperatorExprKind,
        range: TextRange,
    ) -> Result<PureOperatorExpr, AstConstructionError> {
        let expression = PureOperatorExpr {
            kind,
            range: checked_range(range)?,
        };
        validate_pure_operator_expression(&expression)?;
        Ok(expression)
    }

    /// Construct one top-level pure operator declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid name, an empty formal list, duplicate
    /// formal names, malformed exact syntax, or a reversed source range.
    pub fn pure_operator(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        formals: Vec<PureOperatorFormal>,
        result: PureValueClassSyntax,
        body: PureOperatorExpr,
        range: TextRange,
    ) -> Result<PureOperatorDecl, AstConstructionError> {
        if formals.is_empty() {
            return Err(AstConstructionError::new(
                "a pure operator requires at least one formal",
            ));
        }
        let mut names = std::collections::HashSet::new();
        for formal in &formals {
            validate_identifier(&formal.name, "pure operator formal")?;
            validate_pure_value_class(&formal.value_class)?;
            checked_range(formal.range)?;
            if !names.insert(&formal.name) {
                return Err(AstConstructionError::new(format!(
                    "duplicate pure operator formal `{}`",
                    formal.name
                )));
            }
        }
        validate_pure_value_class(&result)?;
        validate_pure_operator_expression(&body)?;
        Ok(PureOperatorDecl {
            visibility,
            name: checked_identifier(name, "pure operator")?,
            formals,
            result,
            body,
            range: checked_range(range)?,
        })
    }

    /// Construct one nominal Connector declaration.
    ///
    /// # Errors
    /// Returns an error for malformed visibility-independent source shape,
    /// name, expression, or byte range.
    pub fn connector(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        syntax: ConnectorSyntax,
        range: TextRange,
    ) -> Result<ConnectorDecl, AstConstructionError> {
        validate_connector_syntax(&syntax)?;
        Ok(ConnectorDecl {
            visibility,
            name: checked_identifier(name, "Connector")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct one named field-physical Connector quantity.
    ///
    /// # Errors
    /// Returns an error for a malformed member name or dimension expression.
    pub fn connector_quantity(
        name: impl Into<String>,
        dimension: Expr,
    ) -> Result<ConnectorQuantitySyntax, AstConstructionError> {
        validate_expression(&dimension)?;
        Ok(ConnectorQuantitySyntax {
            name: checked_identifier(name, "Connector quantity")?,
            dimension,
        })
    }

    /// Construct one reusable Component declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier, member shape, or
    /// byte range. Name resolution and component semantics remain compiler
    /// responsibilities.
    pub fn component(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        items: Vec<ComponentItem>,
        range: TextRange,
    ) -> Result<ComponentDecl, AstConstructionError> {
        for item in &items {
            validate_component_item(item)?;
        }
        Ok(ComponentDecl {
            visibility,
            name: checked_identifier(name, "component")?,
            items,
            range: checked_range(range)?,
        })
    }

    /// Construct one component-local scalar Parameter.
    ///
    /// # Errors
    /// Returns an error for a malformed name, expression, or byte range.
    pub fn component_parameter(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        dimension: Expr,
        default: Option<Expr>,
        range: TextRange,
    ) -> Result<ComponentParameterDecl, AstConstructionError> {
        validate_expression(&dimension)?;
        if let Some(default) = &default {
            validate_expression(default)?;
        }
        Ok(ComponentParameterDecl {
            visibility,
            name: checked_identifier(name, "component Parameter")?,
            dimension,
            default,
            range: checked_range(range)?,
        })
    }

    /// Construct one component-local Port.
    ///
    /// # Errors
    /// Returns an error for malformed Port syntax, a name, or a byte range.
    pub fn component_port(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        syntax: PortSyntax,
        range: TextRange,
    ) -> Result<ComponentPortDecl, AstConstructionError> {
        validate_port_syntax(&syntax)?;
        Ok(ComponentPortDecl {
            visibility,
            name: checked_identifier(name, "component Port")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct one field-physical Port family over a complete exterior.
    ///
    /// # Errors
    /// Returns an error unless the Port is field-physical, its support names
    /// the binder member, and both declarations are structurally valid.
    pub fn component_port_family(
        port: ComponentPortDecl,
        binder: BoundaryFamilyBinderSyntax,
    ) -> Result<ComponentPortFamilyDecl, AstConstructionError> {
        validate_port_syntax(port.syntax())?;
        checked_range(port.range())?;
        validate_boundary_family_binder(&binder)?;
        match port.syntax() {
            PortSyntax::FieldPhysical { support, .. } if support == binder.member() => {}
            PortSyntax::FieldPhysical { .. } => {
                return Err(AstConstructionError::new(
                    "a Port family support must name its boundary binder member",
                ));
            }
            _ => {
                return Err(AstConstructionError::new(
                    "only a field-physical Port can declare a boundary family",
                ));
            }
        }
        Ok(ComponentPortFamilyDecl { port, binder })
    }

    /// Construct the restricted `[member in complete_exterior]` binder.
    ///
    /// # Errors
    /// Returns an error for malformed identifiers or a reversed range.
    pub fn boundary_family_binder(
        member: impl Into<String>,
        set: impl Into<String>,
        range: TextRange,
    ) -> Result<BoundaryFamilyBinderSyntax, AstConstructionError> {
        Ok(BoundaryFamilyBinderSyntax {
            member: checked_identifier(member, "boundary family member")?,
            set: checked_identifier(set, "boundary family support set")?,
            range: checked_range(range)?,
        })
    }

    /// Construct one component-local spatial-support slot.
    ///
    /// # Errors
    /// Returns an error for a malformed slot name, parent name, or byte range.
    /// Support-graph and visibility rules remain compiler responsibilities.
    pub fn support_slot(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        syntax: SupportSlotSyntax,
        range: TextRange,
    ) -> Result<SupportSlotDecl, AstConstructionError> {
        validate_support_slot_syntax(&syntax)?;
        Ok(SupportSlotDecl {
            visibility,
            name: checked_identifier(name, "support slot")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct one public, required continuum Field slot.
    ///
    /// # Errors
    /// Returns an error for malformed names, dimensions, value shapes, or byte
    /// ranges. Exact support and Field compatibility remain compiler checks.
    pub fn field_slot(
        name: impl Into<String>,
        support: impl Into<String>,
        dimension: Expr,
        shape: Option<ValueShapeSyntax>,
        range: TextRange,
    ) -> Result<FieldSlotDecl, AstConstructionError> {
        validate_expression(&dimension)?;
        if let Some(shape) = &shape {
            validate_value_shape(shape)?;
        }
        Ok(FieldSlotDecl {
            name: checked_identifier(name, "Field slot")?,
            support: checked_identifier(support, "Field-slot support")?,
            dimension,
            shape,
            range: checked_range(range)?,
        })
    }

    /// Construct one named flat model from already checked Item values.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier or byte range.
    pub fn model(
        name: impl Into<String>,
        items: Vec<Item>,
        range: TextRange,
    ) -> Result<ModelDecl, AstConstructionError> {
        Ok(ModelDecl {
            name: checked_identifier(name, "model")?,
            items,
            range: checked_range(range)?,
        })
    }

    /// Construct a Domain declaration.
    ///
    /// # Errors
    /// Returns an error for malformed source shape, names, expressions, or
    /// ranges. Geometric and dimensional meaning remains a lowering check.
    pub fn domain(
        name: impl Into<String>,
        syntax: DomainSyntax,
        range: TextRange,
    ) -> Result<DomainDecl, AstConstructionError> {
        validate_domain_syntax(&syntax)?;
        Ok(DomainDecl {
            name: checked_identifier(name, "Domain")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct a Representation declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier or byte range.
    pub fn representation(
        name: impl Into<String>,
        syntax: RepresentationSyntax,
        range: TextRange,
    ) -> Result<RepresentationDecl, AstConstructionError> {
        Ok(RepresentationDecl {
            name: checked_identifier(name, "Representation")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct a scalar or spatial Field declaration.
    ///
    /// `domain` and `representation` must either both be present or both be
    /// absent, as required by the source grammar.
    ///
    /// # Errors
    /// Returns an error for an incomplete spatial scope, non-finite initial
    /// value, malformed expression, identifier, or byte range.
    pub fn field(
        name: impl Into<String>,
        domain: Option<String>,
        representation: Option<String>,
        dimension: Expr,
        initial: f64,
        range: TextRange,
    ) -> Result<FieldDecl, AstConstructionError> {
        Self::field_with_shape(
            name,
            domain,
            representation,
            None,
            dimension,
            Some(initial),
            range,
        )
    }

    /// Construct a scalar or shaped Field declaration.
    ///
    /// `shape = None` preserves the legacy scalar spelling. An explicit shape
    /// is retained in source form for context-dependent lowering.
    ///
    /// # Errors
    /// Returns the same structural errors as [`Self::field`], plus malformed
    /// exact shape extents.
    pub fn field_with_shape(
        name: impl Into<String>,
        domain: Option<String>,
        representation: Option<String>,
        shape: Option<ValueShapeSyntax>,
        dimension: Expr,
        initial: Option<f64>,
        range: TextRange,
    ) -> Result<FieldDecl, AstConstructionError> {
        if domain.is_some() != representation.is_some() {
            return Err(AstConstructionError::new(
                "a spatial Field requires both Domain and Representation names",
            ));
        }
        if let Some(name) = &domain {
            validate_identifier(name, "Field Domain")?;
        }
        if let Some(name) = &representation {
            validate_identifier(name, "Field Representation")?;
        }
        if let Some(shape) = &shape {
            validate_value_shape(shape)?;
        }
        validate_expression(&dimension)?;
        let scalar = shape.as_ref().is_none_or(|shape| {
            matches!(shape, ValueShapeSyntax::Scalar)
                || matches!(shape, ValueShapeSyntax::Exact(extents) if extents.is_empty())
        });
        match (scalar, initial) {
            (true, Some(initial)) => validate_finite(initial, "Field initial value")?,
            (true, None) => {
                return Err(AstConstructionError::new(
                    "scalar Field requires one finite scalar initial value",
                ));
            }
            (false, Some(_)) => {
                return Err(AstConstructionError::new(
                    "non-scalar Field cannot have a scalar initial value",
                ));
            }
            (false, None) => {}
        }
        Ok(FieldDecl {
            name: checked_identifier(name, "Field")?,
            domain,
            representation,
            shape,
            dimension,
            initial,
            range: checked_range(range)?,
        })
    }

    /// Construct a model-level scalar Parameter declaration.
    ///
    /// # Errors
    /// Returns an error for a non-finite value or malformed source shape.
    pub fn parameter(
        name: impl Into<String>,
        dimension: Expr,
        initial: f64,
        range: TextRange,
    ) -> Result<ParameterDecl, AstConstructionError> {
        validate_expression(&dimension)?;
        validate_finite(initial, "Parameter value")?;
        Ok(ParameterDecl {
            name: checked_identifier(name, "Parameter")?,
            dimension,
            initial,
            range: checked_range(range)?,
        })
    }

    /// Construct a model-level Port declaration.
    ///
    /// # Errors
    /// Returns an error for malformed Port syntax, names, or ranges.
    pub fn port(
        name: impl Into<String>,
        syntax: PortSyntax,
        range: TextRange,
    ) -> Result<PortDecl, AstConstructionError> {
        validate_port_syntax(&syntax)?;
        Ok(PortDecl {
            name: checked_identifier(name, "Port")?,
            syntax,
            range: checked_range(range)?,
        })
    }

    /// Construct an exact periodic Clock declaration.
    ///
    /// Rational reduction and nonzero-period checks remain semantic lowering
    /// checks, matching parsed source behavior.
    ///
    /// # Errors
    /// Returns an error for an invalid source identifier or byte range.
    pub fn clock(
        name: impl Into<String>,
        period: RationalSyntax,
        phase: RationalSyntax,
        range: TextRange,
    ) -> Result<ClockDecl, AstConstructionError> {
        Ok(ClockDecl {
            name: checked_identifier(name, "Clock")?,
            period,
            phase,
            range: checked_range(range)?,
        })
    }

    /// Construct an implicit Relation with at least one residual.
    ///
    /// # Errors
    /// Returns an error for an empty residual set or malformed source shape.
    pub fn relation(
        name: impl Into<String>,
        activation: ActivationSyntax,
        domain: Option<String>,
        residuals: Vec<Expr>,
        range: TextRange,
    ) -> Result<RelationDecl, AstConstructionError> {
        if residuals.is_empty() {
            return Err(AstConstructionError::new(
                "a Relation requires at least one residual",
            ));
        }
        if let ActivationSyntax::Periodic(clock) = &activation {
            validate_identifier(clock, "periodic Clock")?;
        }
        if let Some(domain) = &domain {
            validate_identifier(domain, "Relation Domain")?;
        }
        for residual in &residuals {
            validate_expression(residual)?;
        }
        Ok(RelationDecl {
            name: checked_identifier(name, "Relation")?,
            activation,
            domain,
            residuals,
            range: checked_range(range)?,
        })
    }

    /// Construct one continuous Relation family over a complete exterior.
    ///
    /// # Errors
    /// Returns an error unless the Relation is continuous, is attached to the
    /// binder member, and both declarations are structurally valid.
    pub fn relation_family(
        relation: RelationDecl,
        binder: BoundaryFamilyBinderSyntax,
    ) -> Result<RelationFamilyDecl, AstConstructionError> {
        validate_boundary_family_binder(&binder)?;
        if relation.activation() != &ActivationSyntax::Continuous {
            return Err(AstConstructionError::new(
                "a boundary Relation family must be continuous",
            ));
        }
        if relation.domain() != Some(binder.member()) {
            return Err(AstConstructionError::new(
                "a boundary Relation family Domain must name its binder member",
            ));
        }
        checked_range(relation.range())?;
        for residual in relation.residuals() {
            validate_expression(residual)?;
        }
        Ok(RelationFamilyDecl { relation, binder })
    }

    /// Construct a signal or conserving Connection with at least two Ports.
    ///
    /// # Errors
    /// Returns an error for insufficient members or malformed paths/ranges.
    pub fn connection(
        syntax: ConnectionSyntax,
        ports: Vec<NamePath>,
        range: TextRange,
    ) -> Result<ConnectionDecl, AstConstructionError> {
        if syntax == ConnectionSyntax::SpatialPeriodic {
            return Err(AstConstructionError::new(
                "a spatial-periodic Connection requires boundary Port references",
            ));
        }
        if ports.len() < 2 {
            return Err(AstConstructionError::new(
                "a Connection requires at least two Port paths",
            ));
        }
        for path in &ports {
            validate_name_path(path)?;
        }
        Ok(ConnectionDecl {
            syntax,
            ports,
            range: checked_range(range)?,
        })
    }

    /// Construct one conserving Connection with boundary-family Port references.
    ///
    /// # Errors
    /// Returns an error for fewer than two Ports, malformed references, or a
    /// declaration containing neither a family binder nor a selector.
    pub fn boundary_connection(
        binder: Option<BoundaryFamilyBinderSyntax>,
        ports: Vec<BoundaryPortReferenceSyntax>,
        range: TextRange,
    ) -> Result<BoundaryConnectionDecl, AstConstructionError> {
        if ports.len() < 2 {
            return Err(AstConstructionError::new(
                "a Connection requires at least two Port paths",
            ));
        }
        if binder.is_none() && ports.iter().all(|port| port.selector().is_none()) {
            return Err(AstConstructionError::new(
                "a boundary Connection requires a family binder or selector",
            ));
        }
        if let Some(binder) = &binder {
            validate_boundary_family_binder(binder)?;
        }
        for port in &ports {
            validate_boundary_port_reference(port)?;
        }
        Ok(BoundaryConnectionDecl {
            syntax: ConnectionSyntax::Conserving,
            binder,
            ports,
            range: checked_range(range)?,
        })
    }

    /// Construct one exact spatial-periodic pair in a closed Model.
    ///
    /// # Errors
    /// Returns an error unless there are exactly two boundary Ports.
    pub fn spatial_periodic_boundary_connection(
        ports: Vec<BoundaryPortReferenceSyntax>,
        range: TextRange,
    ) -> Result<BoundaryConnectionDecl, AstConstructionError> {
        let connection = BoundaryConnectionDecl {
            syntax: ConnectionSyntax::SpatialPeriodic,
            binder: None,
            ports,
            range: checked_range(range)?,
        };
        validate_boundary_connection(&connection)?;
        Ok(connection)
    }

    /// Construct one Port path with an optional exact boundary selector.
    ///
    /// # Errors
    /// Returns an error for a malformed path or selector.
    pub fn boundary_port_reference(
        port: NamePath,
        selector: Option<BoundaryPortSelectorSyntax>,
    ) -> Result<BoundaryPortReferenceSyntax, AstConstructionError> {
        validate_name_path(&port)?;
        if let Some(selector) = &selector {
            validate_boundary_port_selector(selector)?;
        }
        Ok(BoundaryPortReferenceSyntax { port, selector })
    }

    /// Construct the closed `[member = target]` Port selector.
    ///
    /// # Errors
    /// Returns an error for malformed identifiers or a reversed range.
    pub fn boundary_port_selector(
        member: impl Into<String>,
        target: impl Into<String>,
        range: TextRange,
    ) -> Result<BoundaryPortSelectorSyntax, AstConstructionError> {
        Ok(BoundaryPortSelectorSyntax {
            member: checked_identifier(member, "boundary selector member")?,
            target: checked_identifier(target, "boundary selector target")?,
            range: checked_range(range)?,
        })
    }

    /// Construct a nonempty public model boundary.
    ///
    /// # Errors
    /// Returns an error for an empty boundary or malformed paths/ranges.
    pub fn boundary(
        ports: Vec<NamePath>,
        range: TextRange,
    ) -> Result<BoundaryDecl, AstConstructionError> {
        if ports.is_empty() {
            return Err(AstConstructionError::new(
                "a boundary requires at least one Port path",
            ));
        }
        for path in &ports {
            validate_name_path(path)?;
        }
        Ok(BoundaryDecl {
            ports,
            range: checked_range(range)?,
        })
    }

    /// Construct one compile-time component instance.
    ///
    /// # Errors
    /// Returns an error for malformed names, bindings, or ranges.
    pub fn instance(
        name: impl Into<String>,
        definition: NamePath,
        bindings: Vec<ParameterBindingDecl>,
        range: TextRange,
    ) -> Result<InstanceDecl, AstConstructionError> {
        Self::instance_with_support_bindings(name, definition, bindings, Vec::new(), range)
    }

    /// Construct one compile-time component instance with spatial supports.
    ///
    /// # Errors
    /// Returns an error for malformed names, either binding family, or ranges.
    pub fn instance_with_support_bindings(
        name: impl Into<String>,
        definition: NamePath,
        bindings: Vec<ParameterBindingDecl>,
        support_bindings: Vec<SupportBindingDecl>,
        range: TextRange,
    ) -> Result<InstanceDecl, AstConstructionError> {
        Self::instance_with_slot_bindings(
            name,
            definition,
            bindings,
            support_bindings,
            Vec::new(),
            range,
        )
    }

    /// Construct one compile-time component instance with occurrence-bound
    /// support and Field slots.
    ///
    /// # Errors
    /// Returns an error for malformed names, any binding family, or ranges.
    pub fn instance_with_slot_bindings(
        name: impl Into<String>,
        definition: NamePath,
        bindings: Vec<ParameterBindingDecl>,
        support_bindings: Vec<SupportBindingDecl>,
        field_bindings: Vec<FieldBindingDecl>,
        range: TextRange,
    ) -> Result<InstanceDecl, AstConstructionError> {
        Self::instance_with_boundary_set_bindings(
            name,
            definition,
            bindings,
            support_bindings,
            Vec::new(),
            field_bindings,
            range,
        )
    }

    /// Construct one compile-time component instance with every closed
    /// binding family, including finite complete-exterior bindings.
    ///
    /// # Errors
    /// Returns an error for malformed names, any binding family, or ranges.
    pub fn instance_with_boundary_set_bindings(
        name: impl Into<String>,
        definition: NamePath,
        bindings: Vec<ParameterBindingDecl>,
        support_bindings: Vec<SupportBindingDecl>,
        boundary_set_bindings: Vec<BoundarySetBindingDecl>,
        field_bindings: Vec<FieldBindingDecl>,
        range: TextRange,
    ) -> Result<InstanceDecl, AstConstructionError> {
        validate_name_path(&definition)?;
        for binding in &bindings {
            validate_parameter_binding(binding)?;
        }
        for binding in &support_bindings {
            validate_support_binding(binding)?;
        }
        for binding in &boundary_set_bindings {
            validate_boundary_set_binding(binding)?;
        }
        for binding in &field_bindings {
            validate_field_binding(binding)?;
        }
        Ok(InstanceDecl {
            name: checked_identifier(name, "instance")?,
            definition,
            bindings,
            support_bindings,
            boundary_set_bindings,
            field_bindings,
            range: checked_range(range)?,
        })
    }

    /// Construct one named component Parameter binding.
    ///
    /// # Errors
    /// Returns an error for a malformed target, expression, or byte range.
    pub fn parameter_binding(
        parameter: impl Into<String>,
        value: Expr,
        range: TextRange,
    ) -> Result<ParameterBindingDecl, AstConstructionError> {
        validate_expression(&value)?;
        Ok(ParameterBindingDecl {
            parameter: checked_identifier(parameter, "Parameter binding")?,
            value,
            range: checked_range(range)?,
        })
    }

    /// Construct one named component spatial-support binding.
    ///
    /// # Errors
    /// Returns an error for a malformed slot, target, or byte range.
    pub fn support_binding(
        slot: impl Into<String>,
        target: impl Into<String>,
        range: TextRange,
    ) -> Result<SupportBindingDecl, AstConstructionError> {
        Ok(SupportBindingDecl {
            slot: checked_identifier(slot, "support binding slot")?,
            target: checked_identifier(target, "support binding target")?,
            range: checked_range(range)?,
        })
    }

    /// Construct one finite complete-exterior support binding.
    ///
    /// Empty member lists remain syntactically representable so semantic
    /// validation can issue the same contextual diagnostic as parsed source.
    ///
    /// # Errors
    /// Returns an error for malformed members, a slot, or a byte range.
    pub fn boundary_set_binding(
        slot: impl Into<String>,
        members: Vec<BoundarySetMemberSyntax>,
        range: TextRange,
    ) -> Result<BoundarySetBindingDecl, AstConstructionError> {
        for member in &members {
            validate_boundary_set_member(member)?;
        }
        Ok(BoundarySetBindingDecl {
            slot: checked_identifier(slot, "boundary-set binding slot")?,
            members,
            range: checked_range(range)?,
        })
    }

    /// Construct one named member of a finite complete-exterior binding.
    ///
    /// # Errors
    /// Returns an error for a malformed target or byte range.
    pub fn boundary_set_member(
        target: impl Into<String>,
        range: TextRange,
    ) -> Result<BoundarySetMemberSyntax, AstConstructionError> {
        Ok(BoundarySetMemberSyntax {
            target: checked_identifier(target, "boundary-set member")?,
            range: checked_range(range)?,
        })
    }

    /// Construct one named occurrence-bound Field binding.
    ///
    /// # Errors
    /// Returns an error for a malformed slot, target, or byte range.
    pub fn field_binding(
        slot: impl Into<String>,
        target: impl Into<String>,
        range: TextRange,
    ) -> Result<FieldBindingDecl, AstConstructionError> {
        Ok(FieldBindingDecl {
            slot: checked_identifier(slot, "Field binding slot")?,
            target: checked_identifier(target, "Field binding target")?,
            range: checked_range(range)?,
        })
    }

    /// Construct one source expression from an owned recursive expression kind.
    ///
    /// # Errors
    /// Returns an error for malformed names, non-finite literals, child
    /// expressions, or byte ranges.
    pub fn expression(kind: ExprKind, range: TextRange) -> Result<Expr, AstConstructionError> {
        let expression = Expr {
            kind,
            range: checked_range(range)?,
        };
        validate_expression(&expression)?;
        Ok(expression)
    }

    /// Construct unreduced rational source syntax.
    #[must_use]
    pub const fn rational(numerator: u64, denominator: u64) -> RationalSyntax {
        RationalSyntax {
            numerator,
            denominator,
        }
    }
}

impl NamePath {
    /// Construct a structurally segmented, nonempty source name.
    ///
    /// # Errors
    /// Returns an error when the path is empty, a segment is not a valid
    /// Eqiora identifier, or the source range is reversed.
    pub fn from_segments<I, S>(segments: I, range: TextRange) -> Result<Self, AstConstructionError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments = segments.into_iter().map(Into::into).collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(AstConstructionError::new("a NamePath cannot be empty"));
        }
        for segment in &segments {
            validate_identifier(segment, "NamePath segment")?;
        }
        Ok(Self::from_parsed_segments(segments, checked_range(range)?))
    }
}

fn checked_identifier(
    value: impl Into<String>,
    role: &str,
) -> Result<String, AstConstructionError> {
    let value = value.into();
    validate_identifier(&value, role)?;
    Ok(value)
}

fn validate_identifier(value: &str, role: &str) -> Result<(), AstConstructionError> {
    let mut bytes = value.bytes();
    let valid = matches!(
        bytes.next(),
        Some(first) if first.is_ascii_alphabetic() || first == b'_'
    ) && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(AstConstructionError::new(format!(
            "{role} `{value}` is not an Eqiora identifier"
        )))
    }
}

fn checked_range(range: TextRange) -> Result<TextRange, AstConstructionError> {
    if range.start() <= range.end() {
        Ok(range)
    } else {
        Err(AstConstructionError::new(format!(
            "source range {}..{} is reversed",
            range.start(),
            range.end()
        )))
    }
}

fn validate_finite(value: f64, role: &str) -> Result<(), AstConstructionError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(AstConstructionError::new(format!(
            "{role} must be a finite f64"
        )))
    }
}

fn validate_name_path(path: &NamePath) -> Result<(), AstConstructionError> {
    checked_range(path.range())?;
    let mut segments = path.segments();
    let Some(first) = segments.next() else {
        return Err(AstConstructionError::new("a NamePath cannot be empty"));
    };
    validate_identifier(first, "NamePath segment")?;
    for segment in segments {
        validate_identifier(segment, "NamePath segment")?;
    }
    Ok(())
}

fn validate_expression(expression: &Expr) -> Result<(), AstConstructionError> {
    checked_range(expression.range())?;
    match expression.kind() {
        ExprKind::Number(value) => validate_finite(*value, "expression literal"),
        ExprKind::Name(name) => validate_identifier(name, "expression name"),
        ExprKind::Path(path) => validate_name_path(path),
        ExprKind::BoundaryPortSelection { port, selector } => {
            validate_name_path(port)?;
            validate_boundary_port_selector(selector)
        }
        ExprKind::Unary { value, .. } => validate_expression(value),
        ExprKind::Binary { left, right, .. } => {
            validate_expression(left)?;
            validate_expression(right)
        }
        ExprKind::Call { callee, arguments } => {
            validate_name_path(callee)?;
            if arguments.is_empty() {
                return Err(AstConstructionError::new(
                    "an expression operator call requires at least one argument",
                ));
            }
            for argument in arguments {
                validate_expression(argument)?;
            }
            Ok(())
        }
    }
}

fn validate_exact_integer(value: &ExactIntegerSyntax) -> Result<(), AstConstructionError> {
    checked_range(value.range)?;
    let parsed = value.spelling.parse::<u64>().map_err(|_| {
        AstConstructionError::new(format!(
            "exact integer `{}` must be an unsigned decimal integer fitting in u64",
            value.spelling
        ))
    })?;
    if parsed == value.value {
        Ok(())
    } else {
        Err(AstConstructionError::new(format!(
            "exact integer spelling `{}` does not encode stored value {}",
            value.spelling, value.value
        )))
    }
}

fn validate_pure_value_class(
    value_class: &PureValueClassSyntax,
) -> Result<(), AstConstructionError> {
    match value_class {
        PureValueClassSyntax::Scalar => Ok(()),
        PureValueClassSyntax::Spatial { rank } => validate_exact_integer(rank),
    }
}

fn validate_pure_operator_expression(
    expression: &PureOperatorExpr,
) -> Result<(), AstConstructionError> {
    checked_range(expression.range)?;
    match &expression.kind {
        PureOperatorExprKind::Rational {
            numerator,
            denominator,
        } => {
            validate_exact_integer(numerator)?;
            validate_exact_integer(denominator)?;
            if denominator.value == 0 {
                Err(AstConstructionError::new(
                    "pure operator rational denominator must be nonzero",
                ))
            } else {
                Ok(())
            }
        }
        PureOperatorExprKind::Component {
            formal,
            formal_range,
            result_axes,
        } => {
            validate_identifier(formal, "pure operator component formal")?;
            checked_range(*formal_range)?;
            for axis in result_axes {
                validate_exact_integer(axis)?;
            }
            Ok(())
        }
        PureOperatorExprKind::Delta {
            left_axis,
            right_axis,
        } => {
            validate_exact_integer(left_axis)?;
            validate_exact_integer(right_axis)
        }
        PureOperatorExprKind::Neg(value) => validate_pure_operator_expression(value),
        PureOperatorExprKind::Binary { left, right, .. } => {
            validate_pure_operator_expression(left)?;
            validate_pure_operator_expression(right)
        }
    }
}

fn validate_connector_syntax(syntax: &ConnectorSyntax) -> Result<(), AstConstructionError> {
    match syntax {
        ConnectorSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            validate_expression(across_dimension)?;
            validate_expression(through_dimension)
        }
        ConnectorSyntax::FieldPhysical {
            trace, flux, shape, ..
        } => {
            validate_connector_quantity(trace, "trace")?;
            validate_connector_quantity(flux, "flux")?;
            validate_value_shape(shape)
        }
    }
}

fn validate_component_item(item: &ComponentItem) -> Result<(), AstConstructionError> {
    let range = match item {
        ComponentItem::Parameter(declaration) => declaration.range(),
        ComponentItem::Port(declaration) => declaration.range(),
        ComponentItem::PortFamily(declaration) => {
            validate_port_syntax(declaration.port().syntax())?;
            validate_boundary_family_binder(declaration.binder())?;
            declaration.range()
        }
        ComponentItem::Support(declaration) => declaration.range(),
        ComponentItem::FieldSlot(declaration) => declaration.range(),
        ComponentItem::Representation(declaration) => declaration.range(),
        ComponentItem::Field(declaration) => declaration.range(),
        ComponentItem::Clock(declaration) => declaration.range(),
        ComponentItem::Relation(declaration) => declaration.range(),
        ComponentItem::RelationFamily(declaration) => {
            validate_boundary_family_binder(declaration.binder())?;
            declaration.range()
        }
        ComponentItem::Connection(declaration) => declaration.range(),
        ComponentItem::BoundaryConnection(declaration) => {
            validate_boundary_connection(declaration)?;
            if declaration.syntax() == ConnectionSyntax::SpatialPeriodic {
                return Err(AstConstructionError::new(
                    "a spatial-periodic Connection belongs only to a closed Model",
                ));
            }
            declaration.range()
        }
        ComponentItem::Instance(declaration) => declaration.range(),
    };
    checked_range(range).map(|_| ())
}

fn validate_support_slot_syntax(syntax: &SupportSlotSyntax) -> Result<(), AstConstructionError> {
    match syntax {
        SupportSlotSyntax::Volume { .. } => Ok(()),
        SupportSlotSyntax::Boundary { parent } | SupportSlotSyntax::CompleteExterior { parent } => {
            validate_identifier(parent, "boundary parent support slot")
        }
    }
}

fn validate_port_syntax(syntax: &PortSyntax) -> Result<(), AstConstructionError> {
    match syntax {
        PortSyntax::Signal { dimension, .. } | PortSyntax::ConservingMarker { dimension } => {
            validate_expression(dimension)
        }
        PortSyntax::ScalarPhysical { domain } => {
            validate_identifier(domain, "scalar physical Domain")
        }
        PortSyntax::ScalarPhysicalConnector { connector } => validate_name_path(connector),
        PortSyntax::FieldPhysical { connector, support } => {
            validate_name_path(connector)?;
            validate_identifier(support, "field-physical boundary support")
        }
    }
}

fn validate_connector_quantity(
    quantity: &ConnectorQuantitySyntax,
    role: &str,
) -> Result<(), AstConstructionError> {
    validate_identifier(quantity.name(), &format!("{role} quantity"))?;
    validate_expression(quantity.dimension())
}

fn validate_value_shape(shape: &ValueShapeSyntax) -> Result<(), AstConstructionError> {
    match shape {
        ValueShapeSyntax::Scalar | ValueShapeSyntax::SpatialVector => Ok(()),
        ValueShapeSyntax::Exact(extents) if extents.is_empty() => Err(AstConstructionError::new(
            "an empty exact shape must use the canonical Scalar syntax",
        )),
        ValueShapeSyntax::Exact(extents) if extents.contains(&0) => Err(AstConstructionError::new(
            "exact value-shape extents must be positive",
        )),
        ValueShapeSyntax::Exact(_) => Ok(()),
    }
}

fn validate_parameter_binding(binding: &ParameterBindingDecl) -> Result<(), AstConstructionError> {
    validate_identifier(binding.parameter(), "Parameter binding")?;
    checked_range(binding.range())?;
    validate_expression(binding.value())
}

fn validate_support_binding(binding: &SupportBindingDecl) -> Result<(), AstConstructionError> {
    validate_identifier(binding.slot(), "support binding slot")?;
    validate_identifier(binding.target(), "support binding target")?;
    checked_range(binding.range()).map(|_| ())
}

fn validate_boundary_family_binder(
    binder: &BoundaryFamilyBinderSyntax,
) -> Result<(), AstConstructionError> {
    validate_identifier(binder.member(), "boundary family member")?;
    validate_identifier(binder.set(), "boundary family support set")?;
    checked_range(binder.range()).map(|_| ())
}

fn validate_boundary_port_selector(
    selector: &BoundaryPortSelectorSyntax,
) -> Result<(), AstConstructionError> {
    validate_identifier(selector.member(), "boundary selector member")?;
    validate_identifier(selector.target(), "boundary selector target")?;
    checked_range(selector.range()).map(|_| ())
}

fn validate_boundary_port_reference(
    reference: &BoundaryPortReferenceSyntax,
) -> Result<(), AstConstructionError> {
    validate_name_path(reference.port())?;
    if let Some(selector) = reference.selector() {
        validate_boundary_port_selector(selector)?;
    }
    Ok(())
}

fn validate_boundary_connection(
    connection: &BoundaryConnectionDecl,
) -> Result<(), AstConstructionError> {
    if connection.syntax() == ConnectionSyntax::SpatialPeriodic && connection.ports().len() != 2 {
        return Err(AstConstructionError::new(
            "a spatial-periodic Connection requires exactly two Ports",
        ));
    }
    if connection.syntax() != ConnectionSyntax::SpatialPeriodic && connection.ports().len() < 2 {
        return Err(AstConstructionError::new(
            "a Connection requires at least two Port paths",
        ));
    }
    if connection.syntax() == ConnectionSyntax::Signal {
        return Err(AstConstructionError::new(
            "a boundary Connection cannot use signal semantics",
        ));
    }
    if connection.syntax() == ConnectionSyntax::SpatialPeriodic && connection.binder().is_some() {
        return Err(AstConstructionError::new(
            "a spatial-periodic Connection cannot declare a family binder",
        ));
    }
    if connection.syntax() == ConnectionSyntax::Conserving
        && connection.binder().is_none()
        && connection
            .ports()
            .iter()
            .all(|port| port.selector().is_none())
    {
        return Err(AstConstructionError::new(
            "a boundary Connection requires a family binder or selector",
        ));
    }
    if let Some(binder) = connection.binder() {
        validate_boundary_family_binder(binder)?;
    }
    for port in connection.ports() {
        validate_boundary_port_reference(port)?;
    }
    checked_range(connection.range()).map(|_| ())
}

fn validate_boundary_set_member(
    member: &BoundarySetMemberSyntax,
) -> Result<(), AstConstructionError> {
    validate_identifier(member.target(), "boundary-set member")?;
    checked_range(member.range()).map(|_| ())
}

fn validate_boundary_set_binding(
    binding: &BoundarySetBindingDecl,
) -> Result<(), AstConstructionError> {
    validate_identifier(binding.slot(), "boundary-set binding slot")?;
    for member in binding.members() {
        validate_boundary_set_member(member)?;
    }
    checked_range(binding.range()).map(|_| ())
}

fn validate_field_binding(binding: &FieldBindingDecl) -> Result<(), AstConstructionError> {
    validate_identifier(binding.slot(), "Field binding slot")?;
    validate_identifier(binding.target(), "Field binding target")?;
    checked_range(binding.range()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use crate::{
        BinaryOp, BoundaryPairingSyntax, FrameSyntax, Item, SignalDirectionSyntax, format, parse,
    };

    use super::*;
    use crate::cartesian::CartesianCoordinateSyntax;

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(start, end)
    }

    fn dimension() -> Expr {
        SourceAstFactory::expression(ExprKind::Number(1.0), range(0, 0)).expect("dimension")
    }

    fn path(segments: &[&str]) -> NamePath {
        NamePath::from_segments(segments.iter().copied(), range(0, 0)).expect("path")
    }

    #[test]
    fn owned_flat_model_formats_and_parses_identically() {
        let domain = SourceAstFactory::domain(
            "body",
            DomainSyntax::CartesianBox(vec![(
                CartesianCoordinateSyntax::fixed(0.0, TextRange::new(0, 0)),
                CartesianCoordinateSyntax::fixed(1.0, TextRange::new(0, 0)),
            )]),
            range(0, 0),
        )
        .expect("Domain");
        let representation =
            SourceAstFactory::representation("space", RepresentationSyntax::Continuum, range(0, 0))
                .expect("Representation");
        let field = SourceAstFactory::field(
            "temperature",
            Some("body".to_owned()),
            Some("space".to_owned()),
            dimension(),
            0.0,
            range(0, 0),
        )
        .expect("Field");
        let parameter =
            SourceAstFactory::parameter("gain", dimension(), 2.0, range(0, 0)).expect("Parameter");
        let output = SourceAstFactory::port(
            "output",
            PortSyntax::Signal {
                direction: SignalDirectionSyntax::Output,
                dimension: dimension(),
            },
            range(0, 0),
        )
        .expect("output Port");
        let input = SourceAstFactory::port(
            "input",
            PortSyntax::Signal {
                direction: SignalDirectionSyntax::Input,
                dimension: dimension(),
            },
            range(0, 0),
        )
        .expect("input Port");
        let clock = SourceAstFactory::clock(
            "sample",
            SourceAstFactory::rational(1, 10),
            SourceAstFactory::rational(0, 1),
            range(0, 0),
        )
        .expect("Clock");
        let residual =
            SourceAstFactory::expression(ExprKind::Name("temperature".to_owned()), range(0, 0))
                .expect("residual");
        let relation = SourceAstFactory::relation(
            "balance",
            ActivationSyntax::Continuous,
            Some("body".to_owned()),
            vec![residual],
            range(0, 0),
        )
        .expect("Relation");
        let connection = SourceAstFactory::connection(
            ConnectionSyntax::Signal,
            vec![path(&["output"]), path(&["input"])],
            range(0, 0),
        )
        .expect("Connection");
        let boundary =
            SourceAstFactory::boundary(vec![path(&["input"])], range(0, 0)).expect("boundary");
        let binding = SourceAstFactory::parameter_binding(
            "gain",
            SourceAstFactory::expression(ExprKind::Number(3.0), range(0, 0))
                .expect("binding value"),
            range(0, 0),
        )
        .expect("binding");
        let instance =
            SourceAstFactory::instance("nested", path(&["Reusable"]), vec![binding], range(0, 0))
                .expect("instance");
        let model = SourceAstFactory::model(
            "constructed",
            vec![
                Item::Domain(domain),
                Item::Representation(representation),
                Item::Field(field),
                Item::Parameter(parameter),
                Item::Port(output),
                Item::Port(input),
                Item::Clock(clock),
                Item::Relation(relation),
                Item::Connection(connection),
                Item::Boundary(boundary),
                Item::Instance(instance),
            ],
            range(0, 0),
        )
        .expect("model");
        let document = SourceAstFactory::flat_document(vec![model]).expect("document");

        let source = format(&document);
        let reparsed = parse("constructed.eqi", &source)
            .into_document()
            .expect("factory output parses");

        assert_eq!(format(&reparsed), source);
    }

    #[test]
    fn owned_declaration_only_document_preserves_package_visibility() {
        let connector = SourceAstFactory::connector(
            VisibilitySyntax::Public,
            "Pin",
            ConnectorSyntax::ScalarPhysical {
                across_dimension: dimension(),
                through_dimension: dimension(),
            },
            range(0, 0),
        )
        .expect("Connector");
        let resistance = SourceAstFactory::component_parameter(
            VisibilitySyntax::Public,
            "resistance",
            dimension(),
            Some(dimension()),
            range(0, 0),
        )
        .expect("component Parameter");
        let component = SourceAstFactory::component(
            VisibilitySyntax::Public,
            "Resistor",
            vec![ComponentItem::Parameter(resistance)],
            range(0, 0),
        )
        .expect("component");
        let document = SourceAstFactory::document(vec![connector], vec![component], Vec::new())
            .expect("declaration-only document");

        let source = format(&document);
        let reparsed = parse("library.eqi", &source)
            .into_document()
            .expect("factory output parses");
        assert!(reparsed.models().is_empty());
        assert_eq!(
            reparsed.connectors()[0].visibility(),
            VisibilitySyntax::Public
        );
        assert_eq!(
            reparsed.components()[0].visibility(),
            VisibilitySyntax::Public
        );
        assert_eq!(format(&reparsed), source);
        assert!(SourceAstFactory::document(Vec::new(), Vec::new(), Vec::new()).is_err());
    }

    #[test]
    fn factory_constructs_exact_pure_operator_documents_without_weakening_legacy_document_api() {
        let rank = SourceAstFactory::exact_integer("01", range(10, 12)).expect("rank");
        let formal = SourceAstFactory::pure_operator_formal(
            "x",
            PureValueClassSyntax::Spatial { rank },
            range(8, 12),
        )
        .expect("formal");
        let axis = SourceAstFactory::exact_integer("00", range(30, 32)).expect("axis");
        let body = SourceAstFactory::pure_operator_expression(
            PureOperatorExprKind::Component {
                formal: "x".to_owned(),
                formal_range: range(27, 28),
                result_axes: vec![axis],
            },
            range(17, 33),
        )
        .expect("body");
        let operator = SourceAstFactory::pure_operator(
            VisibilitySyntax::Public,
            "identity",
            vec![formal],
            PureValueClassSyntax::Spatial {
                rank: SourceAstFactory::exact_integer("1", range(14, 15)).expect("result rank"),
            },
            body,
            range(0, 34),
        )
        .expect("operator");
        let document = SourceAstFactory::document_with_pure_operators(
            Vec::new(),
            Vec::new(),
            vec![operator],
            Vec::new(),
        )
        .expect("declaration-only document");

        let source = format(&document);
        assert_eq!(
            source,
            "public pure operator identity(x: spatial[1]) -> spatial[1] = component(x, 0);\n"
        );
        assert!(parse("factory-pure.eqi", &source).into_document().is_ok());
        assert!(
            SourceAstFactory::document_with_pure_operators(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new()
            )
            .is_err()
        );
    }

    #[test]
    fn owned_support_slots_and_bindings_format_and_parse_identically() {
        let body = SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "body",
            SupportSlotSyntax::Volume {
                ambient_dimension: 2,
            },
            range(0, 0),
        )
        .expect("volume support slot");
        let interface = SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "interface",
            SupportSlotSyntax::Boundary {
                parent: "body".to_owned(),
            },
            range(0, 0),
        )
        .expect("boundary support slot");
        let representation =
            SourceAstFactory::representation("space", RepresentationSyntax::Continuum, range(0, 0))
                .expect("Representation");
        let component = SourceAstFactory::component(
            VisibilitySyntax::Private,
            "BoundaryState",
            vec![
                ComponentItem::Support(body),
                ComponentItem::Support(interface),
                ComponentItem::Representation(representation),
            ],
            range(0, 0),
        )
        .expect("component");
        let support = SourceAstFactory::support_binding("body", "fluid", range(0, 0))
            .expect("support binding");
        let instance = SourceAstFactory::instance_with_support_bindings(
            "probe",
            path(&["BoundaryState"]),
            Vec::new(),
            vec![support],
            range(0, 0),
        )
        .expect("support-aware instance");
        let model = SourceAstFactory::model("coupled", vec![Item::Instance(instance)], range(0, 0))
            .expect("model");
        let document =
            SourceAstFactory::document(vec![], vec![component], vec![model]).expect("document");

        let source = format(&document);
        let reparsed = parse("supports.eqi", &source)
            .into_document()
            .expect("factory support source parses");

        assert_eq!(format(&reparsed), source);
        let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
            panic!("model member is an instance");
        };
        assert!(instance.bindings().is_empty());
        assert_eq!(instance.support_bindings()[0].target(), "fluid");
    }

    #[test]
    fn owned_field_slots_and_bindings_format_and_parse_identically() {
        let body = SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "body",
            SupportSlotSyntax::Volume {
                ambient_dimension: 2,
            },
            range(0, 0),
        )
        .expect("volume support slot");
        let state = SourceAstFactory::field_slot(
            "state",
            "body",
            dimension(),
            Some(ValueShapeSyntax::SpatialVector),
            range(0, 0),
        )
        .expect("Field slot");
        let component = SourceAstFactory::component(
            VisibilitySyntax::Private,
            "StateLaw",
            vec![
                ComponentItem::Support(body),
                ComponentItem::FieldSlot(state),
            ],
            range(0, 0),
        )
        .expect("component");
        let support = SourceAstFactory::support_binding("body", "region", range(0, 0))
            .expect("support binding");
        let field = SourceAstFactory::field_binding("state", "temperature", range(0, 0))
            .expect("Field binding");
        let instance = SourceAstFactory::instance_with_slot_bindings(
            "law",
            path(&["StateLaw"]),
            Vec::new(),
            vec![support],
            vec![field],
            range(0, 0),
        )
        .expect("slot-aware instance");
        let model = SourceAstFactory::model("coupled", vec![Item::Instance(instance)], range(0, 0))
            .expect("model");
        let document =
            SourceAstFactory::document(vec![], vec![component], vec![model]).expect("document");

        let source = format(&document);
        let reparsed = parse("field-slots.eqi", &source)
            .into_document()
            .expect("factory Field-slot source parses");

        assert_eq!(format(&reparsed), source);
        let ComponentItem::FieldSlot(slot) = &reparsed.components()[0].items()[1] else {
            panic!("second component member is a Field slot");
        };
        assert_eq!(slot.support(), "body");
        let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
            panic!("model member is an instance");
        };
        assert_eq!(instance.field_bindings()[0].target(), "temperature");

        assert!(
            SourceAstFactory::field_slot(
                "state",
                "body",
                dimension(),
                Some(ValueShapeSyntax::Exact(Vec::new())),
                range(0, 0),
            )
            .is_err()
        );
    }

    #[test]
    fn name_rewrite_preserves_expression_tree_and_ranges() {
        let bare = SourceAstFactory::expression(ExprKind::Name("x".to_owned()), range(10, 11))
            .expect("bare name");
        let qualified_path =
            NamePath::from_segments(["plant", "terminal"], range(20, 34)).expect("qualified");
        let qualified = SourceAstFactory::expression(ExprKind::Path(qualified_path), range(20, 34))
            .expect("qualified name");
        let call = SourceAstFactory::expression(
            ExprKind::Call {
                callee: path(&["across"]),
                arguments: vec![qualified],
            },
            range(13, 35),
        )
        .expect("call");
        let expression = SourceAstFactory::expression(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left: Box::new(bare),
                right: Box::new(call),
            },
            range(10, 35),
        )
        .expect("expression");

        let rewritten = expression.rewrite_name_paths(|name| match name.as_str() {
            "x" => {
                Some(NamePath::from_segments(["scope", "x"], range(100, 200)).expect("replacement"))
            }
            "plant.terminal" => {
                Some(NamePath::from_segments(["terminal"], range(300, 400)).expect("replacement"))
            }
            "across" => Some(
                NamePath::from_segments(["operators", "across"], range(500, 600))
                    .expect("replacement"),
            ),
            _ => None,
        });

        assert_eq!(rewritten.range(), expression.range());
        let ExprKind::Binary { left, right, .. } = rewritten.kind() else {
            panic!("binary topology is preserved");
        };
        assert_eq!(left.range(), range(10, 11));
        let ExprKind::Path(left_path) = left.kind() else {
            panic!("bare name was rewritten to a qualified path");
        };
        assert_eq!(left_path.as_str(), "scope.x");
        assert_eq!(left_path.range(), range(10, 11));
        assert_eq!(right.range(), range(13, 35));
        let ExprKind::Call { callee, arguments } = right.kind() else {
            panic!("Call topology is preserved");
        };
        assert_eq!(callee.as_str(), "operators.across");
        assert_eq!(callee.range(), range(0, 0));
        assert_eq!(arguments[0].range(), range(20, 34));
        assert!(matches!(
            arguments[0].kind(),
            ExprKind::Name(name) if name == "terminal"
        ));
    }

    #[test]
    fn factory_constructs_closed_field_physical_source_shapes() {
        let trace =
            SourceAstFactory::connector_quantity("velocity", dimension()).expect("trace quantity");
        let flux =
            SourceAstFactory::connector_quantity("traction", dimension()).expect("flux quantity");
        let connector = SourceAstFactory::connector(
            VisibilitySyntax::Public,
            "MechanicalBoundary",
            ConnectorSyntax::FieldPhysical {
                trace,
                flux,
                shape: ValueShapeSyntax::Exact(vec![2]),
                frame: FrameSyntax::Spatial,
                pairing: BoundaryPairingSyntax::EuclideanBoundaryDuality,
            },
            range(0, 0),
        )
        .expect("field-physical Connector");
        let field = SourceAstFactory::field_with_shape(
            "velocity",
            None,
            None,
            Some(ValueShapeSyntax::Exact(vec![2])),
            dimension(),
            None,
            range(0, 0),
        )
        .expect("shaped Field");
        let port = SourceAstFactory::port(
            "interface",
            PortSyntax::FieldPhysical {
                connector: path(&["MechanicalBoundary"]),
                support: "wall".to_owned(),
            },
            range(0, 0),
        )
        .expect("field-physical Port");
        let model = SourceAstFactory::model(
            "coupled",
            vec![Item::Field(field), Item::Port(port)],
            range(0, 0),
        )
        .expect("model");
        let document =
            SourceAstFactory::document(vec![connector], Vec::new(), vec![model]).expect("document");
        let source = format(&document);

        assert_eq!(
            format(
                &parse("factory-boundary.eqi", &source)
                    .into_document()
                    .expect("factory source parses")
            ),
            source
        );
        assert!(SourceAstFactory::connector_quantity("not-valid", dimension()).is_err());
        assert!(
            SourceAstFactory::field_with_shape(
                "bad",
                None,
                None,
                Some(ValueShapeSyntax::Exact(vec![0])),
                dimension(),
                None,
                range(0, 0),
            )
            .is_err()
        );
    }

    #[test]
    fn factory_constructs_complete_exterior_families_and_roundtrips() {
        let body = SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "body",
            SupportSlotSyntax::Volume {
                ambient_dimension: 2,
            },
            range(0, 0),
        )
        .expect("body support");
        let exterior = SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "exterior",
            SupportSlotSyntax::CompleteExterior {
                parent: "body".to_owned(),
            },
            range(0, 0),
        )
        .expect("complete exterior support");
        let binder = SourceAstFactory::boundary_family_binder("boundary", "exterior", range(0, 0))
            .expect("family binder");
        let port = SourceAstFactory::component_port(
            VisibilitySyntax::Public,
            "mechanical",
            PortSyntax::FieldPhysical {
                connector: path(&["MechanicalBoundary"]),
                support: "boundary".to_owned(),
            },
            range(0, 0),
        )
        .expect("component Port");
        let port_family =
            SourceAstFactory::component_port_family(port, binder.clone()).expect("Port family");
        let selector =
            SourceAstFactory::boundary_port_selector("boundary", "boundary", range(0, 0))
                .expect("selector");
        let selected_port = SourceAstFactory::expression(
            ExprKind::BoundaryPortSelection {
                port: Box::new(path(&["mechanical"])),
                selector: Box::new(selector.clone()),
            },
            range(0, 0),
        )
        .expect("selected Port expression");
        let residual = SourceAstFactory::expression(
            ExprKind::Call {
                callee: path(&["flux"]),
                arguments: vec![selected_port],
            },
            range(0, 0),
        )
        .expect("family residual");
        let relation = SourceAstFactory::relation(
            "natural",
            ActivationSyntax::Continuous,
            Some("boundary".to_owned()),
            vec![residual],
            range(0, 0),
        )
        .expect("Relation");
        let relation_family =
            SourceAstFactory::relation_family(relation, binder.clone()).expect("Relation family");
        let left = SourceAstFactory::boundary_port_reference(
            path(&["child", "mechanical"]),
            Some(selector.clone()),
        )
        .expect("left family Port");
        let right =
            SourceAstFactory::boundary_port_reference(path(&["mechanical"]), Some(selector))
                .expect("right family Port");
        let connection =
            SourceAstFactory::boundary_connection(Some(binder), vec![left, right], range(0, 0))
                .expect("pointwise Connection");
        let component = SourceAstFactory::component(
            VisibilitySyntax::Private,
            "BoundaryLaw",
            vec![
                ComponentItem::Support(body),
                ComponentItem::Support(exterior),
                ComponentItem::PortFamily(port_family),
                ComponentItem::RelationFamily(relation_family),
                ComponentItem::BoundaryConnection(connection),
            ],
            range(0, 0),
        )
        .expect("component");

        let members = ["x_lower", "x_upper", "y_lower", "y_upper"]
            .into_iter()
            .map(|member| {
                SourceAstFactory::boundary_set_member(member, range(0, 0)).expect("boundary member")
            })
            .collect();
        let exterior_binding =
            SourceAstFactory::boundary_set_binding("exterior", members, range(0, 0))
                .expect("boundary-set binding");
        let instance = SourceAstFactory::instance_with_boundary_set_bindings(
            "law",
            path(&["BoundaryLaw"]),
            Vec::new(),
            vec![
                SourceAstFactory::support_binding("body", "fluid", range(0, 0))
                    .expect("body binding"),
            ],
            vec![exterior_binding],
            Vec::new(),
            range(0, 0),
        )
        .expect("family-aware instance");
        let model = SourceAstFactory::model("coupled", vec![Item::Instance(instance)], range(0, 0))
            .expect("model");
        let document =
            SourceAstFactory::document(Vec::new(), vec![component], vec![model]).expect("document");

        let source = format(&document);
        let reparsed = parse("complete-exterior-factory.eqi", &source)
            .into_document()
            .expect("factory boundary-family source parses");
        assert_eq!(format(&reparsed), source);
        let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
            panic!("model member is an instance");
        };
        assert_eq!(instance.boundary_set_bindings()[0].members().len(), 4);

        let signal_port = SourceAstFactory::component_port(
            VisibilitySyntax::Public,
            "signal",
            PortSyntax::Signal {
                direction: SignalDirectionSyntax::Input,
                dimension: dimension(),
            },
            range(0, 0),
        )
        .expect("signal Port");
        let binder =
            SourceAstFactory::boundary_family_binder("b", "exterior", range(0, 0)).expect("binder");
        assert!(SourceAstFactory::component_port_family(signal_port, binder).is_err());
    }

    #[test]
    fn construction_rejects_unrepresentable_source_shapes() {
        assert!(NamePath::from_segments(Vec::<String>::new(), range(0, 0)).is_err());
        assert!(NamePath::from_segments(["not-valid"], range(0, 0)).is_err());
        assert!(
            SourceAstFactory::connection(
                ConnectionSyntax::Conserving,
                vec![path(&["only_one"])],
                range(0, 0),
            )
            .is_err()
        );
        assert!(SourceAstFactory::expression(ExprKind::Number(1.0), range(2, 1)).is_err());
        assert!(
            SourceAstFactory::support_slot(
                VisibilitySyntax::Public,
                "boundary",
                SupportSlotSyntax::Boundary {
                    parent: "not-valid".to_owned(),
                },
                range(0, 0),
            )
            .is_err()
        );
        assert!(SourceAstFactory::support_binding("body", "not-valid", range(0, 0)).is_err());
    }

    #[test]
    fn spatial_periodic_connection_is_closed_model_only() {
        let ports = ["lower", "upper"]
            .into_iter()
            .map(|name| {
                SourceAstFactory::boundary_port_reference(path(&[name]), None)
                    .expect("periodic Port reference")
            })
            .collect();
        let connection = SourceAstFactory::spatial_periodic_boundary_connection(ports, range(0, 0))
            .expect("closed-model periodic Connection shape");

        let error = SourceAstFactory::component(
            VisibilitySyntax::Private,
            "InvalidPeriodicComponent",
            vec![ComponentItem::BoundaryConnection(connection)],
            range(0, 0),
        )
        .expect_err("periodic Connection must not enter reusable Component syntax");

        assert_eq!(
            error.to_string(),
            "a spatial-periodic Connection belongs only to a closed Model"
        );
    }
}
