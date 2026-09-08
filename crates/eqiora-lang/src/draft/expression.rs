//! Projection of native expressions into the shared AST.

use super::*;

impl DraftExpression {
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
            DraftExpressionKind::Boolean(value) => ExprKind::Boolean(*value),
            DraftExpressionKind::Constant(value) => ExprKind::Number(value.clone()),
            DraftExpressionKind::Complex(real, imaginary) => ExprKind::Call {
                callee: NamePath::from_parsed_segments(
                    vec!["math".to_owned(), "complex".to_owned()],
                    ranges.allocate(path, paths),
                ),
                arguments: [*real, *imaginary]
                    .into_iter()
                    .map(|number| Expr {
                        resolved_nominal: None,
                        kind: ExprKind::Number(
                            crate::DecimalLiteral::from_f64(number)
                                .expect("validated native literal"),
                        ),
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
                    resolved_nominal: None,
                    kind: ExprKind::Number(
                        crate::DecimalLiteral::parse(&index.to_string()).expect("u32 index"),
                    ),
                    range: ranges.allocate(path, paths),
                }),
            },
            DraftExpressionKind::Reference(reference) => ExprKind::Name(reference.name.clone()),
            DraftExpressionKind::Derivative(reference) => ExprKind::Call {
                callee: NamePath::single("derivative".to_owned(), ranges.allocate(path, paths)),
                arguments: vec![Expr {
                    resolved_nominal: None,
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
            DraftExpressionKind::Unary { operator, value } => ExprKind::Unary {
                op: *operator,
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
            resolved_nominal: None,
            kind,
            range: ranges.allocate(path, paths),
        }
    }
}

impl DraftExpression {
    /// Boolean literal with no numeric coercion.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        Self {
            kind: DraftExpressionKind::Boolean(value),
        }
    }
    /// Boolean negation.
    #[must_use]
    pub fn logical_not(self) -> Self {
        Self {
            kind: DraftExpressionKind::Unary {
                operator: UnaryOp::Not,
                value: Box::new(self),
            },
        }
    }
    /// Construct a `equal` predicate.
    #[must_use]
    pub fn equal(self, right: Self) -> Self {
        self.binary(BinaryOp::Equal, right)
    }
    /// Construct a `not_equal` predicate.
    #[must_use]
    pub fn not_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::NotEqual, right)
    }
    /// Construct a `less` predicate.
    #[must_use]
    pub fn less(self, right: Self) -> Self {
        self.binary(BinaryOp::Less, right)
    }
    /// Construct a `less_equal` predicate.
    #[must_use]
    pub fn less_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::LessEqual, right)
    }
    /// Construct a `greater` predicate.
    #[must_use]
    pub fn greater(self, right: Self) -> Self {
        self.binary(BinaryOp::Greater, right)
    }
    /// Construct a `greater_equal` predicate.
    #[must_use]
    pub fn greater_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::GreaterEqual, right)
    }
    /// Construct a `logical_and` predicate.
    #[must_use]
    pub fn logical_and(self, right: Self) -> Self {
        self.binary(BinaryOp::And, right)
    }
    /// Construct a `logical_or` predicate.
    #[must_use]
    pub fn logical_or(self, right: Self) -> Self {
        self.binary(BinaryOp::Or, right)
    }
}
