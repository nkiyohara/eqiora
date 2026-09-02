use crate::ast_property::{PropertyContractDecl, PropertyReleaseDecl};

use super::{
    ComponentDecl, ConnectorDecl, DimensionDecl, ModelDecl, NamePath, PureOperatorDecl, TextRange,
};

/// One explicit, side-effect-free semantic module import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportDecl {
    pub(crate) module: NamePath,
    pub(crate) alias: String,
    pub(crate) range: TextRange,
}

/// One parsed source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub(crate) imports: Vec<ImportDecl>,
    pub(crate) dimensions: Vec<DimensionDecl>,
    pub(crate) property_contracts: Vec<PropertyContractDecl>,
    pub(crate) property_releases: Vec<PropertyReleaseDecl>,
    pub(crate) connectors: Vec<ConnectorDecl>,
    pub(crate) components: Vec<ComponentDecl>,
    pub(crate) pure_operators: Vec<PureOperatorDecl>,
    pub(crate) models: Vec<ModelDecl>,
}

impl Document {
    /// Explicit semantic imports in authored order.
    #[must_use]
    pub fn imports(&self) -> impl ExactSizeIterator<Item = (&NamePath, &str, TextRange)> {
        self.imports
            .iter()
            .map(|import| (&import.module, import.alias.as_str(), import.range))
    }

    /// Ordered compilation-unit structural dimension aliases.
    #[must_use]
    pub fn dimension_syntax(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &super::Expr, super::TextRange)> {
        self.dimensions
            .iter()
            .map(|value| (value.name(), value.expression(), value.range()))
    }

    /// Compilation-unit connector declarations in source order.
    #[must_use]
    pub fn connectors(&self) -> &[ConnectorDecl] {
        &self.connectors
    }

    /// Compilation-unit component declarations in source order.
    #[must_use]
    pub fn components(&self) -> &[ComponentDecl] {
        &self.components
    }

    /// Compilation-unit pure operator declarations in source order.
    #[must_use]
    pub fn pure_operators(&self) -> &[PureOperatorDecl] {
        &self.pure_operators
    }

    /// Model declarations in source order.
    #[must_use]
    pub fn models(&self) -> &[ModelDecl] {
        &self.models
    }
}
