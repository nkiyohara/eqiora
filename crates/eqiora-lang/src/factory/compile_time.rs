use super::{
    AstConstructionError, Expr, LetDecl, ParameterBindingDecl, ParameterDecl, SourceAstFactory,
    TextRange, checked_identifier, checked_range, validate_expression, validate_identifier,
};
use crate::ast::DimensionDecl;

pub(super) fn validate_parameter_binding(
    binding: &ParameterBindingDecl,
) -> Result<(), AstConstructionError> {
    validate_identifier(binding.parameter(), "Parameter binding")?;
    checked_range(binding.range())?;
    validate_expression(binding.value())
}

impl SourceAstFactory {
    /// Construct one compilation-unit structural dimension alias.
    ///
    /// # Errors
    /// Returns an error for a malformed name, expression, or range.
    pub(crate) fn dimension_alias(
        name: impl Into<String>,
        expression: Expr,
        range: TextRange,
    ) -> Result<DimensionDecl, AstConstructionError> {
        validate_expression(&expression)?;
        Ok(DimensionDecl {
            comments: Default::default(),
            name: checked_identifier(name, "dimension alias")?,
            expression,
            range: checked_range(range)?,
        })
    }

    /// Construct a model-level typed Parameter declaration.
    ///
    /// # Errors
    /// Returns an error for a non-finite value or malformed source shape.
    pub fn parameter(
        name: impl Into<String>,
        value_type: crate::ValueTypeSyntax,
        value: Expr,
        range: TextRange,
    ) -> Result<ParameterDecl, AstConstructionError> {
        validate_expression(&value)?;
        if !matches!(
            value.kind(),
            crate::ExprKind::Number(_) | crate::ExprKind::Quantity { .. }
        ) {
            return Err(AstConstructionError::new(
                "Parameter value must be a numeric quantity literal",
            ));
        }
        Ok(ParameterDecl {
            comments: Default::default(),
            name: checked_identifier(name, "Parameter")?,
            value_type,
            value,
            range: checked_range(range)?,
        })
    }

    /// Construct a reusable immutable local expression alias.
    ///
    /// # Errors
    /// Returns an error for malformed source expressions, names, or ranges.
    pub fn let_alias(
        name: impl Into<String>,
        value_type: Option<crate::ValueTypeSyntax>,
        domain: Option<String>,
        activation: Option<String>,
        value: Expr,
        range: TextRange,
    ) -> Result<LetDecl, AstConstructionError> {
        validate_expression(&value)?;
        Ok(LetDecl {
            comments: Default::default(),
            name: checked_identifier(name, "let alias")?,
            value_type,
            domain: domain
                .map(|name| checked_identifier(name, "let support assertion"))
                .transpose()?,
            activation: activation
                .map(|name| checked_identifier(name, "let activation assertion"))
                .transpose()?,
            value,
            range: checked_range(range)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{ExprKind, SourceAstFactory, TextRange, format, parse};

    #[test]
    fn checked_factory_constructs_a_formattable_dimension_prefix() {
        let range = TextRange::new(0, 0);
        let expression = SourceAstFactory::expression(ExprKind::Name("m".to_owned()), range)
            .expect("dimension expression");
        let model = parse("model.eqi", "model M { variable x: Length; }")
            .into_document()
            .expect("model source")
            .models()[0]
            .clone();
        let document = SourceAstFactory::document_with_dimensions(
            vec![("Length".to_owned(), expression, range)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![model],
        )
        .expect("dimension-bearing document");

        assert_eq!(
            format(&document),
            "dimension Length = m;\n\nmodel M {\n  variable x: Length;\n}\n"
        );
    }

    #[test]
    fn checked_factory_retains_optional_let_dimension_assertions() {
        let range = TextRange::new(0, 1);
        let value = SourceAstFactory::expression(ExprKind::Number(1.0), range).expect("value");
        let dimension =
            SourceAstFactory::expression(ExprKind::Name("m".to_owned()), range).expect("dimension");

        let inferred =
            SourceAstFactory::let_alias("inferred", None, None, None, value.clone(), range)
                .expect("inferred alias");
        let annotated = SourceAstFactory::let_alias(
            "annotated",
            Some(crate::ValueTypeSyntax::real(dimension)),
            None,
            None,
            value,
            range,
        )
        .expect("annotated alias");

        assert!(inferred.value_type().is_none());
        assert!(annotated.value_type().is_some());
    }
}
