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
        let mut required = vec![false; self.instructions.len()];
        let mut pending = roots.clone();
        while let Some(id) = pending.pop() {
            let mark = required
                .get_mut(id.0 as usize)
                .ok_or_else(|| ir_builder_error("typed operand is unavailable"))?;
            if *mark {
                continue;
            }
            *mark = true;
            match self.instructions[id.0 as usize] {
                Instruction::Constant(_) | Instruction::TypedConstant(_) | Instruction::Read(_) => {
                }
                Instruction::Neg(a)
                | Instruction::PowI(a, _)
                | Instruction::ToReal(a)
                | Instruction::ToInteger(a)
                | Instruction::Ordinal(a) => pending.push(a),
                Instruction::Add(a, b)
                | Instruction::Sub(a, b)
                | Instruction::Mul(a, b)
                | Instruction::Div(a, b)
                | Instruction::Quotient(a, b)
                | Instruction::Remainder(a, b) => pending.extend([a, b]),
            }
        }
        let mut values: Vec<Option<ValueLiteral>> = Vec::with_capacity(self.instructions.len());
        for (index, instruction) in self.instructions.iter().copied().enumerate() {
            if !required[index] {
                values.push(None);
                continue;
            }
            let read = |id: ValueId| {
                values
                    .get(id.0 as usize)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| ir_builder_error("typed scalar operand is unavailable"))
            };
            let value = match instruction {
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
                    let dimension = base
                        .dim()
                        .pow(exponent, 1)
                        .ok_or_else(|| ir_builder_error("typed power dimension exceeds bounds"))?;
                    literal(DynQuantity::new(base.value().powi(exponent), dimension))?
                }
            };
            values.push(Some(value));
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

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, Id, entity::kinds};
    use eqiora_schema::kernel::ExprDagBuilder;

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
