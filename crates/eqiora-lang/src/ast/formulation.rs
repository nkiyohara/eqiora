//! Private recovered syntax for authored mathematical formulations.

use super::{ComponentDecl, Expr, TextRange};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FormulationDecl {
    pub(crate) relation: String,
    pub(crate) left: Expr,
    pub(crate) right: Expr,
    pub(crate) range: TextRange,
}

impl ComponentDecl {
    /// Authored mathematical formulations in source order.
    ///
    /// They are a compiler sidecar and never alter canonical Model identity.
    #[must_use]
    pub fn formulations(&self) -> impl ExactSizeIterator<Item = (&str, &Expr, &Expr, TextRange)> {
        self.formulations
            .iter()
            .map(|form| (form.relation.as_str(), &form.left, &form.right, form.range))
    }

    /// Full component declaration range, including a visibility modifier.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}
