use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum ExpressionContext {
    Binding,
    Default,
    Let,
}

impl ExpressionContext {
    pub(super) fn unknown_name_message(self, name: &str) -> String {
        match self {
            Self::Binding => format!("unknown Parameter `{name}` in compile-time binding"),
            Self::Default => format!("unknown component Parameter `{name}`"),
            Self::Let => format!("unknown Parameter or let alias `{name}`"),
        }
    }

    pub(super) fn qualified_name_message(self, path: &impl std::fmt::Display) -> String {
        match self {
            Self::Binding => format!(
                "qualified name `{path}` is not allowed in a compile-time Parameter binding"
            ),
            Self::Default => {
                format!("qualified name `{path}` is not allowed in a Parameter default")
            }
            Self::Let => format!("qualified name `{path}` is not allowed in a let alias"),
        }
    }

    pub(super) fn call_message(self, callee: &str) -> String {
        match self {
            Self::Binding => {
                format!("operator `{callee}(...)` is not allowed in a compile-time binding")
            }
            Self::Default => {
                format!("operator `{callee}(...)` is not allowed in a Parameter default")
            }
            Self::Let => format!(
                "static let operator `{callee}(...)` is not supported by compile-time evaluation"
            ),
        }
    }

    pub(super) const fn unsupported_message(self) -> &'static str {
        match self {
            Self::Binding => "binding expression syntax is newer than this compiler",
            Self::Default => "Parameter default syntax is newer than this compiler",
            Self::Let => "let expression syntax is newer than this compiler",
        }
    }
}

pub(super) fn evaluate_parameter_expression(
    file: &str,
    expression: &Expr,
    context: ExpressionContext,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
) -> Result<EvaluatedParameter, Diagnostic> {
    if matches!(
        expression.kind(),
        ExprKind::Array(_) | ExprKind::Index { .. }
    ) || matches!(expression.kind(), ExprKind::Path(path) if path.as_str() == "math.i")
        || matches!(expression.kind(), ExprKind::Call { callee, .. } if callee.as_str() == "math.complex")
    {
        return super::value_expressions::evaluate(file, expression, context, resolve);
    }
    let evaluated = match expression.kind() {
        ExprKind::Number(value) => EvaluatedParameter {
            value: Some(
                ValueLiteral::from_real(
                    ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
                    normalize_zero(*value),
                )
                .expect("finite source literal"),
            ),
            value_type: EvaluatedType::Known(ValueType::scalar(
                ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
            )),
            bare_literal: true,
            expression: Some(LoweringExpression::quantity(
                DynQuantity::new(normalize_zero(*value), DimExponents::DIMENSIONLESS),
                expression.range(),
            )),
            lineage: Some(ParameterLineage::Constant),
        },
        ExprKind::Quantity { value, unit } => {
            let quantity = crate::units::quantity(*value, unit).map_err(|message| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    message,
                )
            })?;
            EvaluatedParameter {
                value: Some(ValueLiteral::try_from(quantity).expect("finite quantity")),
                value_type: EvaluatedType::Known(ValueType::scalar(
                    ScalarDomain::Real,
                    quantity.dim(),
                )),
                bare_literal: false,
                expression: Some(LoweringExpression::quantity(quantity, expression.range())),
                lineage: Some(ParameterLineage::Constant),
            }
        }
        ExprKind::Name(name) => resolve(name, expression.range())?.into(),
        ExprKind::Path(path) => match crate::math::constant(path) {
            Some(value) => EvaluatedParameter {
                value: Some(
                    ValueLiteral::from_real(
                        ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
                        value,
                    )
                    .expect("finite math constant"),
                ),
                value_type: EvaluatedType::Known(ValueType::scalar(
                    ScalarDomain::Real,
                    DimExponents::DIMENSIONLESS,
                )),
                bare_literal: false,
                expression: Some(LoweringExpression::quantity(
                    DynQuantity::new(value, DimExponents::DIMENSIONLESS),
                    expression.range(),
                )),
                lineage: Some(ParameterLineage::Constant),
            },
            None => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    path.range(),
                    context.qualified_name_message(path),
                ));
            }
        },
        ExprKind::Call { callee, arguments }
            if matches!(context, ExpressionContext::Let) && crate::math::is_function(callee) =>
        {
            let [argument] = arguments.as_slice() else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    "static scalar mathematics requires exactly one argument",
                ));
            };
            let operand = evaluate_parameter_expression(file, argument, context, resolve)?;
            let EvaluatedType::Known(value_type) = &operand.value_type else {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    "static scalar mathematics requires a known operand dimension",
                ));
            };
            let function = match callee.as_str() {
                "math.sin" => eqiora_schema::kernel::UnaryMathFunction::Sin,
                "math.sqrt" => eqiora_schema::kernel::UnaryMathFunction::Sqrt,
                _ => unreachable!("compiler-owned scalar mathematics was checked"),
            };
            let inferred = eqiora_schema::kernel::typing::unary_math(
                function,
                &eqiora_schema::kernel::typing::ExpressionType::<()>::new(value_type.clone(), None),
            )
            .map_err(|error| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    expression.range(),
                    error.to_string(),
                )
            })?;
            let value = operand
                .value
                .map(|value| {
                    let value = value
                        .real_scalar_value()
                        .expect("scalar math type checked")
                        .value();
                    if matches!(function, eqiora_schema::kernel::UnaryMathFunction::Sqrt)
                        && value < 0.0
                    {
                        return Err(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            expression.range(),
                            "math.sqrt requires a nonnegative real operand",
                        ));
                    }
                    let value = finite_constant(
                        file,
                        expression.range(),
                        match function {
                            eqiora_schema::kernel::UnaryMathFunction::Sin => value.sin(),
                            eqiora_schema::kernel::UnaryMathFunction::Sqrt => value.sqrt(),
                            _ => unreachable!("admitted scalar mathematics"),
                        },
                    )?;
                    ValueLiteral::from_real(inferred.value_type.clone(), value).map_err(|error| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            expression.range(),
                            error.to_string(),
                        )
                    })
                })
                .transpose()?;
            EvaluatedParameter {
                value,
                value_type: EvaluatedType::Known(inferred.value_type),
                bare_literal: false,
                expression: operand.expression.map(|argument| {
                    LoweringExpression::call(
                        callee.as_str().to_owned(),
                        argument,
                        expression.range(),
                    )
                }),
                lineage: Some(ParameterLineage::Derived),
            }
        }
        ExprKind::Call { callee, .. } => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                context.call_message(callee.as_str()),
            ));
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            let operand = evaluate_parameter_expression(file, value, context, resolve)?;
            let negated = operand
                .value
                .map(|value| {
                    crate::typed_values::negate(&value).map_err(|message| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            expression.range(),
                            message,
                        )
                    })
                })
                .transpose()?;
            EvaluatedParameter {
                value: negated,
                value_type: operand.value_type,
                bare_literal: operand.bare_literal,
                expression: operand
                    .expression
                    .map(|value| LoweringExpression::neg(value, expression.range())),
                lineage: transform_lineage(operand.lineage),
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evaluate_parameter_expression(file, left, context, resolve)?;
            let right = evaluate_parameter_expression(file, right, context, resolve)?;
            combine_parameters(file, expression.range(), *op, left, right)?
        }
        _ => {
            return Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                expression.range(),
                context.unsupported_message(),
            ));
        }
    };
    Ok(evaluated)
}

pub(super) fn evaluate_initializer(
    file: &str,
    expression: &Expr,
    context: ExpressionContext,
    resolve: &mut impl FnMut(&str, TextRange) -> Result<SymbolicParameterValue, Diagnostic>,
    target: ValueType,
    label: &str,
) -> Result<EvaluatedParameter, Diagnostic> {
    let evaluated = if matches!(expression.kind(), ExprKind::Array(_))
        || matches!(expression.kind(), ExprKind::Call { callee, .. } if callee.as_str() == "math.complex")
    {
        super::value_expressions::evaluate_with_target(
            file,
            expression,
            context,
            resolve,
            Some(&target),
        )?
    } else {
        evaluate_parameter_expression(file, expression, context, resolve)?
    };
    coerce_parameter_with_label(file, expression.range(), evaluated, target, label, true)
        .map(Into::into)
}

pub(super) fn coerce_parameter(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    target: ValueType,
) -> Result<SymbolicParameterValue, Diagnostic> {
    coerce_parameter_with_label(file, range, evaluated, target, "Parameter binding", false)
}

pub(super) fn coerce_parameter_with_label(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    target: ValueType,
    label: &str,
    declaration_initializer: bool,
) -> Result<SymbolicParameterValue, Diagnostic> {
    if evaluated.bare_literal
        && (declaration_initializer
            || evaluated.value.as_ref().is_some_and(ValueLiteral::is_zero)
            || target.dimension() == DimExponents::DIMENSIONLESS)
    {
        let literal = ValueLiteral::from_real(
            target.clone(),
            evaluated
                .value
                .as_ref()
                .expect("bare literals have a known value")
                .real_scalar_value()
                .expect("bare real literal")
                .value(),
        )
        .map_err(|error| {
            source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string())
        })?;
        return Ok(SymbolicParameterValue {
            value: Some(literal.clone()),
            value_type: target,
            expression: Some(LoweringExpression::literal(literal, range)),
            lineage: evaluated.lineage,
        });
    }
    match &evaluated.value_type {
        EvaluatedType::Known(actual) if actual.dimension() != target.dimension() => {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!(
                    "{label} has dimension [{}], expected [{}]",
                    actual.dimension(),
                    target.dimension()
                ),
            ));
        }
        EvaluatedType::Known(actual) | EvaluatedType::Deferred(actual)
            if target.scalar_domain() == ScalarDomain::Real
                && target.shape().is_scalar()
                && (actual.scalar_domain() != ScalarDomain::Real
                    || !actual.shape().is_scalar()) =>
        {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                format!("{label} requires a real scalar type"),
            ));
        }
        _ => {}
    }
    use eqiora_schema::kernel::typing::{self, ExpressionType};
    let actual = evaluated
        .value_type
        .value_type()
        .clone()
        .with_dimension(target.dimension());
    let common = typing::additive(
        &ExpressionType::<()>::new(actual.clone(), None),
        &ExpressionType::<()>::new(target.clone(), None),
    )
    .map_err(|error| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, error.to_string()))?;
    if common.value_type != target {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!("{label} cannot implicitly convert complex values to real values"),
        ));
    }
    let embedding = actual.scalar_domain() != target.scalar_domain();
    let expression = evaluated.expression.map(|expression| {
        if embedding {
            expression.embed_complex()
        } else {
            expression
        }
    });
    let lineage = if embedding {
        transform_lineage(evaluated.lineage)
    } else {
        evaluated.lineage
    };
    Ok(SymbolicParameterValue {
        value: evaluated
            .value
            .map(|value| {
                crate::typed_values::retype(&value, target.clone()).map_err(|message| {
                    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message)
                })
            })
            .transpose()?,
        value_type: target,
        expression,
        lineage,
    })
}

pub(super) fn infer_parameter_with_label(
    file: &str,
    range: TextRange,
    evaluated: EvaluatedParameter,
    label: &str,
) -> Result<SymbolicParameterValue, Diagnostic> {
    let EvaluatedType::Known(value_type) = evaluated.value_type else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            format!(
                "{label} dimension cannot be inferred from this expression; add an explicit dimension annotation"
            ),
        ));
    };
    Ok(SymbolicParameterValue {
        value: evaluated.value,
        value_type,
        expression: evaluated.expression,
        lineage: evaluated.lineage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbolic(value_type: ValueType) -> EvaluatedParameter {
        SymbolicParameterValue {
            value: Some(ValueLiteral::from_real(value_type.clone(), 0.0).unwrap()),
            value_type,
            expression: None,
            lineage: Some(ParameterLineage::Constant),
        }
        .into()
    }

    #[test]
    fn symbolic_arithmetic_preserves_complex_array_type() {
        let array = ValueType::scalar(
            ScalarDomain::Complex,
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
        )
        .array(3)
        .unwrap();
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        let evaluated = combine_parameters(
            "types.eqi",
            TextRange::new(0, 1),
            BinaryOp::Mul,
            symbolic(array.clone()),
            symbolic(real),
        )
        .unwrap();
        let inferred =
            infer_parameter_with_label("types.eqi", TextRange::new(0, 1), evaluated, "let alias")
                .unwrap();
        assert_eq!(inferred.value_type, array);
    }

    #[test]
    fn typed_zero_does_not_narrow_to_real_or_erase_array_axes() {
        for value_type in [
            ValueType::scalar(
                ScalarDomain::Complex,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
            ),
            ValueType::scalar(
                ScalarDomain::Real,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
            )
            .array(3)
            .unwrap(),
        ] {
            let error = coerce_parameter(
                "types.eqi",
                TextRange::new(0, 1),
                symbolic(value_type),
                ValueType::scalar(
                    ScalarDomain::Real,
                    DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
                ),
            )
            .unwrap_err();
            assert!(error.message().contains("requires a real scalar type"));
        }
    }

    #[test]
    fn symbolic_arithmetic_rejects_array_vector_substitution() {
        let scalar = ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
        );
        let vector = ValueType::shaped(
            ScalarDomain::Real,
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
            eqiora_core::ValueShape::new([3]).unwrap(),
            eqiora_core::ValueFrame::SpatialCartesian,
        )
        .unwrap();
        assert!(
            combine_parameters(
                "types.eqi",
                TextRange::new(0, 1),
                BinaryOp::Add,
                symbolic(scalar.array(3).unwrap()),
                symbolic(vector),
            )
            .is_err()
        );
    }

    #[test]
    fn complex_and_array_exponents_are_rejected_even_at_zero() {
        for value_type in [
            ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS),
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
                .array(3)
                .unwrap(),
        ] {
            assert!(
                require_dimensionless_exponent(
                    "types.eqi",
                    TextRange::new(0, 1),
                    &symbolic(value_type).value_type,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn deferred_dimensions_do_not_erase_complex_domain_or_array_roles() {
        let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap();
        let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        let deferred = combine_types(
            "types.eqi",
            TextRange::new(0, 1),
            BinaryOp::Pow,
            EvaluatedType::Known(ValueType::scalar(ScalarDomain::Complex, length)),
            EvaluatedType::Known(scalar.clone()),
            None,
        )
        .unwrap();
        assert_eq!(deferred.dimension(), None);
        assert_eq!(deferred.value_type().scalar_domain(), ScalarDomain::Complex);
        let mut evaluated = symbolic(scalar.clone());
        evaluated.value_type = deferred.clone();
        assert!(
            coerce_parameter(
                "types.eqi",
                TextRange::new(0, 1),
                evaluated,
                ValueType::scalar(ScalarDomain::Real, length)
            )
            .unwrap_err()
            .message()
            .contains("real scalar type")
        );
        let array = EvaluatedType::Known(scalar.array(3).unwrap());
        assert!(
            combine_types(
                "types.eqi",
                TextRange::new(0, 1),
                BinaryOp::Add,
                array.clone(),
                deferred.clone(),
                None,
            )
            .is_err()
        );
        let product = combine_types(
            "types.eqi",
            TextRange::new(0, 1),
            BinaryOp::Mul,
            array,
            deferred,
            None,
        )
        .unwrap();
        assert_eq!(product.dimension(), None);
        assert_eq!(product.value_type().scalar_domain(), ScalarDomain::Complex);
        assert_eq!(product.value_type().array_rank(), 1);
        assert_eq!(product.value_type().shape().extents()[0].get(), 3);
    }

    #[test]
    fn inferred_dimension_rejects_deferred_evaluation() {
        let error = infer_parameter_with_label(
            "deferred.eqi",
            TextRange::new(0, 1),
            EvaluatedParameter {
                value: None,
                value_type: EvaluatedType::Deferred(ValueType::scalar(
                    ScalarDomain::Real,
                    DimExponents::DIMENSIONLESS,
                )),
                bare_literal: false,
                expression: None,
                lineage: None,
            },
            "let alias",
        )
        .expect_err("Deferred dimension requires an annotation");

        assert!(error.message().contains("dimension cannot be inferred"));
        assert!(error.message().contains("explicit dimension annotation"));
    }
}
