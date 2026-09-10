//! Physical conservation syntax, before any numerical method is selected.

use super::Expr;

/// Steady fixed-domain outward flux and volumetric production expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct ConservationSyntax {
    pub(crate) flux: Expr,
    pub(crate) source: Expr,
}

impl ConservationSyntax {
    /// Physical outward flux before divergence.
    #[must_use]
    pub const fn flux(&self) -> &Expr {
        &self.flux
    }

    /// Production term, including an explicitly authored zero.
    #[must_use]
    pub const fn source(&self) -> &Expr {
        &self.source
    }
}
