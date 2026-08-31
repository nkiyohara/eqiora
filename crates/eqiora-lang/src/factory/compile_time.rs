use super::{
    AstConstructionError, Expr, Item, LetDecl, ParameterDecl, SourceAstFactory, TextRange,
    checked_identifier, checked_range, validate_expression, validate_finite,
};
use crate::ast::DimensionDecl;

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
            name: checked_identifier(name, "dimension alias")?,
            expression,
            range: checked_range(range)?,
        })
    }

    /// Construct a model-level scalar Parameter declaration.
    ///
    /// # Errors
    /// Returns an error for a non-finite value or malformed source shape.
    pub fn parameter(
        name: impl Into<String>,
        dimension: Expr,
        initial: f64,
        range: TextRange,
    ) -> Result<ParameterDecl, AstConstructionError> {
        validate_expression(&dimension)?;
        validate_finite(initial, "Parameter value")?;
        Ok(ParameterDecl {
            name: checked_identifier(name, "Parameter")?,
            dimension,
            initial,
            range: checked_range(range)?,
        })
    }

    /// Construct a model-local typed compile-time expression alias.
    ///
    /// # Errors
    /// Returns an error for malformed source expressions, names, or ranges.
    pub fn let_alias(
        name: impl Into<String>,
        dimension: Expr,
        value: Expr,
        range: TextRange,
    ) -> Result<Item, AstConstructionError> {
        validate_expression(&dimension)?;
        validate_expression(&value)?;
        Ok(Item::Let(LetDecl {
            name: checked_identifier(name, "let alias")?,
            dimension,
            value,
            range: checked_range(range)?,
        }))
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
        let model = parse("model.eqi", "model M { field x: Length = 0; }")
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
            "dimension Length = m;\n\nmodel M {\n  field x: Length = 0;\n}\n"
        );
    }
}
