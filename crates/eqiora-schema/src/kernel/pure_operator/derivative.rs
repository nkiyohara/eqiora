//! Exact ordered formal differentiation, shared by source compilation and IR.
use super::*;
use eqiora_core::ScalarDomain;

impl CalculusBuilder {
    /// Append the first partial of an explicit real scalar polynomial with
    /// respect to one declared formal. All other formals are held fixed.
    ///
    /// The original input occurrences and product-rule order are retained.
    /// A constant result produces zero with the exact quotient dimension.
    /// Explicit validity requirements retain their original predicate, even
    /// when the derivative value is zero; predicates are not differentiated.
    /// The append is atomic and uses the existing calculus node/depth bounds.
    ///
    /// # Errors
    /// Rejects invalid nodes/formals, unconstrained or non-real scalar input
    /// types, unsupported derivative rules, and unrepresentable dimensions.
    pub fn partial(
        &mut self,
        root: CalculusNodeId,
        formal: u16,
    ) -> Result<CalculusNodeId, PureOperatorError> {
        let selected = self
            .formals
            .get(usize::from(formal))
            .ok_or(PureOperatorError::InvalidFormal(formal))?;
        if self.formals.iter().any(|value| {
            !value.is_invariant_scalar()
                || value.scalar_domain() != Some(ScalarDomain::Real)
                || value.dimension().is_none()
        }) {
            return Err(PureOperatorError::FormalTypeMismatch);
        }
        let output = self.value_type(root)?;
        if output.scalar_domain() != ScalarDomain::Real || !output.shape().is_scalar() {
            return Err(PureOperatorError::FormalTypeMismatch);
        }
        let dimension = output
            .dimension()
            .div(selected.dimension().unwrap())
            .ok_or(PureOperatorError::ResultDimensionOverflow)?;
        let root_index = definition_index(root, self.nodes.len())?;
        let mut reachable = vec![false; root_index + 1];
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let index = definition_index(id, reachable.len())?;
            if std::mem::replace(&mut reachable[index], true) {
                continue;
            }
            // A validity predicate is retained verbatim, not differentiated.
            match &self.nodes[index] {
                CalculusNode::Require { value, .. } => pending.push(*value),
                node => pending.extend(node.operands()),
            }
        }
        let mut staged = Self {
            formals: self.formals.clone(),
            result: self.result,
            nodes: self.nodes.clone(),
            depths: self.depths.clone(),
        };
        let mut derivatives = vec![None; root_index + 1];
        for (index, node) in self.nodes.iter().take(root_index + 1).enumerate() {
            if !reachable[index] {
                continue;
            }
            let derivative = |id: CalculusNodeId| derivatives[id.index() as usize];
            derivatives[index] = match node {
                CalculusNode::Rational { .. } => None,
                CalculusNode::Boolean(_)
                | CalculusNode::Compare(..)
                | CalculusNode::Not(_)
                | CalculusNode::And(..)
                | CalculusNode::Or(..) => None,
                CalculusNode::Require { condition, value } => {
                    let value = match derivative(*value) {
                        Some(value) => value,
                        None => {
                            let dimension = self
                                .value_type(*value)?
                                .dimension()
                                .div(selected.dimension().unwrap())
                                .ok_or(PureOperatorError::ResultDimensionOverflow)?;
                            staged.push(CalculusNode::Rational {
                                value: ExactRational::integer(0),
                                dimension,
                            })?
                        }
                    };
                    Some(staged.push(CalculusNode::Require {
                        condition: *condition,
                        value,
                    })?)
                }
                CalculusNode::FormalComponent {
                    formal: input,
                    axes,
                } if axes.is_empty() => {
                    if *input == formal {
                        Some(staged.push(CalculusNode::Rational {
                            value: ExactRational::integer(1),
                            dimension: DimExponents::DIMENSIONLESS,
                        })?)
                    } else {
                        None
                    }
                }
                CalculusNode::Neg(value) => derivative(*value)
                    .map(|value| staged.push(CalculusNode::Neg(value)))
                    .transpose()?,
                CalculusNode::Add(left, right) => {
                    sum(&mut staged, derivative(*left), derivative(*right))?
                }
                CalculusNode::Mul(left, right) => {
                    let first = derivative(*left)
                        .map(|value| staged.push(CalculusNode::Mul(value, *right)))
                        .transpose()?;
                    let second = derivative(*right)
                        .map(|value| staged.push(CalculusNode::Mul(*left, value)))
                        .transpose()?;
                    sum(&mut staged, first, second)?
                }
                _ => return Err(PureOperatorError::FormalTypeMismatch),
            };
        }
        let result = match derivatives[root_index] {
            Some(value) => value,
            None => staged.push(CalculusNode::Rational {
                value: ExactRational::integer(0),
                dimension,
            })?,
        };
        *self = staged;
        Ok(result)
    }
}

fn sum(
    builder: &mut CalculusBuilder,
    left: Option<CalculusNodeId>,
    right: Option<CalculusNodeId>,
) -> Result<Option<CalculusNodeId>, PureOperatorError> {
    match (left, right) {
        (Some(left), Some(right)) => builder.push(CalculusNode::Add(left, right)).map(Some),
        (Some(value), None) | (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_retains_validity_for_both_variable_and_typed_zero_derivatives() {
        let t = DimExponents::from_integers([0, 0, 0, 0, 1, 0, 0]).unwrap();
        let class = |d| {
            PureValueClass::invariant_scalar()
                .with_dimension(d)
                .with_scalar_domain(ScalarDomain::Real)
                .unwrap()
        };
        let mut builder = CalculusBuilder::new([class(t), class(t)], class(t)).unwrap();
        let x = builder
            .push(CalculusNode::FormalComponent {
                formal: 0,
                axes: Box::new([]),
            })
            .unwrap();
        let zero = builder
            .push(CalculusNode::Rational {
                value: ExactRational::integer(0),
                dimension: t,
            })
            .unwrap();
        let predicate = builder
            .push(CalculusNode::Compare(
                crate::kernel::ComparisonOp::Greater,
                x,
                zero,
            ))
            .unwrap();
        let root = builder
            .push(CalculusNode::Require {
                condition: predicate,
                value: x,
            })
            .unwrap();
        for formal in [0, 1] {
            let derived = builder.partial(root, formal).unwrap();
            let CalculusNode::Require { condition, value } =
                builder.nodes[derived.index() as usize]
            else {
                panic!("guard retained")
            };
            assert_eq!(condition, predicate);
            assert!(
                matches!(builder.nodes[value.index() as usize], CalculusNode::Rational { value, dimension }
                if value == ExactRational::integer(if formal == 0 { 1 } else { 0 }) && dimension == DimExponents::DIMENSIONLESS)
            );
        }
    }
}
