//! Independent structural replay for the exact RFC 0071 `2 x 3 x 4` witness.
//!
//! This module was sealed before the product module existed. It deliberately
//! derives the expected quotient from replayed Model boundary identities and
//! mesh axes. No producer orbit, closure, incidence, or packet table is an
//! oracle input.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use eqiora_artifact::{
    AcceptedModelArtifact, CartesianMeshEnvelopeV1, ModelDecoderLimits, ModelTransactionEnvelope,
    ReplayableCanonicalModelArtifact,
};
use eqiora_compiler::compile;
use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology};
use eqiora_schema::kernel::{BoundarySide, ConnectionSemantics, DomainKind, KernelNode};
use eqiora_sem::{BoundaryJunctionGeometry, KernelProgram};

const SOURCE: &str =
    include_str!("../../../../verify/fluid/cartesian-periodic-topology-3d/models/periodic-box.eqi");
const PERMUTED_SOURCE: &str = include_str!(
    "../../../../verify/fluid/cartesian-periodic-topology-3d/models/periodic-box-permuted.eqi"
);

const AXES: [&[f64]; 3] = [
    &[-2.0, 0.0, 3.0],
    &[1.0, 2.0, 4.5, 8.0],
    &[-1.0, 0.0, 2.0, 6.0, 10.0],
];
const COUNTS: [usize; 3] = [2, 3, 4];

/// Test-only observation seam. The implementation writer may implement this
/// trait for `super::EvidenceHarness`, but may not edit the trait, observation
/// vocabulary, replay, expected values, mutant construction, or acceptance
/// logic below.
pub(super) trait ProductHarness {
    fn project(
        model: &AcceptedModelArtifact,
        mesh: &CartesianMeshEnvelopeV1,
        selected_connections: &[Id<kinds::Connection>],
    ) -> ProductRun;
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProductRun {
    pub result: Result<ProductProjectionObservation, String>,
    pub events: Vec<ProductEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProductEvent {
    PairValidated(RawId),
    GroupAdmitted,
    AbstractInventoryAdmitted(InventoryObservation),
    ProjectionStateAllocated,
    ProjectionPublished,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProductProjectionObservation {
    pub model_artifact_sha256: String,
    pub model_id: String,
    pub semantic_revision: u64,
    pub mesh_artifact_sha256: [u8; 32],
    pub parent: RawId,
    pub connector: RawId,
    pub generators: Vec<GeneratorObservation>,
    pub cycles: Vec<CycleObservation>,
    pub inventory: InventoryObservation,
    pub entities: Vec<EntityObservation>,
    pub face_incidences: Vec<FaceIncidenceObservation>,
    pub cell_face_incidences: Vec<CellFaceIncidenceObservation>,
    pub packets: Vec<PacketObservation>,
    pub exterior_face_count: usize,
    pub persisted_quotient: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GeneratorAuthority {
    SpatialPeriodicPair,
    OrdinaryConservingUnion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TranslationAuthority {
    ParentBounds,
    StoredVector,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GeneratorObservation {
    pub connection: RawId,
    pub lower_port: RawId,
    pub upper_port: RawId,
    pub parent: RawId,
    pub connector: RawId,
    pub axis: usize,
    pub lower_coordinate: f64,
    pub upper_coordinate: f64,
    pub period: f64,
    pub authority: GeneratorAuthority,
    pub translation_authority: TranslationAuthority,
    pub identity_fiber: bool,
    pub lower_outward_sign: i8,
    pub upper_outward_sign: i8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CycleObservation {
    pub word: Vec<i8>,
    pub net_coefficients: [i8; 3],
    pub identity_fiber: bool,
    pub anchor_commutes: bool,
    pub incidence_commutes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InventoryObservation {
    pub cells: usize,
    pub box_entities: usize,
    pub quotient_strata: [usize; 4],
    pub quotient_entities: usize,
    pub quotient_closure_vertex_references: usize,
    pub orbit_outputs: usize,
    pub box_orbit_memberships: usize,
    pub positive_packets: usize,
    pub seam_packets: [usize; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BoxRepresentativeObservation {
    pub anchors: [usize; 3],
    pub base_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EntityObservation {
    pub dimension: usize,
    pub free_axes: Vec<usize>,
    pub quotient_anchor: [usize; 3],
    pub quotient_index: usize,
    pub orbit: Vec<BoxRepresentativeObservation>,
    pub closure_vertices: Vec<usize>,
    pub orientation_code: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct FaceIncidenceObservation {
    pub quotient_face: usize,
    pub positive_side_cell: usize,
    pub negative_side_cell: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CellFaceIncidenceObservation {
    pub cell: usize,
    pub axis: usize,
    pub side: i8,
    pub quotient_face: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PacketObservation {
    pub packet: usize,
    pub axis: usize,
    pub owner_cell: usize,
    pub neighbor_cell: usize,
    pub quotient_face: usize,
    pub normal: [i8; 3],
    pub scatter_signs: [i8; 2],
    pub seam: bool,
    pub lifted_center_distance: Option<f64>,
    pub owner_face_points: Vec<[f64; 3]>,
    pub lifted_neighbor_face_points: Vec<[f64; 3]>,
}

#[derive(Debug)]
struct Fixture {
    model: AcceptedModelArtifact,
    model_json: Vec<u8>,
    transaction_json: Vec<u8>,
    program: KernelProgram,
    mesh: CartesianMeshEnvelopeV1,
    mesh_json: Vec<u8>,
    connections: Vec<Id<kinds::Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationFault {
    DistinctConnections,
    ExactAxisInventory,
    CommonParent,
    CommonConnector,
    EndpointOrientation,
    TranslationAuthority,
    FiberCycles,
    CompleteCycleReceipts,
    SpatialPeriodicAuthority,
    CompleteQuotient,
    CornerCycleAgreement,
    TangentialGeometry,
    IdentityOrientation,
    OutwardSigns,
    SingletonCells,
    PacketBijection,
    ClosedFaceIncidence,
    LiftedSeamDistance,
    CanonicalLastAxisFastestOrder,
    NonPersistedProjection,
    AdmissionBeforeAllocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MutantId {
    OnePairReused,
    MissingAxis,
    DuplicateAxis,
    CrossParent,
    CrossConnector,
    EndpointOrder,
    StoredVector,
    NoncommutingFiber,
    PairwiseOnly,
    OrdinaryUnion,
    FaceOnlyQuotient,
    CornerOrder,
    TangentialShift,
    OrientationPermute,
    OutwardSameSign,
    CellCollapse,
    SeamDouble,
    SeamExterior,
    LongSeam,
    XFastest,
    PersistedQuotient,
    AllocateFirst,
}

impl MutantId {
    const ALL: [Self; 22] = [
        Self::OnePairReused,
        Self::MissingAxis,
        Self::DuplicateAxis,
        Self::CrossParent,
        Self::CrossConnector,
        Self::EndpointOrder,
        Self::StoredVector,
        Self::NoncommutingFiber,
        Self::PairwiseOnly,
        Self::OrdinaryUnion,
        Self::FaceOnlyQuotient,
        Self::CornerOrder,
        Self::TangentialShift,
        Self::OrientationPermute,
        Self::OutwardSameSign,
        Self::CellCollapse,
        Self::SeamDouble,
        Self::SeamExterior,
        Self::LongSeam,
        Self::XFastest,
        Self::PersistedQuotient,
        Self::AllocateFirst,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::OnePairReused => "P3D-ONE-PAIR-REUSED",
            Self::MissingAxis => "P3D-MISSING-AXIS",
            Self::DuplicateAxis => "P3D-DUPLICATE-AXIS",
            Self::CrossParent => "P3D-CROSS-PARENT",
            Self::CrossConnector => "P3D-CROSS-CONNECTOR",
            Self::EndpointOrder => "P3D-ENDPOINT-ORDER",
            Self::StoredVector => "P3D-STORED-VECTOR",
            Self::NoncommutingFiber => "P3D-NONCOMMUTING-FIBER",
            Self::PairwiseOnly => "P3D-PAIRWISE-ONLY",
            Self::OrdinaryUnion => "P3D-ORDINARY-UNION",
            Self::FaceOnlyQuotient => "P3D-FACE-ONLY-QUOTIENT",
            Self::CornerOrder => "P3D-CORNER-ORDER",
            Self::TangentialShift => "P3D-TANGENTIAL-SHIFT",
            Self::OrientationPermute => "P3D-ORIENTATION-PERMUTE",
            Self::OutwardSameSign => "P3D-OUTWARD-SAME-SIGN",
            Self::CellCollapse => "P3D-CELL-COLLAPSE",
            Self::SeamDouble => "P3D-SEAM-DOUBLE",
            Self::SeamExterior => "P3D-SEAM-EXTERIOR",
            Self::LongSeam => "P3D-LONG-SEAM",
            Self::XFastest => "P3D-X-FASTEST",
            Self::PersistedQuotient => "P3D-PERSISTED-QUOTIENT",
            Self::AllocateFirst => "P3D-ALLOCATE-FIRST",
        }
    }

    const fn expected_fault(self) -> ValidationFault {
        match self {
            Self::OnePairReused => ValidationFault::DistinctConnections,
            Self::MissingAxis | Self::DuplicateAxis => ValidationFault::ExactAxisInventory,
            Self::CrossParent => ValidationFault::CommonParent,
            Self::CrossConnector => ValidationFault::CommonConnector,
            Self::EndpointOrder => ValidationFault::EndpointOrientation,
            Self::StoredVector => ValidationFault::TranslationAuthority,
            Self::NoncommutingFiber => ValidationFault::FiberCycles,
            Self::PairwiseOnly => ValidationFault::CompleteCycleReceipts,
            Self::OrdinaryUnion => ValidationFault::SpatialPeriodicAuthority,
            Self::FaceOnlyQuotient => ValidationFault::CompleteQuotient,
            Self::CornerOrder => ValidationFault::CornerCycleAgreement,
            Self::TangentialShift => ValidationFault::TangentialGeometry,
            Self::OrientationPermute => ValidationFault::IdentityOrientation,
            Self::OutwardSameSign => ValidationFault::OutwardSigns,
            Self::CellCollapse => ValidationFault::SingletonCells,
            Self::SeamDouble => ValidationFault::PacketBijection,
            Self::SeamExterior => ValidationFault::ClosedFaceIncidence,
            Self::LongSeam => ValidationFault::LiftedSeamDistance,
            Self::XFastest => ValidationFault::CanonicalLastAxisFastestOrder,
            Self::PersistedQuotient => ValidationFault::NonPersistedProjection,
            Self::AllocateFirst => ValidationFault::AdmissionBeforeAllocation,
        }
    }
}

#[test]
fn registered_reference_slice_rejects_rfc0071_mutants() {
    // Negatives do not count until this complete ordinary source-to-artifact
    // path and independent full replay have succeeded.
    let fixture = fixture(SOURCE);
    let expected = replay_expected(&fixture);
    let selected = [
        fixture.connections[2],
        fixture.connections[0],
        fixture.connections[1],
    ];
    let positive = <super::EvidenceHarness as ProductHarness>::project(
        &fixture.model,
        &fixture.mesh,
        &selected,
    );
    let observation = positive
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("ordinary positive projection failed: {error}"));
    validate_projection(&expected, observation, &positive.events)
        .expect("independent structural replay accepts the ordinary positive");
    assert_traversal_order_is_not_identity(&expected, &positive);

    assert_permutation_preserves_meaning(&fixture);
    assert_current_artifacts_do_not_persist_quotient(&fixture);
    assert_input_rejections_are_nonvacuous(&fixture);

    let mut exercised = BTreeSet::new();
    for mutant in MutantId::ALL {
        let mut run = positive.clone();
        apply_mutant(mutant, &mut run);
        assert_ne!(
            run,
            positive,
            "{} did not alter the positive",
            mutant.name()
        );
        let candidate = run.result.as_ref().expect("mutant retains a replay target");
        assert_eq!(
            validate_projection(&expected, candidate, &run.events),
            Err(mutant.expected_fault()),
            "{} must fail at its named boundary",
            mutant.name(),
        );
        assert!(exercised.insert(mutant.name()));
    }
    assert_eq!(exercised.len(), MutantId::ALL.len());
}

fn assert_traversal_order_is_not_identity(
    expected: &ProductProjectionObservation,
    positive: &ProductRun,
) {
    let mut reordered = positive.clone();
    let observation = reordered.result.as_mut().expect("positive observation");
    observation.generators.reverse();
    observation.cycles.reverse();
    observation.entities.reverse();
    for entity in &mut observation.entities {
        entity.orbit.reverse();
    }
    observation.face_incidences.reverse();
    observation.cell_face_incidences.reverse();
    observation.packets.reverse();
    validate_projection(expected, observation, &reordered.events)
        .expect("traversal and allocation container order are not identities");
}

fn fixture(source: &str) -> Fixture {
    let mut compiled = compile("periodic-box.eqi", source).expect("ordinary .eqi source compiles");
    assert_eq!(compiled.len(), 1, "the evidence source owns one Model");
    let (transaction, model, _) = compiled.remove(0).into_parts();

    let transaction_artifact = ModelTransactionEnvelope::from_transaction(&transaction)
        .expect("the current Transaction owner encodes compiler output");
    let transaction_json = transaction_artifact
        .canonical_json()
        .expect("canonical Transaction bytes");
    let transaction_replay =
        ModelTransactionEnvelope::from_json(&transaction_json, ModelDecoderLimits::default())
            .expect("the current Transaction owner decodes its bytes")
            .to_transaction()
            .expect("the decoded Transaction reconstructs");
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction_replay)
        .expect("the replayed compiler transaction commits");
    let compiled_program =
        KernelProgram::from_snapshot(&store.snapshot(), model).expect("whole Model validates");

    let encoded_model = AcceptedModelArtifact::from_program(&compiled_program)
        .expect("the current Model owner encodes the validated program");
    let model_json = encoded_model
        .canonical_json()
        .expect("canonical Model bytes");
    let model = AcceptedModelArtifact::from_json(&model_json, ModelDecoderLimits::default())
        .expect("the current Model owner decodes its bytes");
    let replayed = model
        .replay_model()
        .expect("the exact Model artifact replays through whole-Model validation");
    assert_eq!(replayed.program(), &compiled_program);
    let program = replayed.program().clone();

    let mesh = CartesianMesh::from_axes(AXES.iter().map(|axis| axis.to_vec()).collect())
        .expect("the evidence-owned nonuniform Cartesian axes are valid");
    let encoded_mesh = CartesianMeshEnvelopeV1::from_mesh(&mesh)
        .expect("the current Cartesian artifact owner captures the mesh");
    let mesh_json = encoded_mesh
        .canonical_json()
        .expect("canonical Cartesian mesh bytes");
    let mesh = CartesianMeshEnvelopeV1::from_json(&mesh_json, Default::default())
        .expect("the current Cartesian artifact owner replays the mesh");

    let mut connections = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Connection(connection)
                if connection.semantics() == ConnectionSemantics::SpatialPeriodic =>
            {
                Some(connection.id())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(connections.len(), 3, "the source owns exactly three pairs");
    connections.sort_by_key(|connection| derive_generator(&program, *connection).axis);

    Fixture {
        model,
        model_json,
        transaction_json,
        program,
        mesh,
        mesh_json,
        connections,
    }
}

fn replay_expected(fixture: &Fixture) -> ProductProjectionObservation {
    let model_replay = fixture.model.replay_model().expect("exact Model replay");
    assert_eq!(model_replay.program(), &fixture.program);
    let model_reference = model_replay.artifact_reference();
    let mesh_reference = fixture
        .mesh
        .artifact_reference()
        .expect("exact Cartesian mesh reference");
    let mesh = fixture.mesh.mesh();

    assert_eq!(mesh.topological_dimension(), 3);
    for axis in 0..3 {
        assert_eq!(mesh.axis_coordinates(axis), Some(AXES[axis]));
        assert_eq!(mesh.axis_cell_count(axis), Some(COUNTS[axis]));
    }
    let side_lengths = AXES.map(|axis| axis[axis.len() - 1] - axis[0]);
    assert!(side_lengths[0] != side_lengths[1]);
    assert!(side_lengths[0] != side_lengths[2]);
    assert!(side_lengths[1] != side_lengths[2]);
    assert!(AXES.iter().any(|axis| {
        let widths = axis
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .collect::<Vec<_>>();
        widths.windows(2).any(|pair| pair[0] != pair[1])
    }));

    let mut generators = fixture
        .connections
        .iter()
        .copied()
        .map(|connection| derive_generator(&fixture.program, connection))
        .collect::<Vec<_>>();
    generators.sort_by_key(|generator| generator.axis);
    let parent = generators[0].parent;
    let connector = generators[0].connector;
    assert!(
        generators
            .iter()
            .all(|generator| generator.parent == parent)
    );
    assert!(
        generators
            .iter()
            .all(|generator| generator.connector == connector)
    );
    assert_eq!(
        generators
            .iter()
            .map(|generator| generator.axis)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );

    for axis in 0..3 {
        let parent_bounds = fixture
            .program
            .resolved_cartesian_bounds(parent.downcast().expect("typed parent"))
            .expect("parent bounds");
        assert_eq!(
            generators[axis].lower_coordinate,
            parent_bounds[axis].lower().value()
        );
        assert_eq!(
            generators[axis].upper_coordinate,
            parent_bounds[axis].upper().value()
        );
        assert_eq!(
            mesh.axis_bounds(axis),
            Some([
                generators[axis].lower_coordinate,
                generators[axis].upper_coordinate
            ])
        );
    }

    let (entities, quotient_closure_vertex_references, box_orbit_memberships) =
        replay_entities(mesh);
    let face_incidences = replay_face_incidences();
    let cell_face_incidences = replay_cell_face_incidences();
    assert_connected(&face_incidences);
    let packets = replay_packets();
    let inventory = InventoryObservation {
        cells: checked_product(&COUNTS),
        box_entities: checked_product(&COUNTS.map(|count| 2 * count + 1)),
        quotient_strata: [24, 72, 72, 24],
        quotient_entities: entities.len(),
        quotient_closure_vertex_references,
        orbit_outputs: entities.len(),
        box_orbit_memberships,
        positive_packets: packets.len(),
        seam_packets: [12, 8, 6],
    };
    assert_eq!(inventory.cells, 24);
    assert_eq!(inventory.box_entities, 315);
    assert_eq!(inventory.quotient_entities, 192);
    assert_eq!(inventory.quotient_closure_vertex_references, 648);
    assert_eq!(inventory.box_orbit_memberships, 315);
    assert_eq!(inventory.positive_packets, 72);

    ProductProjectionObservation {
        model_artifact_sha256: model_reference.artifact().as_str().to_owned(),
        model_id: model_reference.model().to_string(),
        semantic_revision: model_reference.semantic_revision().get(),
        mesh_artifact_sha256: mesh_reference.sha256(),
        parent,
        connector,
        generators,
        cycles: expected_cycles(),
        inventory,
        entities,
        face_incidences,
        cell_face_incidences,
        packets,
        exterior_face_count: 0,
        persisted_quotient: false,
    }
}

fn derive_generator(
    program: &KernelProgram,
    connection: Id<kinds::Connection>,
) -> GeneratorObservation {
    let junction = program
        .compose_boundary_physical_junction(connection)
        .expect("each constituent pair passes the existing semantic validator first");
    let BoundaryJunctionGeometry::CartesianPeriodic(identification) = junction.geometry() else {
        panic!("selected Connection must retain spatial-periodic meaning");
    };
    assert_eq!(identification.ambient_dimension(), 3);

    let mut ports = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection.erase())
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    assert_eq!(ports.len(), 2);
    let mut resolved = ports
        .drain(..)
        .map(|port| {
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
            let parents = program
                .edges()
                .iter()
                .filter(|edge| {
                    edge.kind() == EdgeKind::BoundaryOf && edge.from() == boundary.erase()
                })
                .map(|edge| edge.to())
                .collect::<Vec<_>>();
            assert_eq!(parents.len(), 1);
            (port, connector.erase(), parents[0], *axis, *side)
        })
        .collect::<Vec<_>>();
    resolved.sort_by_key(|entry| match entry.4 {
        BoundarySide::Lower => 0,
        BoundarySide::Upper => 1,
    });
    let lower = resolved[0];
    let upper = resolved[1];
    assert_eq!(lower.4, BoundarySide::Lower);
    assert_eq!(upper.4, BoundarySide::Upper);
    assert_eq!(lower.1, upper.1);
    assert_eq!(lower.2, upper.2);
    assert_eq!(lower.3, upper.3);

    GeneratorObservation {
        connection: connection.erase(),
        lower_port: lower.0,
        upper_port: upper.0,
        parent: lower.2,
        connector: lower.1,
        axis: identification.normal_axis(),
        lower_coordinate: identification.lower_coordinate(),
        upper_coordinate: identification.upper_coordinate(),
        period: identification.period(),
        authority: GeneratorAuthority::SpatialPeriodicPair,
        translation_authority: TranslationAuthority::ParentBounds,
        identity_fiber: true,
        lower_outward_sign: -1,
        upper_outward_sign: 1,
    }
}

fn replay_entities(mesh: &CartesianMesh) -> (Vec<EntityObservation>, usize, usize) {
    let mut entities = Vec::new();
    let mut closure_references = 0;
    let mut orbit_memberships = 0;
    for dimension in 0..=3 {
        for free_axes in combinations(dimension) {
            let family_rank = combinations(dimension)
                .iter()
                .position(|candidate| candidate == &free_axes)
                .expect("free-axis family rank");
            for linear_anchor in 0..checked_product(&COUNTS) {
                let quotient_anchor = delinearize(linear_anchor, COUNTS);
                let quotient_index = family_rank * checked_product(&COUNTS) + linear_anchor;
                let boundary_axes = (0..3)
                    .filter(|axis| !free_axes.contains(axis) && quotient_anchor[*axis] == 0)
                    .collect::<Vec<_>>();
                let mut orbit = Vec::new();
                for subset in 0..(1_usize << boundary_axes.len()) {
                    let mut anchors = quotient_anchor;
                    for (ordinal, &axis) in boundary_axes.iter().enumerate() {
                        if (subset >> ordinal) & 1 == 1 {
                            anchors[axis] = COUNTS[axis];
                        }
                    }
                    let base_index = base_entity_index(dimension, &free_axes, anchors);
                    assert_box_entity(mesh, dimension, &free_axes, anchors, base_index);
                    orbit.push(BoxRepresentativeObservation {
                        anchors,
                        base_index,
                    });
                }
                let closure_vertices = quotient_closure(&free_axes, quotient_anchor);
                closure_references += closure_vertices.len();
                orbit_memberships += orbit.len();
                entities.push(EntityObservation {
                    dimension,
                    free_axes: free_axes.clone(),
                    quotient_anchor,
                    quotient_index,
                    orbit,
                    closure_vertices,
                    orientation_code: 0,
                });
            }
        }
    }
    (entities, closure_references, orbit_memberships)
}

fn combinations(dimension: usize) -> Vec<Vec<usize>> {
    (0_u8..8)
        .filter(|mask| mask.count_ones() as usize == dimension)
        .map(|mask| (0..3).filter(|axis| mask & (1 << axis) != 0).collect())
        .collect()
}

fn delinearize(mut linear: usize, shape: [usize; 3]) -> [usize; 3] {
    let mut indices = [0; 3];
    for axis in (0..3).rev() {
        indices[axis] = linear % shape[axis];
        linear /= shape[axis];
    }
    indices
}

fn flat(indices: [usize; 3]) -> usize {
    (indices[0] * COUNTS[1] + indices[1]) * COUNTS[2] + indices[2]
}

fn checked_product(values: &[usize]) -> usize {
    values
        .iter()
        .try_fold(1_usize, |product, value| product.checked_mul(*value))
        .expect("bounded evidence inventory is representable")
}

fn base_entity_index(dimension: usize, free_axes: &[usize], anchors: [usize; 3]) -> usize {
    let families = combinations(dimension);
    let mut offset = 0;
    for family in families {
        let shape = [0, 1, 2].map(|axis| {
            if family.contains(&axis) {
                COUNTS[axis]
            } else {
                COUNTS[axis] + 1
            }
        });
        if family == free_axes {
            return offset + (anchors[0] * shape[1] + anchors[1]) * shape[2] + anchors[2];
        }
        offset += checked_product(&shape);
    }
    panic!("free-axis family must exist");
}

fn quotient_closure(free_axes: &[usize], anchor: [usize; 3]) -> Vec<usize> {
    (0..(1_usize << free_axes.len()))
        .map(|bits| {
            let mut vertex = anchor;
            for (ordinal, &axis) in free_axes.iter().enumerate() {
                vertex[axis] = (vertex[axis] + ((bits >> ordinal) & 1)) % COUNTS[axis];
            }
            flat(vertex)
        })
        .collect()
}

fn assert_box_entity(
    mesh: &CartesianMesh,
    dimension: usize,
    free_axes: &[usize],
    anchors: [usize; 3],
    base_index: usize,
) {
    let entity = MeshEntity::new(dimension, base_index);
    assert_eq!(mesh.entity_free_axes(entity), Some(free_axes));
    let vertices = mesh
        .entity_vertices(entity)
        .expect("RFC-derived box entity exists");
    assert_eq!(vertices.len(), 1 << dimension);
    for (bits, vertex) in vertices.into_iter().enumerate() {
        let mut expected = anchors;
        for (ordinal, &axis) in free_axes.iter().enumerate() {
            expected[axis] += (bits >> ordinal) & 1;
        }
        assert_eq!(mesh.vertex_multi_index(vertex), Some(expected.as_slice()));
    }
}

fn expected_cycles() -> Vec<CycleObservation> {
    let mut cycles = Vec::new();
    for (first, second) in [(0_i8, 1_i8), (0, 2), (1, 2)] {
        cycles.push(CycleObservation {
            word: vec![first + 1, second + 1, -(first + 1), -(second + 1)],
            net_coefficients: [0, 0, 0],
            identity_fiber: true,
            anchor_commutes: true,
            incidence_commutes: true,
        });
    }
    for order in [
        [0_i8, 1_i8, 2_i8],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        cycles.push(CycleObservation {
            word: order.into_iter().map(|axis| axis + 1).collect(),
            net_coefficients: [1, 1, 1],
            identity_fiber: true,
            anchor_commutes: true,
            incidence_commutes: true,
        });
    }
    cycles
}

fn replay_face_incidences() -> Vec<FaceIncidenceObservation> {
    let cells = checked_product(&COUNTS);
    let mut incidences = Vec::with_capacity(3 * cells);
    for normal_axis in 0..3 {
        let free_axes = (0..3)
            .filter(|axis| *axis != normal_axis)
            .collect::<Vec<_>>();
        let family_rank = combinations(2)
            .iter()
            .position(|candidate| candidate == &free_axes)
            .expect("face family rank");
        for linear_anchor in 0..cells {
            let anchor = delinearize(linear_anchor, COUNTS);
            let mut positive_cell = anchor;
            positive_cell[normal_axis] =
                (anchor[normal_axis] + COUNTS[normal_axis] - 1) % COUNTS[normal_axis];
            let negative_cell = anchor;
            incidences.push(FaceIncidenceObservation {
                quotient_face: family_rank * cells + linear_anchor,
                positive_side_cell: flat(positive_cell),
                negative_side_cell: flat(negative_cell),
            });
        }
    }
    incidences.sort_by_key(|incidence| incidence.quotient_face);
    incidences
}

fn replay_cell_face_incidences() -> Vec<CellFaceIncidenceObservation> {
    let cells = checked_product(&COUNTS);
    let mut incidences = Vec::with_capacity(6 * cells);
    for cell in 0..cells {
        let indices = delinearize(cell, COUNTS);
        for axis in 0..3 {
            let free_axes = (0..3)
                .filter(|candidate| *candidate != axis)
                .collect::<Vec<_>>();
            let family_rank = combinations(2)
                .iter()
                .position(|candidate| candidate == &free_axes)
                .expect("face family rank");
            let negative_face = family_rank * cells + flat(indices);
            let mut positive_anchor = indices;
            positive_anchor[axis] = (positive_anchor[axis] + 1) % COUNTS[axis];
            let positive_face = family_rank * cells + flat(positive_anchor);
            incidences.push(CellFaceIncidenceObservation {
                cell,
                axis,
                side: -1,
                quotient_face: negative_face,
            });
            incidences.push(CellFaceIncidenceObservation {
                cell,
                axis,
                side: 1,
                quotient_face: positive_face,
            });
        }
    }
    incidences
}

fn assert_connected(incidences: &[FaceIncidenceObservation]) {
    let cells = checked_product(&COUNTS);
    let mut adjacency = vec![Vec::new(); cells];
    for incidence in incidences {
        assert_ne!(incidence.positive_side_cell, incidence.negative_side_cell);
        adjacency[incidence.positive_side_cell].push(incidence.negative_side_cell);
        adjacency[incidence.negative_side_cell].push(incidence.positive_side_cell);
    }
    let mut visited = vec![false; cells];
    let mut queue = VecDeque::from([0]);
    visited[0] = true;
    while let Some(cell) = queue.pop_front() {
        for &neighbor in &adjacency[cell] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    assert!(visited.into_iter().all(|seen| seen));
}

fn replay_packets() -> Vec<PacketObservation> {
    let cells = checked_product(&COUNTS);
    let mut packets = Vec::with_capacity(3 * cells);
    for axis in 0..3 {
        let free_axes = (0..3)
            .filter(|candidate| *candidate != axis)
            .collect::<Vec<_>>();
        let family_rank = combinations(2)
            .iter()
            .position(|candidate| candidate == &free_axes)
            .expect("face family rank");
        for owner_cell in 0..cells {
            let owner = delinearize(owner_cell, COUNTS);
            let mut neighbor = owner;
            neighbor[axis] = (neighbor[axis] + 1) % COUNTS[axis];
            let seam = owner[axis] + 1 == COUNTS[axis];
            let mut face_anchor = owner;
            face_anchor[axis] = (owner[axis] + 1) % COUNTS[axis];
            let (lifted_center_distance, owner_face_points, lifted_neighbor_face_points) = if seam {
                let coordinates = AXES[axis];
                let first = coordinates[1] - coordinates[0];
                let last = coordinates[coordinates.len() - 1] - coordinates[coordinates.len() - 2];
                let owner_points = seam_face_points(axis, owner, BoundarySide::Upper, false);
                let neighbor_points = seam_face_points(axis, neighbor, BoundarySide::Lower, true);
                assert_eq!(owner_points, neighbor_points);
                (Some((last + first) / 2.0), owner_points, neighbor_points)
            } else {
                (None, Vec::new(), Vec::new())
            };
            let mut normal = [0; 3];
            normal[axis] = 1;
            packets.push(PacketObservation {
                packet: axis * cells + owner_cell,
                axis,
                owner_cell,
                neighbor_cell: flat(neighbor),
                quotient_face: family_rank * cells + flat(face_anchor),
                normal,
                scatter_signs: [1, -1],
                seam,
                lifted_center_distance,
                owner_face_points,
                lifted_neighbor_face_points,
            });
        }
    }
    packets
}

fn seam_face_points(
    normal_axis: usize,
    tangential_anchor: [usize; 3],
    side: BoundarySide,
    lift_lower: bool,
) -> Vec<[f64; 3]> {
    let free_axes = (0..3)
        .filter(|axis| *axis != normal_axis)
        .collect::<Vec<_>>();
    let period = AXES[normal_axis][AXES[normal_axis].len() - 1] - AXES[normal_axis][0];
    (0..4)
        .map(|bits| {
            let mut point = [0.0; 3];
            for axis in 0..3 {
                if axis == normal_axis {
                    point[axis] = match side {
                        BoundarySide::Lower => AXES[axis][0],
                        BoundarySide::Upper => AXES[axis][AXES[axis].len() - 1],
                    };
                    if lift_lower {
                        point[axis] += period;
                    }
                } else {
                    let ordinal = free_axes
                        .iter()
                        .position(|candidate| *candidate == axis)
                        .expect("tangential axis ordinal");
                    point[axis] = AXES[axis][tangential_anchor[axis] + ((bits >> ordinal) & 1)];
                }
            }
            point
        })
        .collect()
}

fn validate_projection(
    expected: &ProductProjectionObservation,
    actual: &ProductProjectionObservation,
    events: &[ProductEvent],
) -> Result<(), ValidationFault> {
    validate_event_order(expected, events)?;

    if actual.generators.len() != 3 {
        return Err(ValidationFault::ExactAxisInventory);
    }
    let connections = actual
        .generators
        .iter()
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>();
    if connections.len() != 3 {
        return Err(ValidationFault::DistinctConnections);
    }
    let axes = actual
        .generators
        .iter()
        .map(|generator| generator.axis)
        .collect::<BTreeSet<_>>();
    if axes != BTreeSet::from([0, 1, 2]) {
        return Err(ValidationFault::ExactAxisInventory);
    }
    if actual.parent != expected.parent
        || actual
            .generators
            .iter()
            .any(|generator| generator.parent != actual.parent)
    {
        return Err(ValidationFault::CommonParent);
    }
    if actual.connector != expected.connector
        || actual
            .generators
            .iter()
            .any(|generator| generator.connector != actual.connector)
    {
        return Err(ValidationFault::CommonConnector);
    }
    if actual
        .generators
        .iter()
        .any(|generator| generator.authority != GeneratorAuthority::SpatialPeriodicPair)
    {
        return Err(ValidationFault::SpatialPeriodicAuthority);
    }
    let generators_by_axis = actual
        .generators
        .iter()
        .map(|generator| (generator.axis, generator))
        .collect::<BTreeMap<_, _>>();
    for reference in &expected.generators {
        let generator = generators_by_axis
            .get(&reference.axis)
            .expect("exact axis inventory was checked");
        if generator.connection != reference.connection
            || generator.lower_port != reference.lower_port
            || generator.upper_port != reference.upper_port
            || generator.axis != reference.axis
            || generator.lower_coordinate != reference.lower_coordinate
            || generator.upper_coordinate != reference.upper_coordinate
            || generator.period != reference.period
            || generator.period <= 0.0
        {
            return Err(ValidationFault::EndpointOrientation);
        }
        if generator.translation_authority != TranslationAuthority::ParentBounds {
            return Err(ValidationFault::TranslationAuthority);
        }
        if generator.lower_outward_sign != -1 || generator.upper_outward_sign != 1 {
            return Err(ValidationFault::OutwardSigns);
        }
        if !generator.identity_fiber {
            return Err(ValidationFault::FiberCycles);
        }
    }
    if actual.cycles.iter().any(|cycle| !cycle.identity_fiber) {
        return Err(ValidationFault::FiberCycles);
    }
    let pair_cycle_count = actual
        .cycles
        .iter()
        .filter(|cycle| cycle.word.len() == 4)
        .count();
    let corner_cycle_count = actual
        .cycles
        .iter()
        .filter(|cycle| cycle.word.len() == 3)
        .count();
    if pair_cycle_count != 3 || corner_cycle_count != 6 || actual.cycles.len() != 9 {
        return Err(ValidationFault::CompleteCycleReceipts);
    }
    let cycles_by_word = actual
        .cycles
        .iter()
        .map(|cycle| (cycle.word.clone(), cycle))
        .collect::<BTreeMap<_, _>>();
    if cycles_by_word.len() != actual.cycles.len() {
        return Err(ValidationFault::CompleteCycleReceipts);
    }
    for reference in &expected.cycles {
        let Some(cycle) = cycles_by_word.get(&reference.word) else {
            return Err(ValidationFault::CompleteCycleReceipts);
        };
        if cycle.word.len() == 3 && *cycle != reference {
            return Err(ValidationFault::CornerCycleAgreement);
        }
        if cycle.word.len() == 4 && *cycle != reference {
            return Err(ValidationFault::FiberCycles);
        }
    }

    if actual.inventory != expected.inventory
        || actual.entities.len() != expected.entities.len()
        || actual.inventory.quotient_entities != 192
        || actual.inventory.quotient_strata != [24, 72, 72, 24]
        || actual.inventory.quotient_closure_vertex_references != 648
        || actual.inventory.box_orbit_memberships != 315
    {
        return Err(ValidationFault::CompleteQuotient);
    }
    let entity_key = |entity: &EntityObservation| {
        (
            entity.dimension,
            entity.free_axes.clone(),
            entity.quotient_anchor,
        )
    };
    let entities_by_key = actual
        .entities
        .iter()
        .map(|entity| (entity_key(entity), entity))
        .collect::<BTreeMap<_, _>>();
    if entities_by_key.len() != actual.entities.len() {
        return Err(ValidationFault::CompleteQuotient);
    }
    for reference in &expected.entities {
        let Some(entity) = entities_by_key.get(&entity_key(reference)) else {
            return Err(ValidationFault::CompleteQuotient);
        };
        if entity.dimension == 3 && entity.orbit.len() != 1 {
            return Err(ValidationFault::SingletonCells);
        }
        if entity.orientation_code != 0 {
            return Err(ValidationFault::IdentityOrientation);
        }
        if entity.dimension != reference.dimension
            || entity.free_axes != reference.free_axes
            || entity.quotient_anchor != reference.quotient_anchor
            || entity.quotient_index != reference.quotient_index
        {
            return Err(ValidationFault::CanonicalLastAxisFastestOrder);
        }
        if entity.orbit.iter().cloned().collect::<BTreeSet<_>>()
            != reference.orbit.iter().cloned().collect::<BTreeSet<_>>()
            || entity.closure_vertices != reference.closure_vertices
        {
            return Err(ValidationFault::CompleteQuotient);
        }
    }
    let orbit_sizes = actual
        .entities
        .iter()
        .map(|entity| entity.orbit.len())
        .collect::<BTreeSet<_>>();
    if orbit_sizes != BTreeSet::from([1, 2, 4, 8]) {
        return Err(ValidationFault::CompleteQuotient);
    }

    if actual.exterior_face_count != 0
        || actual.face_incidences.len() != 72
        || actual.cell_face_incidences.len() != 144
    {
        return Err(ValidationFault::ClosedFaceIncidence);
    }
    if actual
        .face_incidences
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        != expected
            .face_incidences
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
        || actual
            .cell_face_incidences
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != expected
                .cell_face_incidences
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
    {
        return Err(ValidationFault::ClosedFaceIncidence);
    }
    assert_connected(&actual.face_incidences);

    if actual.packets.len() != 72
        || actual.inventory.positive_packets != 72
        || actual.inventory.seam_packets != [12, 8, 6]
    {
        return Err(ValidationFault::PacketBijection);
    }
    let packet_ids = actual
        .packets
        .iter()
        .map(|packet| packet.packet)
        .collect::<BTreeSet<_>>();
    if packet_ids.len() != 72 || packet_ids != BTreeSet::from_iter(0..72) {
        return Err(ValidationFault::PacketBijection);
    }
    let packets_by_id = actual
        .packets
        .iter()
        .map(|packet| (packet.packet, packet))
        .collect::<BTreeMap<_, _>>();
    for reference in &expected.packets {
        let Some(packet) = packets_by_id.get(&reference.packet) else {
            return Err(ValidationFault::PacketBijection);
        };
        if packet.lifted_center_distance != reference.lifted_center_distance {
            return Err(ValidationFault::LiftedSeamDistance);
        }
        if packet.owner_face_points != reference.owner_face_points
            || packet.lifted_neighbor_face_points != reference.lifted_neighbor_face_points
        {
            return Err(ValidationFault::TangentialGeometry);
        }
        if packet.packet != reference.packet
            || packet.axis != reference.axis
            || packet.owner_cell != reference.owner_cell
            || packet.neighbor_cell != reference.neighbor_cell
            || packet.quotient_face != reference.quotient_face
            || packet.normal != reference.normal
            || packet.scatter_signs != reference.scatter_signs
            || packet.seam != reference.seam
        {
            return Err(ValidationFault::PacketBijection);
        }
    }

    if actual.persisted_quotient {
        return Err(ValidationFault::NonPersistedProjection);
    }
    if actual.model_artifact_sha256 != expected.model_artifact_sha256
        || actual.model_id != expected.model_id
        || actual.semantic_revision != expected.semantic_revision
        || actual.mesh_artifact_sha256 != expected.mesh_artifact_sha256
    {
        return Err(ValidationFault::CompleteQuotient);
    }
    Ok(())
}

fn validate_event_order(
    expected: &ProductProjectionObservation,
    events: &[ProductEvent],
) -> Result<(), ValidationFault> {
    let pair_positions = expected
        .generators
        .iter()
        .map(|generator| {
            let positions = events
                .iter()
                .enumerate()
                .filter_map(|(index, event)| {
                    (*event == ProductEvent::PairValidated(generator.connection)).then_some(index)
                })
                .collect::<Vec<_>>();
            (generator.connection, positions)
        })
        .collect::<BTreeMap<_, _>>();
    if pair_positions
        .values()
        .any(|positions| positions.len() != 1)
    {
        return Err(ValidationFault::AdmissionBeforeAllocation);
    }
    let group = unique_event(events, &ProductEvent::GroupAdmitted)?;
    let inventory = unique_event(
        events,
        &ProductEvent::AbstractInventoryAdmitted(expected.inventory),
    )?;
    let allocation = unique_event(events, &ProductEvent::ProjectionStateAllocated)?;
    let publication = unique_event(events, &ProductEvent::ProjectionPublished)?;
    if pair_positions
        .values()
        .flat_map(|positions| positions.iter().copied())
        .any(|position| position >= group)
        || !(group < inventory && inventory < allocation && allocation < publication)
    {
        return Err(ValidationFault::AdmissionBeforeAllocation);
    }
    Ok(())
}

fn unique_event(events: &[ProductEvent], needle: &ProductEvent) -> Result<usize, ValidationFault> {
    let positions = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| (event == needle).then_some(index))
        .collect::<Vec<_>>();
    if positions.len() == 1 {
        Ok(positions[0])
    } else {
        Err(ValidationFault::AdmissionBeforeAllocation)
    }
}

fn apply_mutant(mutant: MutantId, run: &mut ProductRun) {
    let observation = run.result.as_mut().expect("positive observation");
    match mutant {
        MutantId::OnePairReused => {
            observation.generators[2].connection = observation.generators[1].connection;
        }
        MutantId::MissingAxis => {
            observation.generators.pop();
        }
        MutantId::DuplicateAxis => {
            observation.generators[1].axis = 0;
        }
        MutantId::CrossParent => {
            observation.generators[1].parent = Id::<kinds::Domain>::new().erase();
        }
        MutantId::CrossConnector => {
            observation.generators[1].connector = Id::<kinds::Domain>::new().erase();
        }
        MutantId::EndpointOrder => {
            let generator = &mut observation.generators[0];
            std::mem::swap(&mut generator.lower_port, &mut generator.upper_port);
            std::mem::swap(
                &mut generator.lower_coordinate,
                &mut generator.upper_coordinate,
            );
            generator.period = -generator.period;
        }
        MutantId::StoredVector => {
            observation.generators[0].translation_authority = TranslationAuthority::StoredVector;
        }
        MutantId::NoncommutingFiber => {
            observation.cycles[0].identity_fiber = false;
        }
        MutantId::PairwiseOnly => {
            observation.cycles.retain(|cycle| cycle.word.len() == 4);
        }
        MutantId::OrdinaryUnion => {
            observation.generators[0].authority = GeneratorAuthority::OrdinaryConservingUnion;
        }
        MutantId::FaceOnlyQuotient => {
            observation.entities.retain(|entity| entity.dimension >= 2);
        }
        MutantId::CornerOrder => {
            let corner = observation
                .cycles
                .iter_mut()
                .find(|cycle| cycle.word.len() == 3)
                .expect("corner receipt");
            corner.net_coefficients = [1, 0, 1];
        }
        MutantId::TangentialShift => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| packet.seam)
                .expect("seam packet");
            let tangent = (packet.axis + 1) % 3;
            packet.owner_face_points[0][tangent] += 0.25;
        }
        MutantId::OrientationPermute => {
            observation.entities[0].orientation_code = 1;
        }
        MutantId::OutwardSameSign => {
            observation.generators[0].lower_outward_sign = 1;
        }
        MutantId::CellCollapse => {
            let cell = observation
                .entities
                .iter_mut()
                .find(|entity| entity.dimension == 3)
                .expect("cell entity");
            cell.orbit.push(cell.orbit[0].clone());
        }
        MutantId::SeamDouble => {
            let duplicate = observation
                .packets
                .iter()
                .find(|packet| packet.seam)
                .expect("seam packet")
                .clone();
            observation.packets.push(duplicate);
        }
        MutantId::SeamExterior => {
            observation.exterior_face_count = 1;
        }
        MutantId::LongSeam => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| packet.seam)
                .expect("seam packet");
            let axis = packet.axis;
            packet.lifted_center_distance = Some(AXES[axis][AXES[axis].len() - 1] - AXES[axis][0]);
        }
        MutantId::XFastest => {
            observation.entities[0].quotient_index += observation.inventory.quotient_entities;
        }
        MutantId::PersistedQuotient => {
            observation.persisted_quotient = true;
        }
        MutantId::AllocateFirst => {
            let inventory = run
                .events
                .iter()
                .position(|event| matches!(event, ProductEvent::AbstractInventoryAdmitted(_)))
                .expect("inventory event");
            let allocation = run
                .events
                .iter()
                .position(|event| *event == ProductEvent::ProjectionStateAllocated)
                .expect("allocation event");
            run.events.swap(inventory, allocation);
        }
    }
}

fn assert_permutation_preserves_meaning(original_fixture: &Fixture) {
    let permuted = fixture(PERMUTED_SOURCE);
    let expected = replay_expected(&permuted);
    assert_eq!(
        expected,
        replay_expected(original_fixture),
        "declaration and endpoint order do not change canonical group meaning"
    );
    let selected = [
        permuted.connections[1],
        permuted.connections[2],
        permuted.connections[0],
    ];
    let run = <super::EvidenceHarness as ProductHarness>::project(
        &permuted.model,
        &permuted.mesh,
        &selected,
    );
    let observed = run
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("permuted ordinary path failed: {error}"));
    validate_projection(&expected, observed, &run.events)
        .expect("endpoint/declaration permutation retains the full projection");
}

fn assert_current_artifacts_do_not_persist_quotient(fixture: &Fixture) {
    for (label, bytes) in [
        ("Model", fixture.model_json.as_slice()),
        ("Transaction", fixture.transaction_json.as_slice()),
        ("Cartesian mesh", fixture.mesh_json.as_slice()),
    ] {
        let text = std::str::from_utf8(bytes).expect("canonical JSON is UTF-8");
        assert!(
            !text.contains("quotient"),
            "{label} persisted quotient state"
        );
        assert!(!text.contains("orbit"), "{label} persisted orbit state");
        assert!(
            !text.contains("positive_packet"),
            "{label} persisted packet state"
        );
    }

    let forged_model = inject_unknown_quotient(&fixture.model_json);
    assert!(
        AcceptedModelArtifact::from_json(&forged_model, ModelDecoderLimits::default()).is_err(),
        "the current Model decoder rejects an unauthorized quotient field"
    );
    let forged_transaction = inject_unknown_quotient(&fixture.transaction_json);
    assert!(
        ModelTransactionEnvelope::from_json(&forged_transaction, ModelDecoderLimits::default(),)
            .is_err(),
        "the current Transaction decoder rejects an unauthorized quotient field"
    );
    let forged_mesh = inject_unknown_quotient(&fixture.mesh_json);
    assert!(
        CartesianMeshEnvelopeV1::from_json(&forged_mesh, Default::default()).is_err(),
        "Cartesian mesh v1 rejects an unauthorized quotient field or version"
    );
}

fn inject_unknown_quotient(bytes: &[u8]) -> Vec<u8> {
    let mut forged = bytes.to_vec();
    let end = forged
        .iter()
        .rposition(|byte| *byte == b'}')
        .expect("canonical envelope is a JSON object");
    forged.splice(end..end, b",\"quotient\":{}".iter().copied());
    forged
}

fn assert_input_rejections_are_nonvacuous(fixture: &Fixture) {
    let missing_axis = <super::EvidenceHarness as ProductHarness>::project(
        &fixture.model,
        &fixture.mesh,
        &fixture.connections[..2],
    );
    assert!(
        missing_axis.result.is_err(),
        "two valid pairs cannot form the group"
    );
    for connection in &fixture.connections[..2] {
        assert!(
            missing_axis
                .events
                .contains(&ProductEvent::PairValidated(connection.erase())),
            "P3D-MISSING-AXIS reaches group admission only after both pairs validate"
        );
    }
    assert_no_projection_mutation(&missing_axis.events);

    let repeated = [
        fixture.connections[0],
        fixture.connections[0],
        fixture.connections[1],
    ];
    let one_pair_reused = <super::EvidenceHarness as ProductHarness>::project(
        &fixture.model,
        &fixture.mesh,
        &repeated,
    );
    assert!(
        one_pair_reused.result.is_err(),
        "reusing one pair cannot satisfy the three-Connection inventory"
    );
    assert_no_projection_mutation(&one_pair_reused.events);
}

fn assert_no_projection_mutation(events: &[ProductEvent]) {
    assert!(
        events.iter().all(|event| {
            !matches!(
                event,
                ProductEvent::ProjectionStateAllocated | ProductEvent::ProjectionPublished
            )
        }),
        "a rejected group must not allocate or publish projection state"
    );
}
