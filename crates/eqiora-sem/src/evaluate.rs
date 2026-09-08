//! Scalar expression-DAG evaluation for one explicit semantic context.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{
    Diagnostic, DynQuantity, GraphPath, RawId, ScalarDomain, ValueLiteral, ValueType,
};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};

use crate::{ExpressionBackend, KernelProgram, PhysicalUnknown};

pub(crate) struct ReferenceExpressionBackend;

impl ExpressionBackend for ReferenceExpressionBackend {
    fn evaluate(
        &self,
        owner: RawId,
        expression: &ExprDag,
        roots: &[ExprId],
        resolve: &mut dyn FnMut(SymbolRef) -> Option<ValueLiteral>,
    ) -> Result<Vec<ValueLiteral>, Diagnostic> {
        evaluate_selected(owner, expression, roots, resolve)
    }
}

pub(crate) struct EvalContext<'a> {
    pub(crate) program: &'a KernelProgram,
    pub(crate) discrete_fields: &'a BTreeMap<RawId, ValueLiteral>,
    pub(crate) discrete_ports: &'a BTreeMap<RawId, ValueLiteral>,
    pub(crate) discrete_next: &'a BTreeMap<RawId, ValueLiteral>,
    pub(crate) time: f64,
    pub(crate) fields: &'a BTreeMap<RawId, f64>,
    pub(crate) field_candidates: &'a BTreeMap<RawId, f64>,
    pub(crate) derivatives: &'a BTreeMap<RawId, f64>,
    pub(crate) next_fields: &'a BTreeMap<RawId, f64>,
    pub(crate) ports: &'a BTreeMap<RawId, f64>,
    pub(crate) port_candidates: &'a BTreeMap<RawId, f64>,
    pub(crate) signal_sources: &'a BTreeMap<RawId, RawId>,
    pub(crate) physical: &'a BTreeMap<PhysicalUnknown, f64>,
    pub(crate) physical_candidates: &'a BTreeMap<PhysicalUnknown, f64>,
}

pub(crate) fn evaluate_expression(
    owner: RawId,
    expression: &ExprDag,
    resolve: &mut dyn FnMut(SymbolRef) -> Option<ValueLiteral>,
) -> Result<Vec<ValueLiteral>, Diagnostic> {
    evaluate_selected(owner, expression, expression.roots(), resolve)
}

fn evaluate_selected(
    owner: RawId,
    expression: &ExprDag,
    roots: &[ExprId],
    resolve: &mut dyn FnMut(SymbolRef) -> Option<ValueLiteral>,
) -> Result<Vec<ValueLiteral>, Diagnostic> {
    let mut required = vec![false; expression.nodes().len()];
    let mut pending = roots.to_vec();
    while let Some(id) = pending.pop() {
        let index = id.index() as usize;
        let Some(mark) = required.get_mut(index) else {
            return Err(Diagnostic::error(
                codes::INVALID_EXPRESSION_DAG,
                "requested expression root is unavailable",
            ));
        };
        if *mark {
            continue;
        }
        *mark = true;
        match &expression.nodes()[index] {
            ExprNode::Constant(_) | ExprNode::Symbol(_) => {}
            ExprNode::Sample { value, .. }
            | ExprNode::Hold(value)
            | ExprNode::Neg(value)
            | ExprNode::PowI(value, _)
            | ExprNode::ToReal(value)
            | ExprNode::ToInteger(value)
            | ExprNode::Ordinal(value) => pending.push(*value),
            ExprNode::Add(a, b)
            | ExprNode::Sub(a, b)
            | ExprNode::Mul(a, b)
            | ExprNode::Div(a, b)
            | ExprNode::Quotient(a, b)
            | ExprNode::Remainder(a, b) => pending.extend([*a, *b]),
            _ => {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "expression node is outside the reference execution profile",
                ));
            }
        }
    }
    let mut values = Vec::with_capacity(expression.nodes().len());
    for (index, node) in expression.nodes().iter().enumerate() {
        if !required[index] {
            values.push(None);
            continue;
        }
        let value = match node {
            ExprNode::Constant(value) => value.clone(),
            ExprNode::Symbol(symbol) => resolve(*symbol).ok_or_else(|| {
                Diagnostic::error(
                    codes::MISSING_EXECUTION_INPUT,
                    format!("no reference-execution value is available for {symbol:?}"),
                )
                .with_graph_path(expression_path(owner, index))
            })?,
            ExprNode::Sample { value, .. } | ExprNode::Hold(value) => {
                operand(&values, *value, owner)?.clone()
            }
            ExprNode::Neg(value) => {
                let value = operand(&values, *value, owner)?;
                if value.value_type().scalar_domain() == ScalarDomain::Integer {
                    value.checked_neg().map_err(discrete_error)?
                } else {
                    literal(-real(value)?)?
                }
            }
            ExprNode::Add(left, right) => {
                let left = operand(&values, *left, owner)?;
                let right = operand(&values, *right, owner)?;
                if left.value_type().scalar_domain() == ScalarDomain::Integer {
                    left.checked_add(right).map_err(discrete_error)?
                } else {
                    literal(real(left)?.try_add(real(right)?)?)?
                }
            }
            ExprNode::Sub(left, right) => {
                let left = operand(&values, *left, owner)?;
                let right = operand(&values, *right, owner)?;
                if left.value_type().scalar_domain() == ScalarDomain::Integer {
                    left.checked_sub(right).map_err(discrete_error)?
                } else {
                    literal(real(left)?.try_sub(real(right)?)?)?
                }
            }
            ExprNode::Mul(left, right) => {
                let left = operand(&values, *left, owner)?;
                let right = operand(&values, *right, owner)?;
                if left.value_type().scalar_domain() == ScalarDomain::Integer {
                    left.checked_mul(right).map_err(discrete_error)?
                } else {
                    literal(real(left)?.try_mul(real(right)?)?)?
                }
            }
            ExprNode::Div(left, right) => literal(
                real(operand(&values, *left, owner)?)?
                    .try_div(real(operand(&values, *right, owner)?)?)?,
            )?,
            ExprNode::Quotient(a, b) => operand(&values, *a, owner)?
                .checked_quotient(operand(&values, *b, owner)?)
                .map_err(discrete_error)?,
            ExprNode::Remainder(a, b) => operand(&values, *a, owner)?
                .checked_remainder(operand(&values, *b, owner)?)
                .map_err(discrete_error)?,
            ExprNode::ToReal(value) => operand(&values, *value, owner)?
                .to_real()
                .map_err(discrete_error)?,
            ExprNode::Ordinal(value) => operand(&values, *value, owner)?
                .ordinal()
                .map_err(discrete_error)?,
            ExprNode::ToInteger(value) => operand(&values, *value, owner)?
                .to_integer()
                .map_err(discrete_error)?,
            ExprNode::PowI(base, exponent) => {
                let base = real(operand(&values, *base, owner)?)?;
                let dimension = base.dim().pow(*exponent, 1).ok_or_else(|| {
                    Diagnostic::error(codes::DIMENSION_MISMATCH, "power dimension exceeds bounds")
                })?;
                literal(DynQuantity::new(base.value().powi(*exponent), dimension))?
            }
            _ => {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "expression node is newer than this reference interpreter",
                )
                .with_graph_path(expression_path(owner, index)));
            }
        };
        values.push(Some(value));
    }

    roots
        .iter()
        .map(|root| operand(&values, *root, owner).cloned())
        .collect()
}

pub(crate) fn resolve_symbol(symbol: SymbolRef, context: &EvalContext<'_>) -> Option<ValueLiteral> {
    if let SymbolRef::Parameter(id) = symbol {
        return context.program.typed_value(id.erase()).cloned();
    }
    let discrete = match symbol {
        SymbolRef::Field(id) | SymbolRef::Pre(id) => context.discrete_fields.get(&id.erase()),
        SymbolRef::Next(id) => context.discrete_next.get(&id.erase()),
        SymbolRef::Port(id) => context.discrete_ports.get(
            context
                .signal_sources
                .get(&id.erase())
                .unwrap_or(&id.erase()),
        ),
        _ => None,
    };
    if let Some(value) = discrete {
        return Some(value.clone());
    }
    let value = match symbol {
        SymbolRef::Field(id) => context
            .field_candidates
            .get(&id.erase())
            .or_else(|| context.fields.get(&id.erase()))
            .copied(),
        SymbolRef::Derivative(id) => context.derivatives.get(&id.erase()).copied(),
        SymbolRef::Pre(id) => context.fields.get(&id.erase()).copied(),
        SymbolRef::Next(id) => context.next_fields.get(&id.erase()).copied(),
        SymbolRef::Parameter(id) => context.program.value(id.erase()).map(|value| value.value()),
        SymbolRef::Port(id) => {
            let port = context
                .signal_sources
                .get(&id.erase())
                .copied()
                .unwrap_or_else(|| id.erase());
            context
                .port_candidates
                .get(&port)
                .or_else(|| context.ports.get(&port))
                .copied()
        }
        SymbolRef::Across(id) => resolve_physical(PhysicalUnknown::Across(id), context),
        SymbolRef::Through(id) => resolve_physical(PhysicalUnknown::Through(id), context),
        SymbolRef::Time => Some(context.time),
        _ => None,
    }?;
    ValueLiteral::from_real(context.program.execution_symbol_type(symbol)?, value).ok()
}

fn resolve_physical(unknown: PhysicalUnknown, context: &EvalContext<'_>) -> Option<f64> {
    context
        .physical_candidates
        .get(&unknown)
        .or_else(|| context.physical.get(&unknown))
        .copied()
}

fn operand(
    values: &[Option<ValueLiteral>],
    id: ExprId,
    owner: RawId,
) -> Result<&ValueLiteral, Diagnostic> {
    usize::try_from(id.index())
        .ok()
        .and_then(|index| values.get(index))
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_EXPRESSION_DAG,
                format!("expression operand {} is unavailable", id.index()),
            )
            .with_graph_path(expression_path(
                owner,
                usize::try_from(id.index()).unwrap_or(usize::MAX),
            ))
        })
}

fn discrete_error(error: eqiora_core::InvalidValueLiteral) -> Diagnostic {
    Diagnostic::error(
        codes::NOT_IMPLEMENTED,
        format!("exact discrete operation rejected: {error:?}"),
    )
}

pub(crate) fn real(value: &ValueLiteral) -> Result<DynQuantity, Diagnostic> {
    value.real_scalar_value().ok_or_else(|| {
        Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "numerical reference execution requires a real scalar value",
        )
    })
}

pub(crate) fn literal(value: DynQuantity) -> Result<ValueLiteral, Diagnostic> {
    ValueLiteral::from_real(
        ValueType::scalar(ScalarDomain::Real, value.dim()),
        value.value(),
    )
    .map_err(|_| {
        Diagnostic::error(
            codes::NONFINITE_EVALUATION,
            "reference expression produced a nonfinite value",
        )
    })
}

pub(crate) fn real_values(values: Vec<ValueLiteral>) -> Result<Vec<f64>, Diagnostic> {
    values
        .iter()
        .map(|value| real(value).map(|value| value.value()))
        .collect()
}

fn expression_path(owner: RawId, index: usize) -> GraphPath {
    GraphPath::new([
        "semantic".to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
        "expression".to_owned(),
        index.to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, Id, ScalarDomain, ValueLiteral, ValueType, entity::kinds};
    use eqiora_schema::kernel::ExprDagBuilder;

    #[test]
    fn reference_evaluation_does_not_narrow_typed_constants() {
        let owner = Id::<kinds::Relation>::new().erase();
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        for value_type in [
            real.clone(),
            ValueType::scalar(ScalarDomain::Complex, real.dimension()),
            real.array(3).unwrap(),
        ] {
            let is_real_scalar =
                value_type.scalar_domain() == ScalarDomain::Real && value_type.shape().is_scalar();
            let mut builder = ExprDagBuilder::new();
            let value = ValueLiteral::from_real(value_type, 0.0).unwrap();
            let root = builder.constant(value.clone()).unwrap();
            let result =
                evaluate_expression(owner, &builder.finish([root]).unwrap(), &mut |_| None)
                    .unwrap();
            assert_eq!(result, vec![value]);
            assert_eq!(real_values(result).is_ok(), is_real_scalar);
        }
    }
    #[test]
    fn exact_integer_operations_and_checked_conversion_use_one_backend() {
        let owner = Id::<kinds::Relation>::new().erase();
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
        let values = evaluate_expression(owner, &dag, &mut |_| None).unwrap();
        assert_eq!(values[0].integer_scalar_value(), Some(-2));
        assert_eq!(values[1].integer_scalar_value(), Some(-1));
        assert_eq!(values[2].real_scalar_value().unwrap().value(), -2.);
        assert_eq!(values[3].integer_scalar_value(), Some(-2));
        assert_eq!(values[4].integer_scalar_value(), Some(2));
        assert!(real_values(values).is_err());
        let mut dag = ExprDagBuilder::new();
        let a = dag
            .constant(DynQuantity::new(0.5, DimExponents::DIMENSIONLESS))
            .unwrap();
        let converted = dag.to_integer(a).unwrap();
        assert!(
            evaluate_expression(owner, &dag.finish([converted]).unwrap(), &mut |_| None).is_err()
        );
    }
}
