//! Expression ownership used by the synthetic external root occurrence.

use std::sync::Arc;

use super::{LoweringExpression, LoweringExpressionNode};

impl LoweringExpression {
    pub(crate) fn detached_clone(&self) -> Self {
        self.clone_shared(&mut std::collections::HashMap::new())
    }

    fn clone_shared(&self, cache: &mut std::collections::HashMap<usize, Self>) -> Self {
        let key = Arc::as_ptr(&self.node) as usize;
        if let Some(value) = cache.get(&key) {
            return value.clone();
        }
        let node = match self.node.as_ref() {
            LoweringExpressionNode::Number(value) => LoweringExpressionNode::Number(value.clone()),
            LoweringExpressionNode::Literal(value) => {
                LoweringExpressionNode::Literal(value.clone())
            }
            LoweringExpressionNode::IntegerCall {
                operator,
                arguments,
            } => LoweringExpressionNode::IntegerCall {
                operator: *operator,
                arguments: arguments
                    .iter()
                    .map(|value| value.clone_shared(cache))
                    .collect(),
            },
            LoweringExpressionNode::Array(elements) => LoweringExpressionNode::Array(
                elements
                    .iter()
                    .map(|value| value.clone_shared(cache))
                    .collect(),
            ),
            LoweringExpressionNode::Index { value, index } => LoweringExpressionNode::Index {
                value: value.clone_shared(cache),
                index: *index,
            },
            LoweringExpressionNode::Complex { real, imag } => LoweringExpressionNode::Complex {
                real: real.clone_shared(cache),
                imag: imag.clone_shared(cache),
            },
            LoweringExpressionNode::Name(name) => LoweringExpressionNode::Name(name.clone()),
            LoweringExpressionNode::Not(value) => {
                LoweringExpressionNode::Not(value.clone_shared(cache))
            }
            LoweringExpressionNode::Neg(value) => {
                LoweringExpressionNode::Neg(value.clone_shared(cache))
            }
            LoweringExpressionNode::Select {
                condition,
                then_value,
                else_value,
            } => LoweringExpressionNode::Select {
                condition: condition.clone_shared(cache),
                then_value: then_value.clone_shared(cache),
                else_value: else_value.clone_shared(cache),
            },
            LoweringExpressionNode::Require { condition, value } => {
                LoweringExpressionNode::Require {
                    condition: condition.clone_shared(cache),
                    value: value.clone_shared(cache),
                }
            }
            LoweringExpressionNode::Extremum {
                minimum,
                left,
                right,
            } => LoweringExpressionNode::Extremum {
                minimum: *minimum,
                left: left.clone_shared(cache),
                right: right.clone_shared(cache),
            },
            LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            } => LoweringExpressionNode::Binary {
                operator: *operator,
                left: left.clone_shared(cache),
                right: right.clone_shared(cache),
            },
            LoweringExpressionNode::Call { callee, argument } => LoweringExpressionNode::Call {
                callee: callee.clone(),
                argument: argument.clone_shared(cache),
            },
            LoweringExpressionNode::Sample { value, clock } => LoweringExpressionNode::Sample {
                value: value.clone_shared(cache),
                clock: clock.clone(),
            },
            LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            } => LoweringExpressionNode::PureOperator {
                definition: definition.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.clone_shared(cache))
                    .collect(),
            },
            LoweringExpressionNode::UnknownMath(path) => {
                LoweringExpressionNode::UnknownMath(path.clone())
            }
            LoweringExpressionNode::InvalidValue(message) => {
                LoweringExpressionNode::InvalidValue(message)
            }
            LoweringExpressionNode::Unsupported => LoweringExpressionNode::Unsupported,
        };
        let value = Self {
            node: Arc::new(node),
            range: self.range,
        };
        cache.insert(key, value.clone());
        value
    }
}
