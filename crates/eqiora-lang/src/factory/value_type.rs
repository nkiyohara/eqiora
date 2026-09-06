use super::{AstConstructionError, SourceAstFactory, checked_range, validate_expression};
use crate::{TextRange, ValueTypeSyntax, ValueTypeSyntaxKind};

impl SourceAstFactory {
    /// Construct a bounded mathematical source type without parsing text.
    ///
    /// # Errors
    /// Rejects invalid scalar components, extents, nesting, expressions or ranges.
    pub fn value_type(
        kind: ValueTypeSyntaxKind,
        range: TextRange,
    ) -> Result<ValueTypeSyntax, AstConstructionError> {
        let result = ValueTypeSyntax {
            kind,
            range: checked_range(range)?,
        };
        let mut current = &result;
        let mut count = 1_u64;
        for _ in 0..256 {
            let (element, extents) = match current.kind() {
                ValueTypeSyntaxKind::Scalar { dimension, .. } => {
                    validate_expression(dimension)?;
                    return Ok(result);
                }
                ValueTypeSyntaxKind::Vector { scalar, extent } => {
                    require_scalar(scalar)?;
                    (scalar.as_ref(), std::slice::from_ref(extent))
                }
                ValueTypeSyntaxKind::Tensor { scalar, extents } => {
                    require_scalar(scalar)?;
                    if !(2..=4).contains(&extents.len()) {
                        return Err(AstConstructionError::new(
                            "tensor requires two to four extents; use vector for one spatial axis",
                        ));
                    }
                    (scalar.as_ref(), extents.as_slice())
                }
                ValueTypeSyntaxKind::Array { element, extent } => {
                    (element.as_ref(), std::slice::from_ref(extent))
                }
            };
            for extent in extents {
                if *extent == 0 {
                    return Err(AstConstructionError::new(
                        "type extent must be a positive u32 integer",
                    ));
                }
                count = count.checked_mul(u64::from(*extent)).filter(|n| *n <= 65_536)
                    .ok_or_else(|| AstConstructionError::new(
                        "source resource limit exceeded: mathematical type has more than 65536 elements",
                    ))?;
            }
            current = element;
        }
        Err(AstConstructionError::new(
            "source resource limit exceeded: type nesting exceeds 256",
        ))
    }
}

fn require_scalar(value: &ValueTypeSyntax) -> Result<(), AstConstructionError> {
    if value.is_scalar() {
        Ok(())
    } else {
        Err(AstConstructionError::new(
            "vector and tensor components require a scalar type",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExprKind, Item, format, parse};
    use eqiora_core::ScalarDomain;

    #[test]
    fn component_parameters_share_checked_type_constructors() {
        let checked = eqiora_core::ValueType::scalar(
            ScalarDomain::Complex,
            eqiora_core::DimExponents::DIMENSIONLESS,
        )
        .array(3)
        .unwrap();
        let value_type = ValueTypeSyntax::from_checked(&checked).unwrap();
        let declaration = SourceAstFactory::component_parameter(
            crate::VisibilitySyntax::Public,
            "channels",
            value_type,
            None,
            TextRange::new(0, 0),
        )
        .unwrap();
        assert_eq!(declaration.value_type().to_source(), "array<complex<1>, 3>");
        let document = parse(
            "component.eqi",
            "component C { public parameter channels: array<complex<1>, 3>; }",
        )
        .into_document()
        .unwrap();
        let source = format(&document);
        assert!(source.contains("parameter channels: array<complex<1>, 3>;"));
        assert_eq!(
            format(&parse("reparsed.eqi", &source).into_document().unwrap()),
            source
        );
    }

    #[test]
    fn native_array_nesting_stops_at_the_source_depth_limit() {
        let range = TextRange::new(0, 0);
        let dimension = SourceAstFactory::expression(ExprKind::Number(1.0), range).unwrap();
        let mut value = ValueTypeSyntax::real(dimension);
        for _ in 0..255 {
            value = SourceAstFactory::value_type(
                ValueTypeSyntaxKind::Array {
                    element: Box::new(value),
                    extent: 1,
                },
                range,
            )
            .unwrap();
        }
        assert!(
            SourceAstFactory::value_type(
                ValueTypeSyntaxKind::Array {
                    element: Box::new(value),
                    extent: 1
                },
                range,
            )
            .is_err()
        );
    }

    #[test]
    fn native_types_share_source_constructor_rules_and_canonical_emission() {
        let range = TextRange::new(0, 0);
        let scalar = SourceAstFactory::value_type(
            ValueTypeSyntaxKind::Scalar {
                domain: ScalarDomain::Complex,
                dimension: SourceAstFactory::expression(ExprKind::Name("V".into()), range).unwrap(),
            },
            range,
        )
        .unwrap();
        let vector = SourceAstFactory::value_type(
            ValueTypeSyntaxKind::Vector {
                scalar: Box::new(scalar.clone()),
                extent: 2,
            },
            range,
        )
        .unwrap();
        let array = SourceAstFactory::value_type(
            ValueTypeSyntaxKind::Array {
                element: Box::new(vector.clone()),
                extent: 3,
            },
            range,
        )
        .unwrap();
        let parameter = SourceAstFactory::parameter(
            "channels",
            array,
            SourceAstFactory::expression(ExprKind::Number(0.0), range).unwrap(),
            range,
        )
        .unwrap();
        let parsed = parse(
            "types.eqi",
            "model M { parameter channels: array<vector<complex<V>, 2>, 3> = 0; }",
        )
        .into_document()
        .unwrap();
        let Item::Parameter(source) = &parsed.models()[0].items()[0] else {
            panic!("parameter");
        };
        assert_eq!(
            parameter.value_type().scalar_domain(),
            source.value_type().scalar_domain()
        );
        let model = SourceAstFactory::model(
            parsed.models()[0].visibility(),
            "M",
            vec![Item::Parameter(parameter)],
            range,
        )
        .unwrap();
        let native = SourceAstFactory::document(Vec::new(), Vec::new(), vec![model]).unwrap();
        assert_eq!(format(&native), format(&parsed));

        for kind in [
            ValueTypeSyntaxKind::Vector {
                scalar: Box::new(vector.clone()),
                extent: 2,
            },
            ValueTypeSyntaxKind::Array {
                element: Box::new(scalar.clone()),
                extent: 0,
            },
            ValueTypeSyntaxKind::Array {
                element: Box::new(vector),
                extent: 32_769,
            },
            ValueTypeSyntaxKind::Tensor {
                scalar: Box::new(scalar.clone()),
                extents: vec![2],
            },
            ValueTypeSyntaxKind::Tensor {
                scalar: Box::new(scalar),
                extents: vec![2; 5],
            },
        ] {
            assert!(SourceAstFactory::value_type(kind, range).is_err());
        }
    }
}
