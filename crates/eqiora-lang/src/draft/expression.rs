//! Native expression construction, validation traversal, and shared-AST projection.

use super::*;
use std::ops::{Add, Div, Mul, Neg, Sub};

impl DraftExpression {
    /// Dimensionless numeric literal.
    #[must_use]
    pub const fn constant(value: f64) -> Self {
        Self {
            kind: DraftExpressionKind::Constant(value),
        }
    }

    /// Construct an ordered channel-array expression.
    #[must_use]
    pub fn array(values: impl IntoIterator<Item = Self>) -> Self {
        Self {
            kind: DraftExpressionKind::Array(values.into_iter().collect()),
        }
    }

    /// Select a static channel index. The compiler checks type and bounds.
    #[must_use]
    pub fn index(self, index: u32) -> Self {
        Self {
            kind: DraftExpressionKind::Index {
                value: Box::new(self),
                index,
            },
        }
    }

    /// Construct a complex scalar without discarding either component.
    #[must_use]
    pub fn complex(real: f64, imaginary: f64) -> Self {
        Self {
            kind: DraftExpressionKind::Complex(real, imaginary),
        }
    }

    pub(super) fn reference(symbol: DraftSymbol, name: String, kind: DraftSymbolKind) -> Self {
        Self {
            kind: DraftExpressionKind::Reference(DraftReference { symbol, name, kind }),
        }
    }

    /// Time derivative of one Field.
    #[must_use]
    pub fn derivative(field: &DraftField) -> Self {
        Self {
            kind: DraftExpressionKind::Derivative(DraftReference {
                symbol: field.symbol.clone(),
                name: field.name.clone(),
                kind: DraftSymbolKind::Field,
            }),
        }
    }

    /// Read the across variable of one scalar conserving Port.
    #[must_use]
    pub fn across(port: &DraftConservingPort) -> Self {
        Self {
            kind: DraftExpressionKind::Across(DraftPortReference::from(port)),
        }
    }

    /// Read the through variable of one scalar conserving Port.
    #[must_use]
    pub fn through(port: &DraftConservingPort) -> Self {
        Self {
            kind: DraftExpressionKind::Through(DraftPortReference::from(port)),
        }
    }

    /// Spatial gradient of one expression.
    #[must_use]
    pub fn gradient(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Gradient, value)
    }

    /// Spatial divergence of one expression.
    #[must_use]
    pub fn divergence(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Divergence, value)
    }

    /// Boundary trace of one expression.
    #[must_use]
    pub fn trace(value: Self) -> Self {
        Self::spatial_call(DraftSpatialOperator::Trace, value)
    }

    fn spatial_call(operator: DraftSpatialOperator, value: Self) -> Self {
        Self {
            kind: DraftExpressionKind::SpatialCall {
                operator,
                value: Box::new(value),
            },
        }
    }

    fn binary(self, operator: BinaryOp, right: Self) -> Self {
        Self {
            kind: DraftExpressionKind::Binary {
                operator,
                left: Box::new(self),
                right: Box::new(right),
            },
        }
    }

    pub(super) fn references<'a>(&'a self, output: &mut Vec<DraftExpressionReference<'a>>) {
        match &self.kind {
            DraftExpressionKind::Constant(_) | DraftExpressionKind::Complex(_, _) => {}
            DraftExpressionKind::Array(values) => {
                for value in values {
                    value.references(output);
                }
            }
            DraftExpressionKind::Index { value, .. } => value.references(output),
            DraftExpressionKind::Reference(reference)
            | DraftExpressionKind::Derivative(reference) => {
                output.push(DraftExpressionReference::Value(reference));
            }
            DraftExpressionKind::Across(reference) | DraftExpressionKind::Through(reference) => {
                output.push(DraftExpressionReference::Port(reference));
            }
            DraftExpressionKind::Neg(value) | DraftExpressionKind::SpatialCall { value, .. } => {
                value.references(output);
            }
            DraftExpressionKind::Binary { left, right, .. } => {
                left.references(output);
                right.references(output);
            }
        }
    }

    pub(super) fn contains_invalid_literal(&self) -> bool {
        match &self.kind {
            DraftExpressionKind::Constant(value) => !value.is_finite(),
            DraftExpressionKind::Complex(real, imaginary) => {
                !real.is_finite() || !imaginary.is_finite()
            }
            DraftExpressionKind::Array(values) => {
                values.is_empty() || values.iter().any(Self::contains_invalid_literal)
            }
            DraftExpressionKind::Index { value, .. } => value.contains_invalid_literal(),
            DraftExpressionKind::Reference(_)
            | DraftExpressionKind::Derivative(_)
            | DraftExpressionKind::Across(_)
            | DraftExpressionKind::Through(_) => false,
            DraftExpressionKind::Neg(value) | DraftExpressionKind::SpatialCall { value, .. } => {
                value.contains_invalid_literal()
            }
            DraftExpressionKind::Binary { left, right, .. } => {
                left.contains_invalid_literal() || right.contains_invalid_literal()
            }
        }
    }

    /// Project an authored expression into the shared AST with synthetic ranges.
    #[doc(hidden)]
    #[must_use]
    pub fn source_ast(&self) -> Expr {
        self.ast(
            &GraphPath::new(["argument".to_owned()]),
            &mut RangeAllocator::default(),
            &mut HashMap::new(),
        )
    }

    pub(super) fn ast(
        &self,
        path: &GraphPath,
        ranges: &mut RangeAllocator,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> Expr {
        let kind = match &self.kind {
            DraftExpressionKind::Constant(value) => ExprKind::Number(*value),
            DraftExpressionKind::Complex(real, imaginary) => ExprKind::Call {
                callee: NamePath::from_parsed_segments(
                    vec!["math".to_owned(), "complex".to_owned()],
                    ranges.allocate(path, paths),
                ),
                arguments: [*real, *imaginary]
                    .into_iter()
                    .map(|number| Expr {
                        kind: ExprKind::Number(number),
                        range: ranges.allocate(path, paths),
                    })
                    .collect(),
            },
            DraftExpressionKind::Array(values) => ExprKind::Array(
                values
                    .iter()
                    .map(|value| value.ast(path, ranges, paths))
                    .collect(),
            ),
            DraftExpressionKind::Index { value, index } => ExprKind::Index {
                value: Box::new(value.ast(path, ranges, paths)),
                index: Box::new(Expr {
                    kind: ExprKind::Number(f64::from(*index)),
                    range: ranges.allocate(path, paths),
                }),
            },
            DraftExpressionKind::Reference(reference) => ExprKind::Name(reference.name.clone()),
            DraftExpressionKind::Derivative(reference) => ExprKind::Call {
                callee: NamePath::single("derivative".to_owned(), ranges.allocate(path, paths)),
                arguments: vec![Expr {
                    kind: ExprKind::Name(reference.name.clone()),
                    range: ranges.allocate(path, paths),
                }],
            },
            DraftExpressionKind::Across(reference) => {
                physical_accessor_ast("across", reference, path, ranges, paths)
            }
            DraftExpressionKind::Through(reference) => {
                physical_accessor_ast("through", reference, path, ranges, paths)
            }
            DraftExpressionKind::SpatialCall { operator, value } => ExprKind::Call {
                callee: NamePath::single(
                    operator.source_name().to_owned(),
                    ranges.allocate(path, paths),
                ),
                arguments: vec![value.ast(path, ranges, paths)],
            },
            DraftExpressionKind::Neg(value) => ExprKind::Unary {
                op: UnaryOp::Neg,
                value: Box::new(value.ast(path, ranges, paths)),
            },
            DraftExpressionKind::Binary {
                operator,
                left,
                right,
            } => ExprKind::Binary {
                op: *operator,
                left: Box::new(left.ast(path, ranges, paths)),
                right: Box::new(right.ast(path, ranges, paths)),
            },
        };
        Expr {
            kind,
            range: ranges.allocate(path, paths),
        }
    }
}

impl Neg for DraftExpression {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            kind: DraftExpressionKind::Neg(Box::new(self)),
        }
    }
}

macro_rules! impl_binary_expression_operator {
    ($trait:ident, $method:ident, $operator:expr) => {
        impl $trait for DraftExpression {
            type Output = Self;

            fn $method(self, right: Self) -> Self::Output {
                self.binary($operator, right)
            }
        }
    };
}

impl_binary_expression_operator!(Add, add, BinaryOp::Add);
impl_binary_expression_operator!(Sub, sub, BinaryOp::Sub);
impl_binary_expression_operator!(Mul, mul, BinaryOp::Mul);
impl_binary_expression_operator!(Div, div, BinaryOp::Div);
