//! Fixed nonsmooth recipes shared by the existing expression owners.
use eqiora_core::DimExponents;
use eqiora_schema::kernel::ComparisonOp;

pub(crate) enum Primitive<N> {
    Constant(i8, DimExponents),
    Neg(N),
    Compare(ComparisonOp, N, N),
    Select {
        condition: N,
        then_value: N,
        else_value: N,
    },
    Require {
        condition: N,
        value: N,
    },
}

pub(crate) fn arity(name: &str) -> Option<usize> {
    match name {
        "math.abs" | "math.sign" | "math.step" => Some(1),
        "math.min" | "math.max" => Some(2),
        "math.clamp" => Some(3),
        _ => None,
    }
}

/// At most eight primitive nodes; operands are reused, never substituted as trees.
pub(crate) fn emit<N: Clone, E>(
    name: &str,
    operands: &[N],
    dimension: DimExponents,
    mut append: impl FnMut(Primitive<N>) -> Result<N, E>,
) -> Option<Result<N, E>> {
    if arity(name)? != operands.len() {
        return None;
    }
    Some((|| {
        let x = operands[0].clone();
        match name {
            "math.min" | "math.max" => {
                let y = operands[1].clone();
                let comparison = if name == "math.min" {
                    ComparisonOp::LessEqual
                } else {
                    ComparisonOp::GreaterEqual
                };
                let condition = append(Primitive::Compare(comparison, x.clone(), y.clone()))?;
                append(Primitive::Select {
                    condition,
                    then_value: x,
                    else_value: y,
                })
            }
            "math.clamp" => {
                let lower = operands[1].clone();
                let upper = operands[2].clone();
                let valid = append(Primitive::Compare(
                    ComparisonOp::LessEqual,
                    lower.clone(),
                    upper.clone(),
                ))?;
                let above = append(Primitive::Compare(
                    ComparisonOp::Greater,
                    x.clone(),
                    upper.clone(),
                ))?;
                let high = append(Primitive::Select {
                    condition: above,
                    then_value: upper,
                    else_value: x.clone(),
                })?;
                let below = append(Primitive::Compare(ComparisonOp::Less, x, lower.clone()))?;
                let value = append(Primitive::Select {
                    condition: below,
                    then_value: lower,
                    else_value: high,
                })?;
                append(Primitive::Require {
                    condition: valid,
                    value,
                })
            }
            "math.abs" => {
                let zero = append(Primitive::Constant(0, dimension))?;
                let condition = append(Primitive::Compare(
                    ComparisonOp::GreaterEqual,
                    x.clone(),
                    zero,
                ))?;
                let negative = append(Primitive::Neg(x.clone()))?;
                append(Primitive::Select {
                    condition,
                    then_value: x,
                    else_value: negative,
                })
            }
            "math.sign" => {
                let zero = append(Primitive::Constant(0, dimension))?;
                let negative = append(Primitive::Compare(
                    ComparisonOp::Less,
                    x.clone(),
                    zero.clone(),
                ))?;
                let positive = append(Primitive::Compare(ComparisonOp::Greater, x, zero))?;
                let minus = append(Primitive::Constant(-1, DimExponents::DIMENSIONLESS))?;
                let plus = append(Primitive::Constant(1, DimExponents::DIMENSIONLESS))?;
                let zero = append(Primitive::Constant(0, DimExponents::DIMENSIONLESS))?;
                let nonnegative = append(Primitive::Select {
                    condition: positive,
                    then_value: plus,
                    else_value: zero,
                })?;
                append(Primitive::Select {
                    condition: negative,
                    then_value: minus,
                    else_value: nonnegative,
                })
            }
            "math.step" => {
                let zero = append(Primitive::Constant(0, dimension))?;
                let condition = append(Primitive::Compare(ComparisonOp::GreaterEqual, x, zero))?;
                let plus = append(Primitive::Constant(1, DimExponents::DIMENSIONLESS))?;
                let zero = append(Primitive::Constant(0, DimExponents::DIMENSIONLESS))?;
                append(Primitive::Select {
                    condition,
                    then_value: plus,
                    else_value: zero,
                })
            }
            _ => unreachable!("closed arity vocabulary"),
        }
    })())
}
