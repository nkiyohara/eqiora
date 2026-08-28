//! Revision-scoped geometry identity.

/// Exact content identity of one immutable geometry revision.
///
/// The bytes are supplied by the owner of the canonical geometry encoding.
/// This L2 crate compares and carries them but neither defines that encoding
/// nor recomputes its digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeometryRevisionReference([u8; 32]);

impl GeometryRevisionReference {
    /// Construct an exact reference from canonical geometry digest bytes.
    #[must_use]
    pub const fn from_digest_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Canonical geometry digest bytes.
    #[must_use]
    pub const fn digest_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Entity local to exactly one [`GeometryRevisionTopology`].
///
/// Like a mesh entity, the index has no meaning without its owning revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeometryEntity {
    dimension: usize,
    index: usize,
}

impl GeometryEntity {
    /// Construct a revision-local geometry entity.
    #[must_use]
    pub const fn new(dimension: usize, index: usize) -> Self {
        Self { dimension, index }
    }

    /// Topological dimension.
    #[must_use]
    pub const fn dimension(self) -> usize {
        self.dimension
    }

    /// Zero-based index in the geometry revision's dimension stratum.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Closed entity strata for one exact geometry revision.
///
/// This is intentionally less than a geometry-kernel interface. It supplies
/// only enough information to prove that correspondence members exist in the
/// referenced revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryRevisionTopology {
    reference: GeometryRevisionReference,
    entity_counts: Box<[usize]>,
}

impl GeometryRevisionTopology {
    /// Close the entity strata of one exact geometry revision.
    ///
    /// `entity_counts[d]` is the number of entities with topological
    /// dimension `d`; consequently, the last index is the revision's
    /// topological dimension.
    ///
    /// Lower-dimensional strata may be zero: the bounded contract catalogs
    /// bodies and codimension-one boundaries without inventing vertices or
    /// corners. The top and codimension-one strata must be non-empty.
    ///
    /// # Errors
    /// Returns an error when the geometry dimension is too small.
    /// below dimension one, or `EmptyGeometryStratum` when the top or
    /// codimension-one stratum is empty.
    pub fn new(
        reference: GeometryRevisionReference,
        entity_counts: Vec<usize>,
    ) -> Result<Self, crate::correspondence::GeometryCorrespondenceError> {
        if entity_counts.len() < 2 {
            return Err(
                crate::correspondence::GeometryCorrespondenceError::GeometryDimensionTooSmall,
            );
        }
        let top = entity_counts.len() - 1;
        if let Some(dimension) = [top - 1, top]
            .into_iter()
            .find(|&dimension| entity_counts[dimension] == 0)
        {
            return Err(
                crate::correspondence::GeometryCorrespondenceError::EmptyGeometryStratum {
                    dimension,
                },
            );
        }
        Ok(Self {
            reference,
            entity_counts: entity_counts.into_boxed_slice(),
        })
    }

    /// Exact revision identity.
    #[must_use]
    pub const fn reference(&self) -> GeometryRevisionReference {
        self.reference
    }

    /// Topological dimension of the geometry revision.
    #[must_use]
    pub fn topological_dimension(&self) -> usize {
        self.entity_counts.len() - 1
    }

    /// Entity count for a declared dimension stratum.
    #[must_use]
    pub fn entity_count(&self, dimension: usize) -> Option<usize> {
        self.entity_counts.get(dimension).copied()
    }

    pub(crate) fn contains(&self, entity: GeometryEntity) -> bool {
        self.entity_count(entity.dimension)
            .is_some_and(|count| entity.index < count)
    }
}

/// Boundary orientation derived outward from its exact parent body.
///
/// This zero-sized proof marker carries no sign, vector, or vertex ordering.
/// Numerical normal signs are computed later from geometry plus oriented mesh
/// incidence; mesh [`eqiora_meshing::OrientationCode`] values remain local
/// permutation codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ParentOutward;
