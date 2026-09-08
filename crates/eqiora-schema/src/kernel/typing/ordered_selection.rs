//! Ordered scalar selection retains the complete operand type and compatible support.
use super::*;
use eqiora_core::ScalarDomain;

impl<I: Clone + Eq> ExpressionType<I> {
    /// Type a finite minimum or maximum without numeric promotion or dimension changes.
    /// Only ordinary invariant real/integer scalars are ordered. Equal values select
    /// the first operand; both operands remain part of the expression dependency graph.
    pub fn ordered_selection(self, other: Self) -> Result<Self, TypeViolation<I>> {
        if self.value_type != other.value_type
            || !self.shape().is_scalar()
            || self.frame() != ValueFrame::Invariant
            || self.value_type.index_set().is_some()
            || self.value_type.finite_space().is_some()
            || !matches!(
                self.value_type.scalar_domain(),
                ScalarDomain::Real | ScalarDomain::Integer
            )
            || (self.value_type.scalar_domain() == ScalarDomain::Integer
                && self.dimension() != DimExponents::DIMENSIONLESS)
        {
            return Err(TypeViolation::ScalarDomainMismatch);
        }
        self.equation(other)
    }
}
