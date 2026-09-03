use eqiora_lang::VisibilitySyntax;

use super::{AnalyzedResolvedHierarchy, CompilationNamespaceId};

impl AnalyzedResolvedHierarchy {
    /// Compiler-canonical declarations in `(namespace, path, kind)` order.
    #[must_use]
    pub fn canonical_declarations(&self) -> &[CanonicalDeclarationIdentity] {
        &self.canonical_declarations
    }
}

/// Top-level declaration families currently understood by package lowering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CanonicalDeclarationKind {
    PropertyContract,
    PropertyRelease,
    MaterialComposition,
    PureOperator,
    Connector,
    Component,
    Model,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalDeclarationVisibility {
    Private,
    Public,
}

impl From<VisibilitySyntax> for CanonicalDeclarationVisibility {
    fn from(value: VisibilitySyntax) -> Self {
        match value {
            VisibilitySyntax::Private => Self::Private,
            VisibilitySyntax::Public => Self::Public,
        }
    }
}

/// File-layout-independent canonical declaration emitted by the compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeclarationIdentity {
    pub(super) namespace: CompilationNamespaceId,
    pub(super) path: String,
    pub(super) kind: CanonicalDeclarationKind,
    pub(super) visibility: CanonicalDeclarationVisibility,
    pub(super) canonical_form: String,
}

impl CanonicalDeclarationIdentity {
    #[must_use]
    pub const fn namespace(&self) -> &CompilationNamespaceId {
        &self.namespace
    }
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    #[must_use]
    pub const fn kind(&self) -> CanonicalDeclarationKind {
        self.kind
    }
    #[must_use]
    pub const fn visibility(&self) -> CanonicalDeclarationVisibility {
        self.visibility
    }
    #[must_use]
    pub fn canonical_form(&self) -> &str {
        &self.canonical_form
    }
}
