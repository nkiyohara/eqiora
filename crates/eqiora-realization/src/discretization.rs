use std::num::{NonZeroU16, NonZeroUsize};

/// Discrete function-space family and approximation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Space {
    family: SpaceFamily,
}

impl Space {
    /// Continuous nodal Lagrange space of a strictly positive order.
    #[must_use]
    pub const fn continuous_lagrange(order: NonZeroU16) -> Self {
        Self {
            family: SpaceFamily::ContinuousLagrange { order },
        }
    }

    /// Hierarchical simplex P1 basis enriched by one cell-interior bubble.
    #[must_use]
    pub const fn simplex_p1_bubble() -> Self {
        Self {
            family: SpaceFamily::SimplexP1Bubble,
        }
    }

    /// One cell-local constant degree of freedom.
    #[must_use]
    pub const fn cell_constant() -> Self {
        Self {
            family: SpaceFamily::CellConstant,
        }
    }

    /// Declared family.
    #[must_use]
    pub const fn family(self) -> SpaceFamily {
        self.family
    }
}

/// Inspectable space family; it carries no field meaning or physical unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpaceFamily {
    /// Globally continuous nodal Lagrange basis.
    ContinuousLagrange {
        /// Polynomial order.
        order: NonZeroU16,
    },
    /// Hierarchical simplex P1 basis plus one normalized cell bubble.
    SimplexP1Bubble,
    /// Cell-local piecewise constant basis.
    CellConstant,
}

/// Spatial numerical method family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiscretizationMethod {
    /// Continuous Galerkin finite elements.
    ContinuousGalerkin,
    /// Conservative cell-centered finite volumes.
    CellCenteredFiniteVolume,
}

/// Topology/geometry family admitted by a complete realization path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshKind {
    /// Cartesian topology generated from canonical box bounds.
    GeneratedCartesian,
    /// Content-addressed, fixed-connectivity affine simplex topology.
    ImportedAffineSimplicial,
}

/// Content identity of one independently versioned mesh artifact.
///
/// The Realization layer carries identity, not serialized coordinates or a
/// filesystem location. Artifact adapters reconstruct and validate the mesh
/// bytes before numerical lowering receives a typed mesh revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshArtifactReference([u8; 32]);

impl MeshArtifactReference {
    /// Construct from complete SHA-256 bytes.
    #[must_use]
    pub const fn from_sha256(value: [u8; 32]) -> Self {
        Self(value)
    }

    /// Complete SHA-256 bytes.
    #[must_use]
    pub const fn sha256(self) -> [u8; 32] {
        self.0
    }
}

/// Mesh selection owned by realization rather than by a semantic Domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeshPolicy {
    /// Generate a uniform mesh with this count on every topological axis.
    GeneratedUniform {
        /// Non-zero cells per axis.
        cells_per_axis: NonZeroUsize,
    },
    /// Use one independently versioned affine-simplex mesh artifact.
    ImportedSimplicial {
        /// Content identity resolved by the artifact/control plane.
        artifact: MeshArtifactReference,
    },
}

impl MeshPolicy {
    /// Mesh family required by this policy.
    #[must_use]
    pub const fn kind(self) -> MeshKind {
        match self {
            Self::GeneratedUniform { .. } => MeshKind::GeneratedCartesian,
            Self::ImportedSimplicial { .. } => MeshKind::ImportedAffineSimplicial,
        }
    }
}

/// Explicit integration policy, never hidden inside a mesh or physics relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuadraturePolicy {
    /// Tensor-product Gauss--Legendre points on each axis.
    GaussLegendre {
        /// Non-zero points on each axis.
        points_per_axis: NonZeroUsize,
    },
    /// Cell centroid rule used by the v0 finite-volume path.
    CellCentroid,
    /// Centroid rule on an affine simplex reference cell.
    SimplexCentroid,
    /// Positive triangle rule obtained by a Duffy transform of Gauss--Legendre points.
    TriangleDuffyGaussLegendre {
        /// Non-zero Gauss--Legendre points on each Duffy coordinate.
        points_per_axis: NonZeroUsize,
    },
    /// Dimension-explicit simplex rule obtained by a Duffy transform.
    ///
    /// The spatial dimension belongs to the policy so a realized tetrahedral
    /// rule cannot silently drift into a triangle rule, or vice versa.
    SimplexDuffyGaussLegendre {
        /// Non-zero spatial dimension of the reference simplex.
        spatial_dimension: NonZeroUsize,
        /// Non-zero Gauss--Legendre points on each Duffy coordinate.
        points_per_axis: NonZeroUsize,
    },
}

/// Method, mesh, and integration choices. Space remains a sibling contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Discretization {
    method: DiscretizationMethod,
    mesh: MeshPolicy,
    quadrature: QuadraturePolicy,
}

impl Discretization {
    /// Construct a discretization choice without applying method/space policy.
    #[must_use]
    pub const fn new(
        method: DiscretizationMethod,
        mesh: MeshPolicy,
        quadrature: QuadraturePolicy,
    ) -> Self {
        Self {
            method,
            mesh,
            quadrature,
        }
    }

    /// Numerical method.
    #[must_use]
    pub const fn method(self) -> DiscretizationMethod {
        self.method
    }

    /// Mesh policy.
    #[must_use]
    pub const fn mesh(self) -> MeshPolicy {
        self.mesh
    }

    /// Quadrature policy.
    #[must_use]
    pub const fn quadrature(self) -> QuadraturePolicy {
        self.quadrature
    }
}
