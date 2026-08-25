//! Scope behavior for the synthetic external root occurrence.

use super::Scope;
use crate::lower::LoweringExpression;

impl Scope {
    pub(in crate::hierarchy) fn external_root() -> Self {
        Self {
            detach_parameter_expressions: true,
            ..Self::default()
        }
    }

    pub(in crate::hierarchy) fn child(parent: &Self) -> Self {
        Self {
            detach_parameter_expressions: parent.detach_parameter_expressions,
            ..Self::default()
        }
    }

    pub(super) fn parameter_expression(&self, name: &str) -> LoweringExpression {
        let expression = &self
            .parameter(name)
            .expect("Parameter presence was checked")
            .expression;
        if self.detach_parameter_expressions {
            expression.detached_clone()
        } else {
            expression.clone()
        }
    }
}
