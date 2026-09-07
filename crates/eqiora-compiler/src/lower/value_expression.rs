//! Constructors preserving shared authored expression ownership.

use super::*;

impl LoweringExpression {
    pub(crate) fn from_source(expression: &Expr) -> Self {
        expression::from_source(expression)
    }

    pub(crate) fn quantity(value: DynQuantity, range: TextRange) -> Self {
        match eqiora_core::ValueLiteral::try_from(value) {
            Ok(value) => Self::literal(value, range),
            Err(_) => Self {
                node: Arc::new(LoweringExpressionNode::InvalidValue(
                    "mathematical literal must be finite",
                )),
                range,
            },
        }
    }

    pub(crate) fn literal(value: eqiora_core::ValueLiteral, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Literal(value)),
            range,
        }
    }

    pub(crate) fn array(elements: Vec<Self>, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Array(elements)),
            range,
        }
    }
    pub(crate) fn index(value: Self, index: u32, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Index { value, index }),
            range,
        }
    }
    pub(crate) fn complex(real: Self, imag: Self, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Complex { real, imag }),
            range,
        }
    }
    pub(crate) fn name(name: String, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Name(name)),
            range,
        }
    }

    pub(crate) fn neg(value: Self, range: TextRange) -> Self {
        if let LoweringExpressionNode::Literal(quantity) = value.node.as_ref()
            && quantity.is_zero()
        {
            return Self::literal(quantity.clone(), range);
        }
        Self {
            node: Arc::new(LoweringExpressionNode::Neg(value)),
            range,
        }
    }

    pub(crate) fn binary(operator: BinaryOp, left: Self, right: Self, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            }),
            range,
        }
    }

    pub(crate) fn call(callee: String, argument: Self, range: TextRange) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Call { callee, argument }),
            range,
        }
    }

    pub(crate) fn pure_operator(
        definition: PureOperatorDefinition,
        arguments: Vec<Self>,
        range: TextRange,
    ) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            }),
            range,
        }
    }

    pub(crate) fn embed_complex(self) -> Self {
        let unit = eqiora_core::ValueLiteral::from_real(
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Complex,
                DimExponents::DIMENSIONLESS,
            ),
            1.0,
        )
        .expect("one is a finite complex scalar literal");
        let range = self.range;
        Self::binary(BinaryOp::Mul, Self::literal(unit, range), self, range)
    }

    pub(crate) const fn range(&self) -> TextRange {
        self.range
    }

    #[cfg(test)]
    pub(crate) fn name_value(&self) -> Option<&str> {
        match self.node.as_ref() {
            LoweringExpressionNode::Name(name) => Some(name),
            _ => None,
        }
    }
}
