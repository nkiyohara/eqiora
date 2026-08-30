use super::{
    AstConstructionError, Expr, Item, LetDecl, ParameterDecl, SourceAstFactory, TextRange,
    checked_identifier, checked_range, validate_expression, validate_finite,
};

impl SourceAstFactory {
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
