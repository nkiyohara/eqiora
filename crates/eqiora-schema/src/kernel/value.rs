use eqiora_core::{DimExponents, ScalarDomain, ValueShape};

/// Coordinate-frame meaning of mathematical value components.
///
/// Version one intentionally admits only invariant values and components in
/// the model-global Cartesian spatial frame. Arbitrary local frames and frame
/// transforms require an explicit future contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueFrame {
    /// Components are unchanged by a Cartesian spatial frame change.
    Invariant,
    /// Components are expressed in the model-global Cartesian spatial frame.
    SpatialCartesian,
}

/// Checked mathematical value type. Support and activation belong to its use site.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValueType {
    scalar_domain: ScalarDomain,
    dimension: DimExponents,
    shape: ValueShape,
    frame: ValueFrame,
}

impl ValueType {
    /// Preserve the scalar domain and component meaning with a derived dimension.
    #[must_use]
    pub fn with_dimension(mut self, dimension: DimExponents) -> Self {
        self.dimension = dimension;
        self
    }
    /// Construct an invariant scalar type.
    #[must_use]
    pub fn scalar(scalar_domain: ScalarDomain, dimension: DimExponents) -> Self {
        Self {
            scalar_domain,
            dimension,
            shape: ValueShape::scalar(),
            frame: ValueFrame::Invariant,
        }
    }

    /// Construct an exact shaped mathematical type.
    ///
    /// # Errors
    /// Rejects an unrepresentable component count or a frame-bearing scalar.
    pub fn shaped(
        scalar_domain: ScalarDomain,
        dimension: DimExponents,
        shape: ValueShape,
        frame: ValueFrame,
    ) -> Result<Self, InvalidValueType> {
        if shape.component_count().is_none() {
            return Err(InvalidValueType::ComponentCountOverflow);
        }
        if shape.is_scalar() && frame != ValueFrame::Invariant {
            return Err(InvalidValueType::ScalarFrame);
        }
        Ok(Self {
            scalar_domain,
            dimension,
            shape,
            frame,
        })
    }

    /// Mathematical domain of each scalar component.
    #[must_use]
    pub const fn scalar_domain(&self) -> ScalarDomain {
        self.scalar_domain
    }

    /// Exact physical dimension of each component.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Ordered mathematical component extents.
    #[must_use]
    pub const fn shape(&self) -> &ValueShape {
        &self.shape
    }

    /// Coordinate-frame meaning of the components.
    #[must_use]
    pub const fn frame(&self) -> ValueFrame {
        self.frame
    }
}

/// Invalid mathematical shape/frame combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidValueType {
    /// Component count cannot be represented on this target.
    ComponentCountOverflow,
    /// A scalar cannot carry component-frame axes.
    ScalarFrame,
}

impl core::fmt::Display for InvalidValueType {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ComponentCountOverflow => "mathematical component count is not representable",
            Self::ScalarFrame => "a scalar must have an invariant component frame",
        })
    }
}

impl std::error::Error for InvalidValueType {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_domain_dimension_and_frame_are_independent_type_identity() {
        let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        let complex = ValueType::scalar(ScalarDomain::Complex, real.dimension());
        assert_ne!(real, complex);
        let channels = ValueType::shaped(
            ScalarDomain::Complex,
            real.dimension(),
            ValueShape::new([3]).unwrap(),
            ValueFrame::Invariant,
        )
        .unwrap();
        let vector = ValueType::shaped(
            ScalarDomain::Complex,
            real.dimension(),
            ValueShape::new([3]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        assert_ne!(channels, vector);
        assert_eq!(channels.shape(), vector.shape());
        let dimensioned = ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
        );
        assert_ne!(real, dimensioned);
    }

    #[test]
    fn malformed_shape_frame_types_are_not_constructible() {
        assert_eq!(
            ValueType::shaped(
                ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
                ValueShape::scalar(),
                ValueFrame::SpatialCartesian,
            ),
            Err(InvalidValueType::ScalarFrame)
        );
        assert_eq!(
            ValueType::shaped(
                ScalarDomain::Complex,
                DimExponents::DIMENSIONLESS,
                ValueShape::new([u32::MAX; 3]).unwrap(),
                ValueFrame::Invariant,
            ),
            Err(InvalidValueType::ComponentCountOverflow)
        );
    }
}
