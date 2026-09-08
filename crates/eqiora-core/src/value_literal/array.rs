//! Exact complete values on outer channel axes.
use super::*;
use crate::ValueShape;

impl ValueLiteral {
    /// Pack complete compatible elements into one outer channel axis.
    /// Real elements may embed into complex elements; no scalar-to-array broadcast occurs.
    ///
    /// # Errors
    /// Rejects empty inputs, incompatible types, invalid extents or allocation failure.
    pub fn array(elements: &[&Self]) -> Result<Self, InvalidValueLiteral> {
        let first = elements
            .first()
            .ok_or(InvalidValueLiteral::ComponentCount)?;
        let mut element_type = first.value_type().clone();
        for element in &elements[1..] {
            let next = element.value_type();
            if element_type.dimension() != next.dimension()
                || element_type.shape() != next.shape()
                || element_type.frame() != next.frame()
                || element_type.array_rank() != next.array_rank()
            {
                return Err(InvalidValueLiteral::ScalarDomain);
            }
            element_type = element_type
                .with_common_scalar_domain(next)
                .ok_or(InvalidValueLiteral::ScalarDomain)?;
        }
        let extent =
            u32::try_from(elements.len()).map_err(|_| InvalidValueLiteral::ComponentCount)?;
        let value_type = element_type
            .array(extent)
            .map_err(|_| InvalidValueLiteral::ComponentCount)?;
        if elements.iter().all(|value| value.is_zero()) {
            return if value_type.scalar_domain() == ScalarDomain::Integer {
                Self::from_integer(value_type, 0)
            } else {
                Self::from_real(value_type, 0.0)
            };
        }
        match value_type.scalar_domain() {
            ScalarDomain::Integer => Self::integer(
                value_type,
                elements.iter().flat_map(|value| {
                    value
                        .integer_components()
                        .expect("checked common integer domain")
                }),
            ),
            ScalarDomain::Real | ScalarDomain::Complex => Self::new(
                value_type,
                elements
                    .iter()
                    .flat_map(|value| value.components().expect("checked common floating domain")),
            ),
            ScalarDomain::Boolean | ScalarDomain::Enum => Err(InvalidValueLiteral::ScalarDomain),
        }
    }

    /// Select a complete element along the outermost channel axis.
    /// Spatial tensor axes are never interpreted as channel axes.
    ///
    /// # Errors
    /// Rejects non-arrays, out-of-range indices or allocation failure.
    pub fn index(&self, index: u32) -> Result<Self, InvalidValueLiteral> {
        let value_type = self.value_type();
        let channels = value_type.array_rank();
        if channels == 0 {
            return Err(InvalidValueLiteral::ComponentCount);
        }
        let extents = value_type.shape().extents();
        if index >= extents[0].get() {
            return Err(InvalidValueLiteral::ComponentCount);
        }
        let shape = ValueShape::new(extents[channels..].iter().map(|n| n.get()))
            .map_err(|_| InvalidValueLiteral::ComponentCount)?;
        let mut element = ValueType::shaped(
            value_type.scalar_domain(),
            value_type.dimension(),
            shape,
            value_type.frame(),
        )
        .map_err(|_| InvalidValueLiteral::ScalarDomain)?;
        for extent in extents[1..channels].iter().rev() {
            element = element
                .array(extent.get())
                .map_err(|_| InvalidValueLiteral::ComponentCount)?;
        }
        let count = element
            .shape()
            .component_count()
            .ok_or(InvalidValueLiteral::ComponentCount)?;
        let start = (index as usize)
            .checked_mul(count)
            .ok_or(InvalidValueLiteral::ComponentCount)?;
        if self.is_zero() {
            return if element.scalar_domain() == ScalarDomain::Integer {
                Self::from_integer(element, 0)
            } else {
                Self::from_real(element, 0.0)
            };
        }
        match element.scalar_domain() {
            ScalarDomain::Integer => Self::integer(
                element,
                (start..start + count)
                    .map(|i| self.integer_component(i).expect("checked element slice")),
            ),
            ScalarDomain::Real | ScalarDomain::Complex => Self::new(
                element,
                (start..start + count).map(|i| self.component(i).expect("checked element slice")),
            ),
            ScalarDomain::Boolean | ScalarDomain::Enum => Err(InvalidValueLiteral::ScalarDomain),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimExponents, ValueFrame};
    #[test]
    fn nested_channels_retain_exact_integer_order_and_element_roles() {
        let integer = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let a = ValueLiteral::from_integer(integer.clone(), 9_007_199_254_740_993).unwrap();
        let b = ValueLiteral::from_integer(integer, i64::MIN).unwrap();
        let row = ValueLiteral::array(&[&a, &b]).unwrap();
        let matrix = ValueLiteral::array(&[&row, &row]).unwrap();
        assert_eq!(matrix.value_type().array_rank(), 2);
        assert_eq!(matrix.index(1).unwrap().index(0).unwrap(), a);
        assert_eq!(matrix.index(0).unwrap().index(1).unwrap(), b);
        assert!(matrix.index(2).is_err());
        assert!(a.index(0).is_err());
        assert!(ValueLiteral::array(&[]).is_err());
        let vector_type = ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let vector = ValueLiteral::new(vector_type, [(1., 2.), (3., 4.)]).unwrap();
        assert_eq!(
            ValueLiteral::array(&[&vector]).unwrap().index(0).unwrap(),
            vector
        );
        assert!(vector.index(0).is_err());
    }
    #[test]
    fn compact_zero_channels_do_not_expand_shape_sized_buffers() {
        let ty = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
            .array(u32::MAX)
            .unwrap();
        let zero = ValueLiteral::from_real(ty, 0.0).unwrap();
        let array = ValueLiteral::array(&[&zero, &zero]).unwrap();
        assert!(matches!(array.payload, Payload::Zero));
        assert_eq!(array.index(1).unwrap(), zero);
    }
}
