use std::num::NonZeroU32;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::MeshTopology;

/// Mesh stratum carrying one accepted discrete value per entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscreteFieldAssociation {
    /// Canonical dimension-zero entity order.
    Vertex,
    /// Canonical top-dimensional cell order.
    Cell,
}

/// Closed component shape for a discrete field payload.
///
/// A scalar remains distinct from a one-component vector. Version one assigns
/// no basis, variance, symmetry, unit, or tensor convention to vector
/// components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscreteFieldShape {
    /// One scalar value per entity.
    Scalar,
    /// A fixed-width vector in entity-major component order.
    Vector {
        /// Positive component count.
        components: NonZeroU32,
    },
}

impl DiscreteFieldShape {
    /// Number of stored components per associated entity.
    ///
    /// # Errors
    /// Returns `EQ0809` when the portable `u32` component count cannot be
    /// represented by the local `usize` target.
    pub fn component_count(self) -> Result<usize, Diagnostic> {
        match self {
            Self::Scalar => Ok(1),
            Self::Vector { components } => usize::try_from(components.get()).map_err(|_| {
                invalid_discrete_field(
                    "discrete field component count is not representable on this target",
                )
            }),
        }
    }
}

/// Invariant-checked, mesh-associated, entity-major `f64` values.
///
/// This in-memory payload carries no mesh identity, source name, file path,
/// parser state, unit, or semantic-field binding. A portable artifact layer
/// must bind it to one exact mesh revision before assigning content identity.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteFieldPayload {
    association: DiscreteFieldAssociation,
    shape: DiscreteFieldShape,
    entity_count: usize,
    component_count: usize,
    values: Vec<f64>,
}

impl DiscreteFieldPayload {
    /// Check values against one mesh topology and take ownership of them.
    ///
    /// `Vertex` selects dimension zero. `Cell` selects the topology's
    /// top-dimensional stratum. Values are flat and entity-major: component
    /// `j` of entity `i` is `values[i * component_count + j]`.
    ///
    /// Every zero is normalized to positive zero before acceptance.
    ///
    /// # Errors
    /// Returns `EQ0809` when the selected stratum is absent or empty, shape
    /// arithmetic overflows, the value count differs from the exact required
    /// count, or any value is non-finite.
    pub fn new(
        topology: &dyn MeshTopology,
        association: DiscreteFieldAssociation,
        shape: DiscreteFieldShape,
        mut values: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let dimension = match association {
            DiscreteFieldAssociation::Vertex => 0,
            DiscreteFieldAssociation::Cell => topology.topological_dimension(),
        };
        let entity_count = topology.entity_count(dimension).ok_or_else(|| {
            invalid_discrete_field(format!(
                "discrete field association selects missing mesh stratum {dimension}",
            ))
        })?;
        if entity_count == 0 {
            return Err(invalid_discrete_field(
                "discrete field association selects an empty mesh stratum",
            ));
        }
        let component_count = shape.component_count()?;
        let required_values = entity_count.checked_mul(component_count).ok_or_else(|| {
            invalid_discrete_field("discrete field entity/component product overflows usize")
        })?;
        if values.len() != required_values {
            return Err(invalid_discrete_field(format!(
                "discrete field requires {required_values} values for {entity_count} entities and {component_count} components, received {}",
                values.len(),
            )));
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_discrete_field(
                "discrete field values must all be finite",
            ));
        }
        for value in &mut values {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Ok(Self {
            association,
            shape,
            entity_count,
            component_count,
            values,
        })
    }

    /// Mesh stratum carrying the values.
    #[must_use]
    pub const fn association(&self) -> DiscreteFieldAssociation {
        self.association
    }

    /// Component shape retained as part of future artifact identity.
    #[must_use]
    pub const fn component_shape(&self) -> DiscreteFieldShape {
        self.shape
    }

    /// Number of associated mesh entities.
    #[must_use]
    pub const fn entity_count(&self) -> usize {
        self.entity_count
    }

    /// Number of stored components per associated entity.
    #[must_use]
    pub const fn component_count(&self) -> usize {
        self.component_count
    }

    /// Flat, entity-major values.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Values belonging to one mesh entity, or `None` when out of range.
    #[must_use]
    pub fn entity_values(&self, entity: usize) -> Option<&[f64]> {
        if entity >= self.entity_count {
            return None;
        }
        let start = entity.checked_mul(self.component_count)?;
        let end = start.checked_add(self.component_count)?;
        self.values.get(start..end)
    }
}

fn invalid_discrete_field(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETE_FIELD, message)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use eqiora_core::diagnostic::codes;

    use super::{DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape};
    use crate::{EntityIncidence, MeshEntity, MeshTopology};

    #[derive(Debug)]
    struct CountsOnlyTopology {
        dimension: usize,
        counts: Vec<Option<usize>>,
    }

    impl MeshTopology for CountsOnlyTopology {
        fn topological_dimension(&self) -> usize {
            self.dimension
        }

        fn entity_count(&self, dimension: usize) -> Option<usize> {
            self.counts.get(dimension).copied().flatten()
        }

        fn incidence(
            &self,
            _entity: MeshEntity,
            _target_dimension: usize,
        ) -> Option<Vec<EntityIncidence>> {
            None
        }
    }

    fn topology() -> CountsOnlyTopology {
        CountsOnlyTopology {
            dimension: 3,
            counts: vec![Some(5), Some(9), Some(7), Some(2)],
        }
    }

    #[test]
    fn vertex_scalar_and_cell_vector_use_only_the_selected_stratum() {
        let vertex = DiscreteFieldPayload::new(
            &topology(),
            DiscreteFieldAssociation::Vertex,
            DiscreteFieldShape::Scalar,
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert_eq!(vertex.entity_count(), 5);
        assert_eq!(vertex.entity_values(4), Some(&[5.0][..]));
        assert_eq!(vertex.entity_values(5), None);

        let cell = DiscreteFieldPayload::new(
            &topology(),
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Vector {
                components: NonZeroU32::new(2).unwrap(),
            },
            vec![1.0, 2.0, 3.0, 4.0],
        )
        .unwrap();
        assert_eq!(cell.entity_count(), 2);
        assert_eq!(cell.entity_values(0), Some(&[1.0, 2.0][..]));
        assert_eq!(cell.entity_values(1), Some(&[3.0, 4.0][..]));
    }

    #[test]
    fn scalar_and_one_component_vector_remain_distinct() {
        let scalar = DiscreteFieldPayload::new(
            &topology(),
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Scalar,
            vec![1.0, 2.0],
        )
        .unwrap();
        let vector = DiscreteFieldPayload::new(
            &topology(),
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Vector {
                components: NonZeroU32::MIN,
            },
            vec![1.0, 2.0],
        )
        .unwrap();
        assert_ne!(scalar.component_shape(), vector.component_shape());
        assert_eq!(scalar.values(), vector.values());
    }

    #[test]
    fn zero_sign_is_normalized_before_ownership() {
        let payload = DiscreteFieldPayload::new(
            &topology(),
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Scalar,
            vec![-0.0, 0.0],
        )
        .unwrap();
        assert_eq!(payload.values()[0].to_bits(), 0.0_f64.to_bits());
        assert_eq!(payload.values()[1].to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn invalid_extent_and_values_fail_with_a_stable_code() {
        for result in [
            DiscreteFieldPayload::new(
                &topology(),
                DiscreteFieldAssociation::Cell,
                DiscreteFieldShape::Scalar,
                vec![1.0],
            ),
            DiscreteFieldPayload::new(
                &topology(),
                DiscreteFieldAssociation::Cell,
                DiscreteFieldShape::Scalar,
                vec![1.0, f64::NAN],
            ),
        ] {
            assert_eq!(result.unwrap_err().code(), codes::INVALID_DISCRETE_FIELD);
        }
    }

    #[test]
    fn absent_and_empty_strata_fail_closed() {
        for counts in [vec![Some(2)], vec![Some(2), None], vec![Some(2), Some(0)]] {
            let topology = CountsOnlyTopology {
                dimension: 1,
                counts,
            };
            let error = DiscreteFieldPayload::new(
                &topology,
                DiscreteFieldAssociation::Cell,
                DiscreteFieldShape::Scalar,
                Vec::new(),
            )
            .unwrap_err();
            assert_eq!(error.code(), codes::INVALID_DISCRETE_FIELD);
        }
    }

    #[test]
    fn component_and_entity_product_overflow_fails_before_length_use() {
        let topology = CountsOnlyTopology {
            dimension: 1,
            counts: vec![Some(1), Some(usize::MAX)],
        };
        let error = DiscreteFieldPayload::new(
            &topology,
            DiscreteFieldAssociation::Cell,
            DiscreteFieldShape::Vector {
                components: NonZeroU32::new(2).unwrap(),
            },
            Vec::new(),
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::INVALID_DISCRETE_FIELD);
        assert!(error.message().contains("overflows"));
    }
}
