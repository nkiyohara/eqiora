//! Exact convex-polyhedral authoring and borrowed topology on the common owner.

use super::{CanonicalGeometryKind, CanonicalGeometryLimits, CanonicalGeometryV1, ConvexPolyhedra};
use crate::NamedEntitySet;
use eqiora_core::Diagnostic;

impl CanonicalGeometryV1 {
    /// Admit finite, closed convex polyhedral volumes with disjoint interiors.
    ///
    /// Coordinates and positive classification precision are metres. Each shell
    /// contains its parent's outward-oriented polygon vertex loops. Shell and
    /// vertex order and loop rotation are canonicalized; reversed outward
    /// orientation is rejected. Facets shared by parents have one exact identity
    /// and opposite incidences. Curved, nonconvex, open, degenerate, and overlapping
    /// volumes are outside this family. No numerical tessellation is retained.
    ///
    /// As with `PlanarRegion::new`, named members address the resulting canonical
    /// enumerations, never input-relative indices: sorted vertices, sorted
    /// undirected edge pairs, sorted undirected facet loops, and sorted shells.
    /// Facet loops start at their least vertex; the lexicographically smaller
    /// direction orders global facets. Shells sort their outward loops.
    /// Dimensions 0, 1, 2, and 3 name vertices, edges, facets, and volumes.
    ///
    /// # Errors
    /// Returns `EQ0901` for invalid topology, geometry, precision, membership, or
    /// resource excess. Default canonical limits and a fixed 16,777,216 geometric
    /// floating predicate and 262,144 exact binary-rational predicate budgets
    /// also bound construction. Precision conditions edge lengths and interior
    /// clearance; it never excuses nonplanarity, nonconvexity, or overlap.
    pub fn from_convex_polyhedra(
        vertices_m: Vec<[f64; 3]>,
        outward_shells: Vec<Vec<Vec<usize>>>,
        entity_sets: Vec<NamedEntitySet>,
        tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        ConvexPolyhedra::new(
            vertices_m,
            outward_shells,
            entity_sets,
            tolerance_m,
            CanonicalGeometryLimits::default(),
        )
        .map(|geometry| Self {
            kind: CanonicalGeometryKind::ConvexPolyhedraV1(geometry),
        })
    }

    /// Canonical exact polyhedral vertices in metres, without mesh vertices.
    #[must_use]
    pub fn polyhedral_vertices(&self) -> Option<&[[f64; 3]]> {
        match &self.kind {
            CanonicalGeometryKind::ConvexPolyhedraV1(geometry) => Some(geometry.vertices()),
            _ => None,
        }
    }

    /// Canonically ordered exact facets incident to one canonical volume.
    #[must_use]
    pub fn polyhedral_volume_facets(&self, volume: usize) -> Option<&[usize]> {
        match &self.kind {
            CanonicalGeometryKind::ConvexPolyhedraV1(geometry) => geometry.volume_facets(volume),
            _ => None,
        }
    }

    /// Exact facet vertex loop oriented outward relative to its selected parent.
    /// Returns `None` for a foreign facet/volume incidence or another family.
    #[must_use]
    pub fn polyhedral_outward_facet(&self, facet: usize, volume: usize) -> Option<&[usize]> {
        match &self.kind {
            CanonicalGeometryKind::ConvexPolyhedraV1(geometry) => {
                geometry.outward_facet(facet, volume)
            }
            _ => None,
        }
    }
}
