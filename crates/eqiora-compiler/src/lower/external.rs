//! Expression ownership used by the synthetic external root occurrence.

use std::sync::Arc;

use super::{LoweringExpression, LoweringExpressionNode};

impl LoweringExpression {
    pub(crate) fn detached_clone(&self) -> Self {
        let node = match self.node.as_ref() {
            LoweringExpressionNode::Quantity(value) => LoweringExpressionNode::Quantity(*value),
            LoweringExpressionNode::Name(name) => LoweringExpressionNode::Name(name.clone()),
            LoweringExpressionNode::Neg(value) => {
                LoweringExpressionNode::Neg(value.detached_clone())
            }
            LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            } => LoweringExpressionNode::Binary {
                operator: *operator,
                left: left.detached_clone(),
                right: right.detached_clone(),
            },
            LoweringExpressionNode::Call { callee, argument } => LoweringExpressionNode::Call {
                callee: callee.clone(),
                argument: argument.detached_clone(),
            },
            LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            } => LoweringExpressionNode::PureOperator {
                definition: definition.clone(),
                arguments: arguments
                    .iter()
                    .map(LoweringExpression::detached_clone)
                    .collect(),
            },
            LoweringExpressionNode::UnknownMath(path) => {
                LoweringExpressionNode::UnknownMath(path.clone())
            }
            LoweringExpressionNode::InvalidUnit(message) => {
                LoweringExpressionNode::InvalidUnit(message)
            }
            LoweringExpressionNode::Unsupported => LoweringExpressionNode::Unsupported,
        };
        Self {
            node: Arc::new(node),
            range: self.range,
        }
    }
}
