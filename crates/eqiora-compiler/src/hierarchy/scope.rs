use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
#[cfg(test)]
use eqiora_lang::SourceAstFactory;
use eqiora_lang::{
    ActivationSyntax, BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax, Expr, ExprKind,
    FieldDecl, NamePath, PortSyntax, RelationDecl, TextRange,
};

use crate::diagnostics::source_error;
use crate::identity::FullElaborationIdentity;
use crate::lower::LoweringExpression;
use crate::pure_operator::is_builtin_operator;
use eqiora_schema::kernel::pure_operator::PureOperatorDefinition;
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

use super::flat::SourceLocation;
use super::parameters::ResolvedParameter;
use super::supports::ResolvedBoundarySet;

mod external;
mod lets;

#[derive(Debug, Clone)]
pub(super) struct FlatSymbol {
    pub(super) internal_name: String,
    pub(super) display_name: String,
    pub(super) full_identity: FullElaborationIdentity,
    pub(super) kind: SymbolKind,
}

#[derive(Debug, Clone)]
pub(super) enum SymbolKind {
    Domain,
    Representation,
    Field,
    Parameter,
    Port,
    Clock,
    Relation,
}

impl FlatSymbol {
    fn is_port(&self) -> bool {
        matches!(self.kind, SymbolKind::Port)
    }
}

#[derive(Debug, Clone)]
pub(super) struct InstanceInterface {
    public_ports: BTreeMap<String, FlatSymbol>,
    public_port_families: BTreeMap<String, BoundaryPortFamilyIndex>,
}

impl InstanceInterface {
    pub(super) fn with_public_port_families(
        public_ports: BTreeMap<String, FlatSymbol>,
        public_port_families: BTreeMap<String, BoundaryPortFamilyIndex>,
    ) -> Self {
        Self {
            public_ports,
            public_port_families,
        }
    }
}

/// The exact occurrence members of one source-declared boundary Port family.
///
/// This is deliberately not a collection of values in the semantic model. It
/// is a lexical resolution index: the family spelling and selector binder lead
/// to one ordinary Port identified by the exact Boundary occurrence identity.
#[derive(Debug, Clone)]
pub(super) struct BoundaryPortFamilyIndex {
    selector_member: String,
    members: BTreeMap<FullElaborationIdentity, FlatSymbol>,
}

/// The exact Boundary selected by the currently expanded family binder.
#[derive(Debug, Clone, Copy)]
pub(super) struct ActiveBoundaryMember<'a> {
    member: &'a str,
    boundary: FullElaborationIdentity,
}

impl<'a> ActiveBoundaryMember<'a> {
    pub(super) const fn new(member: &'a str, boundary: FullElaborationIdentity) -> Self {
        Self { member, boundary }
    }
}

#[derive(Debug, Default)]
pub(super) struct Scope {
    symbols: BTreeMap<String, FlatSymbol>,
    port_families: BTreeMap<String, BoundaryPortFamilyIndex>,
    boundary_sets: BTreeMap<String, ResolvedBoundarySet<FullElaborationIdentity>>,
    children: BTreeMap<String, InstanceInterface>,
    spatial_supports: BTreeMap<String, SpatialSupport<FullElaborationIdentity>>,
    field_types: BTreeMap<String, ExpressionType<FullElaborationIdentity>>,
    parameters: BTreeMap<String, ResolvedParameter>,
    pure_operators: BTreeMap<String, PureOperatorDefinition>,
    occurrence_bindings: Vec<SourceLocation>,
    forwarded_parameter_resolution_bindings: Vec<SourceLocation>,
    forwarded_field_resolution_bindings: Vec<SourceLocation>,
    forwarded_boundary_set_resolution_bindings: Vec<SourceLocation>,
    // Preserve source-occurrence DAG shape only for the external root path.
    detach_parameter_expressions: bool,
}

impl Scope {
    pub(super) fn set_pure_operators(
        &mut self,
        definitions: BTreeMap<String, PureOperatorDefinition>,
    ) {
        self.pure_operators = definitions;
    }

    fn pure_operator(&self, path: &NamePath) -> Option<&PureOperatorDefinition> {
        self.pure_operators.get(path.as_str())
    }
    pub(super) fn insert_symbol(&mut self, name: String, symbol: FlatSymbol) -> Option<FlatSymbol> {
        self.symbols.insert(name, symbol)
    }

    pub(super) fn insert_child(
        &mut self,
        name: String,
        child: InstanceInterface,
    ) -> Option<InstanceInterface> {
        self.children.insert(name, child)
    }

    pub(super) fn symbol(&self, name: &str) -> Option<&FlatSymbol> {
        self.symbols.get(name)
    }

    pub(super) fn insert_port_family_member(
        &mut self,
        file: &str,
        range: TextRange,
        family_name: String,
        selector_member: &str,
        boundary: FullElaborationIdentity,
        symbol: FlatSymbol,
    ) -> Result<(), Diagnostic> {
        if !symbol.is_port() {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "boundary Port family member did not lower to an ordinary Port",
            ));
        }
        if self.symbols.contains_key(&family_name) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("Port family `{family_name}` collides with an ordinary symbol"),
            ));
        }

        if let Some(family) = self.port_families.get_mut(&family_name) {
            if family.selector_member != selector_member {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!(
                        "Port family `{family_name}` cannot use both `{}` and `{selector_member}` as its selector member",
                        family.selector_member
                    ),
                ));
            }
            if family.members.contains_key(&boundary) {
                return Err(source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    file,
                    range,
                    format!(
                        "Port family `{family_name}` contains the same exact Boundary more than once"
                    ),
                ));
            }
            family.members.insert(boundary, symbol);
        } else {
            self.port_families.insert(
                family_name,
                BoundaryPortFamilyIndex {
                    selector_member: selector_member.to_owned(),
                    members: BTreeMap::from([(boundary, symbol)]),
                },
            );
        }
        Ok(())
    }

    pub(super) fn port_family(&self, name: &str) -> Option<&BoundaryPortFamilyIndex> {
        self.port_families.get(name)
    }

    pub(super) fn insert_boundary_set(
        &mut self,
        name: String,
        set: ResolvedBoundarySet<FullElaborationIdentity>,
    ) -> Option<ResolvedBoundarySet<FullElaborationIdentity>> {
        self.boundary_sets.insert(name, set)
    }

    pub(super) fn boundary_set(
        &self,
        name: &str,
    ) -> Option<&ResolvedBoundarySet<FullElaborationIdentity>> {
        self.boundary_sets.get(name)
    }

    pub(super) fn insert_spatial_support(
        &mut self,
        name: String,
        support: SpatialSupport<FullElaborationIdentity>,
    ) -> Option<SpatialSupport<FullElaborationIdentity>> {
        self.spatial_supports.insert(name, support)
    }

    pub(super) fn spatial_support(
        &self,
        name: &str,
    ) -> Option<&SpatialSupport<FullElaborationIdentity>> {
        self.spatial_supports.get(name)
    }

    pub(super) fn spatial_support_by_identity(
        &self,
        identity: FullElaborationIdentity,
    ) -> Option<&SpatialSupport<FullElaborationIdentity>> {
        self.spatial_supports
            .values()
            .find(|support| match support {
                SpatialSupport::Volume { domain, .. } | SpatialSupport::Boundary { domain, .. } => {
                    *domain == identity
                }
                SpatialSupport::Interface { .. } => false,
            })
    }

    pub(super) fn insert_field_type(
        &mut self,
        name: String,
        field_type: ExpressionType<FullElaborationIdentity>,
    ) -> Option<ExpressionType<FullElaborationIdentity>> {
        self.field_types.insert(name, field_type)
    }

    pub(super) fn field_type(
        &self,
        name: &str,
    ) -> Option<&ExpressionType<FullElaborationIdentity>> {
        self.field_types.get(name)
    }

    pub(super) fn set_occurrence_bindings(&mut self, bindings: Vec<SourceLocation>) {
        self.occurrence_bindings = bindings;
    }

    pub(super) fn occurrence_bindings(&self) -> &[SourceLocation] {
        &self.occurrence_bindings
    }

    pub(super) fn set_forwarded_parameter_resolution_bindings(
        &mut self,
        bindings: Vec<SourceLocation>,
    ) {
        self.forwarded_parameter_resolution_bindings = bindings;
    }

    pub(super) fn forwarded_parameter_resolution_bindings(&self) -> &[SourceLocation] {
        &self.forwarded_parameter_resolution_bindings
    }

    pub(super) fn set_forwarded_field_resolution_bindings(
        &mut self,
        bindings: Vec<SourceLocation>,
    ) {
        self.forwarded_field_resolution_bindings = bindings;
    }

    pub(super) fn forwarded_field_resolution_bindings(&self) -> &[SourceLocation] {
        &self.forwarded_field_resolution_bindings
    }

    pub(super) fn set_forwarded_boundary_set_resolution_bindings(
        &mut self,
        bindings: Vec<SourceLocation>,
    ) {
        self.forwarded_boundary_set_resolution_bindings = bindings;
    }

    pub(super) fn forwarded_boundary_set_resolution_bindings(&self) -> &[SourceLocation] {
        &self.forwarded_boundary_set_resolution_bindings
    }

    pub(super) fn resolve_port(&self, path: &NamePath) -> Option<&FlatSymbol> {
        let segments = path.segments().collect::<Vec<_>>();
        match segments.as_slice() {
            [name] => self.symbols.get(*name).filter(|symbol| symbol.is_port()),
            [instance, member] => self
                .children
                .get(*instance)
                .and_then(|child| child.public_ports.get(*member)),
            _ => None,
        }
    }

    pub(super) fn resolve_symbol(&self, path: &NamePath) -> Option<&FlatSymbol> {
        let segments = path.segments().collect::<Vec<_>>();
        match segments.as_slice() {
            [name] => self.symbols.get(*name),
            [instance, member] => self
                .children
                .get(*instance)
                .and_then(|child| child.public_ports.get(*member)),
            _ => None,
        }
    }
}

pub(super) fn rewrite_field_scope(
    file: &str,
    declaration: &FieldDecl,
    scope: &Scope,
) -> Result<(Option<String>, Option<String>), Diagnostic> {
    match (declaration.domain(), declaration.representation()) {
        (None, None) => Ok((None, None)),
        (Some(domain), Some(representation)) => {
            let domain = resolve_local_kind(
                file,
                declaration.range(),
                scope,
                domain,
                |kind| matches!(kind, SymbolKind::Domain),
                "Field Domain",
            )?;
            let representation = resolve_local_kind(
                file,
                declaration.range(),
                scope,
                representation,
                |kind| matches!(kind, SymbolKind::Representation),
                "Field Representation",
            )?;
            Ok((
                Some(domain.internal_name.clone()),
                Some(representation.internal_name.clone()),
            ))
        }
        _ => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            "spatial Field requires both Domain and Representation",
        )),
    }
}

pub(super) fn rewrite_model_port(
    file: &str,
    syntax: &PortSyntax,
    range: TextRange,
    scope: &Scope,
) -> Result<PortSyntax, Diagnostic> {
    match syntax {
        PortSyntax::Signal {
            direction,
            dimension,
        } => Ok(PortSyntax::Signal {
            direction: *direction,
            dimension: dimension.clone(),
        }),
        PortSyntax::ConservingMarker { dimension } => Ok(PortSyntax::ConservingMarker {
            dimension: dimension.clone(),
        }),
        PortSyntax::ScalarPhysical { domain } => {
            let domain = resolve_local_kind(
                file,
                range,
                scope,
                domain,
                |kind| matches!(kind, SymbolKind::Domain),
                "scalar physical Domain",
            )?;
            Ok(PortSyntax::ScalarPhysical {
                domain: domain.internal_name.clone(),
            })
        }
        PortSyntax::ScalarPhysicalConnector { .. } => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "model-level Port cannot use a component Connector declaration directly",
        )),
        _ => Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Port syntax is newer than hierarchy elaboration",
        )),
    }
}

pub(super) fn rewrite_relation(
    file: &str,
    declaration: &RelationDecl,
    scope: &Scope,
) -> Result<(ActivationSyntax, Option<String>, Vec<LoweringExpression>), Diagnostic> {
    let activation = match declaration.activation() {
        ActivationSyntax::Continuous => ActivationSyntax::Continuous,
        ActivationSyntax::Periodic(clock) => {
            let clock = resolve_local_kind(
                file,
                declaration.range(),
                scope,
                clock,
                |kind| matches!(kind, SymbolKind::Clock),
                "periodic ClockDomain",
            )?;
            ActivationSyntax::Periodic(clock.internal_name.clone())
        }
        _ => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                declaration.range(),
                "Activation syntax is newer than hierarchy elaboration",
            ));
        }
    };
    let domain = declaration
        .domain()
        .map(|name| {
            resolve_local_kind(
                file,
                declaration.range(),
                scope,
                name,
                |kind| matches!(kind, SymbolKind::Domain),
                "Relation Domain",
            )
            .map(|symbol| symbol.internal_name.clone())
        })
        .transpose()?;
    let residuals = declaration
        .residuals()
        .iter()
        .map(|expression| rewrite_expression(file, expression, scope))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((activation, domain, residuals))
}

pub(super) fn rewrite_expression(
    file: &str,
    expression: &Expr,
    scope: &Scope,
) -> Result<LoweringExpression, Diagnostic> {
    rewrite_expression_with_boundary_member(file, expression, scope, None)
}

pub(super) fn rewrite_expression_with_boundary_member(
    file: &str,
    expression: &Expr,
    scope: &Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<LoweringExpression, Diagnostic> {
    let lowered = match expression.kind() {
        ExprKind::Number(_) | ExprKind::Quantity { .. } => {
            LoweringExpression::from_source(expression)
        }
        ExprKind::Name(name) if name == "time" => {
            LoweringExpression::name(name.clone(), expression.range())
        }
        ExprKind::Name(name) if scope.parameter(name).is_some() => scope.parameter_expression(name),
        ExprKind::Name(name) => {
            let path = NamePath::from_segments([name.clone()], expression.range()).map_err(
                |ast_error| {
                    source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        file,
                        expression.range(),
                        ast_error.message(),
                    )
                },
            )?;
            LoweringExpression::name(
                resolve_expression_symbol(file, &path, scope)?
                    .internal_name
                    .clone(),
                expression.range(),
            )
        }
        ExprKind::Path(path) => match crate::math::constant(path) {
            Some(value) => LoweringExpression::quantity(
                DynQuantity::new(value, DimExponents::DIMENSIONLESS),
                expression.range(),
            ),
            None => LoweringExpression::name(
                resolve_expression_symbol(file, path, scope)?
                    .internal_name
                    .clone(),
                expression.range(),
            ),
        },
        ExprKind::BoundaryPortSelection { port, selector } => LoweringExpression::name(
            resolve_boundary_family_selection(file, port, selector, scope, active)?
                .internal_name
                .clone(),
            expression.range(),
        ),
        ExprKind::Unary {
            op: eqiora_lang::UnaryOp::Neg,
            value,
        } => LoweringExpression::neg(
            rewrite_expression_with_boundary_member(file, value, scope, active)?,
            expression.range(),
        ),
        ExprKind::Binary { op, left, right } => LoweringExpression::binary(
            *op,
            rewrite_expression_with_boundary_member(file, left, scope, active)?,
            rewrite_expression_with_boundary_member(file, right, scope, active)?,
            expression.range(),
        ),
        ExprKind::Call { callee, arguments } if is_builtin_operator(callee) => {
            let [argument] = arguments.as_slice() else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    format!("builtin operator `{callee}` requires exactly one argument"),
                ));
            };
            LoweringExpression::call(
                callee.as_str().to_owned(),
                rewrite_expression_with_boundary_member(file, argument, scope, active)?,
                expression.range(),
            )
        }
        ExprKind::Call { callee, arguments } => {
            let definition = scope.pure_operator(callee).cloned().ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    callee.range(),
                    format!("unresolved pure operator `{callee}`"),
                )
            })?;
            let arguments = arguments
                .iter()
                .map(|argument| {
                    rewrite_expression_with_boundary_member(file, argument, scope, active)
                })
                .collect::<Result<Vec<_>, _>>()?;
            LoweringExpression::pure_operator(definition, arguments, expression.range())
        }
        _ => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                expression.range(),
                "expression syntax is newer than hierarchy elaboration",
            ));
        }
    };
    Ok(lowered)
}

pub(super) fn resolve_boundary_port_reference<'a>(
    file: &str,
    reference: &BoundaryPortReferenceSyntax,
    scope: &'a Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<&'a FlatSymbol, Diagnostic> {
    match reference.selector() {
        Some(selector) => {
            if scope.resolve_port(reference.port()).is_some() {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    selector.range(),
                    format!(
                        "ordinary Port `{}` cannot carry a boundary-family selector",
                        reference.port()
                    ),
                ));
            }
            resolve_boundary_family_selection(file, reference.port(), selector, scope, active)
        }
        None => {
            if resolve_port_family(scope, reference.port()).is_some() {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    reference.port().range(),
                    format!(
                        "Port family `{}` requires an exact boundary selector",
                        reference.port()
                    ),
                ));
            }
            scope.resolve_port(reference.port()).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    reference.port().range(),
                    format!(
                        "`{}` does not select a visible Port in this scope",
                        reference.port()
                    ),
                )
            })
        }
    }
}

fn resolve_expression_symbol<'a>(
    file: &str,
    path: &NamePath,
    scope: &'a Scope,
) -> Result<&'a FlatSymbol, Diagnostic> {
    if resolve_port_family(scope, path).is_some() {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            path.range(),
            format!("Port family `{path}` requires an exact boundary selector"),
        ));
    }
    scope.resolve_symbol(path).ok_or_else(|| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            path.range(),
            if path.is_qualified() {
                format!("qualified name `{path}` does not select a public Port in this scope")
            } else {
                format!("unresolved name `{path}` in this component instance")
            },
        )
    })
}

fn resolve_boundary_family_selection<'a>(
    file: &str,
    path: &NamePath,
    selector: &BoundaryPortSelectorSyntax,
    scope: &'a Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<&'a FlatSymbol, Diagnostic> {
    let Some(family) = resolve_port_family(scope, path) else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            path.range(),
            format!("`{path}` does not select a visible boundary Port family"),
        ));
    };
    if selector.member() != family.selector_member {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            selector.range(),
            format!(
                "selector member `{}` does not match `{}` declared by Port family `{path}`",
                selector.member(),
                family.selector_member
            ),
        ));
    }

    let boundary = resolve_exact_boundary(file, selector, scope, active)?;
    family.members.get(&boundary).ok_or_else(|| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            selector.range(),
            format!(
                "Port family `{path}` has no member on exact Boundary `{}`",
                selector.target()
            ),
        )
    })
}

fn resolve_port_family<'a>(
    scope: &'a Scope,
    path: &NamePath,
) -> Option<&'a BoundaryPortFamilyIndex> {
    let segments = path.segments().collect::<Vec<_>>();
    match segments.as_slice() {
        [name] => scope.port_families.get(*name),
        [instance, member] => scope
            .children
            .get(*instance)
            .and_then(|child| child.public_port_families.get(*member)),
        _ => None,
    }
}

fn resolve_exact_boundary(
    file: &str,
    selector: &BoundaryPortSelectorSyntax,
    scope: &Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<FullElaborationIdentity, Diagnostic> {
    if let Some(active) = active
        && selector.target() == active.member
    {
        return Ok(active.boundary);
    }

    let Some(support) = scope.spatial_support(selector.target()) else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            selector.range(),
            format!(
                "unresolved exact Boundary selector target `{}`",
                selector.target()
            ),
        ));
    };
    match support {
        SpatialSupport::Boundary { domain, .. } => Ok(*domain),
        SpatialSupport::Volume { .. } | SpatialSupport::Interface { .. } => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            selector.range(),
            format!(
                "selector target `{}` is not an exact Boundary support",
                selector.target()
            ),
        )),
    }
}

pub(super) fn resolve_ports<'a>(
    file: &str,
    range: TextRange,
    paths: &[NamePath],
    scope: &'a Scope,
) -> Result<Vec<&'a FlatSymbol>, Diagnostic> {
    let ports = resolve_visible_ports(file, paths, scope)?;
    if ports.len() < 2 {
        Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "Connection requires at least two visible Ports",
        ))
    } else {
        Ok(ports)
    }
}

pub(super) fn resolve_visible_ports<'a>(
    file: &str,
    paths: &[NamePath],
    scope: &'a Scope,
) -> Result<Vec<&'a FlatSymbol>, Diagnostic> {
    paths
        .iter()
        .map(|path| {
            scope.resolve_port(path).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    path.range(),
                    format!("`{path}` does not select a visible Port in this scope"),
                )
            })
        })
        .collect()
}

pub(super) fn resolve_local_kind<'a>(
    file: &str,
    range: TextRange,
    scope: &'a Scope,
    name: &str,
    expected: impl FnOnce(&SymbolKind) -> bool,
    label: &str,
) -> Result<&'a FlatSymbol, Diagnostic> {
    let Some(symbol) = scope.symbols.get(name) else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("unresolved {label} `{name}`"),
        ));
    };
    if !expected(&symbol.kind) {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("`{name}` is not a {label}"),
        ));
    }
    Ok(symbol)
}

#[cfg(test)]
mod tests;
