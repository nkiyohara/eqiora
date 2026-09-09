//! Record syntax construction delegates all mathematical admission to the compiler.
use super::{
    declaration::PyAstType,
    expression::{path, syntax_error},
};
use eqiora::language::{
    Document, SourceAstFactory as Ast, TextRange, ValueTypeSyntaxKind, VisibilitySyntax,
};
use pyo3::prelude::*;

pub(super) fn record_type(name: &str) -> PyResult<PyAstType> {
    Ast::value_type(
        ValueTypeSyntaxKind::Named(path(name)?),
        TextRange::new(0, 1),
    )
    .map(|value| PyAstType { value })
    .map_err(syntax_error)
}

pub(super) fn with_record(
    document: Document,
    name: String,
    members: Vec<(String, PyRef<'_, PyAstType>, u32)>,
    ordinal: u32,
) -> PyResult<Document> {
    let members = members
        .into_iter()
        .map(|(name, kind, ordinal)| {
            Ast::record_member(
                name,
                kind.value.clone(),
                TextRange::new(ordinal, ordinal.saturating_add(1)),
            )
            .map_err(syntax_error)
        })
        .collect::<PyResult<_>>()?;
    let declaration = Ast::record(
        VisibilitySyntax::Public,
        name,
        members,
        TextRange::new(ordinal, ordinal.saturating_add(1)),
    )
    .map_err(syntax_error)?;
    Ok(Ast::with_record(document, declaration))
}
