//! Lexical operator metadata and ordered occurrence application.
use super::*;

impl Scope {
    pub(in crate::hierarchy) fn set_pure_operators(
        &mut self,
        definitions: BTreeMap<String, (PureOperatorDefinition, Vec<String>)>,
    ) {
        self.pure_operators = definitions;
    }

    fn pure_operator(&self, path: &NamePath) -> Option<&(PureOperatorDefinition, Vec<String>)> {
        self.pure_operators.get(path.as_str())
    }
}

pub(super) fn rewrite(
    file: &str,
    expression: &Expr,
    callee: &NamePath,
    arguments: &eqiora_lang::CallArguments,
    scope: &Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<LoweringExpression, Diagnostic> {
    let (definition, names) = scope.pure_operator(callee).cloned().ok_or_else(|| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            callee.range(),
            format!("unresolved pure operator `{callee}`"),
        )
    })?;
    let arguments = crate::pure_operator::ordered_arguments(
        file,
        expression.range(),
        names.iter().map(String::as_str),
        arguments,
    )?
    .into_iter()
    .map(|argument| rewrite_expression_with_boundary_member(file, argument, scope, active))
    .collect::<Result<Vec<_>, _>>()?;
    Ok(LoweringExpression::pure_operator(
        definition,
        arguments,
        expression.range(),
    ))
}
