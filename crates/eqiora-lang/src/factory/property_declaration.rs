//! Current property declarations use the same owned module as parsed source.

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range, validate_expression,
    validate_name_path,
};
use crate::{Document, Expr, NamePath, TextRange, ValueTypeSyntax, VisibilitySyntax};

impl SourceAstFactory {
    /// Append one typed constant-property contract to a compilation module.
    ///
    /// # Errors
    /// Rejects malformed names, result syntax, or source ranges.
    pub fn with_property_contract(
        mut document: Document,
        name: String,
        value_type: ValueTypeSyntax,
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        Self::value_type(value_type.kind().clone(), value_type.range())?;
        document
            .property_contracts
            .push(crate::ast_property::PropertyContractDecl {
                comments: Default::default(),
                visibility: VisibilitySyntax::Public,
                name: checked_identifier(name, "property contract")?,
                value_type,
                range: checked_range(range)?,
            });
        Ok(document)
    }

    /// Append one constant release with explicit scientific source metadata.
    ///
    /// # Errors
    /// Rejects malformed names, expressions, or ranges. The compiler owns
    /// nominal contract, dimensional, scaling, and value admission.
    pub fn with_property_release(
        mut document: Document,
        name: String,
        contract: NamePath,
        source: (Expr, Expr, Expr),
        attribution: (NamePath, NamePath),
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        validate_name_path(&contract)?;
        let (source_value, source_dimension, coherent_si_scale) = source;
        for value in [&source_value, &source_dimension, &coherent_si_scale] {
            validate_expression(value)?;
        }
        let (citation, license) = attribution;
        validate_name_path(&citation)?;
        validate_name_path(&license)?;
        document
            .property_releases
            .push(crate::ast_property::PropertyReleaseDecl {
                comments: Default::default(),
                visibility: VisibilitySyntax::Public,
                name: checked_identifier(name, "property release")?,
                contract,
                source_value,
                source_dimension,
                coherent_si_scale,
                citation,
                license,
                range: checked_range(range)?,
            });
        Ok(document)
    }

    /// Append an explicit finite property composition.
    ///
    /// # Errors
    /// Rejects empty bindings, malformed names, paths, or source ranges.
    pub fn with_material_composition(
        mut document: Document,
        name: String,
        properties: Vec<(String, NamePath, TextRange)>,
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        if properties.is_empty() {
            return Err(AstConstructionError::new(
                "material composition requires a property",
            ));
        }
        let range = checked_range(range)?;
        let properties = properties
            .into_iter()
            .map(|(property, release, property_range)| {
                validate_name_path(&release)?;
                Ok(crate::ast_property::PropertyBindingDecl {
                    comments: Default::default(),
                    property: checked_identifier(property, "material property")?,
                    release,
                    range: checked_range(property_range)?,
                })
            })
            .collect::<Result<_, AstConstructionError>>()?;
        document
            .material_compositions
            .push(crate::ast_property::MaterialCompositionDecl {
                comments: Default::default(),
                visibility: VisibilitySyntax::Public,
                name: checked_identifier(name, "material composition")?,
                properties,
                range,
            });
        Ok(document)
    }
}
