//! Declaration-owned finite enumerations and exhaustive source case arms.
use super::{Expr, NamePath, TextRange, VisibilitySyntax};

/// A module-level enum whose ordered tags belong to one declaration identity.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub(crate) comments: super::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) tags: Vec<NamePath>,
    pub(crate) range: TextRange,
}
impl EnumDecl {
    /// Module visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }
    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Ordered tag tokens, each an unqualified name with its own source range.
    #[must_use]
    pub fn tags(&self) -> &[NamePath] {
        &self.tags
    }
    /// Full declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One explicit qualified enum-tag pattern and its authored result expression.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseArm {
    pub(crate) pattern: NamePath,
    pub(crate) value: Expr,
    pub(crate) range: TextRange,
}
impl CaseArm {
    /// Qualified enum tag, resolved against the scrutinee's exact enum identity.
    #[must_use]
    pub const fn pattern(&self) -> &NamePath {
        &self.pattern
    }
    /// Result for this tag; retained until every arm has been validated.
    #[must_use]
    pub const fn value(&self) -> &Expr {
        &self.value
    }
    /// Pattern-through-result source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
