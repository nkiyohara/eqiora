//! Current property declarations use the same owned module as parsed source.

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range, validate_expression,
    validate_name_path,
};
use crate::{Document, Expr, NamePath, TextRange, ValueTypeSyntax, VisibilitySyntax};

impl SourceAstFactory {
    /// Create or extend a compilation module with one checked property contract.
    ///
    /// # Errors
    /// Rejects malformed names, result syntax, or source ranges.
    pub fn with_property_contract(
        document: Option<Document>,
        name: String,
        value_type: ValueTypeSyntax,
        profile: (
            Vec<(String, ValueTypeSyntax)>,
            eqiora_schema::kernel::PropertyDerivatives,
            Option<NamePath>,
        ),
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        let mut document = declaration_document(document);
        Self::value_type(value_type.kind().clone(), value_type.range())?;
        let (inputs, derivatives, branch) = profile;
        let mut seen = std::collections::BTreeSet::new();
        for (name, value_type) in &inputs {
            super::validate_identifier(name, "property input")?;
            Self::value_type(value_type.kind().clone(), value_type.range())?;
            if !seen.insert(name) {
                return Err(AstConstructionError::new("duplicate property input"));
            }
        }
        if let Some(branch) = &branch {
            validate_name_path(branch)?;
        }
        if inputs.is_empty() && derivatives != eqiora_schema::kernel::PropertyDerivatives::ValueOnly
        {
            return Err(AstConstructionError::new(
                "constant contract has no independent inputs",
            ));
        }
        document
            .property_contracts
            .push(crate::ast_property::PropertyContractDecl {
                comments: Default::default(),
                visibility: VisibilitySyntax::Public,
                name: checked_identifier(name, "property contract")?,
                value_type,
                inputs,
                derivatives,
                branch,
                range: checked_range(range)?,
            });
        Ok(document)
    }

    /// Construct the fixed piecewise-affine table source profile.
    ///
    /// # Errors
    /// Rejects malformed references, column names, dimensions, or ranges.
    pub fn property_table(
        data: NamePath,
        axis: (String, Expr),
        value: (String, Expr),
        validity: [Expr; 2],
        range: TextRange,
    ) -> Result<crate::PropertySourceSyntax, AstConstructionError> {
        validate_name_path(&data)?;
        validate_expression(&axis.1)?;
        validate_expression(&value.1)?;
        for endpoint in &validity {
            validate_expression(endpoint)?;
        }
        Ok(crate::PropertySourceSyntax::Table(Box::new(
            crate::PropertyTableSyntax {
                data,
                axis: checked_identifier(axis.0, "table input")?,
                axis_dimension: axis.1,
                value: checked_identifier(value.0, "table result")?,
                value_dimension: value.1,
                validity,
                range: checked_range(range)?,
            },
        )))
    }

    /// Append one property release with explicit scientific source metadata.
    ///
    /// # Errors
    /// Rejects malformed names, expressions, or ranges. The compiler owns
    /// nominal contract, dimensional, scaling, and value admission.
    pub fn with_property_release(
        document: Option<Document>,
        name: String,
        contract: NamePath,
        source: (crate::PropertySourceSyntax, Expr, Expr),
        attribution: (NamePath, NamePath),
        profile: (Option<Expr>, Option<NamePath>),
        range: TextRange,
    ) -> Result<Document, AstConstructionError> {
        let mut document = declaration_document(document);
        validate_name_path(&contract)?;
        let (validity, branch) = profile;
        if let Some(validity) = &validity {
            validate_expression(validity)?;
        }
        if let Some(branch) = &branch {
            validate_name_path(branch)?;
        }
        let (source_value, source_dimension, coherent_si_scale) = source;
        match &source_value {
            crate::PropertySourceSyntax::Expression(value) => validate_expression(value)?,
            crate::PropertySourceSyntax::Table(table) => {
                validate_name_path(&table.data)?;
                super::validate_identifier(&table.axis, "table input")?;
                super::validate_identifier(&table.value, "table result")?;
                validate_expression(&table.axis_dimension)?;
                validate_expression(&table.value_dimension)?;
                for endpoint in &table.validity {
                    validate_expression(endpoint)?;
                }
                checked_range(table.range)?;
            }
        }
        for value in [&source_dimension, &coherent_si_scale] {
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
                validity,
                branch: Some(branch.unwrap_or(
                    NamePath::from_segments(["single"], range).expect("canonical single branch"),
                )),
                source_dimension: Some(source_dimension),
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

fn declaration_document(document: Option<Document>) -> Document {
    document.unwrap_or_else(|| Document {
        comments: Default::default(),
        imports: Vec::new(),
        enumerations: Vec::new(),
        records: Vec::new(),
        finite_spaces: Vec::new(),
        dimensions: Vec::new(),
        property_contracts: Vec::new(),
        property_releases: Vec::new(),
        material_compositions: Vec::new(),
        connectors: Vec::new(),
        components: Vec::new(),
        pure_operators: Vec::new(),
        models: Vec::new(),
    })
}
