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
            resolved_nominal: None,
            kind,
            range: ranges.allocate(path, paths),
        }
    }
}
