//! Canonical integer call names mapped to shared checked value and type operations.
use eqiora_core::{ScalarDomain, ValueLiteral};
use eqiora_schema::kernel::typing::{ExpressionType, TypeViolation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerBuiltin {
    Quotient,
    Remainder,
    ToReal,
    ToInteger,
}
impl IntegerBuiltin {
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            "quotient" => Some(Self::Quotient),
            "remainder" => Some(Self::Remainder),
            "to_real" => Some(Self::ToReal),
            "to_integer" => Some(Self::ToInteger),
            _ => None,
        }
    }
    pub(crate) fn arity(self) -> usize {
        match self {
            Self::Quotient | Self::Remainder => 2,
            _ => 1,
        }
    }
    pub(crate) fn operand_domain(self) -> ScalarDomain {
        match self {
            Self::ToInteger => ScalarDomain::Real,
            _ => ScalarDomain::Integer,
        }
    }
    pub(crate) fn infer<I: Clone + Eq>(
        self,
        operands: &[ExpressionType<I>],
    ) -> Result<ExpressionType<I>, TypeViolation<I>> {
        match self {
            Self::Quotient | Self::Remainder => {
                operands[0].clone().integer_quotient(operands[1].clone())
            }
            Self::ToReal => operands[0].clone().to_real(),
            Self::ToInteger => operands[0].clone().to_integer(),
        }
    }
    pub(crate) fn evaluate(
        self,
        operands: &[ValueLiteral],
    ) -> Result<ValueLiteral, eqiora_core::InvalidValueLiteral> {
        match self {
            Self::Quotient => operands[0].checked_quotient(&operands[1]),
            Self::Remainder => operands[0].checked_remainder(&operands[1]),
            Self::ToReal => operands[0].to_real(),
            Self::ToInteger => operands[0].to_integer(),
        }
    }
}
