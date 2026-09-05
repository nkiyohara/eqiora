use super::*;

pub(super) fn dimension_expression(
    dimension: DimExponents,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> Expr {
    let mut factors = ["kg", "m", "s", "A", "K", "mol", "cd"]
        .into_iter()
        .zip(dimension.exponents())
        .filter(|(_, (numerator, _))| *numerator != 0)
        .map(|(name, (numerator, denominator))| {
            let base = Expr {
                kind: ExprKind::Name(name.to_owned()),
                range: ranges.allocate(path, paths),
            };
            if (numerator, denominator) == (1, 1) {
                base
            } else {
                let numerator = Expr {
                    kind: ExprKind::Number(f64::from(numerator)),
                    range: ranges.allocate(path, paths),
                };
                let exponent = if denominator == 1 {
                    numerator
                } else {
                    Expr {
                        kind: ExprKind::Binary {
                            op: BinaryOp::Div,
                            left: Box::new(numerator),
                            right: Box::new(Expr {
                                kind: ExprKind::Number(f64::from(denominator)),
                                range: ranges.allocate(path, paths),
                            }),
                        },
                        range: ranges.allocate(path, paths),
                    }
                };
                Expr {
                    kind: ExprKind::Binary {
                        op: BinaryOp::Pow,
                        left: Box::new(base),
                        right: Box::new(exponent),
                    },
                    range: ranges.allocate(path, paths),
                }
            }
        })
        .collect::<Vec<_>>()
        .into_iter();

    let Some(first) = factors.next() else {
        return Expr {
            kind: ExprKind::Number(1.0),
            range: ranges.allocate(path, paths),
        };
    };
    factors.fold(first, |left, right| Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: Box::new(left),
            right: Box::new(right),
        },
        range: ranges.allocate(path, paths),
    })
}
