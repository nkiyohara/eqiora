//! Private three-generator Cartesian periodic topology projection.
//!
//! The projection composes three already-validated spatial-periodic boundary
//! pairs with one exact Cartesian mesh. It remains an internal structural
//! prerequisite: no numerical operator, persisted representation, or public
//! topology API is introduced here.

#![cfg_attr(not(test), allow(dead_code))]

mod group;
mod packet;
mod quotient;

use eqiora_artifact::{
    AcceptedModelArtifact, CartesianMeshEnvelopeV1, ReplayableCanonicalModelArtifact,
};
use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_meshing::{CartesianMesh, MeshTopology};

use group::{Generator, admit_group, derive_generator};
use packet::{PositivePacket, derive_packets};
use quotient::{
    CellFaceIncidence, CycleReceipt, FaceIncidence, Inventory, QuotientEntity,
    derive_cell_face_incidences, derive_cycles, derive_entities, derive_face_incidences,
    derive_inventory, require_connected,
};

const DIMENSION: usize = 3;
const REFERENCE_COUNTS: [usize; DIMENSION] = [2, 3, 4];
const COLLOCATED_COUNTS: [usize; DIMENSION] = [4, 6, 8];

#[derive(Debug, Clone, Copy)]
enum ProjectionProfile {
    ReferenceNonuniform2x3x4,
    CollocatedUniform4x6x8,
}

impl ProjectionProfile {
    fn counts(self) -> [usize; DIMENSION] {
        match self {
            Self::ReferenceNonuniform2x3x4 => REFERENCE_COUNTS,
            Self::CollocatedUniform4x6x8 => COLLOCATED_COUNTS,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CartesianPeriodic3dProjection {
    model_artifact_sha256: String,
    model_id: String,
    semantic_revision: u64,
    mesh_artifact_sha256: [u8; 32],
    parent: RawId,
    connector: RawId,
    generators: Vec<Generator>,
    cycles: Vec<CycleReceipt>,
    inventory: Inventory,
    entities: Vec<QuotientEntity>,
    face_incidences: Vec<FaceIncidence>,
    cell_face_incidences: Vec<CellFaceIncidence>,
    packets: Vec<PositivePacket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionEvent {
    PairValidated(RawId),
    GroupAdmitted,
    AbstractInventoryAdmitted(Inventory),
    ProjectionStateAllocated,
    ProjectionPublished,
}

#[derive(Debug, Clone)]
struct ProjectionRun {
    result: Result<CartesianPeriodic3dProjection, String>,
    events: Vec<ProjectionEvent>,
}

fn run_projection(
    model: &AcceptedModelArtifact,
    mesh_envelope: &CartesianMeshEnvelopeV1,
    selected_connections: &[Id<kinds::Connection>],
) -> ProjectionRun {
    run_projection_for(
        model,
        mesh_envelope,
        selected_connections,
        ProjectionProfile::ReferenceNonuniform2x3x4,
    )
}

fn run_projection_for(
    model: &AcceptedModelArtifact,
    mesh_envelope: &CartesianMeshEnvelopeV1,
    selected_connections: &[Id<kinds::Connection>],
    profile: ProjectionProfile,
) -> ProjectionRun {
    let mut events = Vec::new();
    let result = (|| {
        let replayed = model
            .replay_model()
            .map_err(|error| format!("cannot replay exact Model: {error}"))?;
        let program = replayed.program();
        let mut generators = Vec::with_capacity(selected_connections.len());
        for &connection in selected_connections {
            let generator = derive_generator(program, connection)?;
            events.push(ProjectionEvent::PairValidated(generator.connection));
            generators.push(generator);
        }
        let (parent, connector) = admit_group(program, &mut generators)?;
        events.push(ProjectionEvent::GroupAdmitted);

        let counts = profile.counts();
        let mesh = mesh_envelope.mesh();
        let axes = admit_mesh(mesh, &generators, profile)?;
        let inventory = derive_inventory(counts)?;
        events.push(ProjectionEvent::AbstractInventoryAdmitted(inventory));
        let model_reference = replayed.artifact_reference();
        let mesh_reference = mesh_envelope
            .artifact_reference()
            .map_err(|error| format!("cannot identify exact mesh: {error}"))?;

        events.push(ProjectionEvent::ProjectionStateAllocated);
        let entities = derive_entities(mesh, counts, inventory)?;
        let face_incidences = derive_face_incidences(counts)?;
        require_connected(&face_incidences, inventory.cells)?;
        let cell_face_incidences = derive_cell_face_incidences(counts)?;
        let packets = derive_packets(&axes, counts)?;
        if packets.len() != inventory.positive_packets {
            return Err("positive-packet inventory mismatch".to_owned());
        }
        let projection = CartesianPeriodic3dProjection {
            model_artifact_sha256: model_reference.artifact().as_str().to_owned(),
            model_id: model_reference.model().to_string(),
            semantic_revision: model_reference.semantic_revision().get(),
            mesh_artifact_sha256: mesh_reference.sha256(),
            parent,
            connector,
            generators,
            cycles: derive_cycles(),
            inventory,
            entities,
            face_incidences,
            cell_face_incidences,
            packets,
        };
        events.push(ProjectionEvent::ProjectionPublished);
        Ok(projection)
    })();
    ProjectionRun { result, events }
}

fn admit_mesh(
    mesh: &CartesianMesh,
    generators: &[Generator],
    profile: ProjectionProfile,
) -> Result<[Vec<f64>; DIMENSION], String> {
    if mesh.topological_dimension() != DIMENSION {
        return Err(match profile {
            ProjectionProfile::ReferenceNonuniform2x3x4 => {
                "reference mesh must be full-dimensional Cartesian 3D".to_owned()
            }
            ProjectionProfile::CollocatedUniform4x6x8 => {
                "collocated mesh must be full-dimensional Cartesian 3D".to_owned()
            }
        });
    }
    let counts = profile.counts();
    let mut axes = std::array::from_fn(|_| Vec::new());
    for axis in 0..DIMENSION {
        let coordinates = mesh.axis_coordinates(axis).ok_or_else(|| match profile {
            ProjectionProfile::ReferenceNonuniform2x3x4 => {
                "reference mesh omitted a physical axis".to_owned()
            }
            ProjectionProfile::CollocatedUniform4x6x8 => {
                "collocated mesh omitted a physical axis".to_owned()
            }
        })?;
        if coordinates.len().checked_sub(1) != Some(counts[axis]) {
            return Err(match profile {
                ProjectionProfile::ReferenceNonuniform2x3x4 => {
                    "reference mesh cell counts must be exactly 2 x 3 x 4".to_owned()
                }
                ProjectionProfile::CollocatedUniform4x6x8 => {
                    "collocated mesh cell counts must be exactly 4 x 6 x 8".to_owned()
                }
            });
        }
        let generator = &generators[axis];
        if coordinates.first().copied() != Some(generator.lower_coordinate)
            || coordinates.last().copied() != Some(generator.upper_coordinate)
        {
            return Err(match profile {
                ProjectionProfile::ReferenceNonuniform2x3x4 => {
                    "reference mesh bounds differ from exact parent bounds".to_owned()
                }
                ProjectionProfile::CollocatedUniform4x6x8 => {
                    "collocated mesh bounds differ from exact parent bounds".to_owned()
                }
            });
        }
        axes[axis] = coordinates.to_vec();
    }
    match profile {
        ProjectionProfile::ReferenceNonuniform2x3x4 => admit_reference_shape(&axes)?,
        ProjectionProfile::CollocatedUniform4x6x8 => admit_collocated_shape(&axes)?,
    }
    Ok(axes)
}

fn admit_reference_shape(axes: &[Vec<f64>; DIMENSION]) -> Result<(), String> {
    let side_lengths = axes.each_ref().map(|axis| axis[axis.len() - 1] - axis[0]);
    if side_lengths[0] == side_lengths[1]
        || side_lengths[0] == side_lengths[2]
        || side_lengths[1] == side_lengths[2]
    {
        return Err("reference parent side lengths must be unequal".to_owned());
    }
    if !axes.iter().any(|axis| {
        let mut widths = axis.windows(2).map(|pair| pair[1] - pair[0]);
        let Some(first) = widths.next() else {
            return false;
        };
        widths.any(|width| width != first)
    }) {
        return Err("reference mesh requires at least one nonuniform axis".to_owned());
    }
    Ok(())
}

fn admit_collocated_shape(axes: &[Vec<f64>; DIMENSION]) -> Result<(), String> {
    for axis in axes {
        let cells = axis
            .len()
            .checked_sub(1)
            .ok_or_else(|| "collocated mesh axis is empty".to_owned())?;
        let lower = axis[0];
        let upper = axis[cells];
        let spacing = (upper - lower) / cells as f64;
        let canonical = axis[..cells]
            .iter()
            .enumerate()
            .all(|(index, coordinate)| *coordinate == lower + index as f64 * spacing);
        if !spacing.is_finite() || spacing <= 0.0 || !canonical {
            return Err("collocated mesh axes must be exactly uniform".to_owned());
        }
    }
    Ok(())
}

/// Exact crate-private semantic/topology view for the accepted collocated
/// `4 x 6 x 8` class. Callers supply no counts, packet table, or modulo rule.
#[allow(dead_code)]
pub(crate) fn project_collocated_4x6x8(
    model: &AcceptedModelArtifact,
    mesh_envelope: &CartesianMeshEnvelopeV1,
    selected_connections: &[Id<kinds::Connection>],
) -> Result<CollocatedPeriodic3dView, String> {
    run_projection_for(
        model,
        mesh_envelope,
        selected_connections,
        ProjectionProfile::CollocatedUniform4x6x8,
    )
    .result
    .and_then(CollocatedPeriodic3dView::from_projection)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct CollocatedPeriodic3dView {
    model_artifact_sha256: String,
    model_id: String,
    semantic_revision: u64,
    mesh_artifact_sha256: [u8; 32],
    parent: RawId,
    connector: RawId,
    connections: [RawId; DIMENSION],
    packets: Vec<CollocatedPeriodic3dPacket>,
}

#[allow(dead_code)]
impl CollocatedPeriodic3dView {
    fn from_projection(projection: CartesianPeriodic3dProjection) -> Result<Self, String> {
        let CartesianPeriodic3dProjection {
            model_artifact_sha256,
            model_id,
            semantic_revision,
            mesh_artifact_sha256,
            parent,
            connector,
            generators,
            packets,
            ..
        } = projection;
        if generators.len() != DIMENSION {
            return Err("admitted group does not contain exactly three generators".to_owned());
        }
        // The class geometry is exactly the uniform-axis constants of the
        // accepted parent bounds: `h_d = ell_d / N_d`, `V = h_0 h_1 h_2`,
        // `A_d = V / h_d`. Admission has already bound the mesh bytes to
        // these exact bounds; no coordinate subtraction re-derives them.
        let spacings: [f64; DIMENSION] = std::array::from_fn(|axis| {
            let generator = &generators[axis];
            (generator.upper_coordinate - generator.lower_coordinate)
                / COLLOCATED_COUNTS[axis] as f64
        });
        let volume = spacings[0] * spacings[1] * spacings[2];
        let areas: [f64; DIMENSION] = std::array::from_fn(|axis| volume / spacings[axis]);
        if !volume.is_finite()
            || volume <= 0.0
            || areas.iter().any(|area| !area.is_finite() || *area <= 0.0)
        {
            return Err("collocated cell geometry must be finite and positive".to_owned());
        }
        let connections = generators
            .into_iter()
            .map(|generator| generator.connection)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "admitted group does not contain exactly three generators".to_owned())?;
        Ok(Self {
            model_artifact_sha256,
            model_id,
            semantic_revision,
            mesh_artifact_sha256,
            parent,
            connector,
            connections,
            packets: packets
                .into_iter()
                .map(|packet| CollocatedPeriodic3dPacket {
                    packet: packet.packet,
                    axis: packet.axis,
                    owner_cell: packet.owner_cell,
                    neighbor_cell: packet.neighbor_cell,
                    quotient_face: packet.quotient_face,
                    face_area: areas[packet.axis],
                    lifted_center_distance: spacings[packet.axis],
                    seam: packet.seam,
                    normal: packet.normal,
                })
                .collect(),
        })
    }

    pub(crate) fn model_artifact_sha256(&self) -> &str {
        &self.model_artifact_sha256
    }

    pub(crate) fn model_id(&self) -> &str {
        &self.model_id
    }

    pub(crate) fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }

    pub(crate) fn mesh_artifact_sha256(&self) -> [u8; 32] {
        self.mesh_artifact_sha256
    }

    pub(crate) fn parent(&self) -> RawId {
        self.parent
    }

    pub(crate) fn connector(&self) -> RawId {
        self.connector
    }

    pub(crate) fn connections(&self) -> &[RawId; DIMENSION] {
        &self.connections
    }

    pub(crate) fn counts(&self) -> [usize; DIMENSION] {
        COLLOCATED_COUNTS
    }

    pub(crate) fn packets(&self) -> &[CollocatedPeriodic3dPacket] {
        &self.packets
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct CollocatedPeriodic3dPacket {
    packet: usize,
    axis: usize,
    owner_cell: usize,
    neighbor_cell: usize,
    quotient_face: usize,
    face_area: f64,
    lifted_center_distance: f64,
    seam: bool,
    normal: [i8; DIMENSION],
}

#[allow(dead_code)]
impl CollocatedPeriodic3dPacket {
    pub(crate) fn packet(&self) -> usize {
        self.packet
    }

    pub(crate) fn axis(&self) -> usize {
        self.axis
    }

    pub(crate) fn owner_cell(&self) -> usize {
        self.owner_cell
    }

    pub(crate) fn neighbor_cell(&self) -> usize {
        self.neighbor_cell
    }

    pub(crate) fn quotient_face(&self) -> usize {
        self.quotient_face
    }

    pub(crate) fn face_area(&self) -> f64 {
        self.face_area
    }

    pub(crate) fn lifted_center_distance(&self) -> f64 {
        self.lifted_center_distance
    }

    pub(crate) fn seam(&self) -> bool {
        self.seam
    }

    pub(crate) fn normal(&self) -> [i8; DIMENSION] {
        self.normal
    }

    pub(crate) fn scatter_signs(&self) -> [i8; 2] {
        [1, -1]
    }
}

#[cfg(test)]
pub(super) struct EvidenceHarness;

#[cfg(test)]
impl tests::ProductHarness for EvidenceHarness {
    fn project(
        model: &AcceptedModelArtifact,
        mesh: &CartesianMeshEnvelopeV1,
        selected_connections: &[Id<kinds::Connection>],
    ) -> tests::ProductRun {
        let run = run_projection(model, mesh, selected_connections);
        tests::ProductRun {
            result: run.result.map(to_evidence_observation),
            events: run.events.into_iter().map(to_evidence_event).collect(),
        }
    }
}

#[cfg(test)]
fn to_evidence_event(event: ProjectionEvent) -> tests::ProductEvent {
    match event {
        ProjectionEvent::PairValidated(connection) => {
            tests::ProductEvent::PairValidated(connection)
        }
        ProjectionEvent::GroupAdmitted => tests::ProductEvent::GroupAdmitted,
        ProjectionEvent::AbstractInventoryAdmitted(inventory) => {
            tests::ProductEvent::AbstractInventoryAdmitted(to_evidence_inventory(inventory))
        }
        ProjectionEvent::ProjectionStateAllocated => tests::ProductEvent::ProjectionStateAllocated,
        ProjectionEvent::ProjectionPublished => tests::ProductEvent::ProjectionPublished,
    }
}

#[cfg(test)]
fn to_evidence_inventory(inventory: Inventory) -> tests::InventoryObservation {
    tests::InventoryObservation {
        cells: inventory.cells,
        box_entities: inventory.box_entities,
        quotient_strata: inventory.quotient_strata,
        quotient_entities: inventory.quotient_entities,
        quotient_closure_vertex_references: inventory.quotient_closure_vertex_references,
        orbit_outputs: inventory.orbit_outputs,
        box_orbit_memberships: inventory.box_orbit_memberships,
        positive_packets: inventory.positive_packets,
        seam_packets: inventory.seam_packets,
    }
}

#[cfg(test)]
fn to_evidence_observation(
    projection: CartesianPeriodic3dProjection,
) -> tests::ProductProjectionObservation {
    tests::ProductProjectionObservation {
        model_artifact_sha256: projection.model_artifact_sha256,
        model_id: projection.model_id,
        semantic_revision: projection.semantic_revision,
        mesh_artifact_sha256: projection.mesh_artifact_sha256,
        parent: projection.parent,
        connector: projection.connector,
        generators: projection
            .generators
            .into_iter()
            .map(|generator| tests::GeneratorObservation {
                connection: generator.connection,
                lower_port: generator.lower_port,
                upper_port: generator.upper_port,
                parent: generator.parent,
                connector: generator.connector,
                axis: generator.axis,
                lower_coordinate: generator.lower_coordinate,
                upper_coordinate: generator.upper_coordinate,
                period: generator.period,
                authority: tests::GeneratorAuthority::SpatialPeriodicPair,
                translation_authority: tests::TranslationAuthority::ParentBounds,
                identity_fiber: true,
                lower_outward_sign: -1,
                upper_outward_sign: 1,
            })
            .collect(),
        cycles: projection
            .cycles
            .into_iter()
            .map(|cycle| tests::CycleObservation {
                word: cycle.word,
                net_coefficients: cycle.net_coefficients,
                identity_fiber: true,
                anchor_commutes: true,
                incidence_commutes: true,
            })
            .collect(),
        inventory: to_evidence_inventory(projection.inventory),
        entities: projection
            .entities
            .into_iter()
            .map(|entity| tests::EntityObservation {
                dimension: entity.dimension,
                free_axes: entity.free_axes,
                quotient_anchor: entity.quotient_anchor,
                quotient_index: entity.quotient_index,
                orbit: entity
                    .orbit
                    .into_iter()
                    .map(|representative| tests::BoxRepresentativeObservation {
                        anchors: representative.anchors,
                        base_index: representative.base_index,
                    })
                    .collect(),
                closure_vertices: entity.closure_vertices,
                orientation_code: 0,
            })
            .collect(),
        face_incidences: projection
            .face_incidences
            .into_iter()
            .map(|incidence| tests::FaceIncidenceObservation {
                quotient_face: incidence.quotient_face,
                positive_side_cell: incidence.positive_side_cell,
                negative_side_cell: incidence.negative_side_cell,
            })
            .collect(),
        cell_face_incidences: projection
            .cell_face_incidences
            .into_iter()
            .map(|incidence| tests::CellFaceIncidenceObservation {
                cell: incidence.cell,
                axis: incidence.axis,
                side: incidence.side,
                quotient_face: incidence.quotient_face,
            })
            .collect(),
        packets: projection
            .packets
            .into_iter()
            .map(|packet| tests::PacketObservation {
                packet: packet.packet,
                axis: packet.axis,
                owner_cell: packet.owner_cell,
                neighbor_cell: packet.neighbor_cell,
                quotient_face: packet.quotient_face,
                normal: packet.normal,
                scatter_signs: [1, -1],
                seam: packet.seam,
                lifted_center_distance: packet.lifted_center_distance,
                owner_face_points: packet.owner_face_points,
                lifted_neighbor_face_points: packet.lifted_neighbor_face_points,
            })
            .collect(),
        exterior_face_count: 0,
        persisted_quotient: false,
    }
}

#[cfg(test)]
mod collocated_tests;
#[cfg(test)]
mod tests;
