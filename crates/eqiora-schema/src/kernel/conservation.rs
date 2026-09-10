//! Physical conservation terms retained by their owning Relation.
//!
//! Fixed-domain outward-flux convention: div(flux) = source.
//! These identities name mathematical expressions, never numerical face fluxes.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use super::{ExprDag, ExprId, ExprNode};

/// Exact physical terms of one fixed-domain conservation Law.
///
/// This is a steady balance. Source is explicit, including a typed zero.
/// Support, parameter dependencies, and source identity belong to the Relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConservationTerms {
    flux: ExprId,
    source: ExprId,
}

impl ConservationTerms {
    /// Name terms in the owning Relation's expression arena.
    ///
    /// Construction of the Relation checks the corresponding balance roots;
    /// semantic admission checks physical dimensions and support.
    #[must_use]
    pub const fn new(flux: ExprId, source: ExprId) -> Self {
        Self { flux, source }
    }

    /// Physical outward flux, before divergence or numerical realization.
    #[must_use]
    pub const fn flux(self) -> ExprId {
        self.flux
    }

    /// Physical production per unit volume and time.
    #[must_use]
    pub const fn source(self) -> ExprId {
        self.source
    }

    /// Check exact structural correspondence with the retained balance equation.
    ///
    /// This deliberately does not recognize algebraically similar residuals.
    ///
    /// # Errors
    /// Rejects missing term nodes, extra equations, changed source, reversed flux,
    /// or a changed divergence in the corresponding balance.
    pub fn validate_balance(self, expression: &ExprDag) -> Result<(), Diagnostic> {
        let invalid = || {
            Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "conservation Law requires exact div(outward flux) = source roots",
            )
        };
        let [left, right] = expression.roots() else {
            return Err(invalid());
        };
        if *right != self.source
            || expression.node(self.source).is_none()
            || expression.node(self.flux).is_none()
        {
            return Err(invalid());
        }
        let divergence = *left;
        match expression.node(divergence) {
            Some(ExprNode::Divergence(flux)) if *flux == self.flux => Ok(()),
            _ => Err(invalid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{ExprDagBuilder, SymbolRef};
    use eqiora_core::{Id, entity::kinds};

    #[test]
    fn steady_balance_retains_exact_flux_and_source_identities() {
        let mut builder = ExprDagBuilder::new();
        let field = builder
            .symbol(SymbolRef::Field(Id::<kinds::Field>::new()))
            .unwrap();
        let gradient = builder.gradient(field).unwrap();
        let flux = builder.neg(gradient).unwrap();
        let divergence = builder.divergence(flux).unwrap();
        let source = builder
            .symbol(SymbolRef::Parameter(Id::<kinds::Parameter>::new()))
            .unwrap();
        let other_source = builder
            .symbol(SymbolRef::Parameter(Id::<kinds::Parameter>::new()))
            .unwrap();
        let expression = builder.finish([divergence, source]).unwrap();
        assert!(
            ConservationTerms::new(flux, source)
                .validate_balance(&expression)
                .is_ok()
        );
        assert!(
            ConservationTerms::new(gradient, source)
                .validate_balance(&expression)
                .is_err()
        );
        assert!(
            ConservationTerms::new(flux, other_source)
                .validate_balance(&expression)
                .is_err()
        );
    }
}
