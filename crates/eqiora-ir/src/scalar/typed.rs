//! Typed execution of the existing scalar SSA instruction stream.
use super::*;
use eqiora_core::{DynQuantity, ScalarDomain, ValueLiteral, ValueType};

impl ScalarOperatorIr {
    /// Evaluate requested original DAG roots without projecting exact discrete values
    /// through floating point. Unreachable symbol slots are never resolved.
    ///
    /// # Errors
    /// Rejects missing operands, incompatible values, and checked arithmetic failures.
    pub fn evaluate_typed(
        &self,
        roots: &[eqiora_schema::kernel::ExprId],
        resolve: &mut dyn FnMut(SymbolRef) -> Option<ValueLiteral>,
    ) -> Result<Vec<ValueLiteral>, Diagnostic> {
        let roots = roots
            .iter()
            .map(|id| {
                self.source_values
                    .get(id.index() as usize)
                    .copied()
                    .ok_or_else(|| ir_builder_error("typed source root is unavailable"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        enum Frame {
            Demand(ValueId),
            Apply(ValueId),
            Logical(ValueId),
        }
        let mut values: Vec<Option<ValueLiteral>> = vec![None; self.instructions.len()];
        for &root in &roots {
            let mut pending = vec![Frame::Demand(root)];
            while let Some(frame) = pending.pop() {
                let id = match frame {
                    Frame::Demand(id) | Frame::Apply(id) | Frame::Logical(id) => id,
                };
                let index = id.0 as usize;
                let instruction = *self
                    .instructions
                    .get(index)
                    .ok_or_else(|| ir_builder_error("typed operand is unavailable"))?;
                if values[index].is_some() {
                    continue;
                }
                if let Instruction::And(left, right) | Instruction::Or(left, right) = instruction {
                    if matches!(frame, Frame::Demand(_)) {
                        pending.push(Frame::Logical(id));
                        pending.push(Frame::Demand(left));
                        continue;
                    }
                    if matches!(frame, Frame::Logical(_)) {
                        let left =
                            boolean(values[left.0 as usize].as_ref().expect("demanded left"))?;
                        if matches!(instruction, Instruction::And(_, _)) && !left
                            || matches!(instruction, Instruction::Or(_, _)) && left
                        {
                            values[index] = Some(ValueLiteral::boolean(left));
                        } else {
                            pending.push(Frame::Apply(id));
                            pending.push(Frame::Demand(right));
                        }
                        continue;
                    }
                }
                if matches!(frame, Frame::Demand(_)) {
                    pending.push(Frame::Apply(id));
                    match instruction {
                        Instruction::And(_, _) | Instruction::Or(_, _) => {
                            unreachable!("logical demand is staged separately")
                        }
                        Instruction::Constant(_)
                        | Instruction::TypedConstant(_)
                        | Instruction::Read(_) => {}
                        Instruction::Neg(a)
                        | Instruction::PowI(a, _)
                        | Instruction::ToReal(a)
                        | Instruction::ToInteger(a)
                        | Instruction::Ordinal(a)
                        | Instruction::Not(a) => pending.push(Frame::Demand(a)),
                        Instruction::Compare(_, a, b)
                        | Instruction::Add(a, b)
                        | Instruction::Sub(a, b)
                        | Instruction::Mul(a, b)
                        | Instruction::Div(a, b)
                        | Instruction::Quotient(a, b)
                        | Instruction::Remainder(a, b) => {
                            pending.push(Frame::Demand(b));
                            pending.push(Frame::Demand(a));
                        }
                    }
                    continue;
                }
                let read = |id: ValueId| {
                    values
                        .get(id.0 as usize)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| ir_builder_error("typed scalar operand is unavailable"))
                };
                let value = match instruction {
                    Instruction::Not(value) => ValueLiteral::boolean(!boolean(read(value)?)?),
                    Instruction::And(_, right) | Instruction::Or(_, right) => {
                        ValueLiteral::boolean(boolean(read(right)?)?)
                    }
                    Instruction::Compare(op, left, right) => {
                        compare(op, read(left)?, read(right)?)?
                    }
                    Instruction::Constant(value) => literal(value)?,
                    Instruction::Read(slot) => self
                        .symbols
                        .get(slot_index(slot, index)?)
                        .and_then(|symbol| resolve(*symbol))
                        .ok_or_else(|| ir_builder_error("typed scalar input is unavailable"))?,
                    Instruction::TypedConstant(slot) => self
                        .typed_constants
                        .get(slot as usize)
                        .cloned()
                        .ok_or_else(|| ir_builder_error("typed constant is unavailable"))?,
                    Instruction::Quotient(a, b) => read(a)?
                        .checked_quotient(read(b)?)
                        .map_err(discrete_error)?,
                    Instruction::Remainder(a, b) => read(a)?
                        .checked_remainder(read(b)?)
                        .map_err(discrete_error)?,
                    Instruction::ToReal(a) => read(a)?.to_real().map_err(discrete_error)?,
                    Instruction::ToInteger(a) => read(a)?.to_integer().map_err(discrete_error)?,
                    Instruction::Ordinal(a) => read(a)?.ordinal().map_err(discrete_error)?,
                    Instruction::Neg(value) => {
                        let value = read(value)?;
                        if value.value_type().scalar_domain() == ScalarDomain::Integer {
                            value.checked_neg().map_err(discrete_error)?
                        } else {
                            literal(-real(value)?)?
                        }
                    }
                    Instruction::Add(left, right) => {
                        let left = read(left)?;
                        let right = read(right)?;
                        if left.value_type().scalar_domain() == ScalarDomain::Integer {
                            left.checked_add(right).map_err(discrete_error)?
                        } else {
                            literal(real(left)?.try_add(real(right)?)?)?
                        }
                    }
                    Instruction::Sub(left, right) => {
                        let left = read(left)?;
                        let right = read(right)?;
                        if left.value_type().scalar_domain() == ScalarDomain::Integer {
                            left.checked_sub(right).map_err(discrete_error)?
                        } else {
                            literal(real(left)?.try_sub(real(right)?)?)?
                        }
                    }
                    Instruction::Mul(left, right) => {
                        let left = read(left)?;
                        let right = read(right)?;
                        if left.value_type().scalar_domain() == ScalarDomain::Integer {
                            left.checked_mul(right).map_err(discrete_error)?
                        } else {
                            literal(real(left)?.try_mul(real(right)?)?)?
                        }
                    }
                    Instruction::Div(left, right) => {
                        literal(real(read(left)?)?.try_div(real(read(right)?)?)?)?
                    }
                    Instruction::PowI(base, exponent) => {
                        let base = real(read(base)?)?;
                        let dimension = base.dim().pow(exponent, 1).ok_or_else(|| {
                            ir_builder_error("typed power dimension exceeds bounds")
                        })?;
                        literal(DynQuantity::new(base.value().powi(exponent), dimension))?
                    }
                };
                values[index] = Some(value);
            }
        }
        roots
            .iter()
            .map(|root| {
                values
                    .get(root.0 as usize)
                    .and_then(Option::as_ref)
                    .cloned()
                    .ok_or_else(|| ir_builder_error("typed scalar root is unavailable"))
            })
            .collect()
    }
}

fn real(value: &ValueLiteral) -> Result<DynQuantity, Diagnostic> {
    value.real_scalar_value().ok_or_else(|| {
        Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "scalar IR numerical operation requires a real scalar",
        )
    })
}

fn literal(value: DynQuantity) -> Result<ValueLiteral, Diagnostic> {
    ValueLiteral::from_real(
        ValueType::scalar(ScalarDomain::Real, value.dim()),
        value.value(),
    )
    .map_err(|_| {
        Diagnostic::error(
            codes::NONFINITE_EVALUATION,
            "typed scalar operation produced a nonfinite value",
        )
    })
}

fn discrete_error(error: eqiora_core::InvalidValueLiteral) -> Diagnostic {
    ir_builder_error(format!("exact discrete operation rejected: {error:?}"))
}

fn boolean(value: &ValueLiteral) -> Result<bool, Diagnostic> {
    value.as_bool().ok_or_else(|| {
        Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "logical execution requires a scalar Boolean",
        )
    })
}

fn compare(
    op: eqiora_schema::kernel::ComparisonOp,
    left: &ValueLiteral,
    right: &ValueLiteral,
) -> Result<ValueLiteral, Diagnostic> {
    use eqiora_schema::kernel::ComparisonOp;
    use std::cmp::Ordering;
    let value = match op {
        ComparisonOp::Equal => left.checked_equal(right),
        ComparisonOp::NotEqual => left.checked_equal(right).map(|value| !value),
        ComparisonOp::Less => left
            .checked_order(right)
            .map(|value| value == Ordering::Less),
        ComparisonOp::LessEqual => left
            .checked_order(right)
            .map(|value| value != Ordering::Greater),
        ComparisonOp::Greater => left
            .checked_order(right)
            .map(|value| value == Ordering::Greater),
        ComparisonOp::GreaterEqual => left
            .checked_order(right)
            .map(|value| value != Ordering::Less),
    }
    .map_err(discrete_error)?;
    Ok(ValueLiteral::boolean(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, Id, entity::kinds};
    use eqiora_schema::kernel::ExprDagBuilder;

    #[test]
    fn logical_demand_skips_errors_but_does_not_poison_shared_roots() {
        use eqiora_schema::kernel::ComparisonOp;
        let owner = Id::<kinds::Relation>::new().erase();
        let gate = Id::<kinds::Parameter>::new();
        let integer = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let mut dag = ExprDagBuilder::new();
        let condition = dag.symbol(SymbolRef::Parameter(gate)).unwrap();
        let one = dag
            .constant(ValueLiteral::from_integer(integer.clone(), 1).unwrap())
            .unwrap();
        let zero = dag
            .constant(ValueLiteral::from_integer(integer, 0).unwrap())
            .unwrap();
        let bad = dag.quotient(one, zero).unwrap();
        let bad_predicate = dag.compare(ComparisonOp::Equal, bad, zero).unwrap();
        let conjunction = dag.and(condition, bad_predicate).unwrap();
        let disjunction = dag.or(condition, bad_predicate).unwrap();
        let negated = dag.not(condition).unwrap();
        let dag = dag.finish([conjunction, disjunction, negated]).unwrap();
        let ir = ScalarOperatorIr::lower(&dag).unwrap();
        let _ = owner;
        for (roots, gate_value, expected) in [
            (
                vec![conjunction, conjunction, negated],
                false,
                Some(vec![false, false, true]),
            ),
            (vec![disjunction], true, Some(vec![true])),
            (vec![conjunction, bad_predicate], false, None),
            (vec![conjunction], true, None),
            (vec![disjunction], false, None),
        ] {
            let mut reads = 0;
            let mut resolve = |symbol| {
                assert_eq!(symbol, SymbolRef::Parameter(gate));
                reads += 1;
                Some(ValueLiteral::boolean(gate_value))
            };
            let roots = roots.as_slice();
            let result = ir.evaluate_typed(roots, &mut resolve);
            assert_eq!(
                reads, 1,
                "one memoized node is read once within one demand context"
            );
            if let Some(expected) = expected {
                assert_eq!(
                    result
                        .unwrap()
                        .iter()
                        .map(|value| value.as_bool().unwrap())
                        .collect::<Vec<_>>(),
                    expected
                );
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn requested_integer_rhs_skips_unresolved_target_and_retains_low_bit() {
        let ty = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let mut dag = ExprDagBuilder::new();
        let target = dag
            .symbol(SymbolRef::Next(Id::<kinds::Field>::new()))
            .unwrap();
        let value = dag
            .constant(ValueLiteral::from_integer(ty.clone(), 9_007_199_254_740_993).unwrap())
            .unwrap();
        let one = dag
            .constant(ValueLiteral::from_integer(ty, 1).unwrap())
            .unwrap();
        let rhs = dag.add(value, one).unwrap();
        let residual = dag.sub(target, rhs).unwrap();
        let dag = dag.finish([residual]).unwrap();
        let ir = ScalarOperatorIr::lower(&dag).unwrap();
        let result = ir
            .evaluate_typed(&[rhs], &mut |_| {
                panic!("unused assignment target must not be resolved")
            })
            .unwrap();
        assert_eq!(
            result[0].integer_scalar_value(),
            Some(9_007_199_254_740_994)
        );
        assert!(ir.evaluate(&[0.]).is_err());
    }

    #[test]
    fn quotient_remainder_and_explicit_conversion_preserve_declared_semantics() {
        let ty = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let mut dag = ExprDagBuilder::new();
        let a = dag
            .constant(ValueLiteral::from_integer(ty.clone(), -7).unwrap())
            .unwrap();
        let b = dag
            .constant(ValueLiteral::from_integer(ty, 3).unwrap())
            .unwrap();
        let q = dag.quotient(a, b).unwrap();
        let r = dag.remainder(a, b).unwrap();
        let real = dag.to_real(q).unwrap();
        let integer = dag.to_integer(real).unwrap();
        let index_type = ValueType::index(Id::new(), 3).unwrap();
        let index = dag
            .constant(ValueLiteral::from_integer(index_type, 2).unwrap())
            .unwrap();
        let ordinal = dag.ordinal(index).unwrap();
        let dag = dag.finish([q, r, real, integer, ordinal]).unwrap();
        let ir = ScalarOperatorIr::lower(&dag).unwrap();
        let values = ir.evaluate_typed(dag.roots(), &mut |_| None).unwrap();
        assert_eq!(values[0].integer_scalar_value(), Some(-2));
        assert_eq!(values[1].integer_scalar_value(), Some(-1));
        assert_eq!(values[2].real_scalar_value().unwrap().value(), -2.);
        assert_eq!(values[3].integer_scalar_value(), Some(-2));
        assert_eq!(values[4].integer_scalar_value(), Some(2));
    }
}
