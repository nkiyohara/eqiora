//! Exact static extents reuse the Parameter expression evaluator and source type owner.
use super::*;
use eqiora_lang::{SourceAstFactory, ValueTypeSyntax, ValueTypeSyntaxKind};

pub(in crate::hierarchy) fn specialize_type(
    file: &str,
    syntax: &ValueTypeSyntax,
    values: &SymbolicParameterMap,
) -> Result<ValueTypeSyntax, Diagnostic> {
    let kind = match syntax.kind() {
        ValueTypeSyntaxKind::Array { element, extent } => {
            let (count, _) = structural_extent(file, extent, values)?.ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    extent.range(),
                    "array extent requires an exact static Parameter value in this occurrence",
                )
            })?;
            let extent = SourceAstFactory::expression(
                ExprKind::Number(
                    eqiora_lang::DecimalLiteral::parse(&count.to_string()).expect("u32"),
                ),
                extent.range(),
            )
            .expect("admitted literal and range");
            ValueTypeSyntaxKind::Array {
                element: Box::new(specialize_type(file, element, values)?),
                extent,
            }
        }
        _ => return Ok(syntax.clone()),
    };
    SourceAstFactory::value_type(kind, syntax.range()).map_err(|error| {
        source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            syntax.range(),
            error.message(),
        )
    })
}

pub(in crate::hierarchy) fn extent_expressions(syntax: &ValueTypeSyntax) -> Vec<&Expr> {
    let mut result = Vec::new();
    let mut syntax = syntax;
    while let ValueTypeSyntaxKind::Array { element, extent } = syntax.kind() {
        result.push(extent);
        syntax = element;
    }
    result
}
