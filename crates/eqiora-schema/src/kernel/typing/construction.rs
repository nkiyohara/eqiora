use super::{ExpressionType, TypeViolation, additive};
use eqiora_core::{ScalarDomain, ValueShape, ValueType};

/// Construct one outer channel axis with complete elements and compatible exact supports.
pub fn array<I: Clone + Eq>(
    elements: &[ExpressionType<I>],
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let Some(first) = elements.first() else {
        return Err(TypeViolation::EmptyArray);
    };
    let mut element = first.clone();
    for next in &elements[1..] {
        element = additive(&element, next)?;
    }
    let extent = u32::try_from(elements.len()).map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    element.value_type = element
        .value_type
        .array(extent)
        .map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    Ok(element)
}

/// Select an exact outer channel; Cartesian component axes are not channel axes.
pub fn index<I: Clone>(
    value: ExpressionType<I>,
    index: u32,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let channels = value.value_type.array_rank();
    if channels == 0 {
        return Err(TypeViolation::IndexRequiresArray);
    }
    let extents = value.shape().extents();
    if index >= extents[0].get() {
        return Err(TypeViolation::IndexOutOfBounds);
    }
    let shape = ValueShape::new(extents[channels..].iter().map(|n| n.get()))
        .map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    let mut element = ValueType::shaped(
        value.value_type.scalar_domain(),
        value.dimension(),
        shape,
        value.frame(),
    )
    .map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    for extent in extents[1..channels].iter().rev() {
        element = element
            .array(extent.get())
            .map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    }
    Ok(ExpressionType::new(element, value.support))
}

/// Construct a complex scalar, preserving physical dimension and compatible support.
pub fn complex<I: Clone + Eq>(
    real: ExpressionType<I>,
    imag: ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    if [&real, &imag].iter().any(|value| {
        !value.shape().is_scalar() || value.value_type.scalar_domain() != ScalarDomain::Real
    }) {
        return Err(TypeViolation::ComplexRequiresRealScalars);
    }
    let result = additive(&real, &imag)?;
    Ok(ExpressionType::new(
        ValueType::scalar(ScalarDomain::Complex, result.dimension()),
        result.support,
    ))
}
