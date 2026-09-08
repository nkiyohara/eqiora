use super::{AstConstructionError, SourceAstFactory, checked_range};
use crate::{BinaryOp, Expr, ExprKind, NamePath, TextRange};
use eqiora_core::{DimExponents, ScalarDomain, ValueFrame, ValueLiteral};

impl SourceAstFactory {
    /// Project one complete coherent-SI value into the canonical source vocabulary.
    ///
    /// # Errors
    /// Rejects excessive type cardinality/nesting and nonzero spatial components,
    /// which require an admitted explicit frame-bearing source constructor.
    pub fn value_literal(
        value: &ValueLiteral,
        range: TextRange,
        mut resolve: impl FnMut(eqiora_core::RawId) -> Option<NamePath>,
    ) -> Result<Expr, AstConstructionError> {
        checked_range(range)?;
        let syntax = crate::ValueTypeSyntax::from_checked(value.value_type(), &mut resolve)?;
        if let Some(value) = value.as_bool() {
            return Self::expression(ExprKind::Boolean(value), range);
        }
        let nominal = match syntax.kind() {
            crate::ValueTypeSyntaxKind::Coordinates(name) => Some(("coordinates", name)),
            crate::ValueTypeSyntaxKind::Counts(name) => Some(("counts", name)),
            crate::ValueTypeSyntaxKind::Index(name) => Some(("index", name)),
            _ => None,
        };
        if nominal.is_none() && value.is_zero() && !value.value_type().shape().is_scalar() {
            return Self::expression(
                ExprKind::Number(crate::DecimalLiteral::parse("0.0").expect("exact literal")),
                range,
            );
        }
        if value.value_type().frame() != ValueFrame::Invariant {
            return Err(AstConstructionError::new(
                "nonzero spatial values require an explicit frame-bearing source constructor",
            ));
        }
        fn nested(value: &ValueLiteral, axis: usize, offset: &mut usize, range: TextRange) -> Expr {
            if let Some(extent) = value.value_type().shape().extents().get(axis) {
                return Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Array(
                        (0..extent.get())
                            .map(|_| nested(value, axis + 1, offset, range))
                            .collect(),
                    ),
                    range,
                };
            }
            if let Some(integer) = value.integer_component(*offset) {
                *offset += 1;
                return Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Number(
                        crate::DecimalLiteral::parse(&integer.to_string()).expect("bounded i64"),
                    ),
                    range,
                };
            }
            let (real, imaginary) = value.component(*offset).expect("bounded value component");
            *offset += 1;
            let scalar = |number| Expr {
                resolved_nominal: None,
                kind: if value.value_type().dimension() == DimExponents::DIMENSIONLESS {
                    ExprKind::Number(
                        crate::DecimalLiteral::from_f64(number)
                            .expect("finite ValueLiteral component"),
                    )
                } else {
                    ExprKind::Quantity {
                        value: crate::DecimalLiteral::from_f64(number)
                            .expect("checked finite value component"),
                        unit: Box::new(dimension_expression(
                            value.value_type().dimension(),
                            || range,
                        )),
                    }
                },
                range,
            };
            Expr {
                resolved_nominal: None,
                kind: if value.value_type().scalar_domain() == ScalarDomain::Complex {
                    ExprKind::Call {
                        callee: NamePath::from_parsed_segments(
                            ["math".to_owned(), "complex".to_owned()],
                            range,
                        ),
                        arguments: vec![scalar(real), scalar(imaginary)],
                    }
                } else {
                    scalar(real).kind
                },
                range,
            }
        }
        let result = nested(value, 0, &mut 0, range);
        if let Some((constructor, name)) = nominal {
            let mut expression = Self::expression(
                ExprKind::Call {
                    callee: NamePath::single(constructor.to_owned(), range),
                    arguments: vec![
                        Expr {
                            resolved_nominal: None,
                            kind: ExprKind::Path(name.clone()),
                            range,
                        },
                        result,
                    ],
                },
                range,
            )?;
            Self::bind_nominal_expression(&mut expression, name, value.value_type().clone())?;
            return Ok(expression);
        }
        Self::expression(result.kind, range)
    }
}

pub(crate) fn dimension_expression(
    dimension: DimExponents,
    mut range: impl FnMut() -> TextRange,
) -> Expr {
    let mut factors = ["kg", "m", "s", "A", "K", "mol", "cd"]
        .into_iter()
        .zip(dimension.exponents())
        .filter(|(_, (numerator, _))| *numerator != 0)
        .map(|(name, (numerator, denominator))| {
            let base = Expr {
                resolved_nominal: None,
                kind: ExprKind::Name(name.to_owned()),
                range: range(),
            };
            if (numerator, denominator) == (1, 1) {
                base
            } else {
                let numerator = Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Number(
                        crate::DecimalLiteral::parse(&numerator.to_string())
                            .expect("bounded dimension numerator"),
                    ),
                    range: range(),
                };
                let exponent = if denominator == 1 {
                    numerator
                } else {
                    Expr {
                        resolved_nominal: None,
                        kind: ExprKind::Binary {
                            op: BinaryOp::Div,
                            left: Box::new(numerator),
                            right: Box::new(Expr {
                                resolved_nominal: None,
                                kind: ExprKind::Number(
                                    crate::DecimalLiteral::parse(&denominator.to_string())
                                        .expect("bounded dimension denominator"),
                                ),
                                range: range(),
                            }),
                        },
                        range: range(),
                    }
                };
                Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Binary {
                        op: BinaryOp::Pow,
                        left: Box::new(base),
                        right: Box::new(exponent),
                    },
                    range: range(),
                }
            }
        })
        .collect::<Vec<_>>()
        .into_iter();

    let Some(first) = factors.next() else {
        return Expr {
            resolved_nominal: None,
            kind: ExprKind::Number(crate::DecimalLiteral::parse("1.0").expect("exact literal")),
            range: range(),
        };
    };
    factors.fold(first, |left, right| Expr {
        resolved_nominal: None,
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: Box::new(left),
            right: Box::new(right),
        },
        range: range(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{ValueShape, ValueType};

    #[test]
    fn complete_complex_channels_keep_order_dimensions_and_real_complex_domain() {
        let kind = ValueType::scalar(
            ScalarDomain::Complex,
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
        )
        .array(2)
        .unwrap();
        let literal = ValueLiteral::new(kind, [(1.0, 2.0), (3.0, 0.0)]).unwrap();
        let expression =
            SourceAstFactory::value_literal(&literal, TextRange::new(0, 1), |_| None).unwrap();
        let ExprKind::Array(elements) = expression.kind() else {
            panic!("channel axis")
        };
        for (element, expected) in elements.iter().zip([[1.0, 2.0], [3.0, 0.0]]) {
            let ExprKind::Call { callee, arguments } = element.kind() else {
                panic!("complex constructor")
            };
            assert_eq!(callee.as_str(), "math.complex");
            for (argument, expected) in arguments.iter().zip(expected) {
                let ExprKind::Quantity { value, unit } = argument.kind() else {
                    panic!("coherent quantity")
                };
                assert_eq!(*value, crate::DecimalLiteral::from_f64(expected).unwrap());
                assert!(matches!(unit.kind(), ExprKind::Name(name) if name == "m"));
            }
        }
    }

    #[test]
    fn contextual_spatial_zero_is_compact_but_nonzero_spatial_channels_are_not_reinterpreted() {
        let vector = ValueType::shaped(
            ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let zero = ValueLiteral::from_real(vector.clone(), 0.0).unwrap();
        assert!(matches!(
            SourceAstFactory::value_literal(&zero, TextRange::new(0, 1), |_| None)
                .unwrap()
                .kind(),
            ExprKind::Number(number) if number.is_zero()
        ));
        let value = ValueLiteral::new(vector, [(1.0, 0.0), (2.0, 0.0)]).unwrap();
        assert!(
            SourceAstFactory::value_literal(&value, TextRange::new(0, 1), |_| None)
                .unwrap_err()
                .to_string()
                .contains("frame-bearing")
        );
    }
}
