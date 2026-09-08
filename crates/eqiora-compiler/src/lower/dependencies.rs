//! Original symbol dependencies of shared expression DAGs used by structural elaboration.
use super::{LoweringExpression, LoweringExpressionNode};
use std::collections::BTreeSet;

impl LoweringExpression {
    pub(crate) fn referenced_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            if !seen.insert(std::sync::Arc::as_ptr(&value.node) as usize) {
                continue;
            }
            match value.node.as_ref() {
                LoweringExpressionNode::Name(name) => {
                    names.insert(name.clone());
                }
                LoweringExpressionNode::Not(value)
                | LoweringExpressionNode::Neg(value)
                | LoweringExpressionNode::Index { value, .. }
                | LoweringExpressionNode::Call {
                    argument: value, ..
                }
                | LoweringExpressionNode::Sample { value, .. } => pending.push(value),
                LoweringExpressionNode::Array(values)
                | LoweringExpressionNode::IntegerCall {
                    arguments: values, ..
                }
                | LoweringExpressionNode::PureOperator {
                    arguments: values, ..
                } => pending.extend(values),
                LoweringExpressionNode::Binary { left, right, .. }
                | LoweringExpressionNode::Extremum { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                LoweringExpressionNode::Complex { real, imag } => {
                    pending.push(real);
                    pending.push(imag);
                }
                _ => {}
            }
        }
        names
    }
}
