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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{ExprDagBuilder, ExprNode};
    use eqiora_core::{Id, ValueLiteral, ValueType};

    fn typed(value: ValueType) -> ExpressionType<u32> {
        ExpressionType::new(value, None)
    }

    #[test]
    fn ordered_selection_retains_dimension_and_exact_support_without_promotion() {
        let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap();
        let real = ValueType::scalar(ScalarDomain::Real, length);
        let support = SpatialSupport::Volume {
            domain: 7,
            dimensions: 2,
        };
        let spatial = ExpressionType::new(real.clone(), Some(support.clone()));
        assert_eq!(
            typed(real.clone())
                .ordered_selection(spatial.clone())
                .unwrap(),
            spatial
        );
        assert_eq!(
            spatial
                .clone()
                .ordered_selection(typed(real.clone()))
                .unwrap()
                .support,
            Some(support)
        );
        let foreign = ExpressionType::new(
            real.clone(),
            Some(SpatialSupport::Volume {
                domain: 8,
                dimensions: 2,
            }),
        );
        assert!(spatial.ordered_selection(foreign).is_err());
        assert!(
            typed(real)
                .ordered_selection(typed(ValueType::scalar(
                    ScalarDomain::Real,
                    DimExponents::DIMENSIONLESS
                )))
                .is_err()
        );
        let integer = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        assert_eq!(
            typed(integer.clone())
                .ordered_selection(typed(integer.clone()))
                .unwrap(),
            typed(integer.clone())
        );
        assert!(
            typed(integer)
                .ordered_selection(typed(ValueType::scalar(
                    ScalarDomain::Real,
                    DimExponents::DIMENSIONLESS
                )))
                .is_err()
        );
    }

    #[test]
    fn ordered_selection_rejects_unordered_and_shaped_types() {
        let integer = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        for value in [
            ValueType::boolean(),
            ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS),
            ValueType::index(Id::new(), 2).unwrap(),
            ValueType::counts(Id::new(), 2).unwrap(),
            ValueType::coordinates(Id::new(), 2).unwrap(),
            real.array(2).unwrap(),
            integer.clone().array(2).unwrap(),
            integer.with_dimension(DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap()),
        ] {
            assert!(
                typed(value.clone())
                    .ordered_selection(typed(value))
                    .is_err()
            );
        }
    }

    #[test]
    fn ordered_selection_dag_retains_both_ordered_operands_and_checks_rhs() {
        let integer = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let mut builder = ExprDagBuilder::new();
        let left = builder
            .constant(ValueLiteral::from_integer(integer.clone(), 9_007_199_254_740_993).unwrap())
            .unwrap();
        let right = builder
            .constant(ValueLiteral::from_integer(integer.clone(), 9_007_199_254_740_992).unwrap())
            .unwrap();
        let minimum = builder.min(left, right).unwrap();
        let maximum = builder.max(right, left).unwrap();
        let dag = builder.finish([minimum, left, maximum, right]).unwrap();
        assert_eq!(dag.node(minimum), Some(&ExprNode::Min(left, right)));
        assert_eq!(dag.node(maximum), Some(&ExprNode::Max(right, left)));
        let inferred = TypedResidual::<u32>::infer(dag, None, RootContract::EquationSides, |_| {
            Err::<ExpressionType<u32>, ()>(())
        })
        .unwrap();
        assert_eq!(inferred.node_type(minimum), Some(&typed(integer)));

        let mut missing = ExprDagBuilder::new();
        let existing = missing.constant(ValueLiteral::boolean(false)).unwrap();
        assert!(missing.min(existing, right).is_err());
        assert!(missing.max(right, existing).is_err());

        for minimum in [true, false] {
            let mut incompatible = ExprDagBuilder::new();
            let left = incompatible
                .constant(
                    ValueLiteral::from_real(
                        ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
                        -1.0,
                    )
                    .unwrap(),
                )
                .unwrap();
            let right = incompatible.constant(ValueLiteral::boolean(false)).unwrap();
            let mut builder = incompatible;
            let root = if minimum {
                builder.min(left, right)
            } else {
                builder.max(left, right)
            }
            .unwrap();
            assert!(
                TypedResidual::<u32>::infer(
                    builder.finish([root, left]).unwrap(),
                    None,
                    RootContract::EquationSides,
                    |_| Err::<ExpressionType<u32>, ()>(())
                )
                .is_err()
            );
        }
    }
}
