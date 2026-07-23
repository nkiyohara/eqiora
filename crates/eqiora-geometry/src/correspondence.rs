//! Canonical semantic-geometry-mesh correspondence.

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_meshing::{MeshEntity, MeshTopology};
use eqiora_schema::kernel::BoundarySide;

use crate::{GeometryEntity, GeometryRevisionReference, GeometryRevisionTopology, ParentOutward};

type DomainId = Id<kinds::Domain>;

/// Exact realization of one semantic Cartesian body in one geometry and mesh revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartesianBodyAssignment {
    domain: DomainId,
    geometry: GeometryEntity,
    cells: Box<[MeshEntity]>,
}

impl CartesianBodyAssignment {
    /// Declare one body assignment. [`GeometryMeshCorrespondence::validate`]
    /// closes dimensions, entity ranges, disjointness, and totality.
    #[must_use]
    pub fn new(domain: DomainId, geometry: GeometryEntity, cells: Vec<MeshEntity>) -> Self {
        Self {
            domain,
            geometry,
            cells: cells.into_boxed_slice(),
        }
    }

    /// Semantic Cartesian-box Domain in this semantic revision.
    #[must_use]
    pub const fn domain(&self) -> DomainId {
        self.domain
    }

    /// Exact body entity in the referenced geometry revision.
    #[must_use]
    pub const fn geometry(&self) -> GeometryEntity {
        self.geometry
    }

    /// Canonically ordered top-dimensional mesh cells.
    #[must_use]
    pub const fn cells(&self) -> &[MeshEntity] {
        &self.cells
    }
}

/// Exact realization of one semantic Cartesian boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartesianBoundaryAssignment {
    domain: DomainId,
    parent: DomainId,
    axis: usize,
    side: BoundarySide,
    geometry: GeometryEntity,
    facets: Box<[MeshEntity]>,
}

impl CartesianBoundaryAssignment {
    /// Declare one boundary assignment.
    ///
    /// Its orientation is always [`ParentOutward`]. The validator proves that
    /// every mapped facet has exactly one adjacent cell in `parent`; no caller
    /// supplies a normal sign.
    #[must_use]
    pub fn new(
        domain: DomainId,
        parent: DomainId,
        axis: usize,
        side: BoundarySide,
        geometry: GeometryEntity,
        facets: Vec<MeshEntity>,
    ) -> Self {
        Self {
            domain,
            parent,
            axis,
            side,
            geometry,
            facets: facets.into_boxed_slice(),
        }
    }

    /// Semantic Cartesian-boundary Domain in this semantic revision.
    #[must_use]
    pub const fn domain(&self) -> DomainId {
        self.domain
    }

    /// Exact semantic parent-body Domain.
    #[must_use]
    pub const fn parent(&self) -> DomainId {
        self.parent
    }

    /// Parent Cartesian axis normal to this boundary.
    #[must_use]
    pub const fn axis(&self) -> usize {
        self.axis
    }

    /// Lower or upper side of the parent Cartesian axis.
    #[must_use]
    pub const fn side(&self) -> BoundarySide {
        self.side
    }

    /// Exact boundary entity in the referenced geometry revision.
    #[must_use]
    pub const fn geometry(&self) -> GeometryEntity {
        self.geometry
    }

    /// Canonically ordered mesh facets realizing this boundary.
    #[must_use]
    pub const fn facets(&self) -> &[MeshEntity] {
        &self.facets
    }

    /// Geometric orientation relative to the exact parent body.
    #[must_use]
    pub const fn orientation(&self) -> ParentOutward {
        ParentOutward
    }
}

/// Closed, canonical correspondence for one geometry revision and one full mesh.
///
/// Body cell sets form a total, disjoint partition of the mesh. For each body,
/// its Cartesian boundary assignments form a total, disjoint partition of the
/// facets exposed by that cell subset. A conforming interface facet may
/// therefore occur once for each of its two parent bodies, with two distinct
/// semantic boundary identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryMeshCorrespondence {
    geometry_revision: GeometryRevisionReference,
    dimension: usize,
    bodies: Box<[CartesianBodyAssignment]>,
    boundaries: Box<[CartesianBoundaryAssignment]>,
}

impl GeometryMeshCorrespondence {
    /// Validate and canonicalize one complete correspondence.
    ///
    /// # Errors
    /// Fails closed on dimension or range errors, reused semantic roles,
    /// incomplete cell ownership, or incomplete/overlapping exposed-facet
    /// ownership. This topological proof does not claim that a facet lies on
    /// the Cartesian plane named by `axis` and `side`; an artifact or geometry
    /// adapter with coordinates must close that separate embedding claim.
    pub fn validate<M: MeshTopology + ?Sized>(
        geometry: &GeometryRevisionTopology,
        mesh: &M,
        mut bodies: Vec<CartesianBodyAssignment>,
        mut boundaries: Vec<CartesianBoundaryAssignment>,
    ) -> Result<Self, GeometryCorrespondenceError> {
        let dimension = geometry.topological_dimension();
        if dimension == 0 {
            return Err(GeometryCorrespondenceError::ZeroDimensionalBodyMesh);
        }
        let mesh_dimension = mesh.topological_dimension();
        if mesh_dimension != dimension {
            return Err(GeometryCorrespondenceError::MeshDimensionMismatch {
                geometry: dimension,
                mesh: mesh_dimension,
            });
        }

        bodies.sort_by_key(|body| body.domain.erase());
        if bodies.is_empty() {
            return Err(GeometryCorrespondenceError::NoBodies);
        }
        let cell_count = mesh
            .entity_count(dimension)
            .ok_or(GeometryCorrespondenceError::MissingMeshStratum { dimension })?;
        let facet_dimension = dimension - 1;
        let facet_count = mesh.entity_count(facet_dimension).ok_or(
            GeometryCorrespondenceError::MissingMeshStratum {
                dimension: facet_dimension,
            },
        )?;

        let mut body_indices = BTreeMap::new();
        let mut geometry_bodies = BTreeSet::new();
        let mut cell_owners = vec![None; cell_count];
        for (body_index, body) in bodies.iter_mut().enumerate() {
            if body_indices
                .insert(body.domain.erase(), body_index)
                .is_some()
            {
                return Err(GeometryCorrespondenceError::DuplicateBodyDomain {
                    domain: body.domain,
                });
            }
            validate_geometry_entity(geometry, body.geometry, dimension)?;
            if !geometry_bodies.insert(body.geometry) {
                return Err(GeometryCorrespondenceError::DuplicateBodyGeometry {
                    entity: body.geometry,
                });
            }
            if body.cells.is_empty() {
                return Err(GeometryCorrespondenceError::EmptyBodyCells {
                    domain: body.domain,
                });
            }
            let mut cells = body.cells.to_vec();
            cells.sort_unstable();
            for pair in cells.windows(2) {
                if pair[0] == pair[1] {
                    return Err(GeometryCorrespondenceError::DuplicateBodyCell {
                        domain: body.domain,
                        cell: pair[0],
                    });
                }
            }
            for &cell in &cells {
                validate_mesh_entity(mesh, cell, dimension)?;
                let owner = &mut cell_owners[cell.index()];
                if let Some(first) = *owner {
                    return Err(GeometryCorrespondenceError::CellAssignedToMultipleBodies {
                        cell,
                        first,
                        second: body.domain,
                    });
                }
                *owner = Some(body.domain);
            }
            body.cells = cells.into_boxed_slice();
        }
        let geometry_body_count = geometry
            .entity_count(dimension)
            .ok_or(GeometryCorrespondenceError::EmptyGeometryStratum { dimension })?;
        for index in 0..geometry_body_count {
            let entity = GeometryEntity::new(dimension, index);
            if !geometry_bodies.contains(&entity) {
                return Err(GeometryCorrespondenceError::UnassignedGeometryBody { entity });
            }
        }
        if let Some(index) = cell_owners.iter().position(Option::is_none) {
            return Err(GeometryCorrespondenceError::UnassignedCell {
                cell: MeshEntity::new(dimension, index),
            });
        }

        boundaries.sort_by_key(boundary_sort_key);
        let body_domains = body_indices.keys().copied().collect::<BTreeSet<_>>();
        let mut boundary_domains = BTreeSet::new();
        let mut roles = BTreeSet::new();
        let mut assigned_facets = BTreeSet::new();
        let mut geometry_boundary_uses: BTreeMap<GeometryEntity, (DomainId, Box<[MeshEntity]>)> =
            BTreeMap::new();
        for boundary in &mut boundaries {
            if body_domains.contains(&boundary.domain.erase())
                || !boundary_domains.insert(boundary.domain.erase())
            {
                return Err(GeometryCorrespondenceError::DuplicateBoundaryDomain {
                    domain: boundary.domain,
                });
            }
            let Some(&parent_index) = body_indices.get(&boundary.parent.erase()) else {
                return Err(GeometryCorrespondenceError::UnknownBoundaryParent {
                    boundary: boundary.domain,
                    parent: boundary.parent,
                });
            };
            if boundary.axis >= dimension {
                return Err(GeometryCorrespondenceError::BoundaryAxisOutOfRange {
                    boundary: boundary.domain,
                    axis: boundary.axis,
                    dimension,
                });
            }
            if !roles.insert((boundary.parent.erase(), boundary.axis, boundary.side)) {
                return Err(GeometryCorrespondenceError::DuplicateBoundaryRole {
                    parent: boundary.parent,
                    axis: boundary.axis,
                    side: boundary.side,
                });
            }
            validate_geometry_entity(geometry, boundary.geometry, facet_dimension)?;
            if boundary.facets.is_empty() {
                return Err(GeometryCorrespondenceError::EmptyBoundaryFacets {
                    boundary: boundary.domain,
                });
            }
            let mut facets = boundary.facets.to_vec();
            facets.sort_unstable();
            for pair in facets.windows(2) {
                if pair[0] == pair[1] {
                    return Err(GeometryCorrespondenceError::DuplicateBoundaryFacet {
                        boundary: boundary.domain,
                        facet: pair[0],
                    });
                }
            }
            for &facet in &facets {
                validate_mesh_entity(mesh, facet, facet_dimension)?;
                let parent_incidence =
                    parent_incidence_count(mesh, facet, dimension, &bodies[parent_index])?;
                if parent_incidence != 1 {
                    return Err(GeometryCorrespondenceError::FacetNotExposedByParent {
                        boundary: boundary.domain,
                        parent: boundary.parent,
                        facet,
                        adjacent_parent_cells: parent_incidence,
                    });
                }
                if !assigned_facets.insert((boundary.parent.erase(), facet)) {
                    return Err(GeometryCorrespondenceError::FacetAssignedTwiceForParent {
                        parent: boundary.parent,
                        facet,
                    });
                }
            }
            boundary.facets = facets.into_boxed_slice();
            if let Some((first_parent, first_facets)) =
                geometry_boundary_uses.get(&boundary.geometry)
            {
                if *first_parent == boundary.parent || first_facets.as_ref() != boundary.facets() {
                    return Err(
                        GeometryCorrespondenceError::InconsistentGeometryBoundaryReuse {
                            entity: boundary.geometry,
                            first_parent: *first_parent,
                            second_parent: boundary.parent,
                        },
                    );
                }
            } else {
                geometry_boundary_uses.insert(
                    boundary.geometry,
                    (boundary.parent, boundary.facets.clone()),
                );
            }
        }

        let geometry_boundary_count = geometry.entity_count(facet_dimension).ok_or(
            GeometryCorrespondenceError::EmptyGeometryStratum {
                dimension: facet_dimension,
            },
        )?;
        for index in 0..geometry_boundary_count {
            let entity = GeometryEntity::new(facet_dimension, index);
            if !geometry_boundary_uses.contains_key(&entity) {
                return Err(GeometryCorrespondenceError::UnassignedGeometryBoundary { entity });
            }
        }

        for body in &bodies {
            for axis in 0..dimension {
                for side in [BoundarySide::Lower, BoundarySide::Upper] {
                    if !roles.contains(&(body.domain.erase(), axis, side)) {
                        return Err(GeometryCorrespondenceError::MissingBoundaryRole {
                            parent: body.domain,
                            axis,
                            side,
                        });
                    }
                }
            }
            for facet_index in 0..facet_count {
                let facet = MeshEntity::new(facet_dimension, facet_index);
                if parent_incidence_count(mesh, facet, dimension, body)? == 1
                    && !assigned_facets.contains(&(body.domain.erase(), facet))
                {
                    return Err(GeometryCorrespondenceError::UnassignedExposedFacet {
                        parent: body.domain,
                        facet,
                    });
                }
            }
        }

        Ok(Self {
            geometry_revision: geometry.reference(),
            dimension,
            bodies: bodies.into_boxed_slice(),
            boundaries: boundaries.into_boxed_slice(),
        })
    }

    /// Exact geometry revision shared by every assignment.
    #[must_use]
    pub const fn geometry_revision(&self) -> GeometryRevisionReference {
        self.geometry_revision
    }

    /// Common topological dimension of geometry and mesh.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Canonically ordered body assignments.
    #[must_use]
    pub const fn bodies(&self) -> &[CartesianBodyAssignment] {
        &self.bodies
    }

    /// Canonically ordered boundary assignments.
    #[must_use]
    pub const fn boundaries(&self) -> &[CartesianBoundaryAssignment] {
        &self.boundaries
    }

    pub(crate) fn body(&self, domain: DomainId) -> Option<&CartesianBodyAssignment> {
        self.bodies.iter().find(|body| body.domain == domain)
    }

    pub(crate) fn boundaries_of(
        &self,
        parent: DomainId,
    ) -> impl Iterator<Item = &CartesianBoundaryAssignment> {
        self.boundaries
            .iter()
            .filter(move |boundary| boundary.parent == parent)
    }
}

fn boundary_sort_key(boundary: &CartesianBoundaryAssignment) -> (RawId, usize, u8, RawId) {
    (
        boundary.parent.erase(),
        boundary.axis,
        match boundary.side {
            BoundarySide::Lower => 0,
            BoundarySide::Upper => 1,
        },
        boundary.domain.erase(),
    )
}

fn validate_geometry_entity(
    geometry: &GeometryRevisionTopology,
    entity: GeometryEntity,
    expected_dimension: usize,
) -> Result<(), GeometryCorrespondenceError> {
    if entity.dimension() != expected_dimension || !geometry.contains(entity) {
        return Err(GeometryCorrespondenceError::InvalidGeometryEntity {
            entity,
            expected_dimension,
        });
    }
    Ok(())
}

fn validate_mesh_entity<M: MeshTopology + ?Sized>(
    mesh: &M,
    entity: MeshEntity,
    expected_dimension: usize,
) -> Result<(), GeometryCorrespondenceError> {
    if entity.dimension() != expected_dimension
        || mesh
            .entity_count(expected_dimension)
            .is_none_or(|count| entity.index() >= count)
    {
        return Err(GeometryCorrespondenceError::InvalidMeshEntity {
            entity,
            expected_dimension,
        });
    }
    Ok(())
}

fn parent_incidence_count<M: MeshTopology + ?Sized>(
    mesh: &M,
    facet: MeshEntity,
    cell_dimension: usize,
    body: &CartesianBodyAssignment,
) -> Result<usize, GeometryCorrespondenceError> {
    let incidence = mesh.incidence(facet, cell_dimension).ok_or(
        GeometryCorrespondenceError::UnavailableMeshIncidence {
            entity: facet,
            target_dimension: cell_dimension,
        },
    )?;
    Ok(incidence
        .iter()
        .filter(|entry| body.cells.binary_search(&entry.entity).is_ok())
        .count())
}

/// Closed failure set for geometry-to-mesh correspondence validation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GeometryCorrespondenceError {
    /// Geometry correspondence requires dimension one or greater.
    GeometryDimensionTooSmall,
    /// The top or codimension-one geometry stratum is empty.
    EmptyGeometryStratum { dimension: usize },
    /// Cartesian body correspondence requires positive dimension.
    ZeroDimensionalBodyMesh,
    /// Geometry and mesh dimensions differ.
    MeshDimensionMismatch { geometry: usize, mesh: usize },
    /// The mesh omits a required entity stratum.
    MissingMeshStratum { dimension: usize },
    /// At least one semantic body is required.
    NoBodies,
    /// A referenced geometry entity has the wrong dimension or is out of range.
    InvalidGeometryEntity {
        entity: GeometryEntity,
        expected_dimension: usize,
    },
    /// A referenced mesh entity has the wrong dimension or is out of range.
    InvalidMeshEntity {
        entity: MeshEntity,
        expected_dimension: usize,
    },
    /// Mesh incidence unexpectedly could not be queried.
    UnavailableMeshIncidence {
        entity: MeshEntity,
        target_dimension: usize,
    },
    /// One semantic body occurs more than once.
    DuplicateBodyDomain { domain: DomainId },
    /// One exact geometry body is claimed by multiple semantic bodies.
    DuplicateBodyGeometry { entity: GeometryEntity },
    /// One exact geometry body has no semantic or mesh assignment.
    UnassignedGeometryBody { entity: GeometryEntity },
    /// A semantic body has no cells.
    EmptyBodyCells { domain: DomainId },
    /// One body's cell list repeats an entity.
    DuplicateBodyCell { domain: DomainId, cell: MeshEntity },
    /// Two bodies claim the same mesh cell.
    CellAssignedToMultipleBodies {
        cell: MeshEntity,
        first: DomainId,
        second: DomainId,
    },
    /// A full-mesh cell has no semantic body owner.
    UnassignedCell { cell: MeshEntity },
    /// A boundary Domain is reused or conflicts with a body Domain.
    DuplicateBoundaryDomain { domain: DomainId },
    /// A boundary names no body in this exact correspondence.
    UnknownBoundaryParent {
        boundary: DomainId,
        parent: DomainId,
    },
    /// A Cartesian boundary axis exceeds the common dimension.
    BoundaryAxisOutOfRange {
        boundary: DomainId,
        axis: usize,
        dimension: usize,
    },
    /// Two semantic boundaries claim one parent/axis/side role.
    DuplicateBoundaryRole {
        parent: DomainId,
        axis: usize,
        side: BoundarySide,
    },
    /// A Cartesian parent omits one required axis/side role.
    MissingBoundaryRole {
        parent: DomainId,
        axis: usize,
        side: BoundarySide,
    },
    /// A semantic boundary has no mesh facets.
    EmptyBoundaryFacets { boundary: DomainId },
    /// One exact geometry boundary is reused inconsistently.
    InconsistentGeometryBoundaryReuse {
        entity: GeometryEntity,
        first_parent: DomainId,
        second_parent: DomainId,
    },
    /// One exact geometry boundary has no semantic or mesh assignment.
    UnassignedGeometryBoundary { entity: GeometryEntity },
    /// One boundary repeats a mesh facet.
    DuplicateBoundaryFacet {
        boundary: DomainId,
        facet: MeshEntity,
    },
    /// A mapped facet is not exposed by exactly one cell of its parent body.
    FacetNotExposedByParent {
        boundary: DomainId,
        parent: DomainId,
        facet: MeshEntity,
        adjacent_parent_cells: usize,
    },
    /// Two boundary roles of one parent claim the same exposed facet.
    FacetAssignedTwiceForParent { parent: DomainId, facet: MeshEntity },
    /// An exposed facet of a body has no semantic boundary owner.
    UnassignedExposedFacet { parent: DomainId, facet: MeshEntity },
}

impl fmt::Display for GeometryCorrespondenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid geometry-to-mesh correspondence: {self:?}"
        )
    }
}

impl std::error::Error for GeometryCorrespondenceError {}
