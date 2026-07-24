//! Exact geometry-revision to mesh-revision entity correspondence.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_geometry::{
    CartesianBodyAssignment, CartesianBoundaryAssignment, GeometryMeshCorrespondence,
    GeometryRevisionReference, GeometryRevisionTopology,
};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshEntity, MeshTopology};
use eqiora_schema::kernel::{BoundarySide, ConnectionSemantics, KernelNode};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::geometry_identity::WireGeometryEntity;
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DecoderLimits, GeometryEntityV1,
    GeometryIdentityEnvelopeV1, ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
    check_wire_limits, invalid_artifact,
};

const CORRESPONDENCE_SCHEMA: &str = "eqiora.geometry-mesh-correspondence-envelope/v1";

/// Versioned proof that exact geometry entities correspond to exact cell and
/// facet sets in one affine-simplex mesh revision.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMeshCorrespondenceEnvelopeV1 {
    wire: WireGeometryMeshCorrespondenceV1,
}

/// Exact binary conserving interface derived from accepted Model,
/// geometry, correspondence, and mesh artifacts.
///
/// This proof owns no trace quotient, transfer operator, coupling residual,
/// or FSI solve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConservingGeometryInterfaceV1 {
    model_artifact: ArtifactDigest,
    geometry_artifact: ArtifactDigest,
    mesh_artifact: ArtifactDigest,
    correspondence_artifact: ArtifactDigest,
    connection: Id<kinds::Connection>,
    connector: Id<kinds::Domain>,
    ports: [Id<kinds::Port>; 2],
    boundaries: [Id<kinds::Domain>; 2],
    parents: [Id<kinds::Domain>; 2],
    facet_indices: Vec<usize>,
}

impl ConservingGeometryInterfaceV1 {
    /// Exact Semantic Model artifact from which the interface meaning was replayed.
    #[must_use]
    pub const fn model_artifact(&self) -> &ArtifactDigest {
        &self.model_artifact
    }

    /// Exact geometry identity artifact used to derive the interface entities.
    #[must_use]
    pub const fn geometry_artifact(&self) -> &ArtifactDigest {
        &self.geometry_artifact
    }

    /// Exact mesh artifact owning every returned facet index.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &ArtifactDigest {
        &self.mesh_artifact
    }

    /// Exact correspondence artifact proving the geometry-to-mesh memberships.
    #[must_use]
    pub const fn correspondence_artifact(&self) -> &ArtifactDigest {
        &self.correspondence_artifact
    }

    /// Exact conserving Connection identity.
    #[must_use]
    pub const fn connection(&self) -> Id<kinds::Connection> {
        self.connection
    }

    /// Exact nominal boundary-physical connector identity.
    #[must_use]
    pub const fn connector(&self) -> Id<kinds::Domain> {
        self.connector
    }

    /// Canonically parent-ordered exact Port identities.
    #[must_use]
    pub const fn ports(&self) -> [Id<kinds::Port>; 2] {
        self.ports
    }

    /// Canonically parent-ordered distinct boundary Domain identities.
    #[must_use]
    pub const fn boundaries(&self) -> [Id<kinds::Domain>; 2] {
        self.boundaries
    }

    /// Canonically ordered distinct parent body identities.
    #[must_use]
    pub const fn parents(&self) -> [Id<kinds::Domain>; 2] {
        self.parents
    }

    /// Complete shared mesh facet membership.
    #[must_use]
    pub fn facet_indices(&self) -> &[usize] {
        &self.facet_indices
    }
}

impl GeometryMeshCorrespondenceEnvelopeV1 {
    /// Derive the complete correspondence for a Cartesian geometry revision.
    ///
    /// Cell ownership and relative body frontiers are discovered from the
    /// exact coordinates and incidence of `mesh`; callers do not supply tags,
    /// membership arrays, or normal signs.
    ///
    /// # Errors
    /// Returns `EQ0901` unless every top-dimensional cell belongs to exactly
    /// one body, each body's relative frontier is the exact union of its
    /// complete semantic boundaries, and every referenced artifact matches.
    pub fn new(
        geometry: &GeometryIdentityEnvelopeV1,
        model: &(impl ReplayableCanonicalModelArtifact + ?Sized),
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        geometry.validate_against(model)?;
        let dimension = geometry.dimension();
        if mesh.dimension() != dimension {
            return Err(invalid_artifact("geometry and mesh dimensions differ"));
        }
        let topology = mesh.mesh();
        let cell_count = topology
            .entity_count(dimension)
            .ok_or_else(|| invalid_artifact("mesh has no top-dimensional stratum"))?;
        let facet_dimension = dimension - 1;
        let facet_count = topology
            .entity_count(facet_dimension)
            .ok_or_else(|| invalid_artifact("mesh has no codimension-one stratum"))?;
        let bodies = geometry.bodies();
        let boundaries = geometry.boundaries();
        let tolerance = geometry.tolerance_m();

        let mut body_cells = BTreeMap::<GeometryEntityV1, Vec<usize>>::new();
        let mut cell_owner = vec![None; cell_count];
        for (cell_index, owner) in cell_owner.iter_mut().enumerate() {
            let cell = MeshEntity::new(dimension, cell_index);
            let vertices = entity_coordinates(topology, cell)?;
            let candidates = bodies
                .iter()
                .filter(|body| {
                    vertices
                        .iter()
                        .all(|vertex| point_inside_cartesian(vertex, body.bounds_m(), tolerance))
                })
                .map(|body| body.entity())
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err(invalid_artifact(
                    "every mesh cell must belong to exactly one selected geometry body",
                ));
            }
            let body = candidates[0];
            *owner = Some(body);
            body_cells.entry(body).or_default().push(cell_index);
        }
        if bodies
            .iter()
            .any(|body| body_cells.get(&body.entity()).is_none_or(Vec::is_empty))
        {
            return Err(invalid_artifact(
                "every geometry body requires a nonempty mesh cell subset",
            ));
        }

        let boundary_by_role = boundaries
            .iter()
            .map(|boundary| {
                (
                    (boundary.parent_entity(), boundary.axis(), boundary.side()),
                    *boundary,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut boundary_facets = BTreeMap::<Ulid, Vec<usize>>::new();
        for body in &bodies {
            for facet_index in 0..facet_count {
                let facet = MeshEntity::new(facet_dimension, facet_index);
                let adjacent = topology
                    .incidence(facet, dimension)
                    .ok_or_else(|| invalid_artifact("mesh facet incidence is unavailable"))?;
                let selected = adjacent
                    .iter()
                    .filter(|incidence| cell_owner[incidence.entity.index()] == Some(body.entity()))
                    .collect::<Vec<_>>();
                match selected.len() {
                    0 | 2 => continue,
                    1 => {}
                    _ => {
                        return Err(invalid_artifact(
                            "body-relative mesh frontier has invalid facet adjacency",
                        ));
                    }
                }
                let coordinates = entity_coordinates(topology, facet)?;
                let roles = cartesian_facet_roles(&coordinates, body.bounds_m(), tolerance);
                if roles.len() != 1 {
                    return Err(invalid_artifact(
                        "every relative frontier facet must lie on exactly one Cartesian side",
                    ));
                }
                let (axis, side) = roles[0];
                let Some(boundary) = boundary_by_role.get(&(body.entity(), axis, side)).copied()
                else {
                    return Err(invalid_artifact(
                        "mesh frontier has no exact semantic boundary role",
                    ));
                };
                validate_parent_outward_cell(
                    topology,
                    selected[0].entity,
                    axis,
                    side,
                    body.bounds_m(),
                )?;
                boundary_facets
                    .entry(boundary.domain().ulid())
                    .or_default()
                    .push(facet_index);
            }
        }
        if boundaries.iter().any(|boundary| {
            boundary_facets
                .get(&boundary.domain().ulid())
                .is_none_or(Vec::is_empty)
        }) {
            return Err(invalid_artifact(
                "every semantic Cartesian boundary requires a nonempty exact facet set",
            ));
        }

        let mut geometry_counts = vec![0; dimension + 1];
        geometry_counts[dimension] = bodies.len();
        geometry_counts[facet_dimension] = boundaries
            .iter()
            .map(|boundary| boundary.entity())
            .collect::<BTreeSet<_>>()
            .len();
        let geometry_topology = GeometryRevisionTopology::new(
            GeometryRevisionReference::from_digest_bytes(geometry.digest()?.sha256_bytes()),
            geometry_counts,
        )
        .map_err(|error| invalid_artifact(error.to_string()))?;
        let body_assignments = bodies
            .iter()
            .map(|body| {
                CartesianBodyAssignment::new(
                    body.domain(),
                    body.entity(),
                    body_cells[&body.entity()]
                        .iter()
                        .map(|&index| MeshEntity::new(dimension, index))
                        .collect(),
                )
            })
            .collect();
        let boundary_assignments = boundaries
            .iter()
            .map(|boundary| {
                CartesianBoundaryAssignment::new(
                    boundary.domain(),
                    boundary.parent(),
                    boundary.axis(),
                    boundary.side(),
                    boundary.entity(),
                    boundary_facets[&boundary.domain().ulid()]
                        .iter()
                        .map(|&index| MeshEntity::new(facet_dimension, index))
                        .collect(),
                )
            })
            .collect();
        GeometryMeshCorrespondence::validate(
            &geometry_topology,
            topology,
            body_assignments,
            boundary_assignments,
        )
        .map_err(|error| invalid_artifact(error.to_string()))?;

        let wire_bodies = bodies
            .iter()
            .map(|body| {
                Ok(WireBodyAssignment {
                    domain_ulid: body.domain().ulid().to_string(),
                    geometry_entity: WireGeometryEntity::new(
                        body.entity().dimension(),
                        body.entity().index(),
                    )?,
                    cell_indices: encode_indices(
                        body_cells.get(&body.entity()).expect("each body has cells"),
                        "mesh cell",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut wire_boundaries = boundaries
            .iter()
            .map(|boundary| {
                Ok(WireBoundaryAssignment {
                    domain_ulid: boundary.domain().ulid().to_string(),
                    parent_ulid: boundary.parent().ulid().to_string(),
                    geometry_entity: WireGeometryEntity::new(
                        boundary.entity().dimension(),
                        boundary.entity().index(),
                    )?,
                    axis: u64::try_from(boundary.axis())
                        .map_err(|_| invalid_artifact("boundary axis exceeds portable u64"))?,
                    side: WireBoundarySide::encode(boundary.side()),
                    orientation: WireBoundaryOrientation::ParentOutward,
                    facet_indices: encode_indices(
                        boundary_facets
                            .get(&boundary.domain().ulid())
                            .expect("each boundary has facets"),
                        "mesh facet",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        wire_boundaries.sort_by(|left, right| {
            left.geometry_entity
                .cmp(&right.geometry_entity)
                .then_with(|| left.parent_ulid.cmp(&right.parent_ulid))
                .then_with(|| left.domain_ulid.cmp(&right.domain_ulid))
        });
        let envelope = Self {
            wire: WireGeometryMeshCorrespondenceV1 {
                schema: CORRESPONDENCE_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                geometry_sha256: geometry.digest()?.to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                dimension: u64::try_from(dimension)
                    .map_err(|_| invalid_artifact("geometry dimension exceeds portable u64"))?,
                bodies: wire_bodies,
                boundaries: wire_boundaries,
            },
        };
        envelope.validate_local(DecoderLimits::default())?;
        Ok(envelope)
    }

    /// Decode bounded wire data. Exact referenced resources remain untrusted
    /// until [`Self::validate_against`] succeeds.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!(
                "invalid geometry-mesh correspondence JSON: {error}"
            ))
        })?;
        let envelope = Self { wire };
        envelope.validate_local(limits)?;
        Ok(envelope)
    }

    /// Recompute the complete correspondence from exact resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for stale digests, changed identities, membership
    /// drift, or any now-invalid geometry/mesh relation.
    pub fn validate_against(
        &self,
        geometry: &GeometryIdentityEnvelopeV1,
        model: &(impl ReplayableCanonicalModelArtifact + ?Sized),
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        let expected = Self::new(geometry, model, mesh)?;
        if self != &expected {
            return Err(invalid_artifact(
                "geometry-mesh correspondence differs from exact resource replay",
            ));
        }
        Ok(())
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize geometry-mesh correspondence: {error}"
            ))
        })
    }

    /// Domain-separated identity of the exact correspondence proof.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            CORRESPONDENCE_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Exact referenced geometry revision.
    #[must_use]
    pub fn geometry_artifact(&self) -> ArtifactDigest {
        ArtifactDigest::from_hex(self.wire.geometry_sha256.clone())
            .expect("validated geometry digest")
    }

    /// Exact referenced mesh revision.
    #[must_use]
    pub fn mesh_artifact(&self) -> ArtifactDigest {
        ArtifactDigest::from_hex(self.wire.mesh_sha256.clone()).expect("validated mesh digest")
    }

    /// Mesh cell indices owned by one exact semantic body.
    #[must_use]
    pub fn body_cells(&self, body: Id<kinds::Domain>) -> Option<Vec<usize>> {
        self.body_cell_indices(body)
            .map(|indices| indices.collect())
    }

    /// Borrow the canonical cell-index sequence for one semantic body.
    ///
    /// This allocation-free view supports checked work preflight before a
    /// consumer chooses to own the indices.
    #[must_use]
    pub fn body_cell_indices(
        &self,
        body: Id<kinds::Domain>,
    ) -> Option<impl ExactSizeIterator<Item = usize> + Clone + '_> {
        self.wire
            .bodies
            .iter()
            .find(|assignment| assignment.domain_ulid == body.ulid().to_string())
            .map(|assignment| {
                assignment.cell_indices.iter().map(|&index| {
                    usize::try_from(index).expect("validated body cell index fits local usize")
                })
            })
    }

    /// Mesh facet indices realizing one exact semantic boundary.
    #[must_use]
    pub fn boundary_facets(&self, boundary: Id<kinds::Domain>) -> Option<Vec<usize>> {
        self.wire
            .boundaries
            .iter()
            .find(|assignment| assignment.domain_ulid == boundary.ulid().to_string())
            .and_then(|assignment| decode_indices(&assignment.facet_indices).ok())
    }

    /// Derive the exact binary shared-facet witness for one conserving
    /// Connection.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the Connection joins exactly two compatible
    /// field-boundary Ports on distinct, coincident, opposite sides whose
    /// complete facet sets are identical and two-sided by parent incidence.
    pub fn derive_conserving_interface(
        &self,
        geometry: &GeometryIdentityEnvelopeV1,
        model: &(impl ReplayableCanonicalModelArtifact + ?Sized),
        mesh: &SimplicialMeshEnvelopeV1,
        connection: Id<kinds::Connection>,
    ) -> Result<ConservingGeometryInterfaceV1, Diagnostic> {
        self.validate_against(geometry, model, mesh)?;
        let replay = model.replay_model()?;
        let program = replay.program();
        match program.node(connection.erase()) {
            Some(KernelNode::Connection(definition))
                if definition.semantics() == ConnectionSemantics::Conserving => {}
            _ => {
                return Err(invalid_artifact(
                    "geometry interface requires an exact conserving Connection",
                ));
            }
        }
        let ports = program
            .edges()
            .iter()
            .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection.erase())
            .map(|edge| {
                edge.to()
                    .downcast::<kinds::Port>()
                    .ok_or_else(|| invalid_artifact("Connection member is not a Port"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ports.len() != 2 || ports[0] == ports[1] {
            return Err(invalid_artifact(
                "geometry interface v1 requires exactly two distinct Connection Ports",
            ));
        }
        let geometry_boundaries = geometry.boundaries();
        let mut sides = ports
            .into_iter()
            .map(|port| {
                let Some(KernelNode::Port(definition)) = program.node(port.erase()) else {
                    return Err(invalid_artifact(
                        "geometry interface Connection references no retained Port",
                    ));
                };
                let Some((connector, boundary)) = definition.boundary_physical_contract() else {
                    return Err(invalid_artifact(
                        "geometry interface Port is not field-boundary physical",
                    ));
                };
                let geometry_boundary = geometry_boundaries
                    .iter()
                    .find(|candidate| candidate.domain() == boundary)
                    .copied()
                    .ok_or_else(|| {
                        invalid_artifact(
                            "interface Port boundary is absent from the geometry revision",
                        )
                    })?;
                let facets = self.boundary_facets(boundary).ok_or_else(|| {
                    invalid_artifact("interface boundary has no exact mesh correspondence")
                })?;
                Ok((port, connector, geometry_boundary, facets))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        sides.sort_by_key(|side| side.2.parent().ulid());
        if sides[0].1 != sides[1].1
            || sides[0].2.parent() == sides[1].2.parent()
            || sides[0].2.entity() != sides[1].2.entity()
            || sides[0].2.axis() != sides[1].2.axis()
            || sides[0].2.side() == sides[1].2.side()
            || sides[0].3 != sides[1].3
            || sides[0].3.is_empty()
        {
            return Err(invalid_artifact(
                "interface requires one connector, distinct parents, one shared geometry entity, opposite sides, and equal facets",
            ));
        }
        let bodies = geometry.bodies();
        let first_body = bodies
            .iter()
            .find(|body| body.domain() == sides[0].2.parent())
            .ok_or_else(|| invalid_artifact("first interface parent body is absent"))?;
        let second_body = bodies
            .iter()
            .find(|body| body.domain() == sides[1].2.parent())
            .ok_or_else(|| invalid_artifact("second interface parent body is absent"))?;
        validate_coincident_sides(
            first_body.bounds_m(),
            sides[0].2.axis(),
            sides[0].2.side(),
            second_body.bounds_m(),
            sides[1].2.side(),
            geometry.tolerance_m(),
        )?;
        let first_cells = self
            .body_cells(sides[0].2.parent())
            .ok_or_else(|| invalid_artifact("first interface parent has no cells"))?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let second_cells = self
            .body_cells(sides[1].2.parent())
            .ok_or_else(|| invalid_artifact("second interface parent has no cells"))?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let dimension = geometry.dimension();
        for &facet_index in &sides[0].3 {
            let adjacent = mesh
                .mesh()
                .incidence(MeshEntity::new(dimension - 1, facet_index), dimension)
                .ok_or_else(|| invalid_artifact("interface facet adjacency is unavailable"))?;
            if adjacent.len() != 2
                || adjacent
                    .iter()
                    .filter(|entry| first_cells.contains(&entry.entity.index()))
                    .count()
                    != 1
                || adjacent
                    .iter()
                    .filter(|entry| second_cells.contains(&entry.entity.index()))
                    .count()
                    != 1
            {
                return Err(invalid_artifact(
                    "every interface facet requires one adjacent cell from each exact parent",
                ));
            }
        }
        let facets = sides[0].3.clone();
        Ok(ConservingGeometryInterfaceV1 {
            model_artifact: geometry.model_artifact(),
            geometry_artifact: geometry.digest()?,
            mesh_artifact: mesh.digest()?,
            correspondence_artifact: self.digest()?,
            connection,
            connector: sides[0].1,
            ports: [sides[0].0, sides[1].0],
            boundaries: [sides[0].2.domain(), sides[1].2.domain()],
            parents: [sides[0].2.parent(), sides[1].2.parent()],
            facet_indices: facets,
        })
    }

    pub(crate) fn typed_correspondence(
        &self,
        geometry: &GeometryIdentityEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<GeometryMeshCorrespondence, Diagnostic> {
        let dimension = geometry.dimension();
        let mut counts = vec![0; dimension + 1];
        counts[dimension] = self.wire.bodies.len();
        counts[dimension - 1] = self
            .wire
            .boundaries
            .iter()
            .map(|boundary| boundary.geometry_entity)
            .collect::<BTreeSet<_>>()
            .len();
        let topology = GeometryRevisionTopology::new(
            GeometryRevisionReference::from_digest_bytes(geometry.digest()?.sha256_bytes()),
            counts,
        )
        .map_err(|error| invalid_artifact(error.to_string()))?;
        let bodies = self
            .wire
            .bodies
            .iter()
            .map(|body| {
                Ok(CartesianBodyAssignment::new(
                    Id::from_ulid(parse_ulid(&body.domain_ulid, "correspondence body")?),
                    body.geometry_entity.decode()?,
                    decode_indices(&body.cell_indices)?
                        .into_iter()
                        .map(|index| MeshEntity::new(dimension, index))
                        .collect(),
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let boundaries = self
            .wire
            .boundaries
            .iter()
            .map(|boundary| {
                Ok(CartesianBoundaryAssignment::new(
                    Id::from_ulid(parse_ulid(
                        &boundary.domain_ulid,
                        "correspondence boundary",
                    )?),
                    Id::from_ulid(parse_ulid(&boundary.parent_ulid, "correspondence parent")?),
                    usize::try_from(boundary.axis)
                        .map_err(|_| invalid_artifact("boundary axis exceeds local usize"))?,
                    boundary.side.decode(),
                    boundary.geometry_entity.decode()?,
                    decode_indices(&boundary.facet_indices)?
                        .into_iter()
                        .map(|index| MeshEntity::new(dimension - 1, index))
                        .collect(),
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        GeometryMeshCorrespondence::validate(&topology, mesh.mesh(), bodies, boundaries)
            .map_err(|error| invalid_artifact(error.to_string()))
    }

    fn validate_local(&self, limits: DecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != CORRESPONDENCE_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported geometry-mesh correspondence schema or encoding",
            ));
        }
        ArtifactDigest::from_hex(self.wire.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.wire.mesh_sha256.clone())?;
        let dimension = usize::try_from(self.wire.dimension)
            .map_err(|_| invalid_artifact("correspondence dimension exceeds local usize"))?;
        if dimension == 0 || self.wire.bodies.is_empty() || self.wire.boundaries.is_empty() {
            return Err(invalid_artifact(
                "geometry-mesh correspondence requires positive dimension, bodies, and boundaries",
            ));
        }
        if self.wire.bodies.len() + self.wire.boundaries.len() > limits.max_geometry_entities {
            return Err(invalid_artifact(
                "geometry-mesh assignment count exceeds decoder limits",
            ));
        }
        let mut membership_count = 0_usize;
        let mut domains = BTreeSet::new();
        let mut body_entities = BTreeSet::new();
        for body in &self.wire.bodies {
            parse_ulid(&body.domain_ulid, "correspondence body")?;
            let entity = body.geometry_entity.decode()?;
            let cells = decode_indices(&body.cell_indices)?;
            if entity.dimension() != dimension
                || cells.is_empty()
                || !domains.insert(body.domain_ulid.as_str())
                || !body_entities.insert(entity)
                || !strictly_sorted_unique(&cells)
            {
                return Err(invalid_artifact(
                    "body assignments must be unique, full-dimensional, nonempty, and canonical",
                ));
            }
            membership_count = membership_count
                .checked_add(cells.len())
                .ok_or_else(|| invalid_artifact("geometry membership count overflows usize"))?;
        }
        let mut boundary_entities = BTreeMap::<GeometryEntityV1, (&str, Vec<usize>)>::new();
        for boundary in &self.wire.boundaries {
            parse_ulid(&boundary.domain_ulid, "correspondence boundary")?;
            parse_ulid(&boundary.parent_ulid, "correspondence parent")?;
            let entity = boundary.geometry_entity.decode()?;
            let facets = decode_indices(&boundary.facet_indices)?;
            if entity.dimension() != dimension - 1
                || facets.is_empty()
                || !domains.insert(boundary.domain_ulid.as_str())
                || !strictly_sorted_unique(&facets)
                || boundary.orientation != WireBoundaryOrientation::ParentOutward
                || usize::try_from(boundary.axis).map_or(true, |axis| axis >= dimension)
            {
                return Err(invalid_artifact(
                    "boundary assignments must be unique, codimension-one, nonempty, and parent-outward",
                ));
            }
            if let Some((first_parent, first_facets)) = boundary_entities.get(&entity) {
                if *first_parent == boundary.parent_ulid || *first_facets != facets {
                    return Err(invalid_artifact(
                        "a shared geometry boundary requires distinct parents and identical facet membership",
                    ));
                }
            } else {
                boundary_entities.insert(entity, (&boundary.parent_ulid, facets.clone()));
            }
            membership_count = membership_count
                .checked_add(facets.len())
                .ok_or_else(|| invalid_artifact("geometry membership count overflows usize"))?;
        }
        if membership_count > limits.max_geometry_mesh_memberships
            || !strictly_sorted_assignments(&self.wire.bodies, &self.wire.boundaries)
        {
            return Err(invalid_artifact(
                "geometry-mesh memberships exceed limits or are not canonical",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGeometryMeshCorrespondenceV1 {
    schema: String,
    encoding: String,
    geometry_sha256: String,
    mesh_sha256: String,
    dimension: u64,
    bodies: Vec<WireBodyAssignment>,
    boundaries: Vec<WireBoundaryAssignment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireBodyAssignment {
    domain_ulid: String,
    geometry_entity: WireGeometryEntity,
    cell_indices: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireBoundaryAssignment {
    domain_ulid: String,
    parent_ulid: String,
    geometry_entity: WireGeometryEntity,
    axis: u64,
    side: WireBoundarySide,
    orientation: WireBoundaryOrientation,
    facet_indices: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireBoundarySide {
    Lower,
    Upper,
}

impl WireBoundarySide {
    const fn encode(side: BoundarySide) -> Self {
        match side {
            BoundarySide::Lower => Self::Lower,
            BoundarySide::Upper => Self::Upper,
        }
    }

    const fn decode(self) -> BoundarySide {
        match self {
            Self::Lower => BoundarySide::Lower,
            Self::Upper => BoundarySide::Upper,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireBoundaryOrientation {
    ParentOutward,
}

fn entity_coordinates(
    mesh: &eqiora_meshing::SimplicialMesh,
    entity: MeshEntity,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    mesh.entity_vertices(entity)
        .ok_or_else(|| invalid_artifact("mesh entity has no vertex closure"))?
        .into_iter()
        .map(|vertex| {
            mesh.vertices()
                .get(vertex.index())
                .cloned()
                .ok_or_else(|| invalid_artifact("mesh entity references an unavailable vertex"))
        })
        .collect()
}

fn point_inside_cartesian(point: &[f64], bounds: &[(f64, f64)], tolerance: f64) -> bool {
    point.len() == bounds.len()
        && point
            .iter()
            .zip(bounds)
            .all(|(&coordinate, &(lower, upper))| {
                coordinate >= lower - tolerance && coordinate <= upper + tolerance
            })
}

fn cartesian_facet_roles(
    points: &[Vec<f64>],
    bounds: &[(f64, f64)],
    tolerance: f64,
) -> Vec<(usize, BoundarySide)> {
    bounds
        .iter()
        .enumerate()
        .flat_map(|(axis, &(lower, upper))| {
            [(BoundarySide::Lower, lower), (BoundarySide::Upper, upper)]
                .into_iter()
                .filter(move |&(_, coordinate)| {
                    points
                        .iter()
                        .all(|point| (point[axis] - coordinate).abs() <= tolerance)
                })
                .map(move |(side, _)| (axis, side))
        })
        .collect()
}

fn validate_parent_outward_cell(
    mesh: &eqiora_meshing::SimplicialMesh,
    cell: MeshEntity,
    axis: usize,
    side: BoundarySide,
    bounds: &[(f64, f64)],
) -> Result<(), Diagnostic> {
    let vertices = entity_coordinates(mesh, cell)?;
    let centroid = vertices.iter().map(|vertex| vertex[axis]).sum::<f64>() / vertices.len() as f64;
    let valid = match side {
        BoundarySide::Lower => centroid > bounds[axis].0,
        BoundarySide::Upper => centroid < bounds[axis].1,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_artifact(
            "mesh incidence does not derive the declared parent-outward boundary role",
        ))
    }
}

fn validate_coincident_sides(
    first_bounds: &[(f64, f64)],
    axis: usize,
    first_side: BoundarySide,
    second_bounds: &[(f64, f64)],
    second_side: BoundarySide,
    tolerance: f64,
) -> Result<(), Diagnostic> {
    if first_bounds.len() != second_bounds.len() || axis >= first_bounds.len() {
        return Err(invalid_artifact("interface parent dimensions differ"));
    }
    let first_coordinate = match first_side {
        BoundarySide::Lower => first_bounds[axis].0,
        BoundarySide::Upper => first_bounds[axis].1,
    };
    let second_coordinate = match second_side {
        BoundarySide::Lower => second_bounds[axis].0,
        BoundarySide::Upper => second_bounds[axis].1,
    };
    if (first_coordinate - second_coordinate).abs() > tolerance
        || first_bounds.iter().zip(second_bounds).enumerate().any(
            |(candidate_axis, (&first, &second))| {
                candidate_axis != axis
                    && ((first.0 - second.0).abs() > tolerance
                        || (first.1 - second.1).abs() > tolerance)
            },
        )
    {
        return Err(invalid_artifact(
            "interface boundary embeddings are not coincident complete sides",
        ));
    }
    Ok(())
}

fn encode_indices(indices: &[usize], label: &str) -> Result<Vec<u64>, Diagnostic> {
    indices
        .iter()
        .map(|&index| {
            u64::try_from(index)
                .map_err(|_| invalid_artifact(format!("{label} index exceeds portable u64")))
        })
        .collect()
}

fn decode_indices(indices: &[u64]) -> Result<Vec<usize>, Diagnostic> {
    indices
        .iter()
        .map(|&index| {
            usize::try_from(index)
                .map_err(|_| invalid_artifact("mesh entity index exceeds local usize"))
        })
        .collect()
}

fn strictly_sorted_unique(values: &[usize]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn strictly_sorted_assignments(
    bodies: &[WireBodyAssignment],
    boundaries: &[WireBoundaryAssignment],
) -> bool {
    bodies
        .windows(2)
        .all(|pair| pair[0].geometry_entity < pair[1].geometry_entity)
        && boundaries.windows(2).all(|pair| {
            (
                pair[0].geometry_entity,
                &pair[0].parent_ulid,
                &pair[0].domain_ulid,
            ) < (
                pair[1].geometry_entity,
                &pair[1].parent_ulid,
                &pair[1].domain_ulid,
            )
        })
}

fn parse_ulid(value: &str, label: &str) -> Result<Ulid, Diagnostic> {
    Ulid::from_str(value).map_err(|_| invalid_artifact(format!("{label} ULID is invalid")))
}
