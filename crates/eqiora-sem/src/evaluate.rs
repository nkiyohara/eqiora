//! Scalar expression-DAG evaluation for one explicit semantic context.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath, RawId};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};

use crate::{ExpressionBackend, KernelProgram, PhysicalUnknown};

pub(crate) struct ReferenceExpressionBackend;

impl ExpressionBackend for ReferenceExpressionBackend {
    fn evaluate(
        &self,
        owner: RawId,
        expression: &ExprDag,
        resolve: &mut dyn FnMut(SymbolRef) -> Option<f64>,
    ) -> Result<Vec<f64>, Diagnostic> {
        evaluate_expression(owner, expression, resolve)
    }
}

pub(crate) struct EvalContext<'a> {
    pub(crate) program: &'a KernelProgram,
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
    resolve: &mut dyn FnMut(SymbolRef) -> Option<f64>,
) -> Result<Vec<f64>, Diagnostic> {
    let mut values = Vec::with_capacity(expression.nodes().len());
    for (index, node) in expression.nodes().iter().enumerate() {
        let value = match node {
            ExprNode::Constant(value) => value.value(),
            ExprNode::Symbol(symbol) => resolve(*symbol).ok_or_else(|| {
                Diagnostic::error(
                    codes::MISSING_EXECUTION_INPUT,
                    format!("no reference-execution value is available for {symbol:?}"),
                )
                .with_graph_path(expression_path(owner, index))
            })?,
            ExprNode::Neg(value) => -operand(&values, *value, owner)?,
            ExprNode::Add(left, right) => {
                operand(&values, *left, owner)? + operand(&values, *right, owner)?
            }
            ExprNode::Sub(left, right) => {
                operand(&values, *left, owner)? - operand(&values, *right, owner)?
            }
            ExprNode::Mul(left, right) => {
                operand(&values, *left, owner)? * operand(&values, *right, owner)?
            }
            ExprNode::Div(left, right) => {
                operand(&values, *left, owner)? / operand(&values, *right, owner)?
            }
            ExprNode::PowI(base, exponent) => operand(&values, *base, owner)?.powi(*exponent),
            _ => {
                return Err(Diagnostic::error(
                    codes::NOT_IMPLEMENTED,
                    "expression node is newer than this reference interpreter",
                )
                .with_graph_path(expression_path(owner, index)));
            }
        };
        if !value.is_finite() {
            return Err(Diagnostic::error(
                codes::NONFINITE_EVALUATION,
                format!("expression node {index} evaluated to {value}"),
            )
            .with_graph_path(expression_path(owner, index)));
        }
        values.push(value);
    }

    expression
        .roots()
        .iter()
        .map(|root| operand(&values, *root, owner))
        .collect()
}

pub(crate) fn resolve_symbol(symbol: SymbolRef, context: &EvalContext<'_>) -> Option<f64> {
    match symbol {
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
    }
}

fn resolve_physical(unknown: PhysicalUnknown, context: &EvalContext<'_>) -> Option<f64> {
    context
        .physical_candidates
        .get(&unknown)
        .or_else(|| context.physical.get(&unknown))
        .copied()
}

fn operand(values: &[f64], id: ExprId, owner: RawId) -> Result<f64, Diagnostic> {
    usize::try_from(id.index())
        .ok()
        .and_then(|index| values.get(index))
        .copied()
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

fn expression_path(owner: RawId, index: usize) -> GraphPath {
    GraphPath::new([
        "semantic".to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
        "expression".to_owned(),
        index.to_string(),
    ])
}
