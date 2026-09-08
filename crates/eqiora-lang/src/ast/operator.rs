//! Pure-operator source declarations and their exact syntax.

use super::{Expr, TextRange, ValueTypeSyntax, VisibilitySyntax};

/// One exact, side-effect-free operator definition in source form.
///
/// The shared expression body is checked for exact purity during compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct PureOperatorDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) formals: Vec<PureOperatorFormal>,
    pub(crate) result: PureValueClassSyntax,
    pub(crate) body: Expr,
    pub(crate) range: TextRange,
}

impl PureOperatorDecl {
    /// Package visibility. Unqualified declarations are private by default.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ordered formal arguments.
    #[must_use]
    pub fn formals(&self) -> &[PureOperatorFormal] {
        &self.formals
    }

    /// Declared result value class.
    #[must_use]
    pub const fn result(&self) -> &PureValueClassSyntax {
        &self.result
    }

    /// Exact bounded operator body.
    #[must_use]
    pub const fn body(&self) -> &Expr {
        &self.body
    }

    /// Full declaration range, including visibility and trailing semicolon.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One ordered pure-operator formal.
#[derive(Debug, Clone, PartialEq)]
pub struct PureOperatorFormal {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) value_class: PureValueClassSyntax,
    pub(crate) range: TextRange,
}

impl PureOperatorFormal {
    /// Formal name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared value class.
    #[must_use]
    pub const fn value_class(&self) -> &PureValueClassSyntax {
        &self.value_class
    }

    /// Full formal range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Closed source value classes admitted by a pure operator definition.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PureValueClassSyntax {
    /// One concrete declared value type.
    Typed(ValueTypeSyntax),
    /// One dimension-polymorphic scalar value.
    Scalar,
    /// A spatial value whose rank is retained as exact source syntax.
    Spatial {
        /// Exact tensor rank.
        rank: ExactIntegerSyntax,
    },
}

/// One exact nonnegative integer token with its original source spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExactIntegerSyntax {
    pub(crate) spelling: String,
    pub(crate) value: u64,
    pub(crate) range: TextRange,
}

impl ExactIntegerSyntax {
    /// Exact source spelling, without sign or radix prefix.
    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }

    /// Parsed exact value.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Integer-token range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
