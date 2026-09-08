use super::*;
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
        .expect("valid numeric scalar type")
        .array(3)
        .unwrap();
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
            .expect("valid numeric scalar type");
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
            )
            .expect("valid numeric scalar type"),
            ValueType::scalar(
                ScalarDomain::Real,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
            )
            .expect("valid numeric scalar type")
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
                )
                .expect("valid numeric scalar type"),
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
        )
        .expect("valid numeric scalar type");
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
            ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
                .expect("valid numeric scalar type"),
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
                .expect("valid numeric scalar type")
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
        let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
            .expect("valid numeric scalar type");
        let deferred = combine_types(
            "types.eqi",
            TextRange::new(0, 1),
            BinaryOp::Pow,
            EvaluatedType::Known(
                ValueType::scalar(ScalarDomain::Complex, length)
                    .expect("valid numeric scalar type"),
            ),
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
                ValueType::scalar(ScalarDomain::Real, length).expect("valid numeric scalar type")
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
                value_type: EvaluatedType::Deferred(
                    ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
                        .expect("valid numeric scalar type"),
                ),
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

#[cfg(test)]
mod exact_integer_tests {
    use super::*;

    fn closed(source: &str, integer: bool) -> Result<ValueLiteral, Diagnostic> {
        let source = format!("model M() {{ let value = {source}; }}");
        let document = eqiora_lang::parse("exact.eqi", &source)
            .into_document()
            .unwrap();
        let eqiora_lang::Item::Let(declaration) = &document.models()[0].items()[0] else {
            panic!("let")
        };
        super::super::super::closed_value(
            "exact.eqi",
            declaration.value(),
            ValueType::scalar(
                if integer {
                    ScalarDomain::Integer
                } else {
                    ScalarDomain::Real
                },
                DimExponents::DIMENSIONLESS,
            )
            .expect("valid numeric scalar type"),
        )
    }

    #[test]
    fn exact_integer_initializers_use_checked_shared_arithmetic() {
        for (source, expected) in [
            ("9007199254740993+1", 9007199254740994),
            ("-9223372036854775808", i64::MIN),
            ("quotient(-7,3)", -2),
            ("remainder(-7,3)", -1),
            ("to_integer(4)", 4),
        ] {
            assert_eq!(
                closed(source, true).unwrap().integer_scalar_value(),
                Some(expected),
                "{source}"
            );
        }
        for source in [
            "9223372036854775807+1",
            "-9223372036854775808-1",
            "quotient(-9223372036854775808,-1)",
            "remainder(1,0)",
            "to_integer(1.5)",
            "1/2",
            "to_integer(9223372036854775808)",
        ] {
            assert!(closed(source, true).is_err(), "{source}");
        }
        assert_eq!(
            closed("1/2", false)
                .unwrap()
                .real_scalar_value()
                .unwrap()
                .value(),
            0.5
        );
        assert_eq!(
            closed("to_real(9007199254740993)", false)
                .unwrap()
                .real_scalar_value()
                .unwrap()
                .value(),
            9007199254740992.0
        );
    }
}
