//! Pure-operator source declarations and their exact syntax.

use super::{TextRange, VisibilitySyntax};

/// One exact, side-effect-free operator definition in source form.
///
/// This syntax is deliberately separate from model expressions. It admits
/// only exact rationals, component selection, Kronecker deltas, and bounded
/// arithmetic, so later lowering never has to recover purity from a general
/// expression tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureOperatorDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) formals: Vec<PureOperatorFormal>,
    pub(crate) result: PureValueClassSyntax,
    pub(crate) body: PureOperatorExpr,
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
    pub const fn body(&self) -> &PureOperatorExpr {
        &self.body
    }

    /// Full declaration range, including visibility and trailing semicolon.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One ordered pure-operator formal.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PureValueClassSyntax {
    /// One scalar value.
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

/// Exact expression admitted inside a pure operator declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PureOperatorExpr {
    pub(crate) kind: PureOperatorExprKind,
    pub(crate) range: TextRange,
}

impl PureOperatorExpr {
    /// Exact expression form.
    #[must_use]
    pub const fn kind(&self) -> &PureOperatorExprKind {
        &self.kind
    }

    /// Full expression range, including explicit parentheses when present.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Closed exact expression vocabulary for pure operators.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PureOperatorExprKind {
    /// Exact rational literal `rational(numerator, denominator)`.
    Rational {
        /// Nonnegative numerator; sign is represented by [`Self::Neg`].
        numerator: ExactIntegerSyntax,
        /// Strictly positive denominator.
        denominator: ExactIntegerSyntax,
    },
    /// Select a formal component using one output axis per formal axis.
    Component {
        /// Referenced formal name.
        formal: String,
        /// Exact range of the formal-name occurrence.
        formal_range: TextRange,
        /// Ordered result-axis sequence; empty selects a scalar formal.
        result_axes: Vec<ExactIntegerSyntax>,
    },
    /// Kronecker delta between two result axes.
    Delta {
        /// Left result axis.
        left_axis: ExactIntegerSyntax,
        /// Right result axis.
        right_axis: ExactIntegerSyntax,
    },
    /// Exact prefix negation.
    Neg(Box<PureOperatorExpr>),
    /// Exact infix arithmetic.
    Binary {
        /// Arithmetic operator.
        op: PureOperatorBinaryOp,
        /// Left operand.
        left: Box<PureOperatorExpr>,
        /// Right operand.
        right: Box<PureOperatorExpr>,
    },
}

/// Infix operators admitted by a pure operator body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PureOperatorBinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}
