use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents};
use eqiora_lang::{
    ActivationSyntax, BinaryOp, Expr, ExprKind, RelationDecl, RelationFamilyDecl, UnaryOp,
};
use eqiora_schema::kernel::typing::{self, ExpressionType, SpatialSupport, TypeViolation};

use crate::lower::source_error;
use crate::pure_operator::is_builtin_operator;

use super::PhysicalEndpointSelections;
use super::scope::{
    BoundaryFamilyScope, DefinitionScope, PortContract, SymbolContract, unresolved,
};

pub(super) fn validate_relation_expression(
    scope: &DefinitionScope<'_, '_>,
    declaration: &RelationDecl,
    relation_support: Option<SpatialSupport<String>>,
) -> Result<PhysicalEndpointSelections, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    match declaration.activation() {
        ActivationSyntax::Continuous => {}
        ActivationSyntax::Periodic(clock) => match scope.symbols.get(clock) {
            Some(SymbolContract::Clock) => {}
            Some(_) => diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                format!("`{clock}` is not a periodic ClockDomain"),
            )),
            None => diagnostics.push(unresolved(
                scope.file,
                declaration.range(),
                clock,
                "periodic ClockDomain",
            )),
        },
        _ => diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            scope.file,
            declaration.range(),
            "Activation syntax is newer than definition-body validation",
        )),
    }
    if declaration.residuals().is_empty() {
        diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            scope.file,
            declaration.range(),
            "expression DAG requires at least one node and one residual root",
        ));
        return Err(diagnostics);
    }
    let discrete = matches!(declaration.activation(), ActivationSyntax::Periodic(_));
    let mut checker = ExpressionChecker {
        scope,
        relation_support,
        family_scope: None,
        allow_discrete_symbols: discrete,
        physical_endpoints: PhysicalEndpointSelections::new(),
    };
    for residual in declaration.residuals() {
        let inferred = match checker.check(residual) {
            Ok(inferred) => inferred,
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        };
        if let Err(error) = typing::residual(&inferred, checker.relation_support.as_ref()) {
            diagnostics.push(type_error(scope.file, residual, error));
        }
    }
    if diagnostics.is_empty() {
        Ok(checker.physical_endpoints)
    } else {
        Err(diagnostics)
    }
}

pub(super) fn validate_relation_family_expression(
    scope: &DefinitionScope<'_, '_>,
    declaration: &RelationFamilyDecl,
    family_scope: &BoundaryFamilyScope,
) -> Result<PhysicalEndpointSelections, Vec<Diagnostic>> {
    let relation = declaration.relation();
    let mut diagnostics = Vec::new();
    if !matches!(relation.activation(), ActivationSyntax::Continuous) {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary Relation family must be continuous",
        ));
    }
    if declaration.binder() != family_scope.binder() {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary Relation family is not checked under its declared binder",
        ));
    }
    if relation.domain() != Some(declaration.binder().member()) {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "boundary Relation family support must name its binder member",
        ));
    }
    if relation.residuals().is_empty() {
        diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            scope.file,
            declaration.range(),
            "expression DAG requires at least one node and one residual root",
        ));
        return Err(diagnostics);
    }
    let mut checker = ExpressionChecker {
        scope,
        relation_support: Some(family_scope.support()),
        family_scope: Some(family_scope),
        allow_discrete_symbols: false,
        physical_endpoints: PhysicalEndpointSelections::new(),
    };
    for residual in relation.residuals() {
        let inferred = match checker.check(residual) {
            Ok(inferred) => inferred,
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        };
        if let Err(error) = typing::residual(&inferred, checker.relation_support.as_ref()) {
            diagnostics.push(type_error(scope.file, residual, error));
        }
    }
    if diagnostics.is_empty() {
        Ok(checker.physical_endpoints)
    } else {
        Err(diagnostics)
    }
}

struct ExpressionChecker<'a, 'e, 'd> {
    scope: &'a DefinitionScope<'e, 'd>,
    relation_support: Option<SpatialSupport<String>>,
    family_scope: Option<&'a BoundaryFamilyScope>,
    allow_discrete_symbols: bool,
    physical_endpoints: PhysicalEndpointSelections,
}

impl ExpressionChecker<'_, '_, '_> {
    fn check(&mut self, expression: &Expr) -> Result<ExpressionType<String>, Diagnostic> {
        match expression.kind() {
            ExprKind::Number(_) => Ok(ExpressionType::scalar(DimExponents::DIMENSIONLESS, None)),
            ExprKind::Name(name) if name == "time" => {
                Ok(ExpressionType::scalar(time_dimension(), None))
            }
            ExprKind::Name(name) => self.scalar_local_symbol(expression, name),
            ExprKind::Path(path) => {
                self.scalar_contract(expression, path.as_str(), self.scope.resolve_symbol(path)?)
            }
            ExprKind::BoundaryPortSelection { port, selector } => {
                let Some(family_scope) = self.family_scope else {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        "boundary Port selector is valid only inside a boundary family scope",
                    ));
                };
                let contract =
                    self.scope
                        .resolve_boundary_port_selection(port, selector, family_scope)?;
                self.scalar_contract(expression, port.as_str(), SymbolContract::Port(contract))
            }
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => self.check(value),
            ExprKind::Binary { op, left, right } => self.check_binary(expression, *op, left, right),
            ExprKind::Call { callee, arguments } => self.check_call(expression, callee, arguments),
            _ => Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                self.scope.file,
                expression.range(),
                "expression syntax is newer than definition-body validation",
            )),
        }
    }

    fn scalar_local_symbol(
        &mut self,
        expression: &Expr,
        name: &str,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let contract = self.scope.symbols.get(name).cloned().ok_or_else(|| {
            unresolved(
                self.scope.file,
                expression.range(),
                name,
                "expression symbol",
            )
        })?;
        self.scalar_contract(expression, name, contract)
    }

    fn scalar_contract(
        &mut self,
        expression: &Expr,
        display: &str,
        contract: SymbolContract,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        match contract {
            SymbolContract::Field(inferred) | SymbolContract::Parameter(inferred) => Ok(inferred),
            SymbolContract::Port(contract) => contract.scalar_type().ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.scope.file,
                    expression.range(),
                    format!(
                        "scalar physical Port `{display}` must be read as `across({display})` or `through({display})`"
                    ),
                )
            }),
            SymbolContract::PortFamily(_) => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!(
                    "boundary Port family `{display}` requires an exact `[member = target]` selector"
                ),
            )),
            SymbolContract::Domain(_)
            | SymbolContract::Support(_)
            | SymbolContract::CompleteExterior { .. }
            | SymbolContract::Representation
            | SymbolContract::Clock
            | SymbolContract::Relation => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("`{display}` is not a scalar Field, Parameter, or Port"),
            )),
        }
    }

    fn check_call(
        &mut self,
        expression: &Expr,
        callee: &eqiora_lang::NamePath,
        arguments: &[Expr],
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let callee_name = callee.as_str();
        if !is_builtin_operator(callee) {
            let definition = self.scope.elaborator.resolve_pure_operator(
                &self.scope.namespace,
                callee,
                self.scope.file,
                callee.range(),
            )?;
            let inferred = arguments
                .iter()
                .map(|argument| self.check(argument))
                .collect::<Result<Vec<_>, _>>()?;
            return definition
                .definition
                .instantiate(&inferred)
                .map(|application| application.result_type().clone())
                .map_err(|error| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        format!("invalid application of pure operator `{callee}`: {error}"),
                    )
                });
        }
        let [argument] = arguments else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("builtin operator `{callee_name}` requires exactly one argument"),
            ));
        };
        if matches!(callee_name, "across" | "through" | "flux")
            || (callee_name == "trace" && self.is_boundary_port_selection(argument))
        {
            return self.check_physical_accessor(callee_name, argument);
        }
        if callee_name == "coordinate" {
            let axis = integer_literal(argument)
                .and_then(|axis| usize::try_from(axis).ok())
                .ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        argument.range(),
                        "coordinate(...) requires a non-negative integer literal axis",
                    )
                })?;
            return typing::coordinate(axis, self.relation_support.as_ref())
                .map_err(|error| type_error(self.scope.file, expression, error));
        }
        if callee_name == "sin" {
            return typing::sine(&self.check(argument)?)
                .map_err(|error| type_error(self.scope.file, expression, error));
        }
        if matches!(
            callee_name,
            "grad" | "div" | "symmetric_part" | "isotropic_lift" | "trace" | "normal"
        ) {
            let operand = self.check(argument)?;
            let result = match callee_name {
                "grad" => typing::gradient(&operand),
                "div" => typing::divergence(&operand),
                "symmetric_part" => typing::symmetric_part(&operand),
                "isotropic_lift" => typing::isotropic_lift(&operand),
                "trace" => typing::trace(&operand, self.relation_support.as_ref()),
                "normal" => typing::normal(&operand, self.relation_support.as_ref()),
                _ => unreachable!("spatial operator was matched"),
            };
            return result.map_err(|error| type_error(self.scope.file, expression, error));
        }

        let ExprKind::Name(name) = argument.kind() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                argument.range(),
                format!("{callee_name}(...) requires one Field name"),
            ));
        };
        let inferred = match self.scope.symbols.get(name) {
            Some(SymbolContract::Field(inferred)) => inferred.clone(),
            _ => {
                return Err(unresolved(
                    self.scope.file,
                    argument.range(),
                    name,
                    "Field operator argument",
                ));
            }
        };
        if matches!(callee_name, "pre" | "next") && !self.allow_discrete_symbols {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("continuous Relation cannot use `{callee_name}`"),
            ));
        }
        match callee_name {
            "derivative" => typing::time_derivative(&inferred)
                .map_err(|error| type_error(self.scope.file, expression, error)),
            "pre" | "next" => Ok(inferred),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("unknown scalar operator `{callee_name}`"),
            )),
        }
    }

    fn is_boundary_port_selection(&self, expression: &Expr) -> bool {
        let contract = match expression.kind() {
            ExprKind::Name(name) => self.scope.symbols.get(name).cloned(),
            ExprKind::Path(path) => self.scope.resolve_symbol(path).ok(),
            ExprKind::BoundaryPortSelection { port, selector } => self
                .family_scope
                .and_then(|family_scope| {
                    self.scope
                        .resolve_boundary_port_selection(port, selector, family_scope)
                        .ok()
                })
                .map(SymbolContract::Port),
            _ => None,
        };
        matches!(
            contract,
            Some(SymbolContract::Port(PortContract::BoundaryPhysical { .. }))
        )
    }

    fn check_physical_accessor(
        &mut self,
        callee: &str,
        argument: &Expr,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let (display, contract, endpoint) = match argument.kind() {
            ExprKind::Name(name) => (
                name.clone(),
                self.scope.symbols.get(name).cloned().ok_or_else(|| {
                    unresolved(
                        self.scope.file,
                        argument.range(),
                        name,
                        "scalar physical Port",
                    )
                })?,
                super::ResolvedPhysicalEndpoint::from_expression(argument),
            ),
            ExprKind::Path(path) => (
                path.as_str().to_owned(),
                self.scope.resolve_symbol(path)?,
                super::ResolvedPhysicalEndpoint::from_expression(argument),
            ),
            ExprKind::BoundaryPortSelection { port, selector } => {
                let Some(family_scope) = self.family_scope else {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        argument.range(),
                        "boundary Port selector is valid only inside a boundary family scope",
                    ));
                };
                (
                    format!("{}[{} = {}]", port, selector.member(), selector.target()),
                    SymbolContract::Port(self.scope.resolve_boundary_port_selection(
                        port,
                        selector,
                        family_scope,
                    )?),
                    None,
                )
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.scope.file,
                    argument.range(),
                    format!("`{callee}(...)` requires one scalar physical Port selection"),
                ));
            }
        };
        if let Some(endpoint) = endpoint {
            self.physical_endpoints.insert(endpoint);
        }
        match contract {
            SymbolContract::Port(PortContract::Physical {
                across_dimension,
                through_dimension,
                ..
            }) if matches!(callee, "across" | "through") => Ok(ExpressionType::scalar(
                if callee == "across" {
                    across_dimension
                } else {
                    through_dimension
                },
                None,
            )),
            SymbolContract::Port(PortContract::BoundaryPhysical {
                connector, support, ..
            }) if matches!(callee, "trace" | "flux") => Ok(ExpressionType::shaped(
                if callee == "trace" {
                    connector.trace_dimension()
                } else {
                    connector.flux_dimension()
                },
                connector.shape().clone(),
                connector.frame(),
                Some(support),
            )),
            SymbolContract::Port(PortContract::BoundaryPhysical { .. }) => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                argument.range(),
                format!(
                    "field-physical Port `{display}` must be read as `trace({display})` or `flux({display})`"
                ),
            )),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                argument.range(),
                format!("`{display}` is not compatible with `{callee}(...)`"),
            )),
        }
    }

    fn check_binary(
        &mut self,
        expression: &Expr,
        operator: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        if operator == BinaryOp::Pow {
            let base = self.check(left)?;
            let exponent = integer_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.scope.file,
                    right.range(),
                    "power exponent must be an i32 integer literal",
                )
            })?;
            return typing::power(&base, exponent)
                .map_err(|error| type_error(self.scope.file, expression, error));
        }
        let left = self.check(left)?;
        let right = self.check(right)?;
        let result = match operator {
            BinaryOp::Add | BinaryOp::Sub => typing::additive(&left, &right),
            BinaryOp::Mul => typing::multiply(&left, &right),
            BinaryOp::Div => typing::divide(&left, &right),
            BinaryOp::Pow => unreachable!("power handled above"),
        };
        result.map_err(|error| type_error(self.scope.file, expression, error))
    }
}

fn type_error(file: &str, expression: &Expr, error: TypeViolation<String>) -> Diagnostic {
    let message = match &error {
        TypeViolation::AdditiveTypeMismatch { left, right } if left.shape == right.shape => {
            format!(
                "addition/subtraction combines dimensions [{}] and [{}]",
                left.dimension, right.dimension
            )
        }
        TypeViolation::SinRequiresDimensionlessScalar => {
            "sin(...) requires a dimensionless scalar".to_owned()
        }
        _ => error.to_string(),
    };
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        expression.range(),
        message,
    )
}

fn integer_literal(expression: &Expr) -> Option<i32> {
    let value = match expression.kind() {
        ExprKind::Number(value) => *value,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => match value.kind() {
            ExprKind::Number(value) => -*value,
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

const fn time_dimension() -> DimExponents {
    DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    }
}
