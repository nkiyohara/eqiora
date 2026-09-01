use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Item, ModelDecl};

use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;

use super::expression_eval::{
    ExpressionContext, coerce_parameter_with_label, evaluate_parameter_expression,
    infer_parameter_with_label,
};
use super::{ParameterLineage, SymbolicParameterMap};

pub(in crate::hierarchy) fn resolve_model_lets(
    file: &str,
    model: &ModelDecl,
    values: &mut SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    let let_names = model
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Let(declaration) => Some(declaration.name()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    for item in model.items() {
        let Item::Let(declaration) = item else {
            continue;
        };
        let target = match declaration
            .dimension()
            .map(|dimension| lower_dimension(file, dimension))
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
            &mut |name, range| {
                values.get(name).cloned().ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        range,
                        if let_names.contains(name) {
                            format!(
                                "let alias `{name}` is not available here; aliases may refer only to earlier let declarations"
                            )
                        } else {
                            format!("unknown Parameter or earlier let alias `{name}`")
                        },
                    )
                })
            },
        );
        match evaluated.and_then(|value| match target {
            Some(target) => {
                coerce_parameter_with_label(file, declaration.range(), value, target, "let alias")
            }
            None => infer_parameter_with_label(file, declaration.range(), value, "let alias"),
        }) {
            Ok(mut value) => {
                value.lineage = Some(ParameterLineage::Derived);
                values.insert(declaration.name().to_owned(), value);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
