use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{ComponentDecl, ComponentItem, Item, LetDecl, ModelDecl};

use crate::diagnostics::{source_error, stable_sort};
use crate::value_types::lower_value_type;

use super::expression_eval::{
    ExpressionContext, coerce_parameter_with_label, evaluate_parameter_expression,
    infer_parameter_with_label,
};
use super::{
    ExpressionDefinition, ParameterLineage, SymbolicParameterMap, expression_cycles,
    expression_evaluation_order,
};

pub(in crate::hierarchy) fn resolve_model_lets(
    file: &str,
    model: &ModelDecl,
    values: &mut SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    resolve_lets(
        file,
        model.items().iter().filter_map(|item| match item {
            Item::Let(declaration) => Some(declaration),
            _ => None,
        }),
        values,
    )
}

pub(in crate::hierarchy) fn resolve_component_lets(
    file: &str,
    component: &ComponentDecl,
    values: &mut SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    resolve_lets(
        file,
        component.items().iter().filter_map(|item| match item {
            ComponentItem::Let(declaration) => Some(declaration),
            _ => None,
        }),
        values,
    )
}

fn resolve_lets<'a>(
    file: &str,
    declarations: impl Iterator<Item = &'a LetDecl>,
    values: &mut SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = declarations
        .map(|declaration| (declaration.name().to_owned(), declaration))
        .collect::<BTreeMap<_, _>>();
    let order = alias_order(file, declarations.values().copied())?;
    let mut diagnostics = Vec::new();
    for declaration in order {
        let name = declaration.name().to_owned();
        // Runtime expressions are validated after Field/support interfaces are bound.
        if !is_static_expression(declaration.value(), values) {
            continue;
        }
        let target = match declaration
            .value_type()
            .map(|ty| lower_value_type::<()>(file, ty, None))
            .transpose()
        {
            Ok(target) => target,
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        };
        let evaluated = evaluate_parameter_expression(
            file,
            declaration.value(),
            ExpressionContext::Let,
            &mut |dependency, range| {
                values.get(dependency).cloned().ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        ExpressionContext::Let.unknown_name_message(dependency),
                    )
                })
            },
        );
        let range = declaration.range();
        match evaluated.and_then(|value| match &target {
            Some(target) => {
                coerce_parameter_with_label(file, range, value, target.clone(), "let alias", true)
            }
            None => infer_parameter_with_label(file, range, value, "let alias"),
        }) {
            Ok(mut value) => {
                value.lineage = Some(ParameterLineage::Derived);
                values.insert(name, value);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    stable_sort(&mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// One bounded dependency graph for both static and runtime aliases.
pub(in crate::hierarchy) fn alias_order<'a>(
    file: &str,
    declarations: impl Iterator<Item = &'a LetDecl>,
) -> Result<Vec<&'a LetDecl>, Vec<Diagnostic>> {
    let declarations = declarations
        .map(|d| (d.name().to_owned(), d))
        .collect::<BTreeMap<_, _>>();
    let definitions = declarations
        .iter()
        .map(|(name, d)| {
            let mut dependencies = BTreeMap::new();
            let mut pending = vec![d.value()];
            while let Some(e) = pending.pop() {
                match e.kind() {
                    eqiora_lang::ExprKind::Name(n) => {
                        dependencies.entry(n.clone()).or_insert(e.range());
                    }
                    eqiora_lang::ExprKind::Unary { value, .. } => pending.push(value),
                    eqiora_lang::ExprKind::Binary { left, right, .. } => {
                        pending.push(right);
                        pending.push(left);
                    }
                    eqiora_lang::ExprKind::Call { arguments, .. } => {
                        pending.extend(arguments.iter().rev())
                    }
                    _ => {}
                }
            }
            (
                name.clone(),
                ExpressionDefinition {
                    expression: d.value(),
                    target: None,
                    dependencies,
                    valid: true,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let cycles = expression_cycles(&definitions);
    if !cycles.is_empty() {
        return Err(cycles
            .into_iter()
            .map(|cycle| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    cycle.range,
                    format!("let alias dependency cycle: {}", cycle.path.join(" -> ")),
                )
            })
            .collect());
    }
    Ok(expression_evaluation_order(&definitions, &BTreeSet::new())
        .into_iter()
        .map(|name| declarations[&name])
        .collect())
}

fn is_static_expression(expression: &eqiora_lang::Expr, values: &SymbolicParameterMap) -> bool {
    let mut pending = vec![expression];
    while let Some(e) = pending.pop() {
        match e.kind() {
            eqiora_lang::ExprKind::Number(_) | eqiora_lang::ExprKind::Quantity { .. } => {}
            eqiora_lang::ExprKind::Name(n) if values.contains_key(n) => {}
            eqiora_lang::ExprKind::Path(p) if crate::math::constant(p).is_some() => {}
            eqiora_lang::ExprKind::Unary { value, .. } => pending.push(value),
            eqiora_lang::ExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            eqiora_lang::ExprKind::Call { callee, arguments }
                if !matches!(
                    callee.as_str(),
                    "derivative" | "pre" | "next" | "coordinate"
                ) =>
            {
                pending.extend(arguments)
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests;
