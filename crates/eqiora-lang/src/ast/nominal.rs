//! Source declarations for nominal finite spaces and bounded index sets.

use super::comments::SourceComments;
use super::{Expr, NamePath, TextRange, VisibilitySyntax};

/// An ordered labelled basis with its own declaration identity.
#[derive(Debug, Clone, PartialEq)]
pub struct FiniteSpaceDecl {
    pub(crate) comments: SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) labels: Vec<String>,
    pub(crate) range: TextRange,
}

impl FiniteSpaceDecl {
    /// Compilation-unit visibility.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }

    /// Declared space name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Distinct basis labels in authored order.
    #[must_use]
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Full source declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// A nominal bounded index set whose extent is checked during elaboration.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexSetDecl {
    pub(crate) comments: SourceComments,
    pub(crate) name: String,
    pub(crate) extent: Expr,
    pub(crate) range: TextRange,
}

impl IndexSetDecl {
    /// Declared index-set name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Authored static extent expression, before integer-domain elaboration.
    #[must_use]
    pub const fn extent(&self) -> &Expr {
        &self.extent
    }

    /// Full source declaration range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One family binder over an exact named index set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexFamilyBinderSyntax {
    pub(crate) binder: String,
    pub(crate) set: NamePath,
    pub(crate) range: TextRange,
}

impl IndexFamilyBinderSyntax {
    /// Local index name bound for each family occurrence.
    #[must_use]
    pub fn binder(&self) -> &str {
        &self.binder
    }

    /// Named index set supplying the binder's nominal type and bounds.
    #[must_use]
    pub const fn set(&self) -> &NamePath {
        &self.set
    }

    /// Range including the surrounding brackets.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
