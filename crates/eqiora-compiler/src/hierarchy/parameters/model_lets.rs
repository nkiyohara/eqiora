use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Item, ModelDecl};

use crate::diagnostics::{source_error, stable_sort};
use crate::value_types::lower_value_type;

use super::expression_eval::{
    ExpressionContext, coerce_parameter_with_label, evaluate_parameter_expression,
    infer_parameter_with_label,
};
use super::{
    ExpressionDefinition, ParameterLineage, SymbolicParameterMap, collect_expression_dependencies,
    expression_cycles, expression_evaluation_order,
};

pub(in crate::hierarchy) fn resolve_model_lets(
    file: &str,
    model: &ModelDecl,
    values: &mut SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    let declarations = model
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Let(declaration) => Some((declaration.name().to_owned(), declaration)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut definitions = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (name, declaration) in &declarations {
        let target = declaration
            .value_type()
            .map(|value_type| lower_value_type::<()>(file, value_type, None))
            .transpose();
        let (dependencies, mut errors) = collect_expression_dependencies(
            file,
            declaration.value(),
            |dependency| declarations.contains_key(dependency) || values.contains_key(dependency),
            ExpressionContext::Let,
        );
        let valid = target.is_ok() && errors.is_empty();
        diagnostics.append(&mut errors);
        let target = target.unwrap_or_else(|error| {
            diagnostics.push(error);
            None
        });
        definitions.insert(
            name.clone(),
            ExpressionDefinition {
                expression: declaration.value(),
                target,
                dependencies,
                valid,
            },
        );
    }

    let mut cyclic = BTreeSet::new();
    for cycle in expression_cycles(&definitions) {
        cyclic.extend(cycle.members);
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            cycle.range,
            format!("let alias dependency cycle: {}", cycle.path.join(" -> ")),
        ));
    }
    for name in expression_evaluation_order(&definitions, &cyclic) {
        let definition = &definitions[&name];
        if !definition.valid
            || definition
                .dependencies
                .keys()
                .any(|dependency| !values.contains_key(dependency))
        {
            continue;
        }
        let evaluated = evaluate_parameter_expression(
            file,
            definition.expression,
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
        let range = declarations[&name].range();
        match evaluated.and_then(|value| match &definition.target {
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

#[cfg(test)]
mod tests;
