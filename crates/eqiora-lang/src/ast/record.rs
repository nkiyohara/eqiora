//! Closed nominal product declarations. Members are heterogeneous value types.
use super::{TextRange, ValueTypeSyntax, VisibilitySyntax};

/// A module-level closed record, with declaration-ordered named members.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    pub(crate) comments: super::comments::SourceComments,
    pub(crate) visibility: VisibilitySyntax,
    pub(crate) name: String,
    pub(crate) members: Vec<RecordMemberDecl>,
    pub(crate) range: TextRange,
}

impl RecordDecl {
    /// Exact module visibility of this declaration.
    #[must_use]
    pub const fn visibility(&self) -> VisibilitySyntax {
        self.visibility
    }
    /// Lexical declaration name; equal display names do not imply equal identity.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Members in declaration order, without homogeneous vector coercion.
    #[must_use]
    pub fn members(&self) -> &[RecordMemberDecl] {
        &self.members
    }
    /// Full declaration source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// One named member retaining its complete mathematical type.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordMemberDecl {
    pub(crate) comments: super::comments::SourceComments,
    pub(crate) name: String,
    pub(crate) value_type: ValueTypeSyntax,
    pub(crate) range: TextRange,
}
impl RecordMemberDecl {
    /// Member identifier within its exact parent declaration.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Complete mathematical type, including dimensions and component shape.
    #[must_use]
    pub const fn value_type(&self) -> &ValueTypeSyntax {
        &self.value_type
    }
    /// Member source range including its type.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
