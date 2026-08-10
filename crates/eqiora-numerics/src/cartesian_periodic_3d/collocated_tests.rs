//! Independent structural evidence for the exact route-sealed `4 x 6 x 8` view.
//!
//! Expected packets are instantiated from the accepted literal seal and modulo
//! law. This module observes the real private view, events, and inventory; it
//! does not call a production derivation or admission helper.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::{
    AcceptedModelArtifact, CartesianMeshEnvelopeV1, ModelDecoderLimits, ModelTransactionEnvelope,
    ReplayableCanonicalModelArtifact,
};
use eqiora_compiler::compile;
use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore};
use eqiora_meshing::CartesianMesh;
use eqiora_schema::kernel::{BoundarySide, ConnectionSemantics, DomainKind, KernelNode};
use eqiora_sem::{BoundaryJunctionGeometry, KernelProgram};

const SOURCE: &str =
    include_str!("../../../../verify/fluid/cartesian-periodic-topology-3d/models/periodic-box.eqi");
const PERMUTED_SOURCE: &str = include_str!(
    "../../../../verify/fluid/cartesian-periodic-topology-3d/models/periodic-box-permuted.eqi"
);

const COUNTS: [usize; 3] = [4, 6, 8];
const CELLS: usize = 192;
const PACKETS: usize = 576;
const MODEL_SHA256: &str = "5d5ef599d426103a15ced2b2ad859d69739204ffc912e9e21a49b5cd611b7738";
const MODEL_ID: &str = "06BWXYZM2HZNYK7HQ87V3YSFY6";
const SEMANTIC_REVISION: u64 = 1;
const PREDECESSOR_MESH_SHA256: &str =
    "d2da7e53e2e2e329276582c8b2a786c4fa4df9ce653be8ddedb098c667de2301";

const AXIS_BITS: [&[u64]; 3] = [
    &[
        0xC000_0000_0000_0000,
        0xBFE8_0000_0000_0000,
        0x3FE0_0000_0000_0000,
        0x3FFC_0000_0000_0000,
        0x4008_0000_0000_0000,
    ],
    &[
        0x3FF0_0000_0000_0000,
        0x4001_5555_5555_5556,
        0x400A_AAAA_AAAA_AAAB,
        0x4012_0000_0000_0000,
        0x4016_AAAA_AAAA_AAAB,
        0x401B_5555_5555_5556,
        0x4020_0000_0000_0000,
    ],
    &[
        0xBFF0_0000_0000_0000,
        0x3FD8_0000_0000_0000,
        0x3FFC_0000_0000_0000,
        0x4009_0000_0000_0000,
        0x4012_0000_0000_0000,
        0x4017_8000_0000_0000,
        0x401D_0000_0000_0000,
        0x4021_4000_0000_0000,
        0x4024_0000_0000_0000,
    ],
];
const H_BITS: [u64; 3] = [
    0x3FF4_0000_0000_0000,
    0x3FF2_AAAA_AAAA_AAAB,
    0x3FF6_0000_0000_0000,
];
const V_BITS: u64 = 0x4000_0AAA_AAAA_AAAB;
const AREA_BITS: [u64; 3] = [
    0x3FF9_AAAA_AAAA_AAAB,
    0x3FFB_8000_0000_0000,
    0x3FF7_5555_5555_5556,
];

#[derive(Debug)]
struct Fixture {
    model: AcceptedModelArtifact,
    mesh: CartesianMeshEnvelopeV1,
    mesh_json: Vec<u8>,
    mesh_sha256: [u8; 32],
    connections: [Id<kinds::Connection>; 3],
    parent: RawId,
    connector: RawId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ViewObservation {
    model_artifact_sha256: String,
    model_id: String,
    semantic_revision: u64,
    mesh_artifact_sha256: [u8; 32],
    parent: RawId,
    connector: RawId,
    connections: [RawId; 3],
    counts: [usize; 3],
    packets: Vec<PacketObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PacketObservation {
    packet: usize,
    axis: usize,
    owner_cell: usize,
    neighbor_cell: usize,
    quotient_face: usize,
    face_area_bits: u64,
    lifted_center_distance_bits: u64,
    seam: bool,
    normal: [i8; 3],
    scatter_signs: [i8; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryObservation {
    cells: usize,
    box_entities: usize,
    quotient_strata: [usize; 4],
    quotient_entities: usize,
    quotient_closure_vertex_references: usize,
    orbit_outputs: usize,
    box_orbit_memberships: usize,
    positive_packets: usize,
    seam_packets: [usize; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationFault {
    ModelIdentity,
    MeshIdentity,
    ParentConnectorIdentity,
    ConnectionAxisOrder,
    CountTuple,
    ReverseDuplicate,
    PacketCount,
    PacketIdentity,
    PacketBijection,
    SeamLaw,
    NeighborLaw,
    FaceAreaPartition,
    LiftedDistanceBits,
    NormalLaw,
    QuotientFaceLaw,
    ScatterConsistency,
    OwnershipInventory,
}

#[derive(Debug, Clone, Copy)]
enum ObservationMutant {
    PacketDrop,
    SeamFlag,
    TangentialShift,
    SeamDouble,
    AreaSwap,
    DistanceUlp,
    NormalSign,
    FaceFamily,
    ConnectionOrder,
    MeshIdentity,
    ModelRevision,
    ParentConnectorSwap,
    PacketIdRenumber,
}

impl ObservationMutant {
    const ALL: [Self; 13] = [
        Self::PacketDrop,
        Self::SeamFlag,
        Self::TangentialShift,
        Self::SeamDouble,
        Self::AreaSwap,
        Self::DistanceUlp,
        Self::NormalSign,
        Self::FaceFamily,
        Self::ConnectionOrder,
        Self::MeshIdentity,
        Self::ModelRevision,
        Self::ParentConnectorSwap,
        Self::PacketIdRenumber,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::PacketDrop => "CV4-OBS-PACKET-DROP",
            Self::SeamFlag => "CV4-OBS-SEAM-FLAG",
            Self::TangentialShift => "CV4-OBS-TANGENTIAL-SHIFT",
            Self::SeamDouble => "CV4-OBS-SEAM-DOUBLE",
            Self::AreaSwap => "CV4-OBS-AREA-SWAP",
            Self::DistanceUlp => "CV4-OBS-DISTANCE-ULP",
            Self::NormalSign => "CV4-OBS-NORMAL-SIGN",
            Self::FaceFamily => "CV4-OBS-FACE-FAMILY",
            Self::ConnectionOrder => "CV4-OBS-CONNECTION-ORDER",
            Self::MeshIdentity => "CV4-OBS-MESH-IDENTITY",
            Self::ModelRevision => "CV4-OBS-MODEL-REVISION",
            Self::ParentConnectorSwap => "CV4-OBS-PARENT-CONNECTOR-SWAP",
            Self::PacketIdRenumber => "CV4-OBS-PACKET-ID-RENUMBER",
        }
    }

    const fn expected_fault(self) -> ValidationFault {
        match self {
            Self::PacketDrop => ValidationFault::PacketCount,
            Self::SeamFlag => ValidationFault::SeamLaw,
            Self::TangentialShift => ValidationFault::NeighborLaw,
            Self::SeamDouble => ValidationFault::ReverseDuplicate,
            Self::AreaSwap => ValidationFault::FaceAreaPartition,
            Self::DistanceUlp => ValidationFault::LiftedDistanceBits,
            Self::NormalSign => ValidationFault::NormalLaw,
            Self::FaceFamily => ValidationFault::QuotientFaceLaw,
            Self::ConnectionOrder => ValidationFault::ConnectionAxisOrder,
            Self::MeshIdentity => ValidationFault::MeshIdentity,
            Self::ModelRevision => ValidationFault::ModelIdentity,
            Self::ParentConnectorSwap => ValidationFault::ParentConnectorIdentity,
            Self::PacketIdRenumber => ValidationFault::PacketIdentity,
        }
    }
}

#[test]
fn registered_collocated_4x6x8_view_rejects_class_mutants() {
    // No rejection counts until the complete ordinary source-to-artifact path
    // and independent full packet replay accept this positive.
    assert_literal_seal_and_geometry();
    let fixture = fixture(SOURCE);
    let supplied = [
        fixture.connections[2],
        fixture.connections[0],
        fixture.connections[1],
    ];
    let positive = project_observation(&fixture, &fixture.mesh, &supplied);
    validate_view(&fixture, &positive)
        .expect("the independent packet replay accepts the ordinary positive");

    assert_supply_order_invariance(&fixture, &positive);
    assert_packet_container_order_is_not_identity(&fixture, &positive);
    assert_declaration_permutation_invariance(&positive);
    assert_mesh_does_not_persist_quotient(&fixture);
    assert_positive_events_and_inventory(&fixture, &supplied);

    assert_input_falsifiers(&fixture, &supplied, &positive);
    assert_observation_mutants(&fixture, &positive);
}

fn fixture(source: &str) -> Fixture {
    let mut compiled = compile("periodic-box.eqi", source).expect("ordinary .eqi source compiles");
    assert_eq!(compiled.len(), 1, "the evidence source owns one Model");
    let (transaction, model_id, _) = compiled.remove(0).into_parts();

    let transaction_envelope = ModelTransactionEnvelope::from_transaction(&transaction)
        .expect("the current Transaction owner encodes compiler output");
    let transaction_json = transaction_envelope
        .canonical_json()
        .expect("canonical Transaction bytes");
    let replayed_transaction =
        ModelTransactionEnvelope::from_json(&transaction_json, ModelDecoderLimits::default())
            .expect("the current Transaction owner decodes its bytes")
            .to_transaction()
            .expect("the decoded Transaction reconstructs");
    let mut store = InMemoryGraphStore::new();
    store
        .commit(replayed_transaction)
        .expect("the replayed compiler transaction commits");
    let compiled_program = KernelProgram::from_snapshot(&store.snapshot(), model_id)
        .expect("the whole Model validates");

    let encoded_model = AcceptedModelArtifact::from_program(&compiled_program)
        .expect("the current Model owner encodes the validated program");
    let model_json = encoded_model
        .canonical_json()
        .expect("canonical Model bytes");
    let model = AcceptedModelArtifact::from_json(&model_json, ModelDecoderLimits::default())
        .expect("the current Model owner decodes its bytes");
    let replayed_model = model
        .replay_model()
        .expect("the exact Model artifact replays through whole-Model validation");
    assert_eq!(replayed_model.program(), &compiled_program);
    let model_reference = replayed_model.artifact_reference();
    assert_eq!(model_reference.artifact().as_str(), MODEL_SHA256);
    assert_eq!(model_reference.model().to_string(), MODEL_ID);
    assert_eq!(model_reference.semantic_revision().get(), SEMANTIC_REVISION);

    let (connections, parent, connector) = independently_resolve_group(&compiled_program);
    let axes = sealed_axes();
    let mesh = mesh_envelope_from_axes(axes);
    let mesh_json = mesh
        .canonical_json()
        .expect("canonical collocated Cartesian mesh bytes");
    let mesh_sha256 = mesh
        .artifact_reference()
        .expect("the sealed mesh has a producer identity")
        .sha256();
    assert_ne!(hex_digest(mesh_sha256), PREDECESSOR_MESH_SHA256);

    Fixture {
        model,
        mesh,
        mesh_json,
        mesh_sha256,
        connections,
        parent,
        connector,
    }
}

fn independently_resolve_group(
    program: &KernelProgram,
) -> ([Id<kinds::Connection>; 3], RawId, RawId) {
    let mut pairs = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Connection(connection)
                if connection.semantics() == ConnectionSemantics::SpatialPeriodic =>
            {
                Some(resolve_pair_identity(program, connection.id()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pairs.len(),
        3,
        "the source owns exactly three periodic pairs"
    );
    pairs.sort_by_key(|pair| pair.0);
    assert_eq!(
        pairs.iter().map(|pair| pair.0).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    let parent = pairs[0].2;
    let connector = pairs[0].3;
    assert!(pairs.iter().all(|pair| pair.2 == parent));
    assert!(pairs.iter().all(|pair| pair.3 == connector));

    let bounds = program
        .resolved_cartesian_bounds(parent.downcast().expect("typed parent"))
        .expect("the exact Cartesian parent bounds resolve");
    for axis in 0..3 {
        assert_eq!(bounds[axis].lower().value().to_bits(), AXIS_BITS[axis][0]);
        assert_eq!(
            bounds[axis].upper().value().to_bits(),
            AXIS_BITS[axis][COUNTS[axis]]
        );
    }

    ([pairs[0].1, pairs[1].1, pairs[2].1], parent, connector)
}

fn resolve_pair_identity(
    program: &KernelProgram,
    connection: Id<kinds::Connection>,
) -> (usize, Id<kinds::Connection>, RawId, RawId) {
    let junction = program
        .compose_boundary_physical_junction(connection)
        .expect("each pair passes the existing semantic validator");
    let BoundaryJunctionGeometry::CartesianPeriodic(identification) = junction.geometry() else {
        panic!("selected Connection must retain spatial-periodic meaning");
    };
    assert_eq!(identification.ambient_dimension(), 3);
    let normal_axis = identification.normal_axis();

    let ports = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection.erase())
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    assert_eq!(ports.len(), 2);

    let mut pair_parent = None;
    let mut pair_connector = None;
    let mut lower = false;
    let mut upper = false;
    for port in ports {
        let KernelNode::Port(definition) = program.node(port).expect("pair Port exists") else {
            panic!("pair member must be a Port");
        };
        let (connector, boundary) = definition
            .boundary_physical_contract()
            .expect("pair member is boundary-physical");
        let KernelNode::Domain(boundary_definition) =
            program.node(boundary.erase()).expect("boundary exists")
        else {
            panic!("Port support must be a Domain");
        };
        let DomainKind::CartesianBoundary { axis, side } = boundary_definition.kind() else {
            panic!("Port support must be a Cartesian boundary");
        };
        assert_eq!(*axis, normal_axis);
        match side {
            BoundarySide::Lower => lower = true,
            BoundarySide::Upper => upper = true,
        }
        let parents = program
            .edges()
            .iter()
            .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.from() == boundary.erase())
            .map(|edge| edge.to())
            .collect::<Vec<_>>();
        assert_eq!(parents.len(), 1);
        if let Some(expected) = pair_parent {
            assert_eq!(parents[0], expected);
        } else {
            pair_parent = Some(parents[0]);
        }
        if let Some(expected) = pair_connector {
            assert_eq!(connector.erase(), expected);
        } else {
            pair_connector = Some(connector.erase());
        }
    }
    assert!(
        lower && upper,
        "each pair owns one lower and one upper Port"
    );
    (
        normal_axis,
        connection,
        pair_parent.expect("pair parent"),
        pair_connector.expect("pair Connector"),
    )
}

fn sealed_axes() -> Vec<Vec<f64>> {
    AXIS_BITS
        .iter()
        .map(|axis| axis.iter().copied().map(f64::from_bits).collect())
        .collect()
}

fn mesh_envelope_from_axes(axes: Vec<Vec<f64>>) -> CartesianMeshEnvelopeV1 {
    let mesh = CartesianMesh::from_axes(axes).expect("the test input is a valid Cartesian mesh");
    let encoded = CartesianMeshEnvelopeV1::from_mesh(&mesh)
        .expect("the current Cartesian artifact owner captures the mesh");
    let json = encoded
        .canonical_json()
        .expect("canonical Cartesian mesh bytes");
    CartesianMeshEnvelopeV1::from_json(&json, Default::default())
        .expect("the current Cartesian artifact owner replays the mesh")
}

fn assert_literal_seal_and_geometry() {
    let axes = sealed_axes();
    for axis in 0..3 {
        let lower = axes[axis][0];
        let upper = axes[axis][COUNTS[axis]];
        let spacing = (upper - lower) / COUNTS[axis] as f64;
        assert_eq!(spacing.to_bits(), H_BITS[axis]);
        for (index, expected_bits) in AXIS_BITS[axis][..COUNTS[axis]].iter().enumerate() {
            assert_eq!(
                (lower + index as f64 * spacing).to_bits(),
                *expected_bits,
                "axis {axis} coordinate {index} differs from the literal seal"
            );
        }
        assert_eq!(
            (lower + COUNTS[axis] as f64 * spacing).to_bits(),
            upper.to_bits()
        );
        let widths = axes[axis]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).to_bits())
            .collect::<BTreeSet<_>>();
        assert_eq!(widths.len(), [1, 3, 1][axis]);
    }
    let h = H_BITS.map(f64::from_bits);
    let volume = (h[0] * h[1]) * h[2];
    assert_eq!(volume.to_bits(), V_BITS);
    for axis in 0..3 {
        assert_eq!((volume / h[axis]).to_bits(), AREA_BITS[axis]);
    }
}

fn project_observation(
    fixture: &Fixture,
    mesh: &CartesianMeshEnvelopeV1,
    connections: &[Id<kinds::Connection>],
) -> ViewObservation {
    let view = super::project_collocated_4x6x8(&fixture.model, mesh, connections)
        .unwrap_or_else(|error| panic!("ordinary collocated projection failed: {error}"));
    observe_view(&view)
}

fn observe_view(view: &super::CollocatedPeriodic3dView) -> ViewObservation {
    ViewObservation {
        model_artifact_sha256: view.model_artifact_sha256().to_owned(),
        model_id: view.model_id().to_owned(),
        semantic_revision: view.semantic_revision(),
        mesh_artifact_sha256: view.mesh_artifact_sha256(),
        parent: view.parent(),
        connector: view.connector(),
        connections: *view.connections(),
        counts: view.counts(),
        packets: view
            .packets()
            .iter()
            .map(|packet| PacketObservation {
                packet: packet.packet(),
                axis: packet.axis(),
                owner_cell: packet.owner_cell(),
                neighbor_cell: packet.neighbor_cell(),
                quotient_face: packet.quotient_face(),
                face_area_bits: packet.face_area().to_bits(),
                lifted_center_distance_bits: packet.lifted_center_distance().to_bits(),
                seam: packet.seam(),
                normal: packet.normal(),
                scatter_signs: packet.scatter_signs(),
            })
            .collect(),
    }
}

fn validate_view(fixture: &Fixture, observation: &ViewObservation) -> Result<(), ValidationFault> {
    if observation.model_artifact_sha256 != MODEL_SHA256
        || observation.model_id != MODEL_ID
        || observation.semantic_revision != SEMANTIC_REVISION
    {
        return Err(ValidationFault::ModelIdentity);
    }
    if observation.mesh_artifact_sha256 != fixture.mesh_sha256
        || hex_digest(observation.mesh_artifact_sha256) == PREDECESSOR_MESH_SHA256
    {
        return Err(ValidationFault::MeshIdentity);
    }
    if observation.parent != fixture.parent || observation.connector != fixture.connector {
        return Err(ValidationFault::ParentConnectorIdentity);
    }
    let expected_connections = fixture.connections.map(Id::erase);
    if observation.connections != expected_connections {
        return Err(ValidationFault::ConnectionAxisOrder);
    }
    if observation.counts != COUNTS {
        return Err(ValidationFault::CountTuple);
    }

    for (left_index, left) in observation.packets.iter().enumerate() {
        if observation.packets[left_index + 1..].iter().any(|right| {
            left.axis == right.axis
                && left.owner_cell == right.neighbor_cell
                && left.neighbor_cell == right.owner_cell
        }) {
            return Err(ValidationFault::ReverseDuplicate);
        }
    }
    if observation.packets.len() != PACKETS {
        return Err(ValidationFault::PacketCount);
    }

    let mut by_packet = BTreeMap::new();
    let mut quotient_faces = BTreeSet::new();
    let mut seam_counts = [0_usize; 3];
    let mut owner_counts = [0_usize; CELLS];
    let mut neighbor_counts = [0_usize; CELLS];
    for packet in &observation.packets {
        if packet.axis >= 3
            || packet.owner_cell >= CELLS
            || packet.packet != packet.axis * CELLS + packet.owner_cell
        {
            return Err(ValidationFault::PacketIdentity);
        }
        let expected = expected_packet(packet.axis, packet.owner_cell);
        if packet.seam != expected.seam {
            return Err(ValidationFault::SeamLaw);
        }
        if packet.neighbor_cell != expected.neighbor_cell
            || packet.owner_cell == packet.neighbor_cell
        {
            return Err(ValidationFault::NeighborLaw);
        }
        if packet.face_area_bits != expected.face_area_bits {
            return Err(ValidationFault::FaceAreaPartition);
        }
        if packet.lifted_center_distance_bits != expected.lifted_center_distance_bits {
            return Err(ValidationFault::LiftedDistanceBits);
        }
        if packet.normal != expected.normal {
            return Err(ValidationFault::NormalLaw);
        }
        if packet.quotient_face != expected.quotient_face {
            return Err(ValidationFault::QuotientFaceLaw);
        }
        if packet.scatter_signs != [1, -1] {
            return Err(ValidationFault::ScatterConsistency);
        }
        if by_packet.insert(packet.packet, packet).is_some() {
            return Err(ValidationFault::PacketBijection);
        }
        if !quotient_faces.insert(packet.quotient_face) {
            return Err(ValidationFault::PacketBijection);
        }
        owner_counts[packet.owner_cell] += 1;
        neighbor_counts[packet.neighbor_cell] += 1;
        seam_counts[packet.axis] += usize::from(packet.seam);
    }
    if by_packet.keys().copied().ne(0..PACKETS) || quotient_faces.iter().copied().ne(0..PACKETS) {
        return Err(ValidationFault::PacketBijection);
    }
    if seam_counts != [48, 32, 24] {
        return Err(ValidationFault::SeamLaw);
    }
    if owner_counts.iter().any(|count| *count != 3)
        || neighbor_counts.iter().any(|count| *count != 3)
    {
        return Err(ValidationFault::OwnershipInventory);
    }
    Ok(())
}

fn expected_packet(axis: usize, owner_cell: usize) -> PacketObservation {
    let mut indices = [owner_cell / 48, (owner_cell % 48) / 8, owner_cell % 8];
    let seam = indices[axis] + 1 == COUNTS[axis];
    indices[axis] = (indices[axis] + 1) % COUNTS[axis];
    let neighbor_cell = 48 * indices[0] + 8 * indices[1] + indices[2];
    let mut normal = [0_i8; 3];
    normal[axis] = 1;
    PacketObservation {
        packet: axis * CELLS + owner_cell,
        axis,
        owner_cell,
        neighbor_cell,
        quotient_face: [2, 1, 0][axis] * CELLS + neighbor_cell,
        face_area_bits: AREA_BITS[axis],
        lifted_center_distance_bits: H_BITS[axis],
        seam,
        normal,
        scatter_signs: [1, -1],
    }
}

fn assert_supply_order_invariance(fixture: &Fixture, positive: &ViewObservation) {
    let identity = project_observation(fixture, &fixture.mesh, &fixture.connections);
    assert_eq!(
        identity, *positive,
        "Connection supply order is not identity"
    );
}

fn assert_packet_container_order_is_not_identity(fixture: &Fixture, positive: &ViewObservation) {
    let mut reversed = positive.clone();
    reversed.packets.reverse();
    validate_view(fixture, &reversed)
        .expect("packet container traversal is not an observation identity");
}

fn assert_declaration_permutation_invariance(positive: &ViewObservation) {
    let permuted = fixture(PERMUTED_SOURCE);
    let supplied = [
        permuted.connections[1],
        permuted.connections[2],
        permuted.connections[0],
    ];
    let observed = project_observation(&permuted, &permuted.mesh, &supplied);
    validate_view(&permuted, &observed)
        .expect("the permuted ordinary source retains the exact structural view");
    assert_eq!(
        observed, *positive,
        "declaration order changed canonical meaning"
    );
}

fn assert_mesh_does_not_persist_quotient(fixture: &Fixture) {
    let text = std::str::from_utf8(&fixture.mesh_json).expect("canonical JSON is UTF-8");
    for forbidden in ["quotient", "orbit", "positive_packet"] {
        assert!(
            !text.contains(forbidden),
            "collocated mesh persisted forbidden {forbidden} state"
        );
    }
    let mut forged = fixture.mesh_json.clone();
    let end = forged
        .iter()
        .rposition(|byte| *byte == b'}')
        .expect("canonical envelope is a JSON object");
    forged.splice(end..end, b",\"quotient\":{}".iter().copied());
    assert!(
        CartesianMeshEnvelopeV1::from_json(&forged, Default::default()).is_err(),
        "Cartesian mesh v1 rejects an unauthorized quotient field or version"
    );
}

fn assert_positive_events_and_inventory(fixture: &Fixture, supplied: &[Id<kinds::Connection>]) {
    let run = super::run_projection_for(
        &fixture.model,
        &fixture.mesh,
        supplied,
        super::ProjectionProfile::CollocatedUniform4x6x8,
    );
    assert!(run.result.is_ok(), "the positive receipt owns a projection");
    assert_eq!(run.events.len(), 7);
    assert!(matches!(
        run.events[0],
        super::ProjectionEvent::PairValidated(_)
    ));
    assert!(matches!(
        run.events[1],
        super::ProjectionEvent::PairValidated(_)
    ));
    assert!(matches!(
        run.events[2],
        super::ProjectionEvent::PairValidated(_)
    ));
    assert_eq!(run.events[3], super::ProjectionEvent::GroupAdmitted);
    let super::ProjectionEvent::AbstractInventoryAdmitted(inventory) = run.events[4] else {
        panic!("the accepted inventory is the fifth event");
    };
    assert_eq!(
        observe_inventory(inventory),
        InventoryObservation {
            cells: 192,
            box_entities: 1989,
            quotient_strata: [192, 576, 576, 192],
            quotient_entities: 1536,
            quotient_closure_vertex_references: 5184,
            orbit_outputs: 1536,
            box_orbit_memberships: 1989,
            positive_packets: 576,
            seam_packets: [48, 32, 24],
        }
    );
    assert_eq!(
        run.events[5],
        super::ProjectionEvent::ProjectionStateAllocated
    );
    assert_eq!(run.events[6], super::ProjectionEvent::ProjectionPublished);
}

fn observe_inventory(inventory: super::Inventory) -> InventoryObservation {
    InventoryObservation {
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

fn assert_input_falsifiers(
    fixture: &Fixture,
    supplied: &[Id<kinds::Connection>],
    positive: &ViewObservation,
) {
    let predecessor_mesh = mesh_envelope_from_axes(vec![
        vec![-2.0, 0.0, 3.0],
        vec![1.0, 2.0, 4.5, 8.0],
        vec![-1.0, 0.0, 2.0, 6.0, 10.0],
    ]);
    assert_eq!(
        hex_digest(
            predecessor_mesh
                .artifact_reference()
                .expect("the accepted predecessor mesh has an identity")
                .sha256()
        ),
        PREDECESSOR_MESH_SHA256
    );
    assert_mesh_falsifier(
        "CV4-GRID-SUBSTITUTE-234",
        fixture,
        &predecessor_mesh,
        supplied,
        positive,
    );

    let transposed_mesh = mesh_envelope_from_axes(uniform_axes(
        [8, 6, 4],
        [[-2.0, 3.0], [1.0, 8.0], [-1.0, 10.0]],
    ));
    assert_transposed_mutant_preserves_named_aggregates(&transposed_mesh);
    assert_mesh_falsifier(
        "CV4-COUNT-TRANSPOSE-864",
        fixture,
        &transposed_mesh,
        supplied,
        positive,
    );

    let mut nonuniform_axes = sealed_axes();
    nonuniform_axes[2][4] = f64::from_bits(0x4012_0000_0000_0001);
    let nonuniform_mesh = mesh_envelope_from_axes(nonuniform_axes);
    assert_mesh_falsifier(
        "CV4-NONUNIFORM-ONE-ULP",
        fixture,
        &nonuniform_mesh,
        supplied,
        positive,
    );

    let changed_upper = f64::from_bits(0x4020_0000_0000_0001);
    let changed_bounds_mesh = mesh_envelope_from_axes(uniform_axes(
        COUNTS,
        [[-2.0, 3.0], [1.0, changed_upper], [-1.0, 10.0]],
    ));
    assert_mesh_falsifier(
        "CV4-BOUNDS-ONE-ULP",
        fixture,
        &changed_bounds_mesh,
        supplied,
        positive,
    );

    assert_group_falsifier(
        "CV4-TWO-PAIR-GROUP",
        fixture,
        &fixture.connections[..2],
        supplied,
        positive,
    );
    let duplicate = [
        fixture.connections[0],
        fixture.connections[0],
        fixture.connections[2],
    ];
    assert_group_falsifier(
        "CV4-DUPLICATE-AXIS-GROUP",
        fixture,
        &duplicate,
        supplied,
        positive,
    );
}

fn uniform_axes(counts: [usize; 3], bounds: [[f64; 2]; 3]) -> Vec<Vec<f64>> {
    (0..3)
        .map(|axis| {
            let [lower, upper] = bounds[axis];
            let spacing = (upper - lower) / counts[axis] as f64;
            let mut coordinates = (0..counts[axis])
                .map(|index| lower + index as f64 * spacing)
                .collect::<Vec<_>>();
            coordinates.push(upper);
            coordinates
        })
        .collect()
}

fn assert_transposed_mutant_preserves_named_aggregates(mesh: &CartesianMeshEnvelopeV1) {
    let mesh = mesh.mesh();
    let counts: [usize; 3] =
        std::array::from_fn(|axis| mesh.axis_cell_count(axis).expect("physical axis"));
    assert_eq!(counts, [8, 6, 4]);
    assert_eq!(
        (0..3)
            .map(|axis| mesh.axis_bounds(axis).expect("physical bounds"))
            .collect::<Vec<_>>(),
        [[-2.0, 3.0], [1.0, 8.0], [-1.0, 10.0]]
    );
    let sealed = sealed_axes();
    assert_eq!(mesh.axis_coordinates(1), Some(sealed[1].as_slice()));

    let cells = counts.iter().product::<usize>();
    assert_eq!(cells, CELLS);
    assert_eq!(counts.iter().map(|count| cells / count).sum::<usize>(), 104);
    assert_eq!(
        counts.map(|count| 2 * count + 1).iter().product::<usize>(),
        1989
    );
    assert_eq!(3 * cells, PACKETS);
    assert_eq!(8 * cells, 1536);
    assert_eq!(27 * cells, 5184);
    let spacings: [f64; 3] = [5.0 / 8.0, 7.0 / 6.0, 11.0 / 4.0];
    assert_eq!(
        ((spacings[0] * spacings[1]) * spacings[2]).to_bits(),
        V_BITS
    );
}

fn assert_mesh_falsifier(
    name: &str,
    fixture: &Fixture,
    mutant_mesh: &CartesianMeshEnvelopeV1,
    supplied: &[Id<kinds::Connection>],
    positive: &ViewObservation,
) {
    assert!(
        super::project_collocated_4x6x8(&fixture.model, mutant_mesh, supplied).is_err(),
        "{name} must be rejected by the real entry point"
    );
    let run = super::run_projection_for(
        &fixture.model,
        mutant_mesh,
        supplied,
        super::ProjectionProfile::CollocatedUniform4x6x8,
    );
    assert!(run.result.is_err(), "{name} receipt must retain rejection");
    assert_eq!(run.events.len(), 4, "{name} rejects at mesh admission");
    assert!(
        run.events[..3]
            .iter()
            .all(|event| matches!(event, super::ProjectionEvent::PairValidated(_)))
    );
    assert_eq!(run.events[3], super::ProjectionEvent::GroupAdmitted);
    assert_no_allocation_or_publication(name, &run.events);
    assert_revert_reproduces_positive(name, fixture, supplied, positive);
}

fn assert_group_falsifier(
    name: &str,
    fixture: &Fixture,
    mutant_connections: &[Id<kinds::Connection>],
    supplied: &[Id<kinds::Connection>],
    positive: &ViewObservation,
) {
    assert!(
        super::project_collocated_4x6x8(&fixture.model, &fixture.mesh, mutant_connections).is_err(),
        "{name} must be rejected by the real entry point"
    );
    let run = super::run_projection_for(
        &fixture.model,
        &fixture.mesh,
        mutant_connections,
        super::ProjectionProfile::CollocatedUniform4x6x8,
    );
    assert!(run.result.is_err(), "{name} receipt must retain rejection");
    assert!(
        run.events
            .iter()
            .all(|event| matches!(event, super::ProjectionEvent::PairValidated(_)))
    );
    assert_no_allocation_or_publication(name, &run.events);
    assert_revert_reproduces_positive(name, fixture, supplied, positive);
}

fn assert_no_allocation_or_publication(name: &str, events: &[super::ProjectionEvent]) {
    assert!(
        events.iter().all(|event| !matches!(
            event,
            super::ProjectionEvent::ProjectionStateAllocated
                | super::ProjectionEvent::ProjectionPublished
        )),
        "{name} must fail before allocation or publication"
    );
}

fn assert_revert_reproduces_positive(
    name: &str,
    fixture: &Fixture,
    supplied: &[Id<kinds::Connection>],
    positive: &ViewObservation,
) {
    let restored = project_observation(fixture, &fixture.mesh, supplied);
    assert_eq!(
        restored, *positive,
        "reverting {name}'s one changed variable must reproduce the positive"
    );
}

fn assert_observation_mutants(fixture: &Fixture, positive: &ViewObservation) {
    let mut exercised = BTreeSet::new();
    for mutant in ObservationMutant::ALL {
        let mut observation = positive.clone();
        apply_observation_mutant(mutant, &mut observation);
        assert_ne!(
            observation,
            *positive,
            "{} did not alter the accepted observation",
            mutant.name()
        );
        assert_eq!(
            validate_view(fixture, &observation),
            Err(mutant.expected_fault()),
            "{} must fail at its named boundary",
            mutant.name()
        );
        assert!(exercised.insert(mutant.name()));
    }
    assert_eq!(exercised.len(), ObservationMutant::ALL.len());
}

fn apply_observation_mutant(mutant: ObservationMutant, observation: &mut ViewObservation) {
    match mutant {
        ObservationMutant::PacketDrop => {
            observation.packets.pop().expect("positive packet");
        }
        ObservationMutant::SeamFlag => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| !packet.seam)
                .expect("interior packet");
            packet.seam = true;
        }
        ObservationMutant::TangentialShift => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 0 && packet.seam)
                .expect("axis-0 seam packet");
            packet.neighbor_cell += 8;
        }
        ObservationMutant::SeamDouble => {
            let mut packet = observation
                .packets
                .iter()
                .find(|packet| packet.seam)
                .expect("seam packet")
                .clone();
            std::mem::swap(&mut packet.owner_cell, &mut packet.neighbor_cell);
            packet.normal = packet.normal.map(|value| -value);
            observation.packets.push(packet);
        }
        ObservationMutant::AreaSwap => {
            let axis_0 = observation
                .packets
                .iter()
                .position(|packet| packet.axis == 0)
                .expect("axis-0 packet");
            let axis_2 = observation
                .packets
                .iter()
                .position(|packet| packet.axis == 2)
                .expect("axis-2 packet");
            let saved = observation.packets[axis_0].face_area_bits;
            observation.packets[axis_0].face_area_bits = observation.packets[axis_2].face_area_bits;
            observation.packets[axis_2].face_area_bits = saved;
        }
        ObservationMutant::DistanceUlp => {
            observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 1)
                .expect("axis-1 packet")
                .lifted_center_distance_bits = 0x3FF2_AAAA_AAAA_AAAC;
        }
        ObservationMutant::NormalSign => {
            let packet = observation.packets.first_mut().expect("positive packet");
            packet.normal = packet.normal.map(|value| -value);
        }
        ObservationMutant::FaceFamily => {
            observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 2)
                .expect("axis-2 packet")
                .quotient_face += CELLS;
        }
        ObservationMutant::ConnectionOrder => observation.connections.swap(0, 2),
        ObservationMutant::MeshIdentity => {
            observation.mesh_artifact_sha256 = parse_hex_digest(PREDECESSOR_MESH_SHA256);
        }
        ObservationMutant::ModelRevision => observation.semantic_revision += 1,
        ObservationMutant::ParentConnectorSwap => {
            std::mem::swap(&mut observation.parent, &mut observation.connector);
        }
        ObservationMutant::PacketIdRenumber => {
            let packet = observation.packets.first_mut().expect("positive packet");
            packet.packet = packet.axis * CELLS + packet.neighbor_cell;
        }
    }
}

fn hex_digest(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex_digest(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    std::array::from_fn(|index| {
        u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).expect("sealed digest is hex")
    })
}
