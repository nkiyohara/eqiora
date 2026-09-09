//! Named physical endpoints use the existing compiler-owned AST factories.

use eqiora::language::{
    ComponentItem, ConnectionSyntax, ConnectorDecl, ConnectorSyntax, PortSyntax, SignatureItem,
    SourceAstFactory as Ast, TextRange, VisibilitySyntax,
};
use pyo3::prelude::*;

use super::declaration::{Declaration, PyAstDeclaration, PyAstType};
use super::expression::{PyAstExpression, path, syntax_error};

pub(super) type ConnectorInput<'py> = (
    String,
    String,
    PyRef<'py, PyAstType>,
    String,
    PyRef<'py, PyAstType>,
    u32,
);

pub(super) fn connectors(inputs: Vec<ConnectorInput<'_>>) -> PyResult<Vec<ConnectorDecl>> {
    inputs
        .into_iter()
        .map(
            |(name, across_name, across_type, through_name, through_type, ordinal)| {
                Ast::connector(
                    VisibilitySyntax::Public,
                    name,
                    ConnectorSyntax::ScalarPhysical {
                        across_name,
                        across_type: across_type.value.clone(),
                        through_name,
                        through_type: through_type.value.clone(),
                    },
                    TextRange::new(ordinal, ordinal.saturating_add(1)),
                )
                .map_err(syntax_error)
            },
        )
        .collect()
}

pub(super) fn port(name: String, connector: &str, ordinal: u32) -> PyResult<PyAstDeclaration> {
    let value = Ast::component_port(
        VisibilitySyntax::Public,
        name,
        PortSyntax::ScalarPhysicalConnector {
            connector: path(connector)?,
        },
        TextRange::new(ordinal, ordinal.saturating_add(1)),
    )
    .map_err(syntax_error)?;
    Ok(PyAstDeclaration {
        value: Declaration::Signature(SignatureItem::Port(value)),
    })
}

pub(super) fn connection(
    ports: Vec<PyRef<'_, PyAstExpression>>,
    ordinal: u32,
) -> PyResult<PyAstDeclaration> {
    if ports.len() > 256 {
        return Err(syntax_error("connection exceeds 256 endpoints"));
    }
    let value = Ast::connection(
        ConnectionSyntax::Conserving,
        None,
        ports.into_iter().map(|port| port.value.clone()).collect(),
        TextRange::new(ordinal, ordinal.saturating_add(1)),
    )
    .map_err(syntax_error)?;
    Ok(PyAstDeclaration {
        value: Declaration::Item(ComponentItem::Connection(value)),
    })
}
