use super::{Expr, TextRange};

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

/// Model-local typed compile-time expression alias.
#[derive(Debug, Clone, PartialEq)]
pub struct LetDecl {
    pub(crate) name: String,
    pub(crate) dimension: Expr,
    pub(crate) value: Expr,
    pub(crate) range: TextRange,
}

impl LetDecl {
    /// Returns the declared alias name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the declared dimension expression.
    #[must_use]
    pub const fn dimension(&self) -> &Expr {
        &self.dimension
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
