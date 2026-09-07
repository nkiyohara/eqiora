use crate::entity::kinds;
use crate::{DimExponents, Id, ScalarDomain, ValueShape};

/// Coordinate-frame meaning of mathematical value components.
///
/// Spatial components use the model-global Cartesian frame. Channel-array
/// axes do not introduce a frame or change their element's frame.
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
    array_rank: usize,
    meaning: Meaning,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Meaning {
    Ordinary,
    Coordinates(Id<kinds::FiniteSpace>),
    Counts(Id<kinds::FiniteSpace>),
    Index {
        set: Id<kinds::IndexSet>,
        extent: u32,
    },
}

impl ValueType {
    /// Bounded ordinal tied to one exact IndexSet declaration.
    pub fn index(set: Id<kinds::IndexSet>, extent: u32) -> Result<Self, InvalidValueType> {
        if extent == 0 {
            return Err(InvalidValueType::ArrayExtent);
        }
        let mut value = Self::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        value.meaning = Meaning::Index { set, extent };
        Ok(value)
    }
    /// Exact nominal IndexSet identity, absent for ordinary integers.
    #[must_use]
    pub const fn index_set(&self) -> Option<Id<kinds::IndexSet>> {
        match self.meaning {
            Meaning::Index { set, .. } => Some(set),
            _ => None,
        }
    }
    /// Declared exclusive index bound, validated against the semantic declaration.
    #[must_use]
    pub const fn index_extent(&self) -> Option<u32> {
        match self.meaning {
            Meaning::Index { extent, .. } => Some(extent),
            _ => None,
        }
    }

    /// Signed integer coordinates in one exact finite basis (not a channel array).
    /// The semantic declaration owner validates the supplied extent against its labels.
    pub fn coordinates(
        space: Id<kinds::FiniteSpace>,
        extent: u32,
    ) -> Result<Self, InvalidValueType> {
        Self::finite(space, extent, false)
    }

    /// Nonnegative exact counts in one finite basis, distinct from signed coordinates.
    pub fn counts(space: Id<kinds::FiniteSpace>, extent: u32) -> Result<Self, InvalidValueType> {
        Self::finite(space, extent, true)
    }

    /// Nominal finite basis identity, absent for ordinary scalars and channel arrays.
    #[must_use]
    pub const fn finite_space(&self) -> Option<Id<kinds::FiniteSpace>> {
        match self.meaning {
            Meaning::Ordinary | Meaning::Index { .. } => None,
            Meaning::Coordinates(id) | Meaning::Counts(id) => Some(id),
        }
    }

    /// Whether this value carries the nonnegative count contract.
    #[must_use]
    pub const fn is_count(&self) -> bool {
        matches!(self.meaning, Meaning::Counts(_))
    }

    fn finite(
        space: Id<kinds::FiniteSpace>,
        extent: u32,
        counts: bool,
    ) -> Result<Self, InvalidValueType> {
        let shape = ValueShape::new([extent]).map_err(|_| InvalidValueType::ArrayExtent)?;
        Ok(Self {
            scalar_domain: ScalarDomain::Integer,
            dimension: DimExponents::DIMENSIONLESS,
            shape,
            frame: ValueFrame::Invariant,
            array_rank: 0,
            meaning: if counts {
                Meaning::Counts(space)
            } else {
                Meaning::Coordinates(space)
            },
        })
    }

    /// Promote scalar components to the smallest common domain without changing their roles.
    #[must_use]
    pub fn with_common_scalar_domain(mut self, other: &Self) -> Option<Self> {
        if self.meaning != other.meaning {
            return None;
        }
        self.scalar_domain = self.scalar_domain.common(other.scalar_domain)?;
        Some(self)
    }

    /// Wrap this complete element type in one ordered channel-array axis.
    ///
    /// # Errors
    /// Rejects a zero extent or an unrepresentable component count.
    pub fn array(self, extent: u32) -> Result<Self, InvalidValueType> {
        if self.finite_space().is_some() || self.index_set().is_some() {
            return Err(InvalidValueType::FiniteSpaceShape);
        }
        let shape = ValueShape::new(
            [extent]
                .into_iter()
                .chain(self.shape.extents().iter().map(|n| n.get())),
        )
        .map_err(|_| InvalidValueType::ArrayExtent)?;
        if shape.component_count().is_none() {
            return Err(InvalidValueType::ComponentCountOverflow);
        }
        Ok(Self {
            shape,
            array_rank: self.array_rank + 1,
            ..self
        })
    }

    /// Number of outer channel-array axes, distinct from inner spatial axes.
    #[must_use]
    pub const fn array_rank(&self) -> usize {
        self.array_rank
    }

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
            array_rank: 0,
            meaning: Meaning::Ordinary,
        }
    }

    /// Construct exact channel axes (invariant) or spatial axes (Cartesian).
    /// Wrap spatial elements with [`Self::array`] for arrays of vectors or tensors.
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
            meaning: Meaning::Ordinary,
            array_rank: if frame == ValueFrame::Invariant {
                shape.rank()
            } else {
                0
            },
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
    /// Finite basis coordinates cannot acquire implicit channel axes.
    FiniteSpaceShape,
    /// An array axis must contain at least one element.
    ArrayExtent,
    /// Component count cannot be represented on this target.
    ComponentCountOverflow,
    /// A scalar cannot carry component-frame axes.
    ScalarFrame,
}

impl core::fmt::Display for InvalidValueType {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::FiniteSpaceShape => "finite basis coordinates are not channel arrays",
            Self::ArrayExtent => "array extent must be positive",
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
        assert_eq!(
            real.clone().with_common_scalar_domain(&complex),
            Some(complex.clone())
        );
        assert_eq!(
            complex.clone().with_common_scalar_domain(&real),
            Some(complex)
        );
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
