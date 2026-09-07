use super::{ExpressionType, TypeViolation, additive};
use eqiora_core::{DimExponents, ScalarDomain, ValueFrame, ValueType};

impl<I: Clone + Eq> ExpressionType<I> {
    /// Type a checked integer quotient or remainder, without real promotion.
    pub fn integer_quotient(self, other: Self) -> Result<Self, TypeViolation<I>> {
        integer_scalar(&self)?;
        integer_scalar(&other)?;
        additive(&self, &other)
    }

    /// Explicitly convert an integer scalar to a dimensionless real scalar.
    pub fn to_real(self) -> Result<Self, TypeViolation<I>> {
        integer_scalar(&self)?;
        Ok(Self::new(
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            self.support,
        ))
    }

    /// Admit the type of a checked real-to-integer conversion.
    /// The integral value and exact range are checked when evaluating it.
    pub fn to_integer(self) -> Result<Self, TypeViolation<I>> {
        if self.value_type.scalar_domain() != ScalarDomain::Real
            || !self.shape().is_scalar()
            || self.dimension() != DimExponents::DIMENSIONLESS
        {
            return Err(TypeViolation::ScalarDomainMismatch);
        }
        Ok(Self::new(
            ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
            self.support,
        ))
    }
}

fn integer_scalar<I>(value: &ExpressionType<I>) -> Result<(), TypeViolation<I>> {
    if value.value_type.scalar_domain() != ScalarDomain::Integer
        || !value.shape().is_scalar()
        || value.frame() != ValueFrame::Invariant
        || value.dimension() != DimExponents::DIMENSIONLESS
    {
        return Err(TypeViolation::ScalarDomainMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::typing::{additive, divide, multiply, time_derivative};
    #[test]
    fn discrete_domains_require_explicit_conversion_and_have_no_derivative() {
        let integer: ExpressionType<()> = ExpressionType::new(
            ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
            None,
        );
        let real = integer.clone().to_real().unwrap();
        assert_eq!(real.clone().to_integer().unwrap(), integer);
        assert_eq!(additive(&integer, &integer).unwrap(), integer);
        assert_eq!(multiply(&integer, &integer).unwrap(), integer);
        assert!(additive(&integer, &real).is_err());
        assert!(multiply(&real, &integer).is_err());
        assert!(divide(&integer, &integer).is_err());
        assert!(time_derivative(&integer).is_err());
        assert!(integer.clone().to_integer().is_err());
        assert_eq!(
            integer.clone().integer_quotient(integer.clone()).unwrap(),
            integer
        );
    }
}
