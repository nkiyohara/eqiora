//! Checked construction of exact pure-operator syntax.

use super::*;

impl SourceAstFactory {
    /// Construct one exact integer token used by pure-operator syntax.
    ///
    /// # Errors
    /// Rejects non-decimal or `u64`-overflowing spelling and reversed ranges.
    pub fn exact_integer(
        spelling: impl Into<String>,
        range: TextRange,
    ) -> Result<ExactIntegerSyntax, AstConstructionError> {
        let spelling = spelling.into();
        let value = spelling.parse::<u64>().map_err(|_| {
            AstConstructionError::new(format!(
                "exact integer `{spelling}` must be an unsigned decimal integer fitting in u64"
            ))
        })?;
        Ok(ExactIntegerSyntax {
            spelling,
            value,
            range: checked_range(range)?,
        })
    }

    /// Construct one ordered pure-operator formal.
    ///
    /// # Errors
    /// Returns an error for an invalid name, value class, or source range.
    pub fn pure_operator_formal(
        name: impl Into<String>,
        value_class: PureValueClassSyntax,
        range: TextRange,
    ) -> Result<PureOperatorFormal, AstConstructionError> {
        validate_pure_value_class(&value_class)?;
        Ok(PureOperatorFormal {
            comments: Default::default(),
            name: checked_identifier(name, "pure operator formal")?,
            value_class,
            range: checked_range(range)?,
        })
    }

    /// Construct one top-level pure operator declaration.
    ///
    /// # Errors
    /// Returns an error for an invalid name, an empty formal list, duplicate
    /// formal names, malformed exact syntax, or a reversed source range.
    pub fn pure_operator(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        formals: Vec<PureOperatorFormal>,
        result: PureValueClassSyntax,
        body: Expr,
        range: TextRange,
    ) -> Result<PureOperatorDecl, AstConstructionError> {
        if formals.is_empty() {
            return Err(AstConstructionError::new(
                "a pure operator requires at least one formal",
            ));
        }
        let mut names = std::collections::HashSet::new();
        for formal in &formals {
            validate_identifier(&formal.name, "pure operator formal")?;
            validate_pure_value_class(&formal.value_class)?;
            checked_range(formal.range)?;
            if !names.insert(&formal.name) {
                return Err(AstConstructionError::new(format!(
                    "duplicate pure operator formal `{}`",
                    formal.name
                )));
            }
        }
        validate_pure_value_class(&result)?;
        validate_expression(&body)?;
        Ok(PureOperatorDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "pure operator")?,
            formals,
            result,
            body,
            range: checked_range(range)?,
        })
    }
}
