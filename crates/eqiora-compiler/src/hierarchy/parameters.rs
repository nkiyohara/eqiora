use std::collections::{BTreeMap, BTreeSet, VecDeque};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, ScalarDomain, ValueLiteral, ValueType};
use eqiora_lang::{
    BinaryOp, ComponentDecl, ComponentItem, ComponentParameterDecl, Expr, ExprKind, InstanceDecl,
    TextRange, UnaryOp, VisibilitySyntax,
};

use crate::diagnostics::{source_error, stable_sort};
use crate::identity::FullElaborationIdentity;
use crate::lower::LoweringExpression;

use super::hierarchy_error;

mod dependencies;
mod expression_eval;
mod predicates;
mod value_expressions;
use dependencies::{
    ExpressionDefinition, collect_expression_dependencies, expression_cycles,
    expression_evaluation_order,
};
mod model_lets;
mod model_parameters;
use expression_eval::{
    ExpressionContext, coerce_parameter, coerce_parameter_with_label, evaluate_initializer,
};
pub(super) use model_lets::{alias_order, resolve_component_lets, resolve_model_lets};
pub(super) use model_parameters::{
    resolve_model_parameters, resolve_model_parameters_symbolically,
};

#[derive(Debug, Clone)]
pub(super) struct ResolvedParameter {
    pub(super) value: ValueLiteral,
    pub(super) expression: LoweringExpression,
    pub(super) lineage: ParameterLineage,
}

/// One component Parameter after definition-time symbolic resolution.
///
/// The checked type is retained through symbolic resolution. `value`
/// is absent exactly when the expression depends on at least one required
/// public Parameter whose value belongs to a future component occurrence.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SymbolicParameterValue {
    pub(super) value: Option<ValueLiteral>,
    pub(super) value_type: ValueType,
    pub(super) expression: Option<LoweringExpression>,
    pub(super) lineage: Option<ParameterLineage>,
}

pub(super) type SymbolicParameterMap = BTreeMap<String, SymbolicParameterValue>;

fn component_parameter_type(
    file: &str,
    declaration: &ComponentParameterDecl,
) -> Result<ValueType, Diagnostic> {
    crate::value_types::lower_value_type::<()>(file, declaration.value_type(), None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EvaluatedType {
    Known(ValueType),
    /// Only dimension is unresolved. The dimensionless projection retains
    /// scalar domain, component roles and frame for shared type checking.
    Deferred(ValueType),
}

impl EvaluatedType {
    fn dimension(&self) -> Option<DimExponents> {
        match self {
            Self::Known(value) => Some(value.dimension()),
            Self::Deferred(_) => None,
        }
    }

    fn value_type(&self) -> &ValueType {
        match self {
            Self::Known(value) | Self::Deferred(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EvaluatedParameter {
    value: Option<ValueLiteral>,
    value_type: EvaluatedType,
    bare_literal: bool,
    expression: Option<LoweringExpression>,
    lineage: Option<ParameterLineage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ParameterLineage {
    Constant,
    Parameter(FullElaborationIdentity),
    Derived,
}

impl ResolvedParameter {
    pub(super) fn model_parameter(
        value: ValueLiteral,
        identity: FullElaborationIdentity,
        internal_name: String,
        range: TextRange,
    ) -> Self {
        Self {
            value,
            expression: LoweringExpression::name(internal_name, range),
            lineage: ParameterLineage::Parameter(identity),
        }
    }
}

impl From<ResolvedParameter> for SymbolicParameterValue {
    fn from(parameter: ResolvedParameter) -> Self {
        Self {
            value: Some(parameter.value.clone()),
            value_type: parameter.value.value_type().clone(),
            expression: Some(parameter.expression),
            lineage: Some(parameter.lineage),
        }
    }
}

impl From<SymbolicParameterValue> for EvaluatedParameter {
    fn from(value: SymbolicParameterValue) -> Self {
        Self {
            value: value.value,
            value_type: EvaluatedType::Known(value.value_type),
            bare_literal: false,
            expression: value.expression,
            lineage: value.lineage,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredParameterPolicy {
    RejectUnbound,
    PublicIsFree,
}

struct SymbolicParameterResolver<'a> {
    declaration_file: &'a str,
    declarations: BTreeMap<String, &'a ComponentParameterDecl>,
    overrides: BTreeMap<String, SymbolicParameterValue>,
    resolved: SymbolicParameterMap,
    required_policy: RequiredParameterPolicy,
}

impl<'a> SymbolicParameterResolver<'a> {
    fn component_interface(declaration_file: &'a str, component: &'a ComponentDecl) -> Self {
        Self {
            declaration_file,
            declarations: parameter_declarations(component),
            overrides: BTreeMap::new(),
            resolved: BTreeMap::new(),
            required_policy: RequiredParameterPolicy::PublicIsFree,
        }
    }

    fn instance(
        declaration_file: &'a str,
        binding_file: &str,
        component: &'a ComponentDecl,
        instance: &InstanceDecl,
        resolve_parent: impl FnMut(&str) -> Option<SymbolicParameterValue>,
        resolve_clock: &mut dyn FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let declarations = parameter_declarations(component);
        let overrides = resolve_instance_overrides(
            (declaration_file, binding_file),
            component,
            instance,
            &declarations,
            resolve_parent,
            resolve_clock,
            ExpressionContext::Binding,
        )?;
        Ok(Self {
            declaration_file,
            declarations,
            overrides,
            resolved: BTreeMap::new(),
            required_policy: RequiredParameterPolicy::RejectUnbound,
        })
    }

    fn resolve_all(
        mut self,
        resolve_clock: &mut dyn FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
    ) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut defaults = BTreeMap::new();

        for (name, &declaration) in &self.declarations {
            if let Some(value) = self.overrides.get(name).cloned() {
                self.resolved.insert(name.to_owned(), value);
                continue;
            }

            let target = match component_parameter_type(self.declaration_file, declaration) {
                Ok(target) => Some(target),
                Err(error) => {
                    diagnostics.push(error);
                    None
                }
            };
            let Some(default) = declaration.default() else {
                let Some(target) = target else {
                    continue;
                };
                match (self.required_policy, declaration.visibility()) {
                    (RequiredParameterPolicy::PublicIsFree, VisibilitySyntax::Public) => {
                        self.resolved.insert(
                            name.to_owned(),
                            SymbolicParameterValue {
                                value: None,
                                value_type: target,
                                expression: None,
                                lineage: None,
                            },
                        );
                    }
                    (RequiredParameterPolicy::PublicIsFree, VisibilitySyntax::Private) => {
                        diagnostics.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.declaration_file,
                            declaration.range(),
                            format!("required private Parameter `{name}` has no default"),
                        ));
                    }
                    (RequiredParameterPolicy::RejectUnbound, _) => {
                        diagnostics.push(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.declaration_file,
                            declaration.range(),
                            format!("required Parameter `{name}` has no instance binding"),
                        ));
                    }
                }
                continue;
            };

            let (dependencies, mut errors) = collect_expression_dependencies(
                self.declaration_file,
                default,
                |name| self.declarations.contains_key(name),
                ExpressionContext::Default,
            );
            let valid = target.is_some() && errors.is_empty();
            diagnostics.append(&mut errors);
            defaults.insert(
                name.to_owned(),
                ExpressionDefinition {
                    expression: default,
                    target,
                    dependencies,
                    valid,
                },
            );
        }

        let cycles = expression_cycles(&defaults);
        let mut cyclic = BTreeSet::new();
        for cycle in cycles {
            cyclic.extend(cycle.members.iter().cloned());
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.declaration_file,
                cycle.range,
                format!(
                    "component Parameter dependency cycle: {}",
                    cycle.path.join(" -> ")
                ),
            ));
        }

        for name in expression_evaluation_order(&defaults, &cyclic) {
            let parameter = &defaults[&name];
            if !parameter.valid
                || parameter
                    .dependencies
                    .keys()
                    .any(|dependency| !self.resolved.contains_key(dependency))
            {
                continue;
            }
            let evaluated = evaluate_initializer(
                self.declaration_file,
                parameter.expression,
                ExpressionContext::Default,
                &mut |dependency, range| {
                    self.resolved.get(dependency).cloned().ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.declaration_file,
                            range,
                            format!("unknown component Parameter `{dependency}`"),
                        )
                    })
                },
                parameter.target.clone().expect("valid default target"),
                "Parameter initializer",
                resolve_clock,
            )
            .and_then(|evaluated| {
                coerce_parameter_with_label(
                    self.declaration_file,
                    parameter.expression.range(),
                    evaluated,
                    parameter
                        .target
                        .clone()
                        .expect("valid default has a target dimension"),
                    "Parameter initializer",
                    true,
                )
            });
            match evaluated {
                Ok(value) => {
                    self.resolved.insert(name, value);
                }
                Err(error) => diagnostics.push(error),
            }
        }

        stable_sort(&mut diagnostics);
        if diagnostics.is_empty() {
            Ok(self.resolved)
        } else {
            Err(diagnostics)
        }
    }
}

fn parameter_declarations(component: &ComponentDecl) -> BTreeMap<String, &ComponentParameterDecl> {
    component
        .signature()
        .iter()
        .filter_map(|item| match item {
            eqiora_lang::SignatureItem::Parameter(value) => Some((value.name().to_owned(), value)),
            _ => None,
        })
        .chain(component.items().iter().filter_map(|item| match item {
            ComponentItem::Parameter(value) => Some((value.name().to_owned(), value)),
            _ => None,
        }))
        .collect()
}

fn resolve_instance_overrides(
    (declaration_file, binding_file): (&str, &str),
    component: &ComponentDecl,
    instance: &InstanceDecl,
    declarations: &BTreeMap<String, &ComponentParameterDecl>,
    mut resolve_parent: impl FnMut(&str) -> Option<SymbolicParameterValue>,
    resolve_clock: &mut dyn FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
    context: ExpressionContext<'_>,
) -> Result<BTreeMap<String, SymbolicParameterValue>, Vec<Diagnostic>> {
    let mut overrides = BTreeMap::new();
    let mut bound = BTreeSet::new();
    let mut diagnostics = super::named_bindings::validate_names(binding_file, component, instance);
    for binding in instance
        .bindings()
        .iter()
        .filter(|binding| declarations.contains_key(binding.name()))
    {
        if !bound.insert(binding.name()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "duplicate binding for Parameter `{}` in instance `{}`",
                    binding.name(),
                    instance.name()
                ),
            ));
            continue;
        }
        let Some(declaration) = declarations.get(binding.name()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "unknown public Parameter `{}` on component `{}`",
                    binding.name(),
                    component.name()
                ),
            ));
            continue;
        };
        if declaration.visibility() != VisibilitySyntax::Public {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "private Parameter `{}` cannot be bound on instance `{}`",
                    binding.name(),
                    instance.name()
                ),
            ));
            continue;
        }
        let target = match component_parameter_type(declaration_file, declaration) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        };
        let value = expression_eval::evaluate_with_domain(
            binding_file,
            binding.value(),
            context,
            &mut |name, range| {
                resolve_parent(name).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        binding_file,
                        range,
                        format!(
                            "compile-time binding name `{name}` is not an enclosing scalar Parameter"
                        ),
                    )
                })
            },
         resolve_clock,
         (target.scalar_domain() == ScalarDomain::Integer).then_some(ScalarDomain::Integer))
        .and_then(|value| coerce_parameter(binding_file, binding.range(), value, target));
        match value {
            Ok(value) => {
                overrides.insert(binding.name().to_owned(), value);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if diagnostics.is_empty() {
        Ok(overrides)
    } else {
        Err(diagnostics)
    }
}

/// Resolve a reusable Component's Parameter interface without inventing an
/// occurrence value for any required public Parameter.
pub(super) fn resolve_component_parameters_symbolically(
    declaration_file: &str,
    component: &ComponentDecl,
    mut resolve_clock: impl FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    SymbolicParameterResolver::component_interface(declaration_file, component)
        .resolve_all(&mut resolve_clock)
}

/// Validate a nested instance against an already-resolved child interface.
///
/// Definition checking should prefer this operation to full instance
/// resolution. The child default graph has already been checked exactly once
/// when `child_interface` was constructed, so one definition edge visits only
/// its binding expressions and the child's required public declarations.
pub(super) fn validate_instance_parameters_symbolically(
    declaration_file: &str,
    binding_file: &str,
    component: &ComponentDecl,
    instance: &InstanceDecl,
    parent_parameters: &SymbolicParameterMap,
    child_interface: &SymbolicParameterMap,
    mut resolve_clock: impl FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = parameter_declarations(component);
    if declarations.len() != child_interface.len()
        || declarations
            .keys()
            .any(|name| !child_interface.contains_key(name))
    {
        return Err(vec![hierarchy_error(format!(
            "cached symbolic Parameter interface for component `{}` does not match its declarations",
            component.name()
        ))]);
    }
    let overrides = resolve_instance_overrides(
        (declaration_file, binding_file),
        component,
        instance,
        &declarations,
        |name| parent_parameters.get(name).cloned(),
        &mut resolve_clock,
        instance
            .family()
            .map_or(ExpressionContext::Binding, |family| {
                ExpressionContext::IndexedBinding(family.member())
            }),
    )?;
    let diagnostics = declarations
        .into_iter()
        .filter_map(|(name, declaration)| {
            (declaration.visibility() == VisibilitySyntax::Public
                && declaration.default().is_none()
                && !overrides.contains_key(&name))
            .then(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    declaration_file,
                    declaration.range(),
                    format!("required Parameter `{name}` has no instance binding"),
                )
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) struct ParameterResolver<'a> {
    inner: SymbolicParameterResolver<'a>,
}

impl<'a> ParameterResolver<'a> {
    pub(super) fn new(
        declaration_file: &'a str,
        binding_file: &str,
        component: &'a ComponentDecl,
        instance: &InstanceDecl,
        mut resolve_parent: impl FnMut(&str) -> Option<ResolvedParameter>,
        mut resolve_clock: impl FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        SymbolicParameterResolver::instance(
            declaration_file,
            binding_file,
            component,
            instance,
            |name| resolve_parent(name).map(SymbolicParameterValue::from),
            &mut resolve_clock,
        )
        .map(|inner| Self { inner })
    }

    pub(super) fn resolve_all(
        self,
        mut resolve_clock: impl FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
    ) -> Result<BTreeMap<String, ResolvedParameter>, Vec<Diagnostic>> {
        self.inner
            .resolve_all(&mut resolve_clock)
            .and_then(concrete_parameters)
    }
}

fn concrete_parameters(
    parameters: SymbolicParameterMap,
) -> Result<BTreeMap<String, ResolvedParameter>, Vec<Diagnostic>> {
    parameters
        .into_iter()
        .map(|(name, parameter)| {
            let value = parameter.value.ok_or_else(|| {
                vec![hierarchy_error(format!(
                    "concrete instance Parameter `{name}` remained symbolic"
                ))]
            })?;
            let expression = parameter.expression.ok_or_else(|| {
                vec![hierarchy_error(format!(
                    "concrete instance Parameter `{name}` has no resolved expression"
                ))]
            })?;
            let lineage = parameter.lineage.ok_or_else(|| {
                vec![hierarchy_error(format!(
                    "concrete instance Parameter `{name}` has no exact binding lineage"
                ))]
            })?;
            Ok((
                name,
                ResolvedParameter {
                    value,
                    expression,
                    lineage,
                },
            ))
        })
        .collect()
}

fn combine_parameters(
    file: &str,
    range: TextRange,
    operator: BinaryOp,
    left: EvaluatedParameter,
    right: EvaluatedParameter,
) -> Result<EvaluatedParameter, Diagnostic> {
    let lineage = combine_lineages(left.lineage.clone(), right.lineage.clone());
    let exponent = if operator == BinaryOp::Pow {
        require_dimensionless_exponent(file, range, &right.value_type)?;
        if matches!(
            right.lineage,
            Some(ParameterLineage::Parameter(_) | ParameterLineage::Derived)
        ) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "compile-time power exponent cannot depend on a live Parameter",
            ));
        }
        right
            .value
            .as_ref()
            .map(|value| {
                value
                    .real_scalar_value()
                    .and_then(|quantity| exact_i32(quantity.value()))
                    .ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            range,
                            "compile-time power exponent must be an exact i32 integer",
                        )
                    })
            })
            .transpose()?
    } else {
        None
    };
    let value_type = combine_types(
        file,
        range,
        operator,
        left.value_type,
        right.value_type,
        exponent,
    )?;
    let expression = match (left.expression, right.expression) {
        (Some(left), Some(right)) => Some(LoweringExpression::binary(operator, left, right, range)),
        _ => None,
    };
    let value = match (left.value, right.value) {
        (Some(left), Some(right)) => Some(
            crate::typed_values::binary(
                operator,
                &left,
                &right,
                value_type.value_type().clone(),
                exponent,
            )
            .map_err(|message| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message))?,
        ),
        _ => None,
    };
    Ok(EvaluatedParameter {
        value,
        value_type,
        bare_literal: false,
        expression,
        lineage,
    })
}

fn transform_lineage(lineage: Option<ParameterLineage>) -> Option<ParameterLineage> {
    match lineage {
        Some(ParameterLineage::Constant) => Some(ParameterLineage::Constant),
        Some(ParameterLineage::Parameter(_) | ParameterLineage::Derived) => {
            Some(ParameterLineage::Derived)
        }
        None => None,
    }
}

fn combine_lineages(
    left: Option<ParameterLineage>,
    right: Option<ParameterLineage>,
) -> Option<ParameterLineage> {
    match (left, right) {
        (Some(ParameterLineage::Constant), Some(ParameterLineage::Constant)) => {
            Some(ParameterLineage::Constant)
        }
        (Some(ParameterLineage::Parameter(_) | ParameterLineage::Derived), _)
        | (_, Some(ParameterLineage::Parameter(_) | ParameterLineage::Derived)) => {
            Some(ParameterLineage::Derived)
        }
        _ => None,
    }
}

fn combine_types(
    file: &str,
    range: TextRange,
    operator: BinaryOp,
    left: EvaluatedType,
    right: EvaluatedType,
    exponent: Option<i32>,
) -> Result<EvaluatedType, Diagnostic> {
    use eqiora_schema::kernel::typing::{self, ExpressionType};
    let left_dimension = left.dimension();
    let right_dimension = right.dimension();
    let dimension_known = match operator {
        BinaryOp::Add | BinaryOp::Sub => left_dimension.or(right_dimension).is_some(),
        BinaryOp::Mul | BinaryOp::Div => left_dimension.is_some() && right_dimension.is_some(),
        _ if crate::lower::comparison_operator(operator).is_some()
            || matches!(operator, BinaryOp::And | BinaryOp::Or) =>
        {
            unreachable!("predicates use checked scalar typing")
        }
        BinaryOp::Pow => {
            exponent == Some(0)
                || (exponent.is_some() && left_dimension.is_some())
                || left_dimension == Some(DimExponents::DIMENSIONLESS)
        }
        _ => unreachable!("predicate operator"),
    };
    let projected = |value: &EvaluatedType, dimension| {
        ExpressionType::<()>::new(value.value_type().clone().with_dimension(dimension), None)
    };
    let fallback = DimExponents::DIMENSIONLESS;
    let result = match operator {
        BinaryOp::Add | BinaryOp::Sub => {
            let left = projected(
                &left,
                left_dimension.or(right_dimension).unwrap_or(fallback),
            );
            let right = projected(
                &right,
                right_dimension.or(left_dimension).unwrap_or(fallback),
            );
            if operator == BinaryOp::Add {
                left.sum(right)
            } else {
                typing::additive(&left, &right)
            }
        }
        BinaryOp::Mul | BinaryOp::Div => {
            let left = projected(
                &left,
                if dimension_known {
                    left_dimension.unwrap()
                } else {
                    fallback
                },
            );
            let right = projected(
                &right,
                if dimension_known {
                    right_dimension.unwrap()
                } else {
                    fallback
                },
            );
            if operator == BinaryOp::Mul {
                typing::multiply(&left, &right)
            } else {
                typing::divide(&left, &right)
            }
        }
        _ if crate::lower::comparison_operator(operator).is_some()
            || matches!(operator, BinaryOp::And | BinaryOp::Or) =>
        {
            unreachable!("predicates use checked scalar typing")
        }
        BinaryOp::Pow => typing::power(
            &projected(&left, left_dimension.unwrap_or(fallback)),
            exponent.unwrap_or(1),
        ),
        _ => unreachable!("predicate operator"),
    };
    result
        .map(|value| {
            if dimension_known {
                EvaluatedType::Known(value.value_type)
            } else {
                EvaluatedType::Deferred(value.value_type.with_dimension(fallback))
            }
        })
        .map_err(|error| {
            if matches!(error, typing::TypeViolation::DimensionOverflow { .. }) {
                constant_dimension_overflow(file, range)
            } else {
                source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
            }
        })
}

fn require_dimensionless_exponent(
    file: &str,
    range: TextRange,
    dimension: &EvaluatedType,
) -> Result<(), Diagnostic> {
    match dimension {
        EvaluatedType::Known(value_type) | EvaluatedType::Deferred(value_type)
            if value_type.dimension() != DimExponents::DIMENSIONLESS
                || value_type.scalar_domain() != ScalarDomain::Real
                || !value_type.shape().is_scalar() =>
        {
            Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "compile-time power exponent must be a real dimensionless scalar",
            ))
        }
        EvaluatedType::Known(_) | EvaluatedType::Deferred(_) => Ok(()),
    }
}

fn exact_i32(value: f64) -> Option<i32> {
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn finite_constant(file: &str, range: TextRange, value: f64) -> Result<f64, Diagnostic> {
    value
        .is_finite()
        .then_some(normalize_zero(value))
        .ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "compile-time Parameter evaluation produced a non-finite value",
            )
        })
}

pub(super) fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn constant_dimension_overflow(file: &str, range: TextRange) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        "compile-time dimension arithmetic exceeds rational exponent bounds",
    )
}

#[cfg(test)]
mod tests;

/// Evaluate a closed declaration through the same typed static expression owner.
pub(crate) fn closed_value(
    file: &str,
    expression: &Expr,
    target: ValueType,
) -> Result<ValueLiteral, Diagnostic> {
    let evaluated = evaluate_initializer(
        file,
        expression,
        ExpressionContext::Let,
        &mut |name, range| {
            Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("closed value cannot depend on `{name}`"),
            ))
        },
        target.clone(),
        "declared value",
        &mut |_| None,
    )?;
    let value = coerce_parameter_with_label(
        file,
        expression.range(),
        evaluated,
        target,
        "declared value",
        true,
    )?;
    value.value.ok_or_else(|| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            "closed value remained symbolic",
        )
    })
}

pub(in crate::hierarchy) fn static_index(
    file: &str,
    expression: &Expr,
    values: &SymbolicParameterMap,
) -> Result<u32, Diagnostic> {
    let evaluated = expression_eval::evaluate_with_domain(
        file,
        expression,
        ExpressionContext::Let,
        &mut |name, range| {
            values.get(name).cloned().ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "index depends on an unknown or runtime value",
                )
            })
        },
        &mut |_| None,
        Some(ScalarDomain::Integer),
    )?;
    value_expressions::checked_index(file, expression.range(), &evaluated)
}

pub(crate) use expression_eval::exact_signed_literal;

pub(in crate::hierarchy) fn structural_extent(
    file: &str,
    expression: &Expr,
    values: &SymbolicParameterMap,
) -> Result<Option<(u32, Vec<String>)>, Diagnostic> {
    let value = structural_index(file, expression, values)?;
    if value.as_ref().is_some_and(|(extent, _)| *extent == 0) {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            "index set extent requires a positive exact integer",
        ));
    }
    Ok(value)
}

pub(in crate::hierarchy) fn structural_index(
    file: &str,
    expression: &Expr,
    values: &SymbolicParameterMap,
) -> Result<Option<(u32, Vec<String>)>, Diagnostic> {
    let evaluated = expression_eval::evaluate_with_domain(
        file,
        expression,
        ExpressionContext::Let,
        &mut |name, range| {
            values.get(name).cloned().ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "index set extent depends on an unknown or runtime value",
                )
            })
        },
        &mut |_| None,
        Some(ScalarDomain::Integer),
    )?;
    let Some(value) = evaluated.value else {
        return Ok(None);
    };
    let extent = value
        .integer_scalar_value()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                "structural index requires a nonnegative exact integer within u32 bounds",
            )
        })?;
    let dependencies = evaluated
        .expression
        .map(|expression| expression.referenced_names().into_iter().collect())
        .unwrap_or_default();
    Ok(Some((extent, dependencies)))
}

pub(in crate::hierarchy) fn resolve_instance_parameters_symbolically(
    declaration_file: &str,
    binding_file: &str,
    component: &ComponentDecl,
    instance: &InstanceDecl,
    parent: &SymbolicParameterMap,
    resolve_clock: &mut dyn FnMut(&str) -> Option<Option<eqiora_schema::kernel::RationalTime>>,
) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    SymbolicParameterResolver::instance(
        declaration_file,
        binding_file,
        component,
        instance,
        |name| parent.get(name).cloned(),
        resolve_clock,
    )?
    .resolve_all(resolve_clock)
}
