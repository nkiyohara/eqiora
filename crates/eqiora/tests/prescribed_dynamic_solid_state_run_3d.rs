use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroUsize;

use eqiora::api::{ModelDocument, PrescribedDynamicSolidStateRun3d};
use eqiora::artifact::{
    DiscreteFieldEnvelopeV1, FieldSnapshotEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, JsonDecoderLimits, ModelDecoderLimits, ModelEnvelope,
    PrescribedDynamicSolidRealizationEnvelopeV1, RealizationDecoderLimits,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1, SpatialStateEnvelopeV1,
};
use eqiora::assembly::{
    AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork, REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora::kernel::BoundarySide;
use eqiora::meshing::{MeshQualityGate, SimplicialMesh, VertexId};
use eqiora::realization::RealizationRevision;
use eqiora::solver::{
    BackendId, ExecutionId, ExecutionProvider, ExecutionReport, FixedOrderInnerProduct,
    LinearOperator, LinearProblem, LinearSolution, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, REFERENCE_SOLVER_PROVIDER, ReductionPolicy,
    ReplicatedLinearExecution, SERIAL_EXECUTION_PROVIDER, SERIAL_LINEAR_EXECUTION,
    SolverCapabilities, SolverPlan, SolverProvider, accept_linear_solution,
    accept_linear_solution_with_verifier,
};
use eqiora::{Diagnostic, DimExponents, DynQuantity, Id, kinds};
use eqiora_numerics::{
    common::PhysicalBoundaryDisposition,
    solid::{PrescribedDynamicSolidReference3d, lower_isotropic_elastodynamics_cartesian_3d},
};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Value, json};

macro_rules! expected {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/",
            $name
        ))
    };
}

const DIRECT_SOURCE: &str = include_str!(
    "../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/models/direct.eqi"
);
const SCIENTIFIC_ORACLE: &str = include_str!(
    "../../../verify/solid/prescribed-dynamic-solid-step-3d/expected/accepted-step.json"
);
const LINEAGE_KEY: &str = "model_sha256";

const EXPECTED_MODEL: &[u8] = expected!("model.json");
const EXPECTED_GEOMETRY: &[u8] = expected!("geometry-identity.json");
const EXPECTED_MESH: &[u8] = expected!("mesh.json");
const EXPECTED_CORRESPONDENCE: &[u8] = expected!("correspondence.json");
const EXPECTED_REALIZATION: &[u8] = expected!("realization.json");
const EXPECTED_PRIOR_DISPLACEMENT_BLOCK: &[u8] = expected!("prior-displacement-block.json");
const EXPECTED_PRIOR_VELOCITY_BLOCK: &[u8] = expected!("prior-velocity-block.json");
const EXPECTED_ACCEPTED_DISPLACEMENT_BLOCK: &[u8] = expected!("accepted-displacement-block.json");
const EXPECTED_ACCEPTED_VELOCITY_BLOCK: &[u8] = expected!("accepted-velocity-block.json");
const EXPECTED_PRIOR_DISPLACEMENT_SNAPSHOT: &[u8] = expected!("prior-displacement-snapshot.json");
const EXPECTED_PRIOR_VELOCITY_SNAPSHOT: &[u8] = expected!("prior-velocity-snapshot.json");
const EXPECTED_ACCEPTED_DISPLACEMENT_SNAPSHOT: &[u8] =
    expected!("accepted-displacement-snapshot.json");
const EXPECTED_ACCEPTED_VELOCITY_SNAPSHOT: &[u8] = expected!("accepted-velocity-snapshot.json");
const EXPECTED_PRIOR_STATE: &[u8] = expected!("prior-state.json");
const EXPECTED_ACCEPTED_STATE: &[u8] = expected!("accepted-state.json");
const EXPECTED_RUN: &[u8] = expected!("run.json");

const ZERO: u64 = 0;
const PRIOR_DISPLACEMENT_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [0x3f747ae147ae147b, ZERO, ZERO],
];
const PRIOR_VELOCITY_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
];
const ACCEPTED_DISPLACEMENT_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f8eb851eb851eb8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f8eb851eb851eb8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f8eb851eb851eb8, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f8eb851eb851eb8, ZERO, ZERO],
    [0x3f7eb851eb851eb9, ZERO, ZERO],
];
const ACCEPTED_VELOCITY_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147a, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147a, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147a, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147a, ZERO, ZERO],
    [0x3f847ae147ae147c, ZERO, ZERO],
];

const VERTICES: [[f64; 3]; 9] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.5, 0.5, 0.5],
];
const CELLS: [[usize; 4]; 12] = [
    [8, 0, 6, 2],
    [8, 0, 4, 6],
    [8, 1, 7, 5],
    [8, 1, 3, 7],
    [8, 0, 5, 4],
    [8, 0, 1, 5],
    [8, 2, 7, 3],
    [8, 2, 6, 7],
    [8, 0, 3, 1],
    [8, 0, 2, 3],
    [8, 4, 7, 6],
    [8, 4, 5, 7],
];
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

struct Fixture {
    document: ModelDocument,
    model: ModelEnvelope,
    geometry: GeometryIdentityEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    prior_displacement: Vec<(VertexId, [f64; 3])>,
    prior_velocity: Vec<(VertexId, [f64; 3])>,
    candidate: Vec<(VertexId, [f64; 3])>,
}

impl Fixture {
    fn new() -> Self {
        let document = ModelDocument::compile("prescribed-state-run.eqi", DIRECT_SOURCE)
            .expect("the frozen direct source compiles");
        let model = ModelEnvelope::from_program(document.program()).expect("current Model");
        let body = domain(&document, "body");
        let geometry = GeometryIdentityEnvelopeV1::new(&model, [body], 1.0e-12)
            .expect("exact unit-cube Geometry identity");
        let mesh = exact_mesh(0.1);
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh)
            .expect("exact Geometry-to-Mesh correspondence");
        Self {
            prior_displacement: tagged(PRIOR_DISPLACEMENT_BITS),
            prior_velocity: tagged(PRIOR_VELOCITY_BITS),
            candidate: [1, 3, 5, 7]
                .into_iter()
                .map(|index| {
                    (
                        VertexId::new(index),
                        [f64::from_bits(0x3f8eb851eb851eb8), 0.0, 0.0],
                    )
                })
                .collect(),
            document,
            model,
            geometry,
            mesh,
            correspondence,
        }
    }

    fn realization(&self) -> PrescribedDynamicSolidRealizationEnvelopeV1 {
        PrescribedDynamicSolidRealizationEnvelopeV1::new(
            &self.model,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            RealizationRevision::new(1),
            &self.candidate,
        )
        .expect("the exact standalone-solid Realization is admitted")
    }

    fn owner(&self) -> PrescribedDynamicSolidStateRun3d {
        PrescribedDynamicSolidStateRun3d::solve_reference(
            &self.document,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the exact accepted occurrence publishes atomically")
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
fn compile_frozen_public_signatures(
    document: &ModelDocument,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
    model: &impl ReplayableCanonicalModelArtifact,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    field: Id<kinds::Field>,
    blocks: &[DiscreteFieldEnvelopeV1],
    snapshots: &[FieldSnapshotEnvelopeV1],
) -> Result<(), Diagnostic> {
    let owner: PrescribedDynamicSolidStateRun3d =
        PrescribedDynamicSolidStateRun3d::solve_reference(document, assembly, solver)?;
    let realization: &PrescribedDynamicSolidRealizationEnvelopeV1 = owner.realization();
    let snapshot = FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
        model,
        realization,
        geometry,
        correspondence,
        mesh,
        field,
        blocks,
    )?;
    snapshot.validate_against_prescribed_dynamic_solid(
        model,
        realization,
        geometry,
        correspondence,
        mesh,
        blocks,
    )?;
    let state = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
        model,
        realization,
        geometry,
        correspondence,
        mesh,
        0,
        0.0,
        snapshots,
    )?;
    state.validate_against_prescribed_dynamic_solid(
        model,
        realization,
        geometry,
        correspondence,
        mesh,
        snapshots,
    )?;
    owner.revalidate()
}

#[test]
fn protected_main_resources_and_live_bits_equal_the_precommitted_oracle() {
    let fixture = Fixture::new();
    assert_exact_json(fixture.model.canonical_json().unwrap(), EXPECTED_MODEL);
    assert_exact_json(
        fixture.geometry.canonical_json().unwrap(),
        EXPECTED_GEOMETRY,
    );
    assert_exact_json(fixture.mesh.canonical_json().unwrap(), EXPECTED_MESH);
    assert_exact_json(
        fixture.correspondence.canonical_json().unwrap(),
        EXPECTED_CORRESPONDENCE,
    );

    let mut reference = PrescribedDynamicSolidReference3d::new(
        &fixture.model,
        &fixture.geometry,
        &fixture.mesh,
        &fixture.correspondence,
        DynQuantity::new(0.25, TIME),
        &fixture.prior_displacement,
        &fixture.prior_velocity,
        domain(&fixture.document, "x_upper"),
    )
    .unwrap();
    let accepted = reference
        .accept_candidate(
            0,
            &fixture.candidate,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the accepted #352 path produces the persisted observation");
    assert_coefficient_bits(accepted.displacement(), &ACCEPTED_DISPLACEMENT_BITS);
    assert_coefficient_bits(accepted.velocity(), &ACCEPTED_VELOCITY_BITS);
    assert_eq!(
        accepted.displacement()[8].1[0].to_bits(),
        0x3f7eb851eb851eb9
    );
    assert_eq!(
        format!("{}", accepted.displacement()[8].1[0]),
        "0.007500000000000001"
    );

    let scientific: Value = serde_json::from_str(SCIENTIFIC_ORACLE).unwrap();
    let expected_center = scientific["accepted"]["displacement_m"][8][1][0]
        .as_f64()
        .unwrap();
    let tolerance = scientific["tolerances"]["displacement_and_velocity"]
        .as_f64()
        .unwrap();
    assert_eq!(expected_center, 0.0075);
    assert!((accepted.displacement()[8].1[0] - expected_center).abs() <= tolerance);
}

#[test]
fn application_publishes_the_exact_realization_states_and_singleton_run() {
    let fixture = Fixture::new();
    let owner = fixture.owner();
    owner.revalidate().expect("the complete owner revalidates");

    assert_eq!(owner.model(), &fixture.model);
    assert_exact_json(
        owner.geometry().canonical_json().unwrap(),
        EXPECTED_GEOMETRY,
    );
    assert_exact_json(owner.mesh().canonical_json().unwrap(), EXPECTED_MESH);
    assert_exact_json(
        owner.correspondence().canonical_json().unwrap(),
        EXPECTED_CORRESPONDENCE,
    );
    assert_exact_json(
        owner.realization().canonical_json().unwrap(),
        EXPECTED_REALIZATION,
    );
    assert_exact_json(
        owner.prior_displacement_block().canonical_json().unwrap(),
        EXPECTED_PRIOR_DISPLACEMENT_BLOCK,
    );
    assert_exact_json(
        owner.prior_velocity_block().canonical_json().unwrap(),
        EXPECTED_PRIOR_VELOCITY_BLOCK,
    );
    assert_exact_json(
        owner
            .accepted_displacement_block()
            .canonical_json()
            .unwrap(),
        EXPECTED_ACCEPTED_DISPLACEMENT_BLOCK,
    );
    assert_exact_json(
        owner.accepted_velocity_block().canonical_json().unwrap(),
        EXPECTED_ACCEPTED_VELOCITY_BLOCK,
    );
    assert_exact_json(
        owner
            .prior_displacement_snapshot()
            .canonical_json()
            .unwrap(),
        EXPECTED_PRIOR_DISPLACEMENT_SNAPSHOT,
    );
    assert_exact_json(
        owner.prior_velocity_snapshot().canonical_json().unwrap(),
        EXPECTED_PRIOR_VELOCITY_SNAPSHOT,
    );
    assert_exact_json(
        owner
            .accepted_displacement_snapshot()
            .canonical_json()
            .unwrap(),
        EXPECTED_ACCEPTED_DISPLACEMENT_SNAPSHOT,
    );
    assert_exact_json(
        owner.accepted_velocity_snapshot().canonical_json().unwrap(),
        EXPECTED_ACCEPTED_VELOCITY_SNAPSHOT,
    );
    assert_exact_json(
        owner.prior_state().canonical_json().unwrap(),
        EXPECTED_PRIOR_STATE,
    );
    assert_exact_json(
        owner.accepted_state().canonical_json().unwrap(),
        EXPECTED_ACCEPTED_STATE,
    );
    assert_exact_json(owner.run().canonical_json().unwrap(), EXPECTED_RUN);

    assert_eq!(owner.accepted().generation(), 1);
    assert_coefficient_bits(owner.accepted().displacement(), &ACCEPTED_DISPLACEMENT_BITS);
    assert_coefficient_bits(owner.accepted().velocity(), &ACCEPTED_VELOCITY_BITS);
    assert_flat_bits(
        owner.prior_displacement_block().values(),
        &PRIOR_DISPLACEMENT_BITS,
    );
    assert_flat_bits(owner.prior_velocity_block().values(), &PRIOR_VELOCITY_BITS);
    assert_flat_bits(
        owner.accepted_displacement_block().values(),
        &ACCEPTED_DISPLACEMENT_BITS,
    );
    assert_flat_bits(
        owner.accepted_velocity_block().values(),
        &ACCEPTED_VELOCITY_BITS,
    );

    assert_eq!(owner.prior_state().step(), 0);
    assert_eq!(owner.prior_state().time_s().to_bits(), 0.0f64.to_bits());
    assert_eq!(owner.accepted_state().step(), 1);
    assert_eq!(owner.accepted_state().time_s().to_bits(), 0.25f64.to_bits());
    let field_order = owner
        .accepted_state()
        .fields()
        .into_iter()
        .map(|(_, field, _)| field)
        .collect::<Vec<_>>();
    assert_eq!(
        field_order,
        vec![
            owner.realization().velocity_field(),
            owner.realization().displacement_field(),
        ],
        "State identity uses canonical Field-ULID order"
    );
    assert_eq!(
        owner.run().outputs(),
        vec![owner.accepted_state().digest().unwrap()]
    );
    assert_ne!(
        owner.prior_state().digest().unwrap(),
        owner.accepted_state().digest().unwrap()
    );
    assert_ne!(
        owner.prior_velocity_block().digest().unwrap(),
        owner.accepted_velocity_block().digest().unwrap(),
        "distinct live velocity bits retain distinct block content identities"
    );
    assert_ne!(
        owner.prior_velocity_snapshot().digest().unwrap(),
        owner.accepted_velocity_snapshot().digest().unwrap(),
        "role-specific velocity snapshots retain their distinct block content"
    );
}

#[test]
fn realization_decoder_is_closed_canonical_and_bounded() {
    let bytes = canonical_fixture(EXPECTED_REALIZATION);
    let decoded = PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
        &bytes,
        RealizationDecoderLimits::default(),
    )
    .expect("detached canonical bytes are locally valid");
    assert_exact_json(decoded.canonical_json().unwrap(), EXPECTED_REALIZATION);

    assert_decode_rejected(b"{");
    let mut wrong_schema = realization_wire();
    wrong_schema["schema"] = json!("eqiora.prescribed-solid/v0");
    assert_value_decode_rejected(wrong_schema);
    let mut wrong_encoding = realization_wire();
    wrong_encoding["encoding"] = json!("json");
    assert_value_decode_rejected(wrong_encoding);
    let mut unknown = realization_wire();
    unknown["unexpected"] = json!(true);
    assert_value_decode_rejected(unknown);

    let reordered_bytes = move_first_top_level_member_to_end(EXPECTED_REALIZATION);
    assert_ne!(reordered_bytes, bytes);
    assert_decode_rejected(&reordered_bytes);

    let mut malformed_digest = realization_wire();
    malformed_digest[LINEAGE_KEY] = json!("00");
    assert_value_decode_rejected(malformed_digest);
    let mut uppercase_digest = realization_wire();
    uppercase_digest[LINEAGE_KEY] = json!(
        uppercase_digest[LINEAGE_KEY]
            .as_str()
            .unwrap()
            .to_ascii_uppercase()
    );
    assert_value_decode_rejected(uppercase_digest);
    let mut malformed_ulid = realization_wire();
    malformed_ulid["spatial"]["solid_domain_ulid"] = json!("not-a-ulid");
    assert_value_decode_rejected(malformed_ulid);
    let mut noncanonical_ulid = realization_wire();
    noncanonical_ulid["spatial"]["solid_domain_ulid"] = json!(
        noncanonical_ulid["spatial"]["solid_domain_ulid"]
            .as_str()
            .unwrap()
            .to_ascii_lowercase()
    );
    assert_value_decode_rejected(noncanonical_ulid);

    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
            &bytes,
            RealizationDecoderLimits {
                json: JsonDecoderLimits {
                    max_bytes: bytes.len() - 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .is_err()
    );
    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
            &bytes,
            RealizationDecoderLimits {
                json: JsonDecoderLimits {
                    max_nesting_depth: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .is_err()
    );
    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
            &bytes,
            RealizationDecoderLimits {
                max_realization_fields: 1,
                ..Default::default()
            },
        )
        .is_err()
    );

    for (pointer, replacement) in [
        ("/source/kind", json!("generated")),
        ("/source/realization_revision", json!(2)),
        ("/spatial/spatial_dimension", json!(2)),
        ("/spatial/scalar", json!("f32")),
        ("/spatial/vector_layout", json!("distributed")),
        ("/spatial/space/kind", json!("discontinuous-lagrange")),
        ("/spatial/space/order", json!(2)),
        ("/spatial/discretization/method", json!("finite-volume")),
        (
            "/spatial/discretization/mesh/kind",
            json!("generated-cartesian"),
        ),
        ("/spatial/discretization/quadrature", json!("centroid")),
        ("/time/method", json!("forward-euler")),
        ("/time/duration_s", json!(0.5)),
        ("/solver/operator_properties", json!("general")),
        ("/solver/algorithm", json!("minimum-residual")),
        ("/solver/preconditioner", json!("jacobi")),
        ("/solver/reduction", json!("fast")),
        ("/solver/relative_tolerance", json!(1e-12)),
        ("/solver/absolute_tolerance", json!(1e-14)),
        ("/solver/maximum_iterations", json!(501)),
        ("/placement/target/kind", json!("accelerator")),
        ("/placement/target/threads", json!(2)),
        ("/placement/schedule/kind", json!("online")),
        ("/placement/assembly_execution", json!("host-parallel")),
        ("/placement/solve_execution", json!("host-parallel")),
        ("/placement/verification_execution", json!("host-parallel")),
        ("/placement/layout_artifacts/kind", json!("distributed")),
    ] {
        let mut mutant = realization_wire();
        *mutant.pointer_mut(pointer).unwrap() = replacement;
        assert_value_decode_rejected(mutant);
    }
}

#[test]
fn driven_displacement_mutants_replace_complete_selected_objects() {
    let changed = mutate_driven_object(3, |object| object["value_m"] = json!([0.016, 0.0, 0.0]));
    assert_decode_rejected(&changed);
    let negative_zero =
        mutate_driven_object(3, |object| object["value_m"] = json!([-0.0, 0.0, 0.0]));
    assert_decode_rejected(&negative_zero);
    let wrong_support =
        mutate_driven_object(3, |object| object["value_m"] = json!([0.015, 0.001, 0.0]));
    assert_decode_rejected(&wrong_support);
    let wrong_identity = mutate_driven_object(3, |object| object["vertex_index"] = json!(2));
    assert_decode_rejected(&wrong_identity);

    let mut duplicate = realization_wire();
    let entries = duplicate["driven_total_displacement"]
        .as_array_mut()
        .unwrap();
    let original = entries.clone();
    let selected = unique_entry(entries, 3);
    let replacement = entries[0].clone();
    entries[selected] = replacement;
    for position in 0..entries.len() {
        if position != selected {
            assert_eq!(entries[position], original[position]);
        }
    }
    assert_value_decode_rejected(duplicate);

    let mut missing = realization_wire();
    let entries = missing["driven_total_displacement"].as_array_mut().unwrap();
    let selected = unique_entry(entries, 3);
    let removed = entries.remove(selected);
    assert_eq!(removed["vertex_index"], 3);
    assert_value_decode_rejected(missing);

    let mut reordered = realization_wire();
    let entries = reordered["driven_total_displacement"]
        .as_array_mut()
        .unwrap();
    let before = entries.clone();
    let first = unique_entry(entries, 1);
    let second = unique_entry(entries, 3);
    entries.swap(first, second);
    let mut before_sorted = before.clone();
    let mut after_sorted = entries.clone();
    before_sorted.sort_by_key(|entry| entry["vertex_index"].as_u64().unwrap());
    after_sorted.sort_by_key(|entry| entry["vertex_index"].as_u64().unwrap());
    assert_eq!(before_sorted, after_sorted, "only complete objects moved");
    assert_value_decode_rejected(reordered);

    let overflowing =
        replace_driven_object_raw(3, br#"{"vertex_index":3,"value_m":[1e400,0.0,0.0]}"#);
    assert_decode_rejected(&overflowing);
    let noncanonical =
        replace_driven_object_raw(3, br#"{"vertex_index":3,"value_m":[1.50e-2,0.0,0.0]}"#);
    assert_decode_rejected(&noncanonical);
}

#[test]
fn resource_validation_rejects_stale_roles_gates_and_identity_preserving_semantic_drift() {
    let fixture = Fixture::new();
    let realization = fixture.realization();
    assert_exact_json(realization.canonical_json().unwrap(), EXPECTED_REALIZATION);
    realization
        .validate_against(
            &fixture.model,
            &fixture.geometry,
            &fixture.correspondence,
            &fixture.mesh,
        )
        .unwrap();

    let mut stale = realization_wire();
    stale[LINEAGE_KEY] = json!(fixture.geometry.digest().unwrap().to_string());
    let stale = decode_realization_value(stale);
    assert!(
        stale
            .validate_against(
                &fixture.model,
                &fixture.geometry,
                &fixture.correspondence,
                &fixture.mesh,
            )
            .is_err()
    );

    for pointer in [
        "/geometry_sha256",
        "/correspondence_sha256",
        "/spatial/discretization/mesh/artifact_sha256",
    ] {
        let mut mutant = realization_wire();
        *mutant.pointer_mut(pointer).unwrap() = json!(fixture.model.digest().unwrap().to_string());
        let decoded = decode_realization_value(mutant);
        assert!(
            decoded
                .validate_against(
                    &fixture.model,
                    &fixture.geometry,
                    &fixture.correspondence,
                    &fixture.mesh,
                )
                .is_err(),
            "a valid-format foreign resource digest must fail exact binding at {pointer}"
        );
    }

    let mut wrong_role = realization_wire();
    wrong_role["spatial"]["fixed_boundary_ulid"] = json!(
        fixture
            .geometry
            .boundaries()
            .iter()
            .find(|entry| entry.axis() == 1 && entry.side() == BoundarySide::Lower)
            .unwrap()
            .domain()
            .ulid()
            .to_string()
    );
    let wrong_role = decode_realization_value(wrong_role);
    assert!(
        wrong_role
            .validate_against(
                &fixture.model,
                &fixture.geometry,
                &fixture.correspondence,
                &fixture.mesh,
            )
            .is_err(),
        "detached local validity is not semantic-role validity"
    );
    for (pointer, replacement) in [
        (
            "/model_ulid",
            json!(domain(&fixture.document, "body").ulid().to_string()),
        ),
        ("/semantic_revision", json!(2)),
        (
            "/spatial/solid_domain_ulid",
            json!(domain(&fixture.document, "x_upper").ulid().to_string()),
        ),
        (
            "/spatial/displacement_field_ulid",
            json!(realization.velocity_field().ulid().to_string()),
        ),
    ] {
        let mut mutant = realization_wire();
        *mutant.pointer_mut(pointer).unwrap() = replacement;
        let decoded = PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
            &encode_like(EXPECTED_REALIZATION, &mutant),
            Default::default(),
        )
        .expect("valid-format identity and role drift must reach resource validation");
        assert!(
            decoded
                .validate_against(
                    &fixture.model,
                    &fixture.geometry,
                    &fixture.correspondence,
                    &fixture.mesh,
                )
                .is_err()
        );
    }

    let wrong_geometry = GeometryIdentityEnvelopeV1::new(
        &fixture.model,
        [domain(&fixture.document, "body")],
        2.0e-12,
    )
    .unwrap();
    let wrong_geometry_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&wrong_geometry, &fixture.model, &fixture.mesh)
            .unwrap();
    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::new(
            &fixture.model,
            &wrong_geometry,
            &wrong_geometry_correspondence,
            &fixture.mesh,
            RealizationRevision::new(1),
            &fixture.candidate,
        )
        .is_err()
    );
    let wrong_mesh = exact_mesh(0.2);
    let wrong_mesh_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&fixture.geometry, &fixture.model, &wrong_mesh)
            .unwrap();
    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::new(
            &fixture.model,
            &fixture.geometry,
            &wrong_mesh_correspondence,
            &wrong_mesh,
            RealizationRevision::new(1),
            &fixture.candidate,
        )
        .is_err()
    );

    let semantic = semantic_role_mutant();
    let replay = semantic.model.replay_model().unwrap();
    let lowered = lower_isotropic_elastodynamics_cartesian_3d(replay.program())
        .expect("the identity-preserving mutant replays through the ordinary lowerer");
    let lower = lowered
        .boundary_inventory()
        .boundary(0, BoundarySide::Lower)
        .unwrap();
    assert_eq!(lower.boundary().ulid().to_string(), semantic.lower_domain);
    assert!(matches!(
        lower.disposition(),
        PhysicalBoundaryDisposition::PortBinding { .. }
    ));
    let body = lowered.domain().downcast::<kinds::Domain>().unwrap();
    let semantic_geometry =
        GeometryIdentityEnvelopeV1::new(&semantic.model, [body], 1.0e-12).unwrap();
    let semantic_correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(
        &semantic_geometry,
        &semantic.model,
        &fixture.mesh,
    )
    .unwrap();
    let mut semantic_wire = realization_wire();
    semantic_wire[LINEAGE_KEY] = json!(semantic.model.digest().unwrap().to_string());
    semantic_wire["geometry_sha256"] = json!(semantic_geometry.digest().unwrap().to_string());
    semantic_wire["correspondence_sha256"] =
        json!(semantic_correspondence.digest().unwrap().to_string());
    let semantic_realization = decode_realization_value(semantic_wire);
    assert!(
        semantic_realization
            .validate_against(
                &semantic.model,
                &semantic_geometry,
                &semantic_correspondence,
                &fixture.mesh,
            )
            .is_err(),
        "resource validation must rederive the fixed-boundary role"
    );
}

#[test]
fn blocks_snapshots_and_states_replay_exact_local_grammar_without_owner_role_inference() {
    let fixture = Fixture::new();
    let realization = fixture.realization();
    let prior_displacement = decode_block(EXPECTED_PRIOR_DISPLACEMENT_BLOCK);
    let prior_velocity = decode_block(EXPECTED_PRIOR_VELOCITY_BLOCK);
    let accepted_displacement = decode_block(EXPECTED_ACCEPTED_DISPLACEMENT_BLOCK);
    let accepted_velocity = decode_block(EXPECTED_ACCEPTED_VELOCITY_BLOCK);
    for block in [
        &prior_displacement,
        &prior_velocity,
        &accepted_displacement,
        &accepted_velocity,
    ] {
        block.validate_mesh_artifact(&fixture.mesh).unwrap();
    }

    let prior_displacement_snapshot = decode_snapshot(EXPECTED_PRIOR_DISPLACEMENT_SNAPSHOT);
    let prior_velocity_snapshot = decode_snapshot(EXPECTED_PRIOR_VELOCITY_SNAPSHOT);
    let accepted_displacement_snapshot = decode_snapshot(EXPECTED_ACCEPTED_DISPLACEMENT_SNAPSHOT);
    let accepted_velocity_snapshot = decode_snapshot(EXPECTED_ACCEPTED_VELOCITY_SNAPSHOT);

    let original_block: Value =
        serde_json::from_slice(frozen(EXPECTED_PRIOR_DISPLACEMENT_BLOCK)).unwrap();
    let mut block_mutants = Vec::new();
    let mut wrong_mesh = original_block.clone();
    wrong_mesh["mesh_sha256"] = json!(fixture.geometry.digest().unwrap().to_string());
    block_mutants.push(wrong_mesh);
    let mut wrong_association = original_block.clone();
    wrong_association["association"] = json!("cell");
    block_mutants.push(wrong_association);
    let mut wrong_shape = original_block.clone();
    wrong_shape["component_shape"] = json!({"kind":"scalar"});
    block_mutants.push(wrong_shape);
    let mut wrong_entity_count = original_block.clone();
    wrong_entity_count["entity_count"] = json!(8);
    block_mutants.push(wrong_entity_count);
    let mut missing_coefficient = original_block.clone();
    missing_coefficient["values"].as_array_mut().unwrap().pop();
    block_mutants.push(missing_coefficient);
    let mut wrong_coefficient = original_block.clone();
    wrong_coefficient["values"][24] = json!(0.005000000000000001);
    block_mutants.push(wrong_coefficient);
    let mut wrong_vertex_order = original_block.clone();
    let values = wrong_vertex_order["values"].as_array_mut().unwrap();
    let before = values.clone();
    for component in 0..3 {
        values.swap(component, 3 + component);
    }
    assert_eq!(&values[0..3], &before[3..6]);
    assert_eq!(&values[3..6], &before[0..3]);
    let mut before_sorted = before.chunks_exact(3).collect::<Vec<_>>();
    let mut after_sorted = values.chunks_exact(3).collect::<Vec<_>>();
    before_sorted.sort_by_key(|entry| serde_json::to_string(entry).unwrap());
    after_sorted.sort_by_key(|entry| serde_json::to_string(entry).unwrap());
    assert_eq!(
        before_sorted, after_sorted,
        "only complete vertex vectors moved"
    );
    block_mutants.push(wrong_vertex_order);
    for mutant in block_mutants {
        let decoded = DiscreteFieldEnvelopeV1::from_json(
            &encode_like(EXPECTED_PRIOR_DISPLACEMENT_BLOCK, &mutant),
            Default::default(),
        );
        if let Ok(decoded) = decoded {
            let mesh_valid = decoded.validate_mesh_artifact(&fixture.mesh).is_ok();
            let snapshot_valid = validate_snapshot(
                &fixture,
                &realization,
                &prior_displacement_snapshot,
                std::slice::from_ref(&decoded),
            )
            .is_ok();
            assert!(
                !mesh_valid || !snapshot_valid,
                "a locally valid changed block must fail mesh or exact snapshot binding"
            );
        }
    }

    validate_snapshot(
        &fixture,
        &realization,
        &prior_displacement_snapshot,
        std::slice::from_ref(&prior_displacement),
    )
    .unwrap();
    validate_snapshot(
        &fixture,
        &realization,
        &prior_velocity_snapshot,
        std::slice::from_ref(&prior_velocity),
    )
    .unwrap();
    validate_snapshot(
        &fixture,
        &realization,
        &accepted_displacement_snapshot,
        std::slice::from_ref(&accepted_displacement),
    )
    .unwrap();
    validate_snapshot(
        &fixture,
        &realization,
        &accepted_velocity_snapshot,
        std::slice::from_ref(&accepted_velocity),
    )
    .unwrap();

    for (pointer, replacement) in [
        (
            format!("/{LINEAGE_KEY}"),
            json!(fixture.geometry.digest().unwrap().to_string()),
        ),
        (
            "/realization_sha256".into(),
            json!(fixture.model.digest().unwrap().to_string()),
        ),
        (
            "/geometry_sha256".into(),
            json!(fixture.model.digest().unwrap().to_string()),
        ),
        (
            "/correspondence_sha256".into(),
            json!(fixture.model.digest().unwrap().to_string()),
        ),
        (
            "/mesh_sha256".into(),
            json!(fixture.geometry.digest().unwrap().to_string()),
        ),
        (
            "/support_domain_ulid".into(),
            json!(domain(&fixture.document, "x_upper").ulid().to_string()),
        ),
        (
            "/field_ulid".into(),
            json!(realization.velocity_field().ulid().to_string()),
        ),
        ("/physical/dimension/length".into(), json!(2)),
        ("/physical/value_shape/extents/0".into(), json!(2)),
        ("/physical/frame".into(), json!("invariant")),
        (
            "/representation/blocks/0/discrete_field_sha256".into(),
            json!(prior_velocity.digest().unwrap().to_string()),
        ),
    ] {
        let mut mutant: Value =
            serde_json::from_slice(frozen(EXPECTED_PRIOR_DISPLACEMENT_SNAPSHOT)).unwrap();
        let target = mutant.pointer_mut(&pointer).unwrap();
        let before = target.clone();
        *target = replacement;
        assert_ne!(*target, before, "the selected snapshot member must change");
        let decoded = FieldSnapshotEnvelopeV1::from_json(
            &encode_like(EXPECTED_PRIOR_DISPLACEMENT_SNAPSHOT, &mutant),
            Default::default(),
        )
        .expect("valid-format snapshot drift must reach resource validation");
        assert!(
            validate_snapshot(
                &fixture,
                &realization,
                &decoded,
                std::slice::from_ref(&prior_displacement),
            )
            .is_err()
        );
    }

    let prior_snapshots = [
        prior_displacement_snapshot.clone(),
        prior_velocity_snapshot.clone(),
    ];
    let accepted_snapshots = [
        accepted_displacement_snapshot.clone(),
        accepted_velocity_snapshot.clone(),
    ];
    let prior = decode_state(EXPECTED_PRIOR_STATE);
    let accepted = decode_state(EXPECTED_ACCEPTED_STATE);
    validate_state(&fixture, &realization, &prior, &prior_snapshots).unwrap();
    validate_state(&fixture, &realization, &accepted, &accepted_snapshots).unwrap();

    let prior_with_accepted_content = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
        &fixture.model,
        &realization,
        &fixture.geometry,
        &fixture.correspondence,
        &fixture.mesh,
        0,
        0.0,
        &accepted_snapshots,
    )
    .expect("artifact-local State meaning does not infer the owner's prior role");
    validate_state(
        &fixture,
        &realization,
        &prior_with_accepted_content,
        &accepted_snapshots,
    )
    .unwrap();
    let accepted_with_prior_content = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
        &fixture.model,
        &realization,
        &fixture.geometry,
        &fixture.correspondence,
        &fixture.mesh,
        1,
        0.25,
        &prior_snapshots,
    )
    .expect("artifact-local State meaning does not infer the owner's accepted role");
    validate_state(
        &fixture,
        &realization,
        &accepted_with_prior_content,
        &prior_snapshots,
    )
    .unwrap();

    for (pointer, replacement) in [
        ("/accepted/step", json!(2)),
        ("/accepted/time_s", json!(0.5)),
    ] {
        let mut mutant: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
        *mutant.pointer_mut(pointer).unwrap() = replacement;
        let decoded = SpatialStateEnvelopeV1::from_json(
            &encode_like(EXPECTED_PRIOR_STATE, &mutant),
            Default::default(),
        )
        .unwrap();
        assert!(validate_state(&fixture, &realization, &decoded, &prior_snapshots).is_err());
    }
    let mut missing: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
    missing["fields"].as_array_mut().unwrap().pop();
    let missing = SpatialStateEnvelopeV1::from_json(
        &encode_like(EXPECTED_PRIOR_STATE, &missing),
        Default::default(),
    )
    .unwrap();
    assert!(validate_state(&fixture, &realization, &missing, &prior_snapshots).is_err());

    let mut foreign_snapshot: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
    foreign_snapshot["fields"][0]["snapshot_sha256"] =
        json!(fixture.geometry.digest().unwrap().to_string());
    let foreign_snapshot = SpatialStateEnvelopeV1::from_json(
        &encode_like(EXPECTED_PRIOR_STATE, &foreign_snapshot),
        Default::default(),
    )
    .expect("the foreign snapshot digest remains locally valid State data");
    assert!(validate_state(&fixture, &realization, &foreign_snapshot, &prior_snapshots,).is_err());

    let mut additional_field: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
    let mut foreign_field = additional_field["fields"][1].clone();
    foreign_field["field_ulid"] = json!("7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
    additional_field["fields"]
        .as_array_mut()
        .unwrap()
        .push(foreign_field);
    let additional_field = SpatialStateEnvelopeV1::from_json(
        &encode_like(EXPECTED_PRIOR_STATE, &additional_field),
        Default::default(),
    )
    .expect("the additional distinct Field remains locally valid State data");
    assert!(validate_state(&fixture, &realization, &additional_field, &prior_snapshots,).is_err());

    let mut reordered: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
    reordered["fields"].as_array_mut().unwrap().swap(0, 1);
    assert!(
        SpatialStateEnvelopeV1::from_json(
            &encode_like(EXPECTED_PRIOR_STATE, &reordered),
            Default::default(),
        )
        .is_err()
    );
    let mut duplicate: Value = serde_json::from_slice(frozen(EXPECTED_PRIOR_STATE)).unwrap();
    let first = duplicate["fields"][0].clone();
    duplicate["fields"].as_array_mut().unwrap().push(first);
    assert!(
        SpatialStateEnvelopeV1::from_json(
            &encode_like(EXPECTED_PRIOR_STATE, &duplicate),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn application_construction_is_failure_atomic_for_backend_evidence_drift() {
    let document = ModelDocument::compile("failure-atomic.eqi", DIRECT_SOURCE).unwrap();
    assert!(
        PrescribedDynamicSolidStateRun3d::solve_reference(
            &document,
            &RejectAssembly,
            &REFERENCE_LINEAR_SOLVER,
        )
        .is_err()
    );
    assert!(
        PrescribedDynamicSolidStateRun3d::solve_reference(
            &document,
            &REFERENCE_ASSEMBLY_BACKEND,
            &RejectSolver,
        )
        .is_err()
    );
    for mutation in [
        SolverEvidenceMutation::Provider,
        SolverEvidenceMutation::Plan(
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1e-12,
                1e-15,
                NonZeroUsize::new(500).unwrap(),
            )
            .unwrap()
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Reproducible),
        ),
        SolverEvidenceMutation::Execution,
        SolverEvidenceMutation::Verification,
    ] {
        assert!(
            PrescribedDynamicSolidStateRun3d::solve_reference(
                &document,
                &REFERENCE_ASSEMBLY_BACKEND,
                &MutatedSolverEvidence(mutation),
            )
            .is_err(),
            "substituted accepted execution evidence must publish no owner"
        );
    }
}

struct SemanticMutant {
    model: ModelEnvelope,
    lower_domain: String,
}

fn semantic_role_mutant() -> SemanticMutant {
    let original_bytes = frozen(EXPECTED_MODEL);
    let mut original: Value = serde_json::from_slice(original_bytes).unwrap();
    let lower_domain = cartesian_domain(&original, 0, "lower");
    let upper_domain = cartesian_domain(&original, 0, "upper");
    let lower = unique_relation_on(&original, &lower_domain, trace_only_relation);
    let upper = unique_relation_on(&original, &upper_domain, live_self_cancellation_relation);
    let lower_port = unique_relation_port(&original, &lower);
    let upper_port = unique_relation_port(&original, &upper);
    assert_ne!(lower_port, upper_port);

    let lower_residual_port =
        &relation(&original, &lower)["definition"]["residuals"]["nodes"][0]["symbol"]["id"];
    assert_eq!(lower_residual_port["kind"], "port");
    assert_eq!(lower_residual_port["ulid"], lower_port);
    let upper_residual_port =
        &relation(&original, &upper)["definition"]["residuals"]["nodes"][0]["symbol"]["id"];
    assert_eq!(upper_residual_port["kind"], "port");
    assert_eq!(upper_residual_port["ulid"], upper_port);

    let original_ids = id_set(&original);
    let original_edges = original["edges"].clone();
    let lower_residual = relation(&original, &lower)["definition"]["residuals"].clone();
    let mut retargeted = relation(&original, &upper)["definition"]["residuals"].clone();
    let mut replacements = 0;
    for node in retargeted["nodes"].as_array_mut().unwrap() {
        if node["op"] == "symbol"
            && matches!(
                node["symbol"]["kind"].as_str(),
                Some("port-trace" | "port-flux")
            )
        {
            assert_eq!(node["symbol"]["id"]["ulid"], upper_port);
            node["symbol"]["id"]["ulid"] = json!(lower_port);
            replacements += 1;
        }
    }
    assert_eq!(replacements, 4);
    relation_mut(&mut original, &lower)["definition"]["residuals"] = retargeted;
    assert_eq!(id_set(&original), original_ids);
    assert_eq!(original["edges"], original_edges);

    let mut restored = original.clone();
    relation_mut(&mut restored, &lower)["definition"]["residuals"] = lower_residual;
    assert_eq!(encode_like(EXPECTED_MODEL, &restored), original_bytes);
    let mutated_bytes = encode_like(EXPECTED_MODEL, &original);
    assert_ne!(mutated_bytes, original_bytes);
    let model = ModelEnvelope::from_json(&mutated_bytes, ModelDecoderLimits::default())
        .expect("the one-subtree current-Model mutant remains canonical");
    assert_eq!(model.canonical_json().unwrap(), mutated_bytes);
    SemanticMutant {
        model,
        lower_domain,
    }
}

fn cartesian_domain(document: &Value, axis: u64, side: &str) -> String {
    exactly_one(
        document["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| {
                node["id"]["kind"] == "domain"
                    && node["definition"]["kind"] == "domain"
                    && node["definition"]["domain"]["kind"] == "cartesian-boundary"
                    && node["definition"]["domain"]["axis"] == axis
                    && node["definition"]["domain"]["side"] == side
            })
            .map(|node| node["id"]["ulid"].as_str().unwrap().to_owned()),
        "Cartesian boundary Domain",
    )
}

fn unique_relation_on(document: &Value, domain: &str, predicate: fn(&Value) -> bool) -> String {
    let candidates = document["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| {
            edge["kind"] == "applies-on"
                && edge["to"]["kind"] == "domain"
                && edge["to"]["ulid"] == domain
                && edge["from"]["kind"] == "relation"
        })
        .map(|edge| edge["from"]["ulid"].as_str().unwrap().to_owned())
        .filter(|id| predicate(relation(document, id)))
        .collect::<Vec<_>>();
    exactly_one(candidates, "structurally matching Relation")
}

fn trace_only_relation(node: &Value) -> bool {
    let residual = &node["definition"]["residuals"];
    residual["roots"] == json!([0])
        && residual["nodes"]
            .as_array()
            .is_some_and(|nodes| nodes.len() == 1)
        && residual["nodes"][0]["op"] == "symbol"
        && residual["nodes"][0]["symbol"]["kind"] == "port-trace"
        && residual["nodes"][0]["symbol"]["id"]["kind"] == "port"
}

fn live_self_cancellation_relation(node: &Value) -> bool {
    let residual = &node["definition"]["residuals"];
    let nodes = residual["nodes"].as_array().unwrap();
    if nodes.len() != 6 || residual["roots"] != json!([2, 5]) {
        return false;
    }
    let port = &nodes[0]["symbol"]["id"];
    nodes[0]["op"] == "symbol"
        && nodes[0]["symbol"]["kind"] == "port-trace"
        && port["kind"] == "port"
        && nodes[1] == nodes[0]
        && nodes[2] == json!({"op":"sub","left":0,"right":1})
        && nodes[3]["op"] == "symbol"
        && nodes[3]["symbol"]["kind"] == "port-flux"
        && nodes[3]["symbol"]["id"] == *port
        && nodes[4] == nodes[3]
        && nodes[5] == json!({"op":"sub","left":3,"right":4})
}

fn unique_relation_port(document: &Value, relation_id: &str) -> String {
    let edge_port = |kind: &str| {
        exactly_one(
            document["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|edge| {
                    edge["kind"] == kind
                        && edge["from"]["kind"] == "relation"
                        && edge["from"]["ulid"] == relation_id
                        && edge["to"]["kind"] == "port"
                })
                .map(|edge| edge["to"]["ulid"].as_str().unwrap().to_owned()),
            "Relation Port edge",
        )
    };
    let dependency = edge_port("depends-on");
    let owned = edge_port("has-port");
    assert_eq!(dependency, owned);
    dependency
}

fn relation<'a>(document: &'a Value, id: &str) -> &'a Value {
    exactly_one(
        document["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["id"]["kind"] == "relation" && node["id"]["ulid"] == id),
        "Relation node",
    )
}

fn relation_mut<'a>(document: &'a mut Value, id: &str) -> &'a mut Value {
    let nodes = document["nodes"].as_array_mut().unwrap();
    let matches = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node["id"]["kind"] == "relation" && node["id"]["ulid"] == id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    &mut nodes[matches[0]]
}

fn id_set(document: &Value) -> BTreeSet<String> {
    document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| format!("{}:{}", node["id"]["kind"], node["id"]["ulid"]))
        .collect()
}

fn realization_wire() -> Value {
    serde_json::from_slice(frozen(EXPECTED_REALIZATION)).unwrap()
}

fn decode_realization_value(value: Value) -> PrescribedDynamicSolidRealizationEnvelopeV1 {
    PrescribedDynamicSolidRealizationEnvelopeV1::from_json(
        &encode_like(EXPECTED_REALIZATION, &value),
        Default::default(),
    )
    .expect("the mutant remains locally canonical and detached")
}

fn assert_value_decode_rejected(value: Value) {
    assert_decode_rejected(&encode_like(EXPECTED_REALIZATION, &value));
}

fn assert_decode_rejected(bytes: &[u8]) {
    assert!(
        PrescribedDynamicSolidRealizationEnvelopeV1::from_json(bytes, Default::default()).is_err()
    );
}

fn mutate_driven_object(index: u64, mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut wire = realization_wire();
    let entries = wire["driven_total_displacement"].as_array_mut().unwrap();
    let before = entries.clone();
    let selected = unique_entry(entries, index);
    mutate(&mut entries[selected]);
    for position in 0..entries.len() {
        if position != selected {
            assert_eq!(entries[position], before[position]);
        }
    }
    encode_like(EXPECTED_REALIZATION, &wire)
}

fn replace_driven_object_raw(index: u64, replacement: &[u8]) -> Vec<u8> {
    let bytes = canonical_fixture(EXPECTED_REALIZATION);
    let wire = realization_wire();
    let entries = wire["driven_total_displacement"].as_array().unwrap();
    let selected = unique_entry(entries, index);
    let object = encode_like(EXPECTED_REALIZATION, &entries[selected]);
    let matches = bytes
        .windows(object.len())
        .enumerate()
        .filter(|(_, window)| *window == object)
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "one complete object is selected");
    let position = matches[0];
    let mut mutated = Vec::new();
    mutated.extend_from_slice(&bytes[..position]);
    mutated.extend_from_slice(replacement);
    mutated.extend_from_slice(&bytes[position + object.len()..]);
    assert_eq!(&mutated[..position], &bytes[..position]);
    assert_eq!(
        &mutated[position + replacement.len()..],
        &bytes[position + object.len()..]
    );
    mutated
}

fn unique_entry(entries: &[Value], index: u64) -> usize {
    let positions = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry["vertex_index"] == index)
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1, "selected vertex object must be unique");
    positions[0]
}

fn decode_block(bytes: &[u8]) -> DiscreteFieldEnvelopeV1 {
    DiscreteFieldEnvelopeV1::from_json(frozen(bytes), Default::default()).unwrap()
}

fn decode_snapshot(bytes: &[u8]) -> FieldSnapshotEnvelopeV1 {
    FieldSnapshotEnvelopeV1::from_json(frozen(bytes), Default::default()).unwrap()
}

fn decode_state(bytes: &[u8]) -> SpatialStateEnvelopeV1 {
    SpatialStateEnvelopeV1::from_json(frozen(bytes), Default::default()).unwrap()
}

fn validate_snapshot(
    fixture: &Fixture,
    realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
    snapshot: &FieldSnapshotEnvelopeV1,
    blocks: &[DiscreteFieldEnvelopeV1],
) -> Result<(), Diagnostic> {
    snapshot.validate_against_prescribed_dynamic_solid(
        &fixture.model,
        realization,
        &fixture.geometry,
        &fixture.correspondence,
        &fixture.mesh,
        blocks,
    )
}

fn validate_state(
    fixture: &Fixture,
    realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
    state: &SpatialStateEnvelopeV1,
    snapshots: &[FieldSnapshotEnvelopeV1],
) -> Result<(), Diagnostic> {
    state.validate_against_prescribed_dynamic_solid(
        &fixture.model,
        realization,
        &fixture.geometry,
        &fixture.correspondence,
        &fixture.mesh,
        snapshots,
    )
}

fn exact_mesh(minimum_mean_ratio: f64) -> SimplicialMeshEnvelopeV1 {
    SimplicialMeshEnvelopeV1::from_mesh(
        &SimplicialMesh::new(
            3,
            VERTICES.iter().map(|value| value.to_vec()).collect(),
            CELLS.iter().map(|value| value.to_vec()).collect(),
            MeshQualityGate::new(minimum_mean_ratio).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn tagged(bits: [[u64; 3]; 9]) -> Vec<(VertexId, [f64; 3])> {
    bits.into_iter()
        .enumerate()
        .map(|(index, value)| (VertexId::new(index), value.map(f64::from_bits)))
        .collect()
}

fn assert_coefficient_bits(actual: &[(VertexId, [f64; 3])], expected: &[[u64; 3]; 9]) {
    assert_eq!(actual.len(), expected.len());
    for (index, ((vertex, value), expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(vertex.index(), index);
        assert_eq!(value.map(f64::to_bits), *expected);
    }
}

fn assert_flat_bits(actual: &[f64], expected: &[[u64; 3]; 9]) {
    assert_eq!(
        actual
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected.iter().flatten().copied().collect::<Vec<_>>()
    );
}

fn frozen(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

#[derive(Debug)]
enum JsonTemplate {
    Leaf,
    Array(Vec<JsonTemplate>),
    Object(Vec<(String, JsonTemplate)>),
}

impl<'de> Deserialize<'de> for JsonTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonTemplateVisitor)
    }
}

struct JsonTemplateVisitor;

impl<'de> Visitor<'de> for JsonTemplateVisitor {
    type Value = JsonTemplate;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(JsonTemplate::Leaf)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut entries = Vec::new();
        while let Some(entry) = sequence.next_element()? {
            entries.push(entry);
        }
        Ok(JsonTemplate::Array(entries))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key()? {
            entries.push((key, map.next_value()?));
        }
        Ok(JsonTemplate::Object(entries))
    }
}

fn canonical_fixture(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, b'\n' | b'\r'))
        .collect()
}

fn encode_like(template_bytes: &[u8], value: &Value) -> Vec<u8> {
    let compact = canonical_fixture(template_bytes);
    let template: JsonTemplate = serde_json::from_slice(&compact).unwrap();
    let mut catalog = BTreeMap::new();
    collect_key_orders(&template, &mut catalog);
    let mut encoded = Vec::new();
    encode_ordered(value, Some(&template), &catalog, &mut encoded);
    encoded
}

fn collect_key_orders(template: &JsonTemplate, catalog: &mut BTreeMap<Vec<String>, Vec<String>>) {
    match template {
        JsonTemplate::Leaf => {}
        JsonTemplate::Array(entries) => {
            for entry in entries {
                collect_key_orders(entry, catalog);
            }
        }
        JsonTemplate::Object(entries) => {
            let order = entries
                .iter()
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            let mut signature = order.clone();
            signature.sort();
            catalog.entry(signature).or_insert(order);
            for (_, entry) in entries {
                collect_key_orders(entry, catalog);
            }
        }
    }
}

fn encode_ordered(
    value: &Value,
    template: Option<&JsonTemplate>,
    catalog: &BTreeMap<Vec<String>, Vec<String>>,
    output: &mut Vec<u8>,
) {
    match value {
        Value::Array(entries) => {
            output.push(b'[');
            let templates = match template {
                Some(JsonTemplate::Array(entries)) => Some(entries.as_slice()),
                _ => None,
            };
            for (index, entry) in entries.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                let child =
                    templates.and_then(|entries| entries.get(index).or_else(|| entries.last()));
                encode_ordered(entry, child, catalog, output);
            }
            output.push(b']');
        }
        Value::Object(entries) => {
            output.push(b'{');
            let mut signature = entries.keys().cloned().collect::<Vec<_>>();
            signature.sort();
            let template_entries = match template {
                Some(JsonTemplate::Object(entries)) => Some(entries.as_slice()),
                _ => None,
            };
            let template_order = template_entries.map(|entries| {
                entries
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>()
            });
            let exact_template = template_order.as_ref().is_some_and(|order| {
                let mut keys = order.clone();
                keys.sort();
                keys == signature
            });
            let mut order = if exact_template {
                template_order.unwrap()
            } else if let Some(order) = catalog.get(&signature) {
                order.clone()
            } else if let Some(order) = template_order {
                order
                    .into_iter()
                    .filter(|key| entries.contains_key(key))
                    .collect()
            } else {
                Vec::new()
            };
            for key in entries.keys() {
                if !order.contains(key) {
                    order.push(key.clone());
                }
            }
            for (index, key) in order.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).unwrap();
                output.push(b':');
                let child = template_entries.and_then(|entries| {
                    entries
                        .iter()
                        .find(|(candidate, _)| candidate == key)
                        .map(|(_, value)| value)
                });
                encode_ordered(&entries[key], child, catalog, output);
            }
            output.push(b'}');
        }
        _ => serde_json::to_writer(&mut *output, value).unwrap(),
    }
}

fn move_first_top_level_member_to_end(fixture: &[u8]) -> Vec<u8> {
    let bytes = canonical_fixture(fixture);
    let mut depth = 0_u32;
    let mut quoted = false;
    let mut escaped = false;
    let mut separator = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b',' if depth == 1 => {
                separator = Some(index);
                break;
            }
            _ => {}
        }
    }
    let separator = separator.expect("the fixture has more than one top-level member");
    let mut reordered = Vec::with_capacity(bytes.len());
    reordered.push(b'{');
    reordered.extend_from_slice(&bytes[separator + 1..bytes.len() - 1]);
    reordered.push(b',');
    reordered.extend_from_slice(&bytes[1..separator]);
    reordered.push(b'}');
    reordered
}

fn assert_exact_json(actual: Vec<u8>, expected: &[u8]) {
    assert_eq!(actual, canonical_fixture(expected));
}

fn domain(document: &ModelDocument, name: &str) -> Id<kinds::Domain> {
    document.aliases()[name].downcast().unwrap()
}

fn exactly_one<T>(items: impl IntoIterator<Item = T>, label: &str) -> T {
    let mut items = items.into_iter();
    let first = items.next().unwrap_or_else(|| panic!("missing {label}"));
    assert!(items.next().is_none(), "{label} is not structurally unique");
    first
}

#[derive(Debug)]
struct RejectAssembly;

impl AssemblyBackend for RejectAssembly {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        Err(Diagnostic::error(
            eqiora::diagnostic::codes::ASSEMBLY_FAILED,
            "oracle-injected assembly failure",
        ))
    }
}

#[derive(Debug)]
struct RejectSolver;

impl LinearSolverBackend for RejectSolver {
    fn provider(&self) -> SolverProvider {
        REFERENCE_LINEAR_SOLVER.provider()
    }

    fn capabilities(&self) -> SolverCapabilities {
        REFERENCE_LINEAR_SOLVER.capabilities()
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        Err(Diagnostic::error(
            eqiora::diagnostic::codes::NUMERICAL_SOLVE_FAILED,
            "oracle-injected solver failure",
        ))
    }
}

const MUTATED_EXECUTION: ExecutionId = ExecutionId::new("eqiora.oracle.non-serial");
const MUTATED_EXECUTION_PROVIDER: ExecutionProvider =
    ExecutionProvider::new(MUTATED_EXECUTION, env!("CARGO_PKG_VERSION"), &[]);
const MUTATED_SOLVER_PROVIDER: SolverProvider = SolverProvider::new(
    BackendId::new("eqiora.oracle.substituted-solver"),
    env!("CARGO_PKG_VERSION"),
    &[],
);

fn non_serial_execution_report() -> ExecutionReport {
    ExecutionReport::host(MUTATED_EXECUTION, NonZeroUsize::new(2).unwrap())
}

#[derive(Debug, Clone, Copy)]
enum SolverEvidenceMutation {
    Provider,
    Plan(SolverPlan),
    Execution,
    Verification,
}

#[derive(Debug)]
struct MutatedSolverEvidence(SolverEvidenceMutation);

impl LinearSolverBackend for MutatedSolverEvidence {
    fn provider(&self) -> SolverProvider {
        REFERENCE_SOLVER_PROVIDER
    }

    fn capabilities(&self) -> SolverCapabilities {
        REFERENCE_LINEAR_SOLVER.capabilities()
    }

    fn solve_with_execution(
        &self,
        problem: &LinearProblem<'_>,
        plan: SolverPlan,
        execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        let produced = REFERENCE_LINEAR_SOLVER.solve_with_execution(problem, plan, execution)?;
        let (values, report) = produced.into_parts();
        match self.0 {
            SolverEvidenceMutation::Provider => accept_linear_solution_with_verifier(
                problem,
                plan,
                MUTATED_SOLVER_PROVIDER,
                report.execution_provider(),
                report.execution(),
                report.reason(),
                report.completed_iterations(),
                report.reported_residual_norm(),
                values,
                &SERIAL_LINEAR_EXECUTION,
            ),
            SolverEvidenceMutation::Plan(mutated) => accept_linear_solution(
                problem,
                mutated,
                REFERENCE_SOLVER_PROVIDER,
                report.reason(),
                report.completed_iterations(),
                report.reported_residual_norm(),
                values,
            ),
            SolverEvidenceMutation::Execution => accept_linear_solution_with_verifier(
                problem,
                plan,
                REFERENCE_SOLVER_PROVIDER,
                MUTATED_EXECUTION_PROVIDER,
                non_serial_execution_report(),
                report.reason(),
                report.completed_iterations(),
                report.reported_residual_norm(),
                values,
                &SERIAL_LINEAR_EXECUTION,
            ),
            SolverEvidenceMutation::Verification => accept_linear_solution_with_verifier(
                problem,
                plan,
                REFERENCE_SOLVER_PROVIDER,
                SERIAL_EXECUTION_PROVIDER,
                ExecutionReport::host_serial(),
                report.reason(),
                report.completed_iterations(),
                report.reported_residual_norm(),
                values,
                &NonSerialVerifier,
            ),
        }
    }
}

#[derive(Debug)]
struct NonSerialVerifier;

impl ReplicatedLinearExecution for NonSerialVerifier {
    fn provider(&self) -> ExecutionProvider {
        MUTATED_EXECUTION_PROVIDER
    }

    fn report(&self) -> ExecutionReport {
        non_serial_execution_report()
    }

    fn require_reduction(&self, policy: ReductionPolicy) -> Result<(), Diagnostic> {
        SERIAL_LINEAR_EXECUTION.require_reduction(policy)
    }

    fn apply(
        &self,
        operator: &dyn LinearOperator,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        SERIAL_LINEAR_EXECUTION.apply(operator, input, output)
    }

    fn inner_product(&self, action: FixedOrderInnerProduct<'_>) -> Result<f64, Diagnostic> {
        SERIAL_LINEAR_EXECUTION.inner_product(action)
    }
}
