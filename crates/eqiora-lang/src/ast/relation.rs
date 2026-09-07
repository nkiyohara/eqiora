//! Ordered source equalities and their activation/support owner.

use super::{BoundaryFamilyBinderSyntax, Expr, TextRange};

/// Simultaneous mathematical conditions used only for fresh initialization.
#[derive(Debug, Clone, PartialEq)]
pub struct InitialDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) equations: Vec<Equation>,
    pub(crate) range: TextRange,
}

impl InitialDecl {
    /// Conditions in authored order; their semantics are simultaneous.
    #[must_use]
    pub fn equations(&self) -> &[Equation] {
        &self.equations
    }

    /// Full initialization-block range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Relation declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationDecl {
    pub(crate) comments: crate::ast::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) activation: ActivationSyntax,
    pub(crate) domain: Option<String>,
    pub(crate) equations: Vec<Equation>,
    pub(crate) range: TextRange,
}

impl RelationDecl {
    /// Source name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Continuous or exact-periodic activation.
    #[must_use]
    pub const fn activation(&self) -> &ActivationSyntax {
        &self.activation
    }

    /// Domain on which the equations hold, for a spatial Relation.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Simultaneous equalities in authored order, not sequential assignments.
    #[must_use]
    pub fn equations(&self) -> &[Equation] {
        &self.equations
    }

    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One ordered authored equality. Residual construction belongs to checked lowering.
#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    pub(crate) left: Expr,
    pub(crate) right: Expr,
    pub(crate) range: TextRange,
}

impl Equation {
    /// Authored left-hand expression.
    #[must_use]
    pub const fn left(&self) -> &Expr {
        &self.left
    }

    /// Authored right-hand expression, including an explicitly written zero.
    #[must_use]
    pub const fn right(&self) -> &Expr {
        &self.right
    }

    /// Equality range excluding the terminating semicolon.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One continuous Relation expanded once per complete-exterior member.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationFamilyDecl {
    pub(crate) relation: RelationDecl,
    pub(crate) binder: BoundaryFamilyBinderSyntax,
}

impl RelationFamilyDecl {
    /// Underlying continuous Relation declaration.
    #[must_use]
    pub const fn relation(&self) -> &RelationDecl {
        &self.relation
    }

    /// Restricted boundary-member binder.
    #[must_use]
    pub const fn binder(&self) -> &BoundaryFamilyBinderSyntax {
        &self.binder
    }

    /// Full family declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.relation.range
    }
}

/// Source activation syntax.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ActivationSyntax {
    /// Active throughout model time.
    Continuous,
    /// Active at ticks of the named ClockDomain.
    Periodic(String),
}
