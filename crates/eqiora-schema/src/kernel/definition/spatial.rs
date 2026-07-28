//! What a spatial Domain is: its extent, its boundaries, and how it names one.
//!
//! Continuous geometry is Model meaning. A Domain either describes a box
//! outright or names an authored geometry by digest, and in both cases the
//! shape is settled here rather than by whichever mesh happens to realize it.

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id};

use super::*;

/// One finite Cartesian coordinate interval in coherent SI length units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisBounds {
    lower: DynQuantity,
    upper: DynQuantity,
}

impl AxisBounds {
    /// Construct finite, increasing length bounds.
    ///
    /// # Errors
    /// Returns `EQ0302` when either value is not a length, is non-finite, or
    /// does not form an increasing interval.
    pub fn new(lower: DynQuantity, upper: DynQuantity) -> Result<Self, Diagnostic> {
        let length = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        if lower.dim() != length || upper.dim() != length {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian axis bounds must have physical dimension length",
            ));
        }
        if !lower.value().is_finite()
            || !upper.value().is_finite()
            || upper.value() <= lower.value()
        {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian axis bounds must be finite and strictly increasing",
            ));
        }
        Ok(Self { lower, upper })
    }

    /// Lower coordinate in coherent SI units.
    #[must_use]
    pub const fn lower(self) -> DynQuantity {
        self.lower
    }

    /// Upper coordinate in coherent SI units.
    #[must_use]
    pub const fn upper(self) -> DynQuantity {
        self.upper
    }
}

/// Outward side of one Cartesian coordinate axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundarySide {
    /// Side at the lower coordinate bound.
    Lower,
    /// Side at the upper coordinate bound.
    Upper,
}

/// Content identity of one authored geometry, as raw digest bytes.
///
/// The Kernel names a geometry rather than describing one. Carrying the digest
/// instead of the shape keeps a Model's meaning exact while leaving the
/// geometry artifact outside the Kernel, and it is why two Realizations that
/// differ only in mesh are still one Model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeometryDigest([u8; 32]);

impl GeometryDigest {
    /// One geometry identity from its exact digest bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact digest bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical continuous-domain shape, independent of any mesh.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DomainKind {
    /// Identity-only domain retained for non-spatial and schema-defined uses.
    Abstract,
    /// Runtime-dimensional Cartesian box in physical space.
    CartesianBox { bounds: Vec<AxisBounds> },
    /// One oriented side of a parent Cartesian box. The parent is supplied by
    /// exactly one `BoundaryOf` graph edge.
    CartesianBoundary { axis: usize, side: BoundarySide },
    /// One region of an authored geometry, named by content digest and by an
    /// entity set within it. The shape is the geometry's; this Domain selects
    /// part of it and gives that part meaning.
    GeometryRegion {
        geometry: GeometryDigest,
        entity_set: String,
    },
    /// One boundary of a parent geometry region, named by an entity set of the
    /// same geometry. The parent is supplied by exactly one `BoundaryOf` graph
    /// edge, so the digest is never repeated here: a boundary free to name a
    /// different geometry from its parent would be a boundary of nothing.
    GeometryBoundary { entity_set: String },
    /// One nominal scalar conserving domain. The Domain ID is part of the
    /// physical type; dimensions alone never make two domains compatible.
    ScalarPhysical {
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    /// One nominal field-valued boundary connector. The Domain ID plus closed
    /// trace/flux role is the exact quantity identity.
    BoundaryPhysical {
        connector: BoundaryPhysicalConnector,
    },
}

/// Domain definition. Continuous geometry is model meaning; meshes and
/// geometry maps remain realization concerns.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainDef {
    id: Id<kinds::Domain>,
    kind: DomainKind,
}

impl DomainDef {
    /// Construct an abstract domain for schema-defined or non-spatial use.
    #[must_use]
    pub const fn new(id: Id<kinds::Domain>) -> Self {
        Self {
            id,
            kind: DomainKind::Abstract,
        }
    }

    /// Construct a non-empty Cartesian box of arbitrary runtime dimension.
    ///
    /// # Errors
    /// Returns `EQ0302` for an empty axis set.
    pub fn cartesian_box(
        id: Id<kinds::Domain>,
        bounds: Vec<AxisBounds>,
    ) -> Result<Self, Diagnostic> {
        if bounds.is_empty() {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian Domain requires at least one coordinate axis",
            )
            .with_graph_path(kernel_path(id.erase())));
        }
        Ok(Self {
            id,
            kind: DomainKind::CartesianBox { bounds },
        })
    }

    /// Construct one oriented boundary selector. Whole-model validation
    /// checks the axis against its unique `BoundaryOf` parent.
    #[must_use]
    pub const fn cartesian_boundary(
        id: Id<kinds::Domain>,
        axis: usize,
        side: BoundarySide,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::CartesianBoundary { axis, side },
        }
    }

    /// Construct one region selected from an authored geometry.
    ///
    /// # Errors
    /// Returns `EQ0302` for an unnamed entity set.
    pub fn geometry_region(
        id: Id<kinds::Domain>,
        geometry: GeometryDigest,
        entity_set: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            id,
            kind: DomainKind::GeometryRegion {
                geometry,
                entity_set: named_entity_set(id, entity_set)?,
            },
        })
    }

    /// Construct one boundary selected from its parent region's geometry.
    ///
    /// Whole-model validation checks that exactly one `BoundaryOf` parent is a
    /// geometry region. Artifact admission separately checks that both names
    /// select compatible entity sets in the referenced geometry.
    ///
    /// # Errors
    /// Returns `EQ0302` for an unnamed entity set.
    pub fn geometry_boundary(
        id: Id<kinds::Domain>,
        entity_set: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            id,
            kind: DomainKind::GeometryBoundary {
                entity_set: named_entity_set(id, entity_set)?,
            },
        })
    }

    /// Construct a nominal scalar conserving domain.
    #[must_use]
    pub const fn scalar_physical(
        id: Id<kinds::Domain>,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::ScalarPhysical {
                across_dimension,
                through_dimension,
            },
        }
    }

    /// Construct a nominal field-valued boundary connector Domain.
    #[must_use]
    pub const fn boundary_physical(
        id: Id<kinds::Domain>,
        connector: BoundaryPhysicalConnector,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::BoundaryPhysical { connector },
        }
    }

    /// Typed node ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Domain> {
        self.id
    }

    /// Canonical domain kind.
    #[must_use]
    pub const fn kind(&self) -> &DomainKind {
        &self.kind
    }
}
