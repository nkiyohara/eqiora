use super::*;
use crate::kernel::ValueType;
use eqiora_core::ScalarDomain;

#[test]
fn arrays_of_vectors_are_not_spatial_tensors_with_the_same_extents() {
    let vector = ValueType::shaped(
        ScalarDomain::Complex,
        DimExponents::DIMENSIONLESS,
        ValueShape::new([2]).unwrap(),
        ValueFrame::SpatialCartesian,
    )
    .unwrap();
    let array = ExpressionType::new(vector.array(2).unwrap(), Some(volume("body")));
    let tensor = ExpressionType::new(
        ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap(),
        Some(volume("body")),
    );
    assert_eq!(array.shape(), tensor.shape());
    assert_eq!(array.frame(), tensor.frame());
    assert_ne!(array.value_type, tensor.value_type);
    assert_eq!(array.value_type.array_rank(), 1);
    assert_eq!(tensor.value_type.array_rank(), 0);
    assert!(additive(&array, &tensor).is_err());
    assert!(gradient(&array).is_err());
    assert!(divergence(&array).is_err());
    assert!(symmetric_part(&array).is_err());
    assert!(symmetric_part(&tensor).is_ok());

    let scalar = ExpressionType::scalar(DimExponents::DIMENSIONLESS, None);
    for result in [
        additive(&array, &array).unwrap(),
        multiply(&scalar, &array).unwrap(),
        multiply(&array, &scalar).unwrap(),
        divide(&array, &scalar).unwrap(),
    ] {
        assert_eq!(result.value_type, array.value_type);
    }
    let derivative = time_derivative(&array).unwrap();
    assert_eq!(derivative.value_type.array_rank(), 1);
    assert_eq!(derivative.shape(), array.shape());
    assert_eq!(derivative.value_type.scalar_domain(), ScalarDomain::Complex);
    assert_ne!(derivative.dimension(), array.dimension());
    let boundary = SpatialSupport::Boundary {
        domain: "wall",
        parent: "body",
        dimensions: 2,
    };
    let traced = trace(&array, Some(&boundary)).unwrap();
    assert_eq!(traced.value_type, array.value_type);
    assert_eq!(traced.support, Some(boundary.clone()));
    assert!(normal(&array, Some(&boundary)).is_err());
}

#[test]
fn nested_channel_arrays_preserve_axis_order_and_reject_zero_extents() {
    let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    assert!(scalar.clone().array(0).is_err());
    let nested = scalar.array(2).unwrap().array(3).unwrap();
    assert_eq!(nested.array_rank(), 2);
    assert_eq!(nested.shape(), &ValueShape::new([3, 2]).unwrap());
    assert_eq!(nested.frame(), ValueFrame::Invariant);
}
