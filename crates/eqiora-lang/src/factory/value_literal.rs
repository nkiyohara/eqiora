use super::{AstConstructionError, SourceAstFactory, checked_range};
use crate::{BinaryOp, Expr, ExprKind, NamePath, TextRange};
use eqiora_core::{DimExponents, ScalarDomain, ValueFrame, ValueLiteral};

impl SourceAstFactory {
    /// Project one complete coherent-SI value into the canonical source vocabulary.
    ///
    /// # Errors
    /// Rejects excessive type cardinality/nesting, a frame on an invariant value,
    /// or nonzero spatial components without an explicit frame.
    pub fn value_literal(
        value: &ValueLiteral,
        frame: Option<NamePath>,
        range: TextRange,
        mut resolve: impl FnMut(eqiora_core::RawId) -> Option<NamePath>,
    ) -> Result<Expr, AstConstructionError> {
        checked_range(range)?;
        if frame.is_some() && value.value_type().frame() == ValueFrame::Invariant {
            return Err(AstConstructionError::new(
                "invariant values cannot supply a spatial frame",
            ));
        }
        if let Some(frame) = &frame {
            super::validate_name_path(frame)?;
        }
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
        if frame.is_none()
            && nominal.is_none()
            && value.is_zero()
            && !value.value_type().shape().is_scalar()
        {
            return Self::expression(
                ExprKind::Number(crate::DecimalLiteral::parse("0.0").expect("exact literal")),
                range,
            );
        }
        if value.value_type().frame() != ValueFrame::Invariant && frame.is_none() {
            return Err(AstConstructionError::new(
                "nonzero spatial values require an explicit frame-bearing source constructor",
            ));
        }
        fn nested(
            value: &ValueLiteral,
            axis: usize,
            offset: &mut usize,
            range: TextRange,
            frame: Option<&NamePath>,
        ) -> Expr {
            if axis == value.value_type().array_rank()
                && let Some(frame) = frame
            {
                let components = nested(value, axis, offset, range, None);
                return Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Call {
                        callee: NamePath::single("tensor_value".to_owned(), range),
                        arguments: tensor_arguments(frame.clone(), components, range),
                    },
                    range,
                };
            }
            if let Some(extent) = value.value_type().shape().extents().get(axis) {
                return Expr {
                    resolved_nominal: None,
                    kind: ExprKind::Array(
                        (0..extent.get())
                            .map(|_| nested(value, axis + 1, offset, range, frame))
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
                        arguments: crate::CallArguments::Positional(vec![
                            scalar(real),
                            scalar(imaginary),
                        ]),
                    }
                } else {
                    scalar(real).kind
                },
                range,
            }
        }
        let result = nested(value, 0, &mut 0, range, frame.as_ref());
        if let Some((constructor, name)) = nominal {
            let mut expression = Self::expression(
                ExprKind::Call {
                    callee: NamePath::single(constructor.to_owned(), range),
                    arguments: crate::CallArguments::Positional(vec![
                        Expr {
                            resolved_nominal: None,
                            kind: ExprKind::Path(name.clone()),
                            range,
                        },
                        result,
                    ]),
                },
                range,
            )?;
            Self::bind_nominal_expression(&mut expression, name, value.value_type().clone())?;
            return Ok(expression);
        }
        Self::expression(result.kind, range)
    }
}

fn frame_expression(frame: NamePath, range: TextRange) -> Expr {
    Expr {
        resolved_nominal: None,
        kind: if frame.is_qualified() {
            ExprKind::Path(frame)
        } else {
            ExprKind::Name(frame.as_str().to_owned())
        },
        range,
    }
}

impl SourceAstFactory {
    /// Construct a spatial coefficient in the named support's Cartesian frame.
    /// The frame reference does not give the coefficient spatial support.
    ///
    /// # Errors
    /// Rejects invalid names, ranges, or expressions exceeding source bounds.
    pub fn tensor_value(
        frame: NamePath,
        components: Expr,
        range: TextRange,
    ) -> Result<Expr, AstConstructionError> {
        super::validate_name_path(&frame)?;
        Self::expression(
            ExprKind::Call {
                callee: NamePath::single("tensor_value".to_owned(), range),
                arguments: tensor_arguments(frame, components, range),
            },
            range,
        )
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
            SourceAstFactory::value_literal(&literal, None, TextRange::new(0, 1), |_| None)
                .unwrap();
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
            SourceAstFactory::value_literal(&zero, None, TextRange::new(0, 1), |_| None)
                .unwrap()
                .kind(),
            ExprKind::Number(number) if number.is_zero()
        ));
        let value = ValueLiteral::new(vector, [(1.0, 0.0), (2.0, 0.0)]).unwrap();
        let range = TextRange::new(0, 1);
        let projected = SourceAstFactory::value_literal(
            &value,
            Some(NamePath::single("body".to_owned(), range)),
            range,
            |_| None,
        )
        .unwrap();
        let ExprKind::Call { arguments, .. } = projected.kind() else {
            panic!("framed vector")
        };
        assert!(matches!(arguments[1].kind(), ExprKind::Array(values) if values.len() == 2));
        assert!(
            SourceAstFactory::value_literal(&value, None, TextRange::new(0, 1), |_| None)
                .unwrap_err()
                .to_string()
                .contains("frame-bearing")
        );
    }
    #[test]
    fn framed_spatial_elements_keep_outer_channels_and_component_order() {
        let spatial = ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let literal = ValueLiteral::new(
            spatial.array(2).unwrap(),
            [
                (2.0, 11.0),
                (3.0, 13.0),
                (5.0, 17.0),
                (7.0, 19.0),
                (23.0, 29.0),
                (31.0, 37.0),
                (41.0, 43.0),
                (47.0, 53.0),
            ],
        )
        .unwrap();
        let range = TextRange::new(0, 1);
        let frame = NamePath::single("body".to_owned(), range);
        let result =
            SourceAstFactory::value_literal(&literal, Some(frame.clone()), range, |_| None)
                .unwrap();
        let ExprKind::Array(channels) = result.kind() else {
            panic!("channel array")
        };
        assert_eq!(channels.len(), 2);
        let mut components = Vec::new();
        for element in channels {
            let ExprKind::Call { callee, arguments } = element.kind() else {
                panic!("spatial element")
            };
            assert_eq!(callee.as_str(), "tensor_value");
            let ExprKind::Array(rows) = arguments[1].kind() else {
                panic!("matrix rows")
            };
            for row in rows {
                let ExprKind::Array(entries) = row.kind() else {
                    panic!("matrix columns")
                };
                for entry in entries {
                    let ExprKind::Call { arguments, .. } = entry.kind() else {
                        panic!("complex entry")
                    };
                    components.push(
                        arguments
                            .iter()
                            .map(|e| match e.kind() {
                                ExprKind::Number(value) => value.to_f64().unwrap(),
                                _ => panic!("component"),
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
        assert_eq!(
            components,
            vec![
                vec![2., 11.],
                vec![3., 13.],
                vec![5., 17.],
                vec![7., 19.],
                vec![23., 29.],
                vec![31., 37.],
                vec![41., 43.],
                vec![47., 53.]
            ]
        );
        let invariant = ValueLiteral::from_real(
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            0.0,
        )
        .unwrap();
        assert!(SourceAstFactory::value_literal(&invariant, Some(frame), range, |_| None).is_err());
    }
    #[test]
    fn framed_projection_reuses_type_cardinality_and_expression_depth_bounds() {
        let range = TextRange::new(0, 1);
        let frame = NamePath::single("body".to_owned(), range);
        let huge = ValueType::shaped(
            ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap()
        .array(32769)
        .unwrap();
        let zero = ValueLiteral::from_real(huge, 0.0).unwrap();
        assert!(
            SourceAstFactory::value_literal(&zero, Some(frame.clone()), range, |_| None)
                .unwrap_err()
                .message()
                .contains("65536")
        );
        let mut components = SourceAstFactory::expression(
            ExprKind::Number(crate::DecimalLiteral::parse("1").unwrap()),
            range,
        )
        .unwrap();
        for _ in 0..254 {
            components =
                SourceAstFactory::expression(ExprKind::Array(vec![components]), range).unwrap();
        }
        SourceAstFactory::tensor_value(frame.clone(), components.clone(), range).unwrap();
        components =
            SourceAstFactory::expression(ExprKind::Array(vec![components]), range).unwrap();
        assert!(SourceAstFactory::tensor_value(frame, components, range).is_err());
    }
}

fn tensor_arguments(frame: NamePath, components: Expr, range: TextRange) -> crate::CallArguments {
    crate::CallArguments::Named(vec![
        crate::NamedBindingDecl {
            comments: Default::default(),
            name: "frame".to_owned(),
            value: frame_expression(frame, range),
            range,
        },
        crate::NamedBindingDecl {
            comments: Default::default(),
            name: "components".to_owned(),
            value: components,
            range,
        },
    ])
}
