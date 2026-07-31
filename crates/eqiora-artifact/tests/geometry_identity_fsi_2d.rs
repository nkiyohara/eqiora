use std::collections::BTreeSet;

use eqiora_artifact::{
    GeometryAssociationArtifactError, GeometryDecoderLimits, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, GeometryRevisionAssociationEnvelopeV1, ModelEnvelopeV1,
    ModelEnvelopeV2, ModelEnvelopeV3, ModelEnvelopeV4, ModelEnvelopeV5,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
};
use eqiora_compiler::compile;
use eqiora_core::Id;
use eqiora_core::entity::kinds;
use eqiora_geometry::{BodyAssociationCandidate, RetentionRejection};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore};
use eqiora_meshing::{MeshQualityGate, MeshTopology, SimplicialMesh};
use eqiora_schema::kernel::{BoundarySide, DomainKind, KernelNode};
use eqiora_sem::KernelProgram;

const MODEL: &str = include_str!(
    "../../../verify/geometry/fixed-reference-interface-identity-2d/models/interface.eqi"
);

const REPLAYABLE_GEOMETRY_MODEL: &str = r#"
model Main {
  domain left = box(0, 1, 0, 1);
  domain left_x_lower = boundary(left, axis = 0, side = lower);
  domain left_x_upper = boundary(left, axis = 0, side = upper);
  domain left_y_lower = boundary(left, axis = 1, side = lower);
  domain left_y_upper = boundary(left, axis = 1, side = upper);

  domain right = box(1, 2, 0, 1);
  domain right_x_lower = boundary(right, axis = 0, side = lower);
  domain right_x_upper = boundary(right, axis = 0, side = upper);
  domain right_y_lower = boundary(right, axis = 1, side = lower);
  domain right_y_upper = boundary(right, axis = 1, side = upper);

  representation scalar_space = continuum;
  field marker on left as scalar_space: 1 = 0;
  relation retain continuous on left {
    marker = 0;
  }
}
"#;

#[test]
fn geometry_replays_every_explicit_model_wire_without_erasing_identity() {
    let (program, bodies) = replayable_geometry_program();
    let mesh = mesh(2.0);
    let v1 = ModelEnvelopeV1::from_program(&program).unwrap();
    let v2 = ModelEnvelopeV2::from_program(&program).unwrap();
    let v3 = ModelEnvelopeV3::from_program(&program).unwrap();
    let v4 = ModelEnvelopeV4::from_program(&program).unwrap();
    let v5 = ModelEnvelopeV5::from_program(&program).unwrap();

    let v1 = ModelEnvelopeV1::from_json(&v1.canonical_json().unwrap(), Default::default()).unwrap();
    let v2 = ModelEnvelopeV2::from_json(&v2.canonical_json().unwrap(), Default::default()).unwrap();
    let v3 = ModelEnvelopeV3::from_json(&v3.canonical_json().unwrap(), Default::default()).unwrap();
    let v4 = ModelEnvelopeV4::from_json(&v4.canonical_json().unwrap(), Default::default()).unwrap();
    let v5 = ModelEnvelopeV5::from_json(&v5.canonical_json().unwrap(), Default::default()).unwrap();

    let first = replay_geometry(&v1, bodies, &mesh);
    let second = replay_geometry(&v2, bodies, &mesh);
    let third = replay_geometry(&v3, bodies, &mesh);
    let fourth = replay_geometry(&v4, bodies, &mesh);
    let fifth = replay_geometry(&v5, bodies, &mesh);
    for candidate in [&second, &third, &fourth, &fifth] {
        assert_eq!(first.0.bodies(), candidate.0.bodies());
        assert_eq!(first.0.boundaries(), candidate.0.boundaries());
        for body in bodies {
            assert_eq!(first.1.body_cells(body), candidate.1.body_cells(body));
        }
    }

    let model_digests = [
        first.0.model_artifact(),
        second.0.model_artifact(),
        third.0.model_artifact(),
        fourth.0.model_artifact(),
        fifth.0.model_artifact(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(model_digests.len(), 5, "wire-domain identity remains exact");
    assert!(first.0.validate_against(&v2).is_err());
    assert!(second.0.validate_against(&v3).is_err());
    assert!(third.0.validate_against(&v4).is_err());
    assert!(fourth.0.validate_against(&v5).is_err());
}

#[test]
fn geometry_tolerance_owns_cartesian_classification_meaning() {
    let (program, bodies) = replayable_geometry_program();
    let model = ModelEnvelopeV1::from_program(&program).unwrap();
    let tight = GeometryIdentityEnvelopeV1::new(&model, bodies, 1.0e-9).unwrap();
    let loose = GeometryIdentityEnvelopeV1::new(&model, bodies, 1.0e-6).unwrap();
    assert_eq!(tight.bodies(), loose.bodies());
    assert_eq!(tight.boundaries(), loose.boundaries());
    assert_ne!(tight.digest().unwrap(), loose.digest().unwrap());

    let offset_mesh = mesh_with_interface(2.0, 1.0 + 1.0e-7);
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::new(&tight, &model, &offset_mesh).is_err(),
        "tight geometry classification must reject the displaced interface"
    );
    let admitted = GeometryMeshCorrespondenceEnvelopeV1::new(&loose, &model, &offset_mesh).unwrap();
    assert_eq!(admitted.geometry_artifact(), loose.digest().unwrap());
}

fn replay_geometry(
    model: &impl ReplayableCanonicalModelArtifact,
    bodies: [Id<kinds::Domain>; 2],
    mesh: &SimplicialMeshEnvelopeV1,
) -> (
    GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1,
) {
    let geometry = GeometryIdentityEnvelopeV1::new(model, bodies, 1.0e-12).unwrap();
    let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, model, mesh).unwrap();
    geometry.validate_against(model).unwrap();
    correspondence
        .validate_against(&geometry, model, mesh)
        .unwrap();
    (geometry, correspondence)
}

fn replayable_geometry_program() -> (KernelProgram, [Id<kinds::Domain>; 2]) {
    let compiled = compile("replayable-geometry.eqi", REPLAYABLE_GEOMETRY_MODEL)
        .unwrap()
        .remove(0);
    let (transaction, model_id, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model_id).unwrap();
    let mut bodies = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(definition)
                if matches!(definition.kind(), DomainKind::CartesianBox { .. }) =>
            {
                Some(definition.id())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    bodies.sort_by_key(Id::ulid);
    (program, [bodies[0], bodies[1]])
}

#[test]
fn geometry_identity_fsi_2d() {
    let source = fixture(MODEL, 2.0);
    let geometry = GeometryIdentityEnvelopeV1::new(
        &source.model,
        [source.bodies[1], source.bodies[0]],
        1.0e-12,
    )
    .unwrap();
    let geometry_reordered = GeometryIdentityEnvelopeV1::new(
        &source.model,
        [source.bodies[0], source.bodies[1]],
        1.0e-12,
    )
    .unwrap();
    assert_eq!(
        geometry.canonical_json().unwrap(),
        geometry_reordered.canonical_json().unwrap()
    );
    assert_eq!(geometry.bodies().len(), 2);
    assert_eq!(geometry.boundaries().len(), 8);
    assert_eq!(
        geometry
            .boundaries()
            .iter()
            .map(|boundary| boundary.entity())
            .collect::<BTreeSet<_>>()
            .len(),
        7,
        "the two semantic interface boundaries share one geometry entity"
    );

    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &source.model, &source.mesh).unwrap();
    assert_eq!(
        correspondence.body_cells(source.bodies[0]).unwrap().len(),
        4
    );
    assert_eq!(
        correspondence.body_cells(source.bodies[1]).unwrap().len(),
        4
    );
    let first_interface = boundary(&source.program, source.bodies[0], 0, BoundarySide::Upper);
    let second_interface = boundary(&source.program, source.bodies[1], 0, BoundarySide::Lower);
    let first_facets = correspondence.boundary_facets(first_interface).unwrap();
    let second_facets = correspondence.boundary_facets(second_interface).unwrap();
    assert_eq!(first_facets, second_facets);
    assert_eq!(first_facets.len(), 2);
    let witness = correspondence
        .derive_conserving_interface(&geometry, &source.model, &source.mesh, source.connection)
        .unwrap();
    assert_eq!(witness.model_artifact(), &geometry.model_artifact());
    assert_eq!(witness.geometry_artifact(), &geometry.digest().unwrap());
    assert_eq!(witness.mesh_artifact(), &source.mesh.digest().unwrap());
    assert_eq!(
        witness.correspondence_artifact(),
        &correspondence.digest().unwrap()
    );
    assert_eq!(
        witness
            .boundaries()
            .into_iter()
            .map(|id| id.ulid())
            .collect::<BTreeSet<_>>(),
        [first_interface, second_interface]
            .into_iter()
            .map(|id| id.ulid())
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        witness
            .parents()
            .into_iter()
            .map(|id| id.ulid())
            .collect::<BTreeSet<_>>(),
        source
            .bodies
            .into_iter()
            .map(|id| id.ulid())
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(witness.facet_indices(), first_facets);
    let interface_roles = geometry
        .boundaries()
        .into_iter()
        .filter(|boundary| witness.boundaries().contains(&boundary.domain()))
        .map(|boundary| (boundary.axis(), boundary.side()))
        .collect::<Vec<_>>();
    assert_eq!(interface_roles.len(), 2);
    assert_eq!(interface_roles[0].0, interface_roles[1].0);
    assert_ne!(interface_roles[0].1, interface_roles[1].1);

    let geometry_bytes = geometry.canonical_json().unwrap();
    let decoded_geometry =
        GeometryIdentityEnvelopeV1::from_json(&geometry_bytes, Default::default()).unwrap();
    decoded_geometry.validate_against(&source.model).unwrap();
    assert_eq!(
        decoded_geometry.digest().unwrap(),
        geometry.digest().unwrap()
    );
    let correspondence_bytes = correspondence.canonical_json().unwrap();
    let decoded_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_json(&correspondence_bytes, Default::default())
            .unwrap();
    decoded_correspondence
        .validate_against(&geometry, &source.model, &source.mesh)
        .unwrap();

    let target_source = MODEL.replace("box(1, 2, 0, 1)", "box(1, 3, 0, 1)");
    let target = fixture(&target_source, 3.0);
    assert_ne!(source.bodies, target.bodies);
    let target_geometry =
        GeometryIdentityEnvelopeV1::new(&target.model, target.bodies, 1.0e-12).unwrap();
    let target_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&target_geometry, &target.model, &target.mesh)
            .unwrap();
    let retained = GeometryRevisionAssociationEnvelopeV1::new(
        &source.model,
        &geometry,
        &correspondence,
        &source.mesh,
        &target.model,
        &target_geometry,
        &target_correspondence,
        &target.mesh,
        vec![
            BodyAssociationCandidate::new(source.bodies[1], target.bodies[1]),
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[0]),
        ],
    )
    .unwrap();
    retained
        .validate_against(
            &source.model,
            &geometry,
            &correspondence,
            &source.mesh,
            &target.model,
            &target_geometry,
            &target_correspondence,
            &target.mesh,
        )
        .unwrap();
    let retained_reordered = GeometryRevisionAssociationEnvelopeV1::new(
        &source.model,
        &geometry,
        &correspondence,
        &source.mesh,
        &target.model,
        &target_geometry,
        &target_correspondence,
        &target.mesh,
        vec![
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[0]),
            BodyAssociationCandidate::new(source.bodies[1], target.bodies[1]),
        ],
    )
    .unwrap();
    assert_eq!(
        retained.canonical_json().unwrap(),
        retained_reordered.canonical_json().unwrap()
    );

    assert_retention_rejection(
        AssociationSide::new(&source, &geometry, &correspondence),
        AssociationSide::new(&target, &target_geometry, &target_correspondence),
        vec![BodyAssociationCandidate::new(
            source.bodies[0],
            target.bodies[0],
        )],
        |error| matches!(error, RetentionRejection::Missing { .. }),
    );
    assert_retention_rejection(
        AssociationSide::new(&source, &geometry, &correspondence),
        AssociationSide::new(&target, &target_geometry, &target_correspondence),
        vec![
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[0]),
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[1]),
        ],
        |error| matches!(error, RetentionRejection::Split { .. }),
    );
    assert_retention_rejection(
        AssociationSide::new(&source, &geometry, &correspondence),
        AssociationSide::new(&target, &target_geometry, &target_correspondence),
        vec![
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[0]),
            BodyAssociationCandidate::new(source.bodies[1], target.bodies[0]),
        ],
        |error| matches!(error, RetentionRejection::Merged { .. }),
    );
    assert_retention_rejection(
        AssociationSide::new(&source, &geometry, &correspondence),
        AssociationSide::new(&target, &target_geometry, &target_correspondence),
        vec![
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[0]),
            BodyAssociationCandidate::new(source.bodies[0], target.bodies[1]),
            BodyAssociationCandidate::new(source.bodies[1], target.bodies[0]),
            BodyAssociationCandidate::new(source.bodies[1], target.bodies[1]),
        ],
        |error| matches!(error, RetentionRejection::Ambiguous { .. }),
    );

    let different_mesh = mesh(2.1);
    assert!(
        correspondence
            .validate_against(&geometry, &source.model, &different_mesh)
            .is_err()
    );
    let mut forged: serde_json::Value = serde_json::from_slice(&correspondence_bytes).unwrap();
    forged["boundaries"][0]["facet_indices"] = serde_json::json!([]);
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::from_json(
            &serde_json::to_vec(&forged).unwrap(),
            Default::default()
        )
        .is_err()
    );
    assert!(
        GeometryIdentityEnvelopeV1::from_json(
            &geometry_bytes,
            GeometryDecoderLimits {
                max_geometry_entities: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn geometry_identity_falsifiers_fail_closed() {
    let source = fixture(MODEL, 2.0);
    let geometry = GeometryIdentityEnvelopeV1::new(&source.model, source.bodies, 1.0e-12).unwrap();
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &source.model, &source.mesh).unwrap();
    let geometry_bytes = geometry.canonical_json().unwrap();
    let correspondence_bytes = correspondence.canonical_json().unwrap();
    let first_interface = boundary(&source.program, source.bodies[0], 0, BoundarySide::Upper);
    let second_interface = boundary(&source.program, source.bodies[1], 0, BoundarySide::Lower);
    let witness = correspondence
        .derive_conserving_interface(&geometry, &source.model, &source.mesh, source.connection)
        .unwrap();

    let mut stale_model_geometry: serde_json::Value =
        serde_json::from_slice(&geometry_bytes).unwrap();
    stale_model_geometry["model_sha256"] =
        serde_json::Value::String(correspondence.digest().unwrap().to_string());
    let stale_model_geometry = GeometryIdentityEnvelopeV1::from_json(
        &serde_json::to_vec(&stale_model_geometry).unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(
        stale_model_geometry
            .validate_against(&source.model)
            .is_err()
    );

    assert!(
        GeometryIdentityEnvelopeV1::new(&source.model, [first_interface], 1.0e-12).is_err(),
        "a boundary Domain cannot masquerade as a geometry body"
    );
    let line_mesh = SimplicialMesh::new(
        1,
        vec![vec![0.0], vec![1.0]],
        vec![vec![0, 1]],
        MeshQualityGate::new(0.05).unwrap(),
    )
    .unwrap();
    let line_mesh = SimplicialMeshEnvelopeV1::from_mesh(&line_mesh).unwrap();
    assert!(
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &source.model, &line_mesh).is_err(),
        "a wrong-dimensional mesh must fail before correspondence exposure"
    );

    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        wire["geometry_sha256"] =
            serde_json::Value::String(correspondence.digest().unwrap().to_string());
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        wire["mesh_sha256"] = serde_json::Value::String(geometry.digest().unwrap().to_string());
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        wire["bodies"][0]["domain_ulid"] =
            serde_json::Value::String(witness.connector().ulid().to_string());
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        wire["boundaries"][0]["parent_ulid"] =
            serde_json::Value::String(source.bodies[1].ulid().to_string());
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        wire["boundaries"][0]["domain_ulid"] =
            serde_json::Value::String(witness.connector().ulid().to_string());
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        let duplicate = wire["bodies"][0]["cell_indices"][0].clone();
        wire["bodies"][0]["cell_indices"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        let overlap = wire["bodies"][0]["cell_indices"][0].clone();
        let cells = wire["bodies"][1]["cell_indices"].as_array_mut().unwrap();
        cells.push(overlap);
        sort_json_u64(cells);
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        let cells = wire["bodies"][1]["cell_indices"].as_array_mut().unwrap();
        cells.push(serde_json::json!(u64::MAX));
        sort_json_u64(cells);
    });

    let used_facets = geometry
        .boundaries()
        .into_iter()
        .flat_map(|boundary| correspondence.boundary_facets(boundary.domain()).unwrap())
        .collect::<BTreeSet<_>>();
    let interior_facet = (0..source.mesh.mesh().entity_count(1).unwrap())
        .find(|index| !used_facets.contains(index))
        .unwrap();
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        let facets = wire["boundaries"][0]["facet_indices"]
            .as_array_mut()
            .unwrap();
        facets.push(serde_json::json!(interior_facet));
        sort_json_u64(facets);
    });
    assert_correspondence_mutation_rejected(&correspondence_bytes, &geometry, &source, |wire| {
        let boundaries = wire["boundaries"].as_array_mut().unwrap();
        let first = boundaries
            .iter_mut()
            .find(|boundary| boundary["domain_ulid"] == first_interface.ulid().to_string())
            .unwrap();
        first["facet_indices"].as_array_mut().unwrap().pop();
    });

    let same_side_model = MODEL.replace(
        "support face = solid_x_lower",
        "support face = solid_x_upper",
    );
    assert!(
        compile("same-side-interface.eqi", &same_side_model).is_err(),
        "equal-facing or noncoincident Connection sides must fail before Model exposure"
    );

    let shadow_model = MODEL.replace(
        "  domain fluid_x_upper = boundary(fluid, axis = 0, side = upper);",
        "  domain fluid_x_upper = boundary(fluid, axis = 0, side = upper);\n  domain shadow_fluid_x_upper = boundary(fluid, axis = 0, side = upper);",
    );
    let shadow = fixture(&shadow_model, 2.0);
    assert!(
        GeometryIdentityEnvelopeV1::new(&shadow.model, shadow.bodies, 1.0e-12).is_err(),
        "a coincident shadow Domain cannot duplicate one semantic boundary role"
    );

    assert_ne!(first_interface, second_interface);
}

fn assert_correspondence_mutation_rejected(
    bytes: &[u8],
    geometry: &GeometryIdentityEnvelopeV1,
    source: &Fixture,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let mut wire = serde_json::from_slice(bytes).unwrap();
    mutate(&mut wire);
    let bytes = serde_json::to_vec(&wire).unwrap();
    if let Ok(decoded) = GeometryMeshCorrespondenceEnvelopeV1::from_json(&bytes, Default::default())
    {
        assert!(
            decoded
                .validate_against(geometry, &source.model, &source.mesh)
                .is_err(),
            "a forged correspondence survived exact replay"
        );
    }
}

fn sort_json_u64(values: &mut [serde_json::Value]) {
    values.sort_by_key(|value| value.as_u64().unwrap());
}

#[derive(Clone, Copy)]
struct AssociationSide<'a> {
    fixture: &'a Fixture,
    geometry: &'a GeometryIdentityEnvelopeV1,
    correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
}

impl<'a> AssociationSide<'a> {
    const fn new(
        fixture: &'a Fixture,
        geometry: &'a GeometryIdentityEnvelopeV1,
        correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Self {
        Self {
            fixture,
            geometry,
            correspondence,
        }
    }
}

fn assert_retention_rejection(
    source: AssociationSide<'_>,
    target: AssociationSide<'_>,
    candidates: Vec<BodyAssociationCandidate>,
    predicate: impl FnOnce(&RetentionRejection) -> bool,
) {
    let error = GeometryRevisionAssociationEnvelopeV1::new(
        &source.fixture.model,
        source.geometry,
        source.correspondence,
        &source.fixture.mesh,
        &target.fixture.model,
        target.geometry,
        target.correspondence,
        &target.fixture.mesh,
        candidates,
    )
    .unwrap_err();
    match error {
        GeometryAssociationArtifactError::Retention(error) => assert!(predicate(&error)),
        GeometryAssociationArtifactError::Artifact(error) => {
            panic!("expected retention rejection, got {error}")
        }
    }
}

struct Fixture {
    model: ModelEnvelopeV4,
    program: KernelProgram,
    mesh: SimplicialMeshEnvelopeV1,
    bodies: [Id<kinds::Domain>; 2],
    connection: Id<kinds::Connection>,
}

fn fixture(source: &str, right_upper: f64) -> Fixture {
    let compiled = compile("geometry-interface.eqi", source).unwrap().remove(0);
    let (transaction, model_id, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model_id).unwrap();
    let model = ModelEnvelopeV4::from_program(&program).unwrap();
    let mut bodies = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Domain(definition)
                if matches!(definition.kind(), DomainKind::CartesianBox { .. }) =>
            {
                let bounds = program
                    .resolved_cartesian_bounds(definition.id())
                    .expect("accepted Cartesian bounds");
                Some((bounds[0].lower().value(), definition.id()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    bodies.sort_by(|left, right| left.0.total_cmp(&right.0));
    let connection = program
        .nodes()
        .find_map(|node| match node {
            KernelNode::Connection(definition) => Some(definition.id()),
            _ => None,
        })
        .unwrap();
    Fixture {
        model,
        program,
        mesh: mesh(right_upper),
        bodies: [bodies[0].1, bodies[1].1],
        connection,
    }
}

fn boundary(
    program: &KernelProgram,
    parent: Id<kinds::Domain>,
    axis: usize,
    side: BoundarySide,
) -> Id<kinds::Domain> {
    program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.to() == parent.erase())
        .find_map(|edge| match program.node(edge.from()) {
            Some(KernelNode::Domain(definition))
                if definition.kind() == &DomainKind::CartesianBoundary { axis, side } =>
            {
                Some(definition.id())
            }
            _ => None,
        })
        .unwrap()
}

fn mesh(right_upper: f64) -> SimplicialMeshEnvelopeV1 {
    mesh_with_interface(right_upper, 1.0)
}

fn mesh_with_interface(right_upper: f64, interface: f64) -> SimplicialMeshEnvelopeV1 {
    let mesh = SimplicialMesh::new(
        2,
        vec![
            vec![0.0, 0.0],
            vec![interface, 0.0],
            vec![0.0, 0.5],
            vec![interface, 0.5],
            vec![0.0, 1.0],
            vec![interface, 1.0],
            vec![right_upper, 0.0],
            vec![right_upper, 0.5],
            vec![right_upper, 1.0],
        ],
        vec![
            vec![0, 1, 3],
            vec![0, 3, 2],
            vec![2, 3, 5],
            vec![2, 5, 4],
            vec![1, 6, 7],
            vec![1, 7, 3],
            vec![3, 7, 8],
            vec![3, 8, 5],
        ],
        MeshQualityGate::new(0.05).unwrap(),
    )
    .unwrap();
    SimplicialMeshEnvelopeV1::from_mesh(&mesh).unwrap()
}
