use crate::ast::{
    ComponentDecl, ConnectorDecl, Document, Expr, ModelDecl, PureOperatorDecl, TextRange,
};

use super::{AstConstructionError, SourceAstFactory};

impl SourceAstFactory {
    /// Close one compilation unit with an ordered structural-dimension prefix.
    ///
    /// # Errors
    /// Returns an error when the compilation unit contains no declaration.
    pub fn document_with_dimensions(
        dimensions: Vec<(String, Expr, TextRange)>,
        connectors: Vec<ConnectorDecl>,
        components: Vec<ComponentDecl>,
        pure_operators: Vec<PureOperatorDecl>,
        models: Vec<ModelDecl>,
    ) -> Result<Document, AstConstructionError> {
        if dimensions.is_empty()
            && connectors.is_empty()
            && components.is_empty()
            && pure_operators.is_empty()
            && models.is_empty()
        {
            return Err(AstConstructionError::new(
                "a source document requires at least one top-level declaration",
            ));
        }
        let dimensions = dimensions
            .into_iter()
            .map(|(name, expression, range)| Self::dimension_alias(name, expression, range))
            .collect::<Result<_, _>>()?;
        Ok(Document {
            dimensions,
            property_contracts: Vec::new(),
            property_releases: Vec::new(),
            connectors,
            components,
            pure_operators,
            models,
        })
    }

    /// Close one nonempty compilation unit into a formatter-compatible document.
    ///
    /// A declarations-only document is valid source syntax for a package
    /// library. Executable compilation still requires at least one Model.
    ///
    /// # Errors
    /// Returns an error when the compilation unit is empty.
    pub fn document(
        connectors: Vec<ConnectorDecl>,
        components: Vec<ComponentDecl>,
        models: Vec<ModelDecl>,
    ) -> Result<Document, AstConstructionError> {
        if connectors.is_empty() && components.is_empty() && models.is_empty() {
            return Err(AstConstructionError::new(
                "a source document requires at least one top-level declaration",
            ));
        }
        Ok(Document {
            dimensions: Vec::new(),
            property_contracts: Vec::new(),
            property_releases: Vec::new(),
            connectors,
            components,
            pure_operators: Vec::new(),
            models,
        })
    }

    /// Close one nonempty compilation unit including pure operators.
    ///
    /// # Errors
    /// Returns an error when the compilation unit is empty.
    pub fn document_with_pure_operators(
        connectors: Vec<ConnectorDecl>,
        components: Vec<ComponentDecl>,
        pure_operators: Vec<PureOperatorDecl>,
        models: Vec<ModelDecl>,
    ) -> Result<Document, AstConstructionError> {
        if connectors.is_empty()
            && components.is_empty()
            && pure_operators.is_empty()
            && models.is_empty()
        {
            return Err(AstConstructionError::new(
                "a source document requires at least one top-level declaration",
            ));
        }
        Ok(Document {
            dimensions: Vec::new(),
            property_contracts: Vec::new(),
            property_releases: Vec::new(),
            connectors,
            components,
            pure_operators,
            models,
        })
    }

    /// Close one or more flat models into a formatter-compatible document.
    ///
    /// # Errors
    /// Returns an error when `models` is empty, matching the source grammar.
    pub fn flat_document(models: Vec<ModelDecl>) -> Result<Document, AstConstructionError> {
        if models.is_empty() {
            return Err(AstConstructionError::new(
                "a source document requires at least one model",
            ));
        }
        Ok(Document {
            dimensions: Vec::new(),
            property_contracts: Vec::new(),
            property_releases: Vec::new(),
            connectors: Vec::new(),
            components: Vec::new(),
            pure_operators: Vec::new(),
            models,
        })
    }
}
