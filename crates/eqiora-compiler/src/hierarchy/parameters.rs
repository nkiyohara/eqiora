use std::collections::{BTreeMap, BTreeSet, VecDeque};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_lang::{
    BinaryOp, ComponentDecl, ComponentItem, ComponentParameterDecl, Expr, ExprKind, InstanceDecl,
    TextRange, UnaryOp, VisibilitySyntax,
};

use crate::diagnostics::stable_sort;
use crate::identity::FullElaborationIdentity;
use crate::lower::{LoweringExpression, checked_dimensions, lower_dimension, source_error};

use super::hierarchy_error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ConstantValue {
    pub(super) value: f64,
    pub(super) dimension: DimExponents,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedParameter {
    pub(super) value: ConstantValue,
    pub(super) expression: LoweringExpression,
    pub(super) lineage: ParameterLineage,
}

/// One component Parameter after definition-time symbolic resolution.
///
/// The dimension is always the declaration's concrete SI dimension. `value`
/// is absent exactly when the expression depends on at least one required
/// public Parameter whose value belongs to a future component occurrence.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SymbolicParameterValue {
    pub(super) value: Option<f64>,
    pub(super) dimension: DimExponents,
    pub(super) expression: Option<LoweringExpression>,
    pub(super) lineage: Option<ParameterLineage>,
}

pub(super) type SymbolicParameterMap = BTreeMap<String, SymbolicParameterValue>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvaluatedDimension {
    Known(DimExponents),
    /// A power with a symbolic exponent can defer its result dimension until
    /// an enclosing declaration or operator supplies the expected dimension.
    Deferred,
}

#[derive(Debug, Clone, PartialEq)]
struct EvaluatedParameter {
    value: Option<f64>,
    dimension: EvaluatedDimension,
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
        value: ConstantValue,
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
            value: Some(parameter.value.value),
            dimension: parameter.value.dimension,
            expression: Some(parameter.expression),
            lineage: Some(parameter.lineage),
        }
    }
}

impl From<SymbolicParameterValue> for EvaluatedParameter {
    fn from(value: SymbolicParameterValue) -> Self {
        Self {
            value: value.value,
            dimension: EvaluatedDimension::Known(value.dimension),
            bare_literal: false,
            expression: value.expression,
            lineage: value.lineage,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ExpressionContext {
    Binding,
    Default,
}

impl ExpressionContext {
    fn qualified_name_message(self, path: &impl std::fmt::Display) -> String {
        match self {
            Self::Binding => format!(
                "qualified name `{path}` is not allowed in a compile-time Parameter binding"
            ),
            Self::Default => {
                format!("qualified name `{path}` is not allowed in a Parameter default")
            }
        }
    }

    fn call_message(self, callee: &str) -> String {
        match self {
            Self::Binding => {
                format!("operator `{callee}(...)` is not allowed in a compile-time binding")
            }
            Self::Default => {
                format!("operator `{callee}(...)` is not allowed in a Parameter default")
            }
        }
    }

    const fn unsupported_message(self) -> &'static str {
        match self {
            Self::Binding => "binding expression syntax is newer than this compiler",
            Self::Default => "Parameter default syntax is newer than this compiler",
        }
    }
}

fn evaluate_parameter_expression(
    file: &str,
    expression: &Expr,
    context: ExpressionContext,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
) -> Result<EvaluatedParameter, Diagnostic> {
    let evaluated = match expression.kind() {
        ExprKind::Number(value) => EvaluatedParameter {
            value: Some(normalize_zero(*value)),
            dimension: EvaluatedDimension::Known(DimExponents::DIMENSIONLESS),
            bare_literal: true,
            expression: Some(LoweringExpression::quantity(
                DynQuantity::new(normalize_zero(*value), DimExponents::DIMENSIONLESS),
                expression.range(),
            )),
            lineage: Some(ParameterLineage::Constant),
        },
        ExprKind::Name(name) => resolve(name, expression.range())?.into(),
        ExprKind::Path(path) => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                path.range(),
                context.qualified_name_message(path),
            ));
        }
        ExprKind::Call { callee, .. } => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                context.call_message(callee.as_str()),
            ));
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            let operand = evaluate_parameter_expression(file, value, context, resolve)?;
            let negated = operand
                .value
                .map(|value| finite_constant(file, expression.range(), -value))
                .transpose()?;
            EvaluatedParameter {
                value: negated,
                dimension: operand.dimension,
                bare_literal: operand.bare_literal,
                expression: operand
                    .expression
                    .map(|value| LoweringExpression::neg(value, expression.range())),
                lineage: transform_lineage(operand.lineage),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evaluate_parameter_expression(file, left, context, resolve)?;
            let right = evaluate_parameter_expression(file, right, context, resolve)?;
            combine_parameters(file, expression.range(), *op, left, right)?
        }
        _ => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                expression.range(),
                context.unsupported_message(),
            ));
        }
    };
    Ok(evaluated)
}

fn coerce_parameter(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    target: DimExponents,
) -> Result<SymbolicParameterValue, Diagnostic> {
    let dimension = if evaluated.bare_literal {
        EvaluatedDimension::Known(target)
    } else {
        evaluated.dimension
    };
    if let EvaluatedDimension::Known(dimension) = dimension
        && dimension != target
    {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!(
                "Parameter binding has dimension [{}], expected [{}]",
                dimension, target
            ),
        ));
    }
    let expression = evaluated.expression.map(|expression| {
        if evaluated.bare_literal {
            expression.with_quantity_dimension(target)
        } else {
            expression
        }
    });
    // A deferred power dimension is an occurrence-time obligation. The
    // declaration target constrains its symbolic interface here; the same
    // evaluator reconstructs and proves it once all occurrence values exist.
    Ok(SymbolicParameterValue {
        value: evaluated.value,
        dimension: target,
        expression,
        lineage: evaluated.lineage,
    })
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

            let target = match lower_dimension(self.declaration_file, declaration.dimension()) {
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
                                dimension: target,
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
                coerce_parameter(
                    self.declaration_file,
                    parameter.expression.range(),
                    evaluated,
                    parameter
                        .target
                        .expect("valid default has a target dimension"),
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
    target: Option<DimExponents>,
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
            ExprKind::Number(_) => {}
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
        let target = match lower_dimension(declaration_file, declaration.dimension()) {
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
                            value: ConstantValue {
                                value,
                                dimension: parameter.dimension,
                            },
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
        require_dimensionless_exponent(file, range, right.dimension)?;
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
    let dimension = combine_dimensions(
        file,
        range,
        operator,
        left.dimension,
        right.dimension,
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
        dimension,
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

fn combine_dimensions(
    file: &str,
    range: TextRange,
    operator: BinaryOp,
    left: EvaluatedDimension,
    right: EvaluatedDimension,
    exponent: Option<i32>,
) -> Result<EvaluatedDimension, Diagnostic> {
    match operator {
        BinaryOp::Add | BinaryOp::Sub => match (left, right) {
            (EvaluatedDimension::Known(left), EvaluatedDimension::Known(right))
                if left == right =>
            {
                Ok(EvaluatedDimension::Known(left))
            }
            (EvaluatedDimension::Known(left), EvaluatedDimension::Known(right)) => {
                Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    format!(
                        "compile-time addition/subtraction combines dimensions [{}] and [{}]",
                        left, right
                    ),
                ))
            }
            (EvaluatedDimension::Known(dimension), EvaluatedDimension::Deferred)
            | (EvaluatedDimension::Deferred, EvaluatedDimension::Known(dimension)) => {
                Ok(EvaluatedDimension::Known(dimension))
            }
            (EvaluatedDimension::Deferred, EvaluatedDimension::Deferred) => {
                Ok(EvaluatedDimension::Deferred)
            }
        },
        BinaryOp::Mul | BinaryOp::Div => match (left, right) {
            (EvaluatedDimension::Known(left), EvaluatedDimension::Known(right)) => {
                let operation = if operator == BinaryOp::Mul {
                    i8::checked_add
                } else {
                    i8::checked_sub
                };
                checked_dimensions(left, right, operation)
                    .map(EvaluatedDimension::Known)
                    .ok_or_else(|| constant_dimension_overflow(file, range))
            }
            _ => Ok(EvaluatedDimension::Deferred),
        },
        BinaryOp::Pow => match (left, exponent) {
            (EvaluatedDimension::Known(dimension), Some(exponent)) => {
                scale_dimension(dimension, exponent)
                    .map(EvaluatedDimension::Known)
                    .ok_or_else(|| constant_dimension_overflow(file, range))
            }
            (EvaluatedDimension::Deferred, Some(0)) => {
                Ok(EvaluatedDimension::Known(DimExponents::DIMENSIONLESS))
            }
            (EvaluatedDimension::Known(dimension), None)
                if dimension == DimExponents::DIMENSIONLESS =>
            {
                Ok(EvaluatedDimension::Known(DimExponents::DIMENSIONLESS))
            }
            _ => Ok(EvaluatedDimension::Deferred),
        },
    }
}

fn require_dimensionless_exponent(
    file: &str,
    range: TextRange,
    dimension: EvaluatedDimension,
) -> Result<(), Diagnostic> {
    match dimension {
        EvaluatedDimension::Known(dimension) if dimension != DimExponents::DIMENSIONLESS => {
            Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "compile-time power exponent must be dimensionless",
            ))
        }
        EvaluatedDimension::Known(_) | EvaluatedDimension::Deferred => Ok(()),
    }
}

fn exact_i32(value: f64) -> Option<i32> {
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn scale_dimension(dimension: DimExponents, exponent: i32) -> Option<DimExponents> {
    fn scale(value: i8, exponent: i32) -> Option<i8> {
        i32::from(value)
            .checked_mul(exponent)
            .and_then(|value| i8::try_from(value).ok())
    }
    Some(DimExponents {
        mass: scale(dimension.mass, exponent)?,
        length: scale(dimension.length, exponent)?,
        time: scale(dimension.time, exponent)?,
        current: scale(dimension.current, exponent)?,
        temperature: scale(dimension.temperature, exponent)?,
        amount: scale(dimension.amount, exponent)?,
        luminous_intensity: scale(dimension.luminous_intensity, exponent)?,
    })
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
        "compile-time dimension exponent arithmetic overflows i8",
    )
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use eqiora_lang::{ComponentItem, Document, parse};

    use super::*;

    fn document(source: &str) -> Document {
        let source = format!("{source}\nmodel Root {{}}\n");
        parse("parameters.eqi", &source)
            .into_compilation_document()
            .expect("test source parses")
    }

    fn component<'a>(document: &'a Document, name: &str) -> &'a ComponentDecl {
        document
            .components()
            .iter()
            .find(|component| component.name() == name)
            .expect("component exists")
    }

    fn length(exponent: i8) -> DimExponents {
        DimExponents {
            length: exponent,
            ..DimExponents::DIMENSIONLESS
        }
    }

    #[test]
    fn required_public_parameters_are_typed_free_variables() {
        let document = document(
            r#"
component Symbolic {
  public parameter base: m;
  public parameter exponent: 1;
  public parameter area: m ^ 2 = base ^ exponent;
  parameter offset: m = 2;
}
"#,
        );
        let parameters = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&document, "Symbolic"),
        )
        .expect("open typed interface resolves");

        assert_eq!(parameters["base"].value, None);
        assert_eq!(parameters["base"].dimension, length(1));
        assert_eq!(parameters["exponent"].value, None);
        assert_eq!(
            parameters["exponent"].dimension,
            DimExponents::DIMENSIONLESS
        );
        assert_eq!(parameters["area"].value, None);
        assert_eq!(parameters["area"].dimension, length(2));
        assert_eq!(parameters["offset"].value, Some(2.0));
        assert_eq!(parameters["offset"].dimension, length(1));
    }

    #[test]
    fn required_private_parameter_has_no_symbolic_witness() {
        let document = document("component Invalid { parameter hidden: m; }");
        let diagnostics = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&document, "Invalid"),
        )
        .expect_err("private required Parameter is uninhabitable");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("required private Parameter `hidden` has no default")
        }));
    }

    #[test]
    fn nested_instance_validates_symbolic_parent_bindings_with_cached_child() {
        let document = document(
            r#"
component Child {
  public parameter base: m;
  public parameter exponent: 1;
  public parameter area: m ^ 2 = base ^ exponent;
}
component Parent {
  public parameter length: m;
  instance child: Child(base = length, exponent = 2);
}
"#,
        );
        let parent = component(&document, "Parent");
        let child = component(&document, "Child");
        let instance = parent
            .items()
            .iter()
            .find_map(|item| match item {
                ComponentItem::Instance(instance) => Some(instance),
                _ => None,
            })
            .expect("nested instance exists");
        let parent_parameters = resolve_component_parameters_symbolically("parameters.eqi", parent)
            .expect("parent interface resolves");
        let child_interface = resolve_component_parameters_symbolically("parameters.eqi", child)
            .expect("child interface resolves once");
        validate_instance_parameters_symbolically(
            "parameters.eqi",
            "parameters.eqi",
            child,
            instance,
            &parent_parameters,
            &child_interface,
        )
        .expect("cached interface validates the definition edge");
    }

    #[test]
    fn symbolic_instances_preserve_binding_diagnostics() {
        let document = document(
            r#"
component Child {
  public parameter required: m;
  parameter hidden: m = 1;
}
component Parent {
  public parameter length: m;
  instance missing: Child;
  instance unknown: Child(other = length);
  instance private: Child(hidden = length);
  instance duplicate: Child(required = length, required = length);
}
"#,
        );
        let parent = component(&document, "Parent");
        let child = component(&document, "Child");
        let parent_parameters = resolve_component_parameters_symbolically("parameters.eqi", parent)
            .expect("parent interface resolves");
        let child_interface = resolve_component_parameters_symbolically("parameters.eqi", child)
            .expect("child interface resolves once");
        let instances = parent
            .items()
            .iter()
            .filter_map(|item| match item {
                ComponentItem::Instance(instance) => Some((instance.name(), instance)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();

        let expected = [
            (
                "missing",
                "required Parameter `required` has no instance binding",
            ),
            (
                "unknown",
                "unknown public Parameter `other` on component `Child`",
            ),
            (
                "private",
                "private Parameter `hidden` cannot be bound on instance `private`",
            ),
            (
                "duplicate",
                "duplicate binding for Parameter `required` in instance `duplicate`",
            ),
        ];
        for (instance, message) in expected {
            let diagnostics = validate_instance_parameters_symbolically(
                "parameters.eqi",
                "parameters.eqi",
                child,
                instances[instance],
                &parent_parameters,
                &child_interface,
            )
            .expect_err("invalid binding fails closed");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message().contains(message)),
                "expected `{message}`, got {diagnostics:#?}"
            );
        }
    }

    #[test]
    fn ten_thousand_parameter_chains_and_cycles_are_iterative() {
        const COUNT: usize = 10_000;

        let mut chain = String::from("component Chain {\n");
        writeln!(chain, "  parameter p00000: 1 = 1;").expect("write to String");
        for index in 1..COUNT {
            writeln!(chain, "  parameter p{index:05}: 1 = p{:05};", index - 1)
                .expect("write to String");
        }
        chain.push_str("}\n");
        let chain_document = document(&chain);
        let parameters = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&chain_document, "Chain"),
        )
        .expect("deep acyclic graph resolves without recursive calls");
        assert_eq!(parameters.len(), COUNT);
        assert_eq!(parameters["p09999"].value, Some(1.0));

        let mut cycle = String::from("component Cycle {\n");
        for index in 0..COUNT {
            writeln!(
                cycle,
                "  parameter p{index:05}: 1 = p{:05};",
                (index + 1) % COUNT
            )
            .expect("write to String");
        }
        cycle.push_str("}\n");
        let cycle_document = document(&cycle);
        let diagnostics = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&cycle_document, "Cycle"),
        )
        .expect_err("one large SCC fails without recursive calls");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
        assert!(diagnostics[0].source_span().is_some());
        assert!(
            diagnostics[0]
                .message()
                .starts_with("component Parameter dependency cycle: p00000 -> p00001")
        );
        assert!(diagnostics[0].message().ends_with("p09999 -> p00000"));
    }

    #[test]
    fn parameter_self_loop_has_one_source_spanned_type_diagnostic() {
        let document = document("component Loop { parameter value: 1 = value; }");
        let diagnostics = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&document, "Loop"),
        )
        .expect_err("self dependency is a cycle");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
        let span = diagnostics[0]
            .source_span()
            .expect("cycle points to the dependency name");
        assert_eq!(span.file, "parameters.eqi");
        assert!(span.start < span.end);
        assert_eq!(
            diagnostics[0].message(),
            "component Parameter dependency cycle: value -> value"
        );
    }

    #[test]
    fn symbolic_default_outcome_is_declaration_order_independent() {
        let forward = document(
            r#"
component Ordered {
  parameter base: 1 = 2;
  parameter shifted: 1 = base + 3;
  parameter scaled: 1 = shifted * 4;
}
"#,
        );
        let reverse = document(
            r#"
component Ordered {
  parameter scaled: 1 = shifted * 4;
  parameter shifted: 1 = base + 3;
  parameter base: 1 = 2;
}
"#,
        );

        let forward = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&forward, "Ordered"),
        )
        .expect("forward declarations resolve");
        let reverse = resolve_component_parameters_symbolically(
            "parameters.eqi",
            component(&reverse, "Ordered"),
        )
        .expect("reverse declarations resolve");
        assert_eq!(forward, reverse);
        assert_eq!(forward["scaled"].value, Some(20.0));
    }
}
