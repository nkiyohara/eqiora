use super::{Expr, TextRange, ValueTypeSyntax};

/// Compilation-unit structural dimension alias.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DimensionDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
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
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) value_type: ValueTypeSyntax,
    pub(crate) value: Expr,
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
    pub fn dimension(&self) -> &Expr {
        self.value_type.dimension()
    }

    /// Complete declared mathematical type.
    #[must_use]
    pub const fn value_type(&self) -> &ValueTypeSyntax {
        &self.value_type
    }

    /// Returns the numeric value, including any explicit input unit.
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

/// Immutable local expression alias with optional type and intrinsic-support assertions.
#[derive(Debug, Clone, PartialEq)]
pub struct LetDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) value_type: Option<ValueTypeSyntax>,
    pub(crate) domain: Option<String>,
    pub(crate) value: Expr,
    pub(crate) range: TextRange,
}

impl LetDecl {
    /// Returns the declared alias name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the optional complete mathematical type assertion.
    #[must_use]
    pub const fn value_type(&self) -> Option<&ValueTypeSyntax> {
        self.value_type.as_ref()
    }

    /// Returns the optional assertion of the expression's intrinsic support.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Returns the immutable expression.
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
