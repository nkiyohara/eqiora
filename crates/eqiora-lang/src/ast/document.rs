use crate::ast_property::{PropertyContractDecl, PropertyReleaseDecl};

use super::{
    ComponentDecl, ConnectorDecl, DimensionDecl, Item, NamePath, PureOperatorDecl, TextRange,
    VisibilitySyntax,
};

/// A named model and its declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) items: Vec<Item>,
    pub(crate) range: TextRange,
}

impl ModelDecl {
    /// Module visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declarations in source order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Full model declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One explicit, side-effect-free semantic module import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportDecl {
    pub(crate) module: NamePath,
    pub(crate) alias: String,
    pub(crate) range: TextRange,
}

/// One source-owned logical module identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleDecl {
    pub(crate) name: NamePath,
    pub(crate) range: TextRange,
}

/// One parsed source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub(crate) module: Option<ModuleDecl>,
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
    /// Explicit logical module identity, when this source does not belong to
    /// the caller-selected implicit `main` module.
    #[must_use]
    pub fn module(&self) -> Option<(&NamePath, TextRange)> {
        self.module
            .as_ref()
            .map(|module| (&module.name, module.range))
    }

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
