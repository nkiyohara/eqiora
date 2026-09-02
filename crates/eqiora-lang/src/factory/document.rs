use crate::ast::document::{ImportDecl, ModuleDecl};
use crate::ast::{
    ComponentDecl, ConnectorDecl, Document, Expr, ModelDecl, NamePath, PureOperatorDecl, TextRange,
};

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range, validate_name_path,
};

impl SourceAstFactory {
    /// Give one source document an explicit logical module identity.
    ///
    /// # Errors
    /// Returns an error for an invalid qualified name, source range, or a
    /// document that already has a module declaration.
    pub fn with_module(
        mut document: Document,
        name: NamePath,
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        validate_name_path(&name)?;
        if document.module.is_some() {
            return Err(AstConstructionError::new(
                "a source document has exactly one logical module identity",
            ));
        }
        document.module = Some(ModuleDecl {
            name,
            range: checked_range(range)?,
        });
        Ok(document)
    }

    /// Add one explicit semantic module import to a source document.
    ///
    /// # Errors
    /// Returns an error for an invalid qualified module, alias, or source range.
    pub fn with_import(
        mut document: Document,
        module: NamePath,
        alias: impl Into<String>,
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        validate_name_path(&module)?;
        let import = ImportDecl {
            module,
            alias: checked_identifier(alias, "module import alias")?,
            range: checked_range(range)?,
        };
        if document
            .imports
            .iter()
            .any(|existing| existing.alias == import.alias)
        {
            return Err(AstConstructionError::new(format!(
                "duplicate module import alias `{}`",
                import.alias
            )));
        }
        document.imports.push(import);
        Ok(document)
    }

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
            module: None,
            imports: Vec::new(),
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
            module: None,
            imports: Vec::new(),
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
            module: None,
            imports: Vec::new(),
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
            module: None,
            imports: Vec::new(),
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

#[cfg(test)]
mod tests {
    use crate::{NamePath, SourceAstFactory, TextRange, format, parse};

    #[test]
    fn checked_factory_adds_one_canonical_module_import_prefix() {
        let range = TextRange::new(0, 0);
        let document = parse("main.eqi", "model Main {}")
            .into_document()
            .expect("base document");
        let document = SourceAstFactory::with_module(
            document,
            NamePath::from_segments(["models", "main"], range).unwrap(),
            range,
        )
        .expect("checked module identity");
        let document = SourceAstFactory::with_import(
            document,
            NamePath::from_segments(["library", "parts"], range).unwrap(),
            "lib",
            range,
        )
        .expect("checked import");
        assert_eq!(
            format(&document),
            "module models.main;\nimport library.parts as lib;\n\nmodel Main {\n}\n"
        );

        assert!(
            SourceAstFactory::with_module(
                document.clone(),
                NamePath::from_segments(["other"], range).unwrap(),
                range,
            )
            .is_err()
        );

        assert!(
            SourceAstFactory::with_import(
                document,
                NamePath::from_segments(["other"], range).unwrap(),
                "lib",
                range,
            )
            .is_err()
        );
    }
}
