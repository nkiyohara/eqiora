//! Scalar logical values and domain-neutral equation-side compatibility.
use super::*;
use crate::kernel::ComparisonOp;
use eqiora_core::{ScalarDomain, ValueType};

impl<I: Clone + Eq> ExpressionType<I> {
    /// Validate both sides of an equation without performing arithmetic.
    pub fn equation(self, other: Self) -> Result<Self, TypeViolation<I>> {
        for value in [&self, &other] {
            if value.value_type.scalar_domain() == ScalarDomain::Boolean
                && value.value_type != ValueType::boolean()
            {
                return Err(TypeViolation::ScalarDomainMismatch);
            }
        }
        equation_compatible(&self, &other)
    }

    /// Type an exact scalar comparison, with no discrete-to-real promotion.
    pub fn compare(self, op: ComparisonOp, other: Self) -> Result<Self, TypeViolation<I>> {
        let equality = matches!(op, ComparisonOp::Equal | ComparisonOp::NotEqual);
        if !self.shape().is_scalar()
            || !other.shape().is_scalar()
            || self.frame() != ValueFrame::Invariant
            || other.frame() != ValueFrame::Invariant
        {
            return Err(TypeViolation::ScalarDomainMismatch);
        }
        let left = self.value_type.scalar_domain();
        let right = other.value_type.scalar_domain();
        if self.value_type.index_set().is_some() || other.value_type.index_set().is_some() {
            if !equality || self.value_type != other.value_type {
                return Err(TypeViolation::ScalarDomainMismatch);
            }
        } else if !equality
            && (left != ScalarDomain::Real && left != ScalarDomain::Integer || right != left)
        {
            return Err(TypeViolation::ScalarDomainMismatch);
        }
        let compatible = self.equation(other)?;
        Ok(Self::new(ValueType::boolean(), compatible.support))
    }

    /// Type Boolean negation.
    pub fn not(self) -> Result<Self, TypeViolation<I>> {
        if self.value_type != ValueType::boolean() {
            return Err(TypeViolation::ScalarDomainMismatch);
        }
        Ok(self)
    }
    /// Type Boolean conjunction.
    pub fn and(self, other: Self) -> Result<Self, TypeViolation<I>> {
        let left = self.not()?;
        let right = other.not()?;
        left.equation(right)
    }
    /// Type Boolean disjunction.
    pub fn or(self, other: Self) -> Result<Self, TypeViolation<I>> {
        self.and(other)
    }
}

pub(super) fn validate_equations<I: Clone + Eq, E>(
    expression: &ExprDag,
    inferred: &[Option<ExpressionType<I>>],
    relation_support: Option<&SpatialSupport<I>>,
    root_contract: RootContract,
    errors: &mut Vec<TypedResidualError<I, E>>,
) {
    if expression.roots().len() % 2 != 0 {
        errors.push(TypedResidualError::Type {
            node_index: expression.roots()[0].index(),
            error: TypeViolation::ScalarDomainMismatch,
        });
    } else {
        for pair in expression.roots().chunks_exact(2) {
            let Some(left) = inferred_type(&inferred, pair[0]) else {
                continue;
            };
            let Some(right) = inferred_type(&inferred, pair[1]) else {
                continue;
            };
            let result = left.equation(right).and_then(|value| {
                if matches!(root_contract, RootContract::EquationSides) {
                    residual(&value, relation_support)
                } else {
                    Ok(())
                }
            });
            if let Err(error) = result {
                errors.push(TypedResidualError::Type {
                    node_index: pair[0].index(),
                    error,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{ExprDagBuilder, RelationDef};
    use eqiora_core::{Id, ValueLiteral, entity::kinds};

    fn ty(value: ValueType) -> ExpressionType<u32> {
        ExpressionType::new(value, None)
    }

    #[test]
    fn comparison_domains_do_not_coerce_discrete_values() {
        let integer = ty(ValueType::scalar(
            ScalarDomain::Integer,
            DimExponents::DIMENSIONLESS,
        ));
        let real = ty(ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ));
        let complex = ty(ValueType::scalar(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
        ));
        let boolean = ty(ValueType::boolean());
        assert_eq!(
            integer
                .clone()
                .compare(ComparisonOp::Less, integer.clone())
                .unwrap(),
            boolean
        );
        assert!(integer.compare(ComparisonOp::Equal, real.clone()).is_err());
        assert!(
            real.clone()
                .compare(ComparisonOp::Equal, complex.clone())
                .is_ok()
        );
        assert!(
            complex
                .clone()
                .compare(ComparisonOp::Less, complex)
                .is_err()
        );
        assert!(
            boolean
                .clone()
                .compare(ComparisonOp::Equal, boolean.clone())
                .is_ok()
        );
        assert!(
            boolean
                .clone()
                .compare(ComparisonOp::Less, boolean.clone())
                .is_err()
        );
        assert!(boolean.clone().and(real.clone()).is_err());
        assert!(boolean.clone().sum(boolean.clone()).is_err());
        assert!(additive(&boolean, &boolean).is_err());
        assert!(multiply(&boolean, &boolean).is_err());
        assert!(divide(&boolean, &boolean).is_err());
        assert!(power(&boolean, 0).is_err());
        assert!(time_derivative(&boolean).is_err());
        let array = ty(real.value_type.array(2).unwrap());
        assert!(array.clone().compare(ComparisonOp::Equal, array).is_err());
        let set = Id::<kinds::IndexSet>::new();
        let index = ty(ValueType::index(set, 3).unwrap());
        assert!(
            index
                .clone()
                .compare(ComparisonOp::Equal, index.clone())
                .is_ok()
        );
        assert!(
            index
                .clone()
                .compare(ComparisonOp::Less, index.clone())
                .is_err()
        );
        assert!(
            index
                .compare(
                    ComparisonOp::Equal,
                    ty(ValueType::index(Id::new(), 3).unwrap())
                )
                .is_err()
        );
    }

    #[test]
    fn equation_pairs_accept_boolean_and_reject_malformed_or_foreign_support() {
        let mut builder = ExprDagBuilder::new();
        let left = builder.constant(ValueLiteral::boolean(true)).unwrap();
        let right = builder.constant(ValueLiteral::boolean(false)).unwrap();
        let relation = RelationDef::new(Id::new(), builder.finish([left, right]).unwrap()).unwrap();
        assert_eq!(
            relation.equation_sides().collect::<Vec<_>>(),
            vec![(left, right)]
        );
        TypedResidual::<u32>::infer(
            relation.expression().clone(),
            None,
            RootContract::EquationSides,
            |_| Err::<ExpressionType<u32>, ()>(()),
        )
        .unwrap();
        let mut builder = ExprDagBuilder::new();
        let root = builder.constant(ValueLiteral::boolean(false)).unwrap();
        assert!(RelationDef::new(Id::new(), builder.finish([root]).unwrap()).is_err());
        let spatial = |domain| {
            ExpressionType::new(
                ValueType::boolean(),
                Some(SpatialSupport::Volume {
                    domain,
                    dimensions: 1,
                }),
            )
        };
        assert!(ty(ValueType::boolean()).and(spatial(1)).is_ok());
        assert!(spatial(1).or(spatial(2)).is_err());
        assert!(spatial(1).equation(spatial(2)).is_err());
    }
}
