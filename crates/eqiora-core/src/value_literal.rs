use crate::{DynQuantity, ScalarDomain, ValueType};

/// A finite real literal embedded in a complete mathematical type.
///
/// A shaped value admits only contextual zero. The stored literal is not
/// permission to extract a complex or shaped value as a real scalar.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueLiteral {
    value_type: ValueType,
    literal: f64,
}

impl ValueLiteral {
    /// Embed a real literal, or contextual zero, into the declared type.
    ///
    /// # Errors
    /// Rejects non-finite values and nonzero literals for shaped types.
    pub fn new(value_type: ValueType, literal: f64) -> Result<Self, InvalidValueLiteral> {
        if !literal.is_finite() {
            return Err(InvalidValueLiteral::NonFinite);
        }
        if !value_type.shape().is_scalar() && literal != 0.0 {
            return Err(InvalidValueLiteral::NonzeroShape);
        }
        Ok(Self {
            value_type,
            literal: if literal == 0.0 { 0.0 } else { literal },
        })
    }

    /// Complete mathematical type, including scalar domain and component roles.
    #[must_use]
    pub const fn value_type(&self) -> &ValueType {
        &self.value_type
    }

    /// Literal used to initialize this value, not an untyped numerical projection.
    #[must_use]
    pub const fn literal(&self) -> f64 {
        self.literal
    }

    /// Extract a numerical quantity only for an invariant real scalar.
    #[must_use]
    pub const fn real_scalar_value(&self) -> Option<DynQuantity> {
        if matches!(self.value_type.scalar_domain(), ScalarDomain::Real)
            && self.value_type.shape().is_scalar()
        {
            Some(DynQuantity::new(self.literal, self.value_type.dimension()))
        } else {
            None
        }
    }
}

impl TryFrom<DynQuantity> for ValueLiteral {
    type Error = InvalidValueLiteral;

    fn try_from(value: DynQuantity) -> Result<Self, Self::Error> {
        Self::new(
            ValueType::scalar(ScalarDomain::Real, value.dim()),
            value.value(),
        )
    }
}

/// A literal cannot initialize the requested mathematical type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidValueLiteral {
    /// Mathematical literals must be finite.
    NonFinite,
    /// A shaped value needs contextual zero rather than scalar broadcasting.
    NonzeroShape,
}

impl core::fmt::Display for InvalidValueLiteral {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::NonFinite => "mathematical literal must be finite",
            Self::NonzeroShape => "a shaped value requires a contextual zero literal",
        })
    }
}

impl std::error::Error for InvalidValueLiteral {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DimExponents;

    #[test]
    fn literals_preserve_type_and_only_real_scalars_extract_as_quantities() {
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        let complex = ValueType::scalar(ScalarDomain::Complex, real.dimension());
        let scalar = ValueLiteral::new(real.clone(), 2.0).unwrap();
        assert_eq!(
            scalar.real_scalar_value(),
            Some(DynQuantity::new(2.0, real.dimension()))
        );
        let embedded = ValueLiteral::new(complex.clone(), 2.0).unwrap();
        assert_eq!(embedded.value_type(), &complex);
        assert_eq!(embedded.literal(), 2.0);
        assert_eq!(embedded.real_scalar_value(), None);
        for value_type in [complex, real.array(3).unwrap()] {
            let zero = ValueLiteral::new(value_type.clone(), -0.0).unwrap();
            assert_eq!(zero.value_type(), &value_type);
            assert_eq!(zero.literal().to_bits(), 0.0_f64.to_bits());
            assert_eq!(zero.real_scalar_value(), None);
        }
    }

    #[test]
    fn literals_reject_nonfinite_values_and_scalar_broadcasting() {
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        for literal in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                ValueLiteral::new(real.clone(), literal),
                Err(InvalidValueLiteral::NonFinite)
            );
        }
        assert_eq!(
            ValueLiteral::new(real.array(3).unwrap(), 1.0),
            Err(InvalidValueLiteral::NonzeroShape),
        );
    }
}
