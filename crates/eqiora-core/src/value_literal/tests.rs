use super::*;
use crate::{DimExponents, ValueFrame, ValueShape};

fn real() -> ValueType {
    ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
}
fn complex() -> ValueType {
    ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
}

#[test]
fn complex_and_spatial_channel_values_retain_every_ordered_component() {
    let scalar = ValueLiteral::new(complex(), [(2.0, -3.0)]).unwrap();
    assert_eq!(scalar.component(0), Some((2.0, -3.0)));
    assert_eq!(scalar.real_scalar_value(), None);
    let element = ValueType::shaped(
        ScalarDomain::Complex,
        DimExponents::DIMENSIONLESS,
        ValueShape::new([2]).unwrap(),
        ValueFrame::SpatialCartesian,
    )
    .unwrap();
    let ty = element.array(3).unwrap();
    // Three ordered channels, each with two Cartesian components.
    let expected = [
        (1.0, 2.0),
        (3.0, 4.0),
        (5.0, 6.0),
        (7.0, 8.0),
        (9.0, 10.0),
        (11.0, 12.0),
    ];
    let value = ValueLiteral::new(ty.clone(), expected).unwrap();
    assert_eq!(value.value_type(), &ty);
    assert_eq!(value.component_count(), 6);
    assert_eq!(value.components().collect::<Vec<_>>(), expected);
    assert_eq!(value.components().next_back(), Some((11.0, 12.0)));
    assert_eq!(value.component(6), None);
    assert_eq!(value.real_scalar_value(), None);
    assert!(!value.is_zero());
}

#[test]
fn exact_real_scalar_extraction_does_not_erase_shape_or_domain() {
    let value = ValueLiteral::try_from(DynQuantity::new(7.0, real().dimension())).unwrap();
    assert_eq!(
        value.real_scalar_value(),
        Some(DynQuantity::new(7.0, real().dimension()))
    );
    for ty in [real().array(1).unwrap(), complex()] {
        let value = ValueLiteral::new(ty, [(7.0, 0.0)]).unwrap();
        assert_eq!(value.real_scalar_value(), None);
    }
    assert_eq!(
        ValueLiteral::from_real(complex(), 5.0)
            .unwrap()
            .component(0),
        Some((5.0, 0.0))
    );
}

#[test]
fn zero_is_canonical_compact_and_does_not_broadcast_nonzero_scalars() {
    let ty = complex().array(u32::MAX).unwrap();
    let zero = ValueLiteral::from_real(ty, -0.0).unwrap();
    assert!(matches!(zero.payload, Payload::Zero));
    assert_eq!(zero.component_count(), u32::MAX as usize);
    assert_eq!(zero.component(u32::MAX as usize - 1), Some((0.0, 0.0)));
    assert_eq!(zero.component(u32::MAX as usize), None);
    assert_eq!(zero.components().len(), u32::MAX as usize);
    let ty = real().array(3).unwrap();
    let explicit = ValueLiteral::new(ty.clone(), [(-0.0, -0.0); 3]).unwrap();
    assert_eq!(explicit, ValueLiteral::from_real(ty.clone(), 0.0).unwrap());
    assert!(explicit.is_zero());
    for (r, i) in explicit.components() {
        assert_eq!(r.to_bits(), 0.0_f64.to_bits());
        assert_eq!(i.to_bits(), 0.0_f64.to_bits());
    }
    let mixed = ValueLiteral::new(ty.clone(), [(-0.0, -0.0), (1.0, -0.0), (-0.0, 0.0)]).unwrap();
    assert_eq!(mixed.component(2).unwrap().0.to_bits(), 0.0_f64.to_bits());
    assert_eq!(mixed.component(1).unwrap().1.to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        ValueLiteral::from_real(ty, 1.0),
        Err(InvalidValueLiteral::NonzeroShape)
    );
}

#[test]
fn component_cardinality_is_checked_without_unbounded_collection() {
    let ty = real().array(2).unwrap();
    for input in [vec![], vec![(1.0, 0.0)], vec![(1.0, 0.0); 3]] {
        assert_eq!(
            ValueLiteral::new(ty.clone(), input),
            Err(InvalidValueLiteral::ComponentCount)
        );
    }
    let mut reads = 0;
    let input = std::iter::from_fn(|| {
        reads += 1;
        Some((0.0, 0.0))
    });
    assert_eq!(
        ValueLiteral::new(ty, input),
        Err(InvalidValueLiteral::ComponentCount)
    );
    assert_eq!(reads, 3);
    let huge = real().array(u32::MAX).unwrap();
    assert_eq!(
        ValueLiteral::new(huge, [(1.0, 0.0)]),
        Err(InvalidValueLiteral::ComponentCount)
    );
}

#[test]
fn invalid_components_cannot_enter_the_value() {
    assert_eq!(
        ValueLiteral::new(real(), [(0.0, 1.0)]),
        Err(InvalidValueLiteral::ImaginaryInReal)
    );
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for pair in [(invalid, 0.0), (0.0, invalid)] {
            assert_eq!(
                ValueLiteral::new(complex(), [pair]),
                Err(InvalidValueLiteral::NonFinite)
            );
        }
        assert_eq!(
            ValueLiteral::from_real(real(), invalid),
            Err(InvalidValueLiteral::NonFinite)
        );
    }
}
