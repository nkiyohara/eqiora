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

mod expression_eval;
mod model_lets;
use expression_eval::{
    ExpressionContext, coerce_parameter, coerce_parameter_with_label, evaluate_parameter_expression,
};
pub(super) use model_lets::resolve_model_lets;

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
    pub(super) value: Option<f64>,
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
    value: Option<f64>,
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
            value: Some(parameter.value.literal()),
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
    ) -> Result<Self, Vec<Diagnostic>> {
        let declarations = parameter_declarations(component);
        let overrides = resolve_instance_overrides(
            declaration_file,
            binding_file,
            component,
            instance,
            &declarations,
            resolve_parent,
        )?;
        Ok(Self {
            declaration_file,
            declarations,
            overrides,
            resolved: BTreeMap::new(),
            required_policy: RequiredParameterPolicy::RejectUnbound,
        })
    }

    fn resolve_all(mut self) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
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

            let (dependencies, mut errors) =
                collect_default_dependencies(self.declaration_file, default, &self.declarations);
            let valid = target.is_some() && errors.is_empty();
            diagnostics.append(&mut errors);
            defaults.insert(
                name.to_owned(),
                DefaultParameter {
                    expression: default,
                    target,
                    dependencies,
                    valid,
                },
            );
        }

        let cycles = parameter_cycles(&defaults);
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

        for name in parameter_evaluation_order(&defaults, &cyclic) {
            let parameter = &defaults[&name];
            if !parameter.valid
                || parameter
                    .dependencies
                    .keys()
                    .any(|dependency| !self.resolved.contains_key(dependency))
            {
                continue;
            }
            let evaluated = evaluate_parameter_expression(
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

struct DefaultParameter<'a> {
    expression: &'a Expr,
    target: Option<ValueType>,
    dependencies: BTreeMap<String, TextRange>,
    valid: bool,
}

struct ParameterCycle {
    members: Vec<String>,
    path: Vec<String>,
    range: TextRange,
}

fn collect_default_dependencies(
    file: &str,
    expression: &Expr,
    declarations: &BTreeMap<String, &ComponentParameterDecl>,
) -> (BTreeMap<String, TextRange>, Vec<Diagnostic>) {
    let mut dependencies = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression.kind() {
            ExprKind::Number(_) | ExprKind::Quantity { .. } => {}
            ExprKind::Name(name) => {
                if declarations.contains_key(name) {
                    dependencies
                        .entry(name.to_owned())
                        .and_modify(|range| {
                            if range_key(expression.range()) < range_key(*range) {
                                *range = expression.range();
                            }
                        })
                        .or_insert(expression.range());
                } else {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        expression.range(),
                        format!("unknown component Parameter `{name}`"),
                    ));
                }
            }
            ExprKind::Path(path) if crate::math::constant(path).is_some() => {}
            ExprKind::Path(path) => diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                path.range(),
                ExpressionContext::Default.qualified_name_message(path),
            )),
            ExprKind::Call { callee, .. } => diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                ExpressionContext::Default.call_message(callee.as_str()),
            )),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            _ => diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                expression.range(),
                ExpressionContext::Default.unsupported_message(),
            )),
        }
    }
    stable_sort(&mut diagnostics);
    (dependencies, diagnostics)
}

fn parameter_cycles(defaults: &BTreeMap<String, DefaultParameter<'_>>) -> Vec<ParameterCycle> {
    let adjacency = defaults
        .iter()
        .map(|(name, parameter)| {
            let dependencies = parameter
                .dependencies
                .keys()
                .filter(|dependency| defaults.contains_key(*dependency))
                .cloned()
                .collect::<Vec<_>>();
            (name.clone(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();
    let reverse = reverse_adjacency(&adjacency);
    let mut visited = BTreeSet::new();
    let mut finished = Vec::with_capacity(adjacency.len());
    for root in adjacency.keys() {
        if !visited.insert(root.clone()) {
            continue;
        }
        let mut stack = vec![(root.clone(), 0_usize)];
        while !stack.is_empty() {
            let child = {
                let (node, next) = stack.last_mut().expect("nonempty DFS stack");
                let neighbors = &adjacency[node];
                if *next < neighbors.len() {
                    let child = neighbors[*next].clone();
                    *next += 1;
                    Some(child)
                } else {
                    None
                }
            };
            if let Some(child) = child {
                if visited.insert(child.clone()) {
                    stack.push((child, 0));
                }
            } else {
                let (node, _) = stack.pop().expect("nonempty DFS stack");
                finished.push(node);
            }
        }
    }

    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for root in finished.into_iter().rev() {
        if !assigned.insert(root.clone()) {
            continue;
        }
        let mut component = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            component.push(node.clone());
            for parent in reverse[&node].iter().rev() {
                if assigned.insert(parent.clone()) {
                    pending.push(parent.clone());
                }
            }
        }
        component.sort();
        let is_cycle = component.len() > 1
            || adjacency[&component[0]]
                .iter()
                .any(|dependency| dependency == &component[0]);
        if is_cycle {
            components.push(canonical_parameter_cycle(component, &adjacency, defaults));
        }
    }
    components.sort_by(|left, right| left.members.cmp(&right.members));
    components
}

fn reverse_adjacency(adjacency: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let mut reverse = adjacency
        .keys()
        .map(|name| (name.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (name, dependencies) in adjacency {
        for dependency in dependencies {
            reverse
                .get_mut(dependency)
                .expect("dependency is an active default")
                .push(name.clone());
        }
    }
    for parents in reverse.values_mut() {
        parents.sort();
    }
    reverse
}

fn canonical_parameter_cycle(
    members: Vec<String>,
    adjacency: &BTreeMap<String, Vec<String>>,
    defaults: &BTreeMap<String, DefaultParameter<'_>>,
) -> ParameterCycle {
    let member_set = members.iter().cloned().collect::<BTreeSet<_>>();
    let start = members.first().expect("cyclic component is nonempty");
    let next = adjacency[start]
        .iter()
        .find(|dependency| member_set.contains(*dependency))
        .expect("a strongly connected component has an internal edge");
    let range = defaults[start].dependencies[next];
    let path = if next == start {
        vec![start.clone(), start.clone()]
    } else {
        let mut parents = BTreeMap::<String, String>::new();
        let mut visited = BTreeSet::from([next.clone()]);
        let mut pending = VecDeque::from([next.clone()]);
        while let Some(node) = pending.pop_front() {
            if &node == start {
                break;
            }
            for child in &adjacency[&node] {
                if member_set.contains(child) && visited.insert(child.clone()) {
                    parents.insert(child.clone(), node.clone());
                    pending.push_back(child.clone());
                }
            }
        }
        let mut tail = vec![start.clone()];
        let mut cursor = start;
        while cursor != next {
            cursor = &parents[cursor];
            tail.push(cursor.clone());
        }
        tail.reverse();
        let mut path = vec![start.clone()];
        path.extend(tail);
        path
    };
    ParameterCycle {
        members,
        path,
        range,
    }
}

fn parameter_evaluation_order(
    defaults: &BTreeMap<String, DefaultParameter<'_>>,
    cyclic: &BTreeSet<String>,
) -> Vec<String> {
    let mut indegree = defaults
        .keys()
        .filter(|name| !cyclic.contains(*name))
        .map(|name| (name.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = indegree
        .keys()
        .map(|name| (name.clone(), Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (name, parameter) in defaults {
        if cyclic.contains(name) {
            continue;
        }
        for dependency in parameter.dependencies.keys() {
            if defaults.contains_key(dependency) && !cyclic.contains(dependency) {
                *indegree.get_mut(name).expect("acyclic default is indexed") += 1;
                dependents
                    .get_mut(dependency)
                    .expect("acyclic dependency is indexed")
                    .push(name.clone());
            }
        }
    }
    for values in dependents.values_mut() {
        values.sort();
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(name, &count)| (count == 0).then_some(name.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(indegree.len());
    while let Some(name) = ready.pop_first() {
        order.push(name.clone());
        for dependent in &dependents[&name] {
            let count = indegree
                .get_mut(dependent)
                .expect("acyclic dependent is indexed");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    order
}

fn range_key(range: TextRange) -> (u32, u32) {
    (range.start(), range.end())
}

fn parameter_declarations(component: &ComponentDecl) -> BTreeMap<String, &ComponentParameterDecl> {
    component
        .items()
        .iter()
        .filter_map(|item| match item {
            ComponentItem::Parameter(value) => Some((value.name().to_owned(), value)),
            _ => None,
        })
        .collect()
}

fn resolve_instance_overrides(
    declaration_file: &str,
    binding_file: &str,
    component: &ComponentDecl,
    instance: &InstanceDecl,
    declarations: &BTreeMap<String, &ComponentParameterDecl>,
    mut resolve_parent: impl FnMut(&str) -> Option<SymbolicParameterValue>,
) -> Result<BTreeMap<String, SymbolicParameterValue>, Vec<Diagnostic>> {
    let mut overrides = BTreeMap::new();
    let mut bound = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for binding in instance.bindings() {
        if !bound.insert(binding.parameter()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "duplicate binding for Parameter `{}` in instance `{}`",
                    binding.parameter(),
                    instance.name()
                ),
            ));
            continue;
        }
        let Some(declaration) = declarations.get(binding.parameter()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "unknown public Parameter `{}` on component `{}`",
                    binding.parameter(),
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
                    binding.parameter(),
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
        let value = evaluate_parameter_expression(
            binding_file,
            binding.value(),
            ExpressionContext::Binding,
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
        )
        .and_then(|value| coerce_parameter(binding_file, binding.range(), value, target));
        match value {
            Ok(value) => {
                overrides.insert(binding.parameter().to_owned(), value);
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
) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    SymbolicParameterResolver::component_interface(declaration_file, component).resolve_all()
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
        declaration_file,
        binding_file,
        component,
        instance,
        &declarations,
        |name| parent_parameters.get(name).cloned(),
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
    ) -> Result<Self, Vec<Diagnostic>> {
        SymbolicParameterResolver::instance(
            declaration_file,
            binding_file,
            component,
            instance,
            |name| resolve_parent(name).map(SymbolicParameterValue::from),
        )
        .map(|inner| Self { inner })
    }

    pub(super) fn resolve_all(
        self,
    ) -> Result<BTreeMap<String, ResolvedParameter>, Vec<Diagnostic>> {
        self.inner.resolve_all().and_then(|parameters| {
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
                            value: ValueLiteral::new(parameter.value_type, value)
                                .map_err(|error| vec![hierarchy_error(error.to_string())])?,
                            expression,
                            lineage,
                        },
                    ))
                })
                .collect()
        })
    }
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
            .map(|value| {
                exact_i32(value).ok_or_else(|| {
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
        (Some(left), Some(right)) => {
            let value = match operator {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                BinaryOp::Div => left / right,
                BinaryOp::Pow => left.powi(exponent.expect("known exponent was validated")),
            };
            Some(finite_constant(file, range, value)?)
        }
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
        BinaryOp::Pow => {
            exponent == Some(0)
                || (exponent.is_some() && left_dimension.is_some())
                || left_dimension == Some(DimExponents::DIMENSIONLESS)
        }
    };
    let projected = |value: &EvaluatedType, dimension| {
        ExpressionType::<()>::new(value.value_type().clone().with_dimension(dimension), None)
    };
    let fallback = DimExponents::DIMENSIONLESS;
    let result = match operator {
        BinaryOp::Add | BinaryOp::Sub => typing::additive(
            &projected(
                &left,
                left_dimension.or(right_dimension).unwrap_or(fallback),
            ),
            &projected(
                &right,
                right_dimension.or(left_dimension).unwrap_or(fallback),
            ),
        ),
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
        BinaryOp::Pow => typing::power(
            &projected(&left, left_dimension.unwrap_or(fallback)),
            exponent.unwrap_or(1),
        ),
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
