use super::{Expr, TextRange};

/// Compilation-unit structural dimension alias.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DimensionDecl {
    pub(crate) name: String,
    pub(crate) expression: Expr,
    pub(crate) range: TextRange,
}

impl DimensionDecl {
    /// Returns the declared alias name.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the structural dimension expression.
    #[must_use]
    pub(crate) const fn expression(&self) -> &Expr {
        &self.expression
    }

    /// Returns the declaration's source range.
    #[must_use]
    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }
}

/// Parameter source declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDecl {
    pub(crate) name: String,
    pub(crate) dimension: Expr,
    pub(crate) initial: f64,
    pub(crate) range: TextRange,
}

impl ParameterDecl {
    /// Returns the declared name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the declared dimension expression.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
    }

    /// Returns the initial scalar value.
    #[must_use]
    pub const fn initial(&self) -> f64 {
        self.initial
    }

    /// Returns the declaration's source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Model-local compile-time expression alias with an optional dimension assertion.
#[derive(Debug, Clone, PartialEq)]
pub struct LetDecl {
    pub(crate) name: String,
    pub(crate) dimension: Option<Expr>,
    pub(crate) value: Expr,
    pub(crate) range: TextRange,
}

impl LetDecl {
    /// Returns the declared alias name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the optional dimension assertion.
    #[must_use]
    pub const fn dimension(&self) -> Option<&Expr> {
        self.dimension.as_ref()
    }

    /// Returns the compile-time value expression.
    #[must_use]
    pub const fn value(&self) -> &Expr {
        &self.value
    }

    /// Returns the declaration's source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
