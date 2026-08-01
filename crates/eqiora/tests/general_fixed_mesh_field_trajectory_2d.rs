use std::collections::BTreeMap;

use eqiora::api::{
    FixedMeshFieldTrajectoryReplay2dV1, FixedReferenceFsiResult2d,
    snapshot_fixed_reference_fsi_solution_v1,
};
use eqiora::artifact::{
    ArtifactDigest, DiscreteFieldEnvelopeV1, FieldSnapshotEnvelopeV1, RunManifestV2,
    SimplicialMeshEnvelopeV1, SpatialStateEnvelopeV1, SpatialTrajectoryEnvelopeV1,
    SpatialTrajectorySegmentEnvelopeV1, ValidatedFixedSpatialContextV1,
};
use eqiora::meshing::{DiscreteFieldAssociation, MeshQualityGate, SimplicialMesh};
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use support::fixed_reference_fsi::direct_document;

mod support;

struct AcceptedCatalog {
    result: FixedReferenceFsiResult2d,
    segments: Vec<SpatialTrajectorySegmentEnvelopeV1>,
    snapshots: Vec<FieldSnapshotEnvelopeV1>,
    blocks: Vec<DiscreteFieldEnvelopeV1>,
}

#[test]
fn complete_product_trajectory_replays_independently_of_catalog_order() {
    let catalog = accepted_catalog();
    let result = &catalog.result;
    let product_replay = result
        .trajectory_replay()
        .expect("the ordinary FSI result retains every durable dependency");
    assert_eq!(
        product_replay.trajectory().digest().unwrap(),
        result.trajectory().digest().unwrap()
    );

    let context = fixed_context(result);
    let expected_fields = context
        .represented_fields()
        .iter()
        .map(|entry| entry.field())
        .collect::<Vec<_>>();
    let dt = result
        .realization()
        .plan()
        .unwrap()
        .time_step()
        .duration()
        .value();
    assert_eq!(
        result
            .states()
            .iter()
            .map(SpatialStateEnvelopeV1::step)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        result
            .states()
            .iter()
            .map(SpatialStateEnvelopeV1::time_s)
            .collect::<Vec<_>>(),
        vec![dt, 2.0 * dt]
    );
    assert_eq!(expected_fields.len(), 4);
    assert_eq!(result.trajectory().fields(), expected_fields);
    for state in result.states() {
        assert_eq!(
            state
                .fields()
                .into_iter()
                .map(|(_, field, _)| field)
                .collect::<Vec<_>>(),
            expected_fields
        );
    }

    let snapshots_by_digest = catalog
        .snapshots
        .iter()
        .map(|snapshot| (snapshot.digest().unwrap(), snapshot))
        .collect::<BTreeMap<_, _>>();
    let fluid_velocity = result.solutions()[0].fields().fluid_velocity();
    for state in result.states() {
        for (_, field, snapshot_digest) in state.fields() {
            let associations = snapshots_by_digest[&snapshot_digest]
                .block_artifacts()
                .into_iter()
                .map(|(association, _)| association)
                .collect::<Vec<_>>();
            if field == fluid_velocity {
                assert_eq!(
                    associations,
                    vec![
                        DiscreteFieldAssociation::Vertex,
                        DiscreteFieldAssociation::Cell,
                    ]
                );
            } else {
                assert_eq!(associations, vec![DiscreteFieldAssociation::Vertex]);
            }
        }
    }

    let first_root = SpatialTrajectoryEnvelopeV1::start(&context, &catalog.segments[0]).unwrap();
    assert_eq!(result.trajectory().generation(), 1);
    assert_eq!(
        result.trajectory().previous_root(),
        Some(first_root.digest().unwrap())
    );
    assert_eq!(
        result.trajectory().segment_artifacts(),
        catalog
            .segments
            .iter()
            .map(|segment| segment.digest().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result.run().outputs(),
        vec![result.trajectory().digest().unwrap()]
    );
    assert_eq!(result.mesh_artifact().dimension(), 2);
    assert_unique_catalog(
        &catalog.segments,
        SpatialTrajectorySegmentEnvelopeV1::digest,
    );
    assert_unique_catalog(result.states(), SpatialStateEnvelopeV1::digest);
    assert_unique_catalog(&catalog.snapshots, FieldSnapshotEnvelopeV1::digest);
    assert_unique_catalog(&catalog.blocks, DiscreteFieldEnvelopeV1::digest);

    let mut segments = catalog.segments.clone();
    let mut states = result.states().to_vec();
    let mut snapshots = catalog.snapshots.clone();
    let mut blocks = catalog.blocks.clone();
    segments.reverse();
    states.reverse();
    snapshots.reverse();
    blocks.reverse();
    let replay = FixedMeshFieldTrajectoryReplay2dV1::new(
        result.model(),
        result.realization(),
        result.geometry(),
        result.correspondence(),
        result.mesh_artifact(),
        result.trajectory(),
        &segments,
        &states,
        &snapshots,
        &blocks,
        result.run(),
    )
    .expect("catalog declaration order is not trajectory identity");
    assert_eq!(
        replay.trajectory().digest().unwrap(),
        result.trajectory().digest().unwrap()
    );
}

#[test]
fn missing_duplicate_unused_and_wrong_run_dependencies_fail_closed() {
    let catalog = accepted_catalog();
    let result = &catalog.result;

    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments[..1],
            result.states(),
            &catalog.snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "a missing segment must fail"
    );
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            &result.states()[..1],
            &catalog.snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "a missing state must fail"
    );
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &catalog.snapshots[..catalog.snapshots.len() - 1],
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "a missing snapshot must fail"
    );
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &catalog.snapshots,
            &catalog.blocks[..catalog.blocks.len() - 1],
            result.run(),
        )
        .is_err(),
        "a missing numerical block must fail"
    );

    let mut duplicate_snapshots = catalog.snapshots.clone();
    duplicate_snapshots.push(duplicate_snapshots[0].clone());
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &duplicate_snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "a duplicate catalog identity must fail"
    );

    let mut unused_snapshots = catalog.snapshots.clone();
    let mut extra_json: serde_json::Value =
        serde_json::from_slice(&unused_snapshots[0].canonical_json().unwrap()).unwrap();
    extra_json["physical"]["frame"] = if extra_json["physical"]["frame"] == "invariant" {
        "spatial-cartesian".into()
    } else {
        "invariant".into()
    };
    unused_snapshots.push(
        FieldSnapshotEnvelopeV1::from_json(
            &serde_json::to_vec(&extra_json).unwrap(),
            Default::default(),
        )
        .expect("the extra snapshot is locally valid but not referenced"),
    );
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &unused_snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "an unused catalog declaration must fail"
    );

    let mut empty_run_json: serde_json::Value =
        serde_json::from_slice(&result.run().canonical_json().unwrap()).unwrap();
    empty_run_json["output_sha256"] = serde_json::json!([]);
    let empty_run = RunManifestV2::from_json(
        &serde_json::to_vec(&empty_run_json).unwrap(),
        Default::default(),
    )
    .expect("an output-empty Run is locally valid");
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &catalog.snapshots,
            &catalog.blocks,
            &empty_run,
        )
        .is_err(),
        "the Run must name the trajectory root"
    );

    let mut extra_run_json: serde_json::Value =
        serde_json::from_slice(&result.run().canonical_json().unwrap()).unwrap();
    let outputs = extra_run_json["output_sha256"].as_array_mut().unwrap();
    outputs.push(result.model().digest().unwrap().to_string().into());
    outputs.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    let extra_run = RunManifestV2::from_json(
        &serde_json::to_vec(&extra_run_json).unwrap(),
        Default::default(),
    )
    .expect("a Run with two unique sorted outputs is locally valid");
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &catalog.snapshots,
            &catalog.blocks,
            &extra_run,
        )
        .is_err(),
        "this profile rejects a Run output superset"
    );
}

#[test]
fn dimensional_physical_and_immutable_prefix_drift_fail_closed() {
    let catalog = accepted_catalog();
    let result = &catalog.result;

    let one_dimensional = SimplicialMeshEnvelopeV1::from_mesh(
        &SimplicialMesh::new(
            1,
            vec![vec![0.0], vec![1.0]],
            vec![vec![0, 1]],
            MeshQualityGate::new(0.05).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        replay(
            &catalog,
            &one_dimensional,
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &catalog.snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "the published profile is explicitly two-dimensional"
    );

    let mut root_json: serde_json::Value =
        serde_json::from_slice(&result.trajectory().canonical_json().unwrap()).unwrap();
    root_json["previous_root_sha256"] = result.model().digest().unwrap().to_string().into();
    let mutated_root = SpatialTrajectoryEnvelopeV1::from_json(
        &serde_json::to_vec(&root_json).unwrap(),
        Default::default(),
    )
    .expect("a foreign prior-root digest is locally well formed");
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            &mutated_root,
            &catalog.segments,
            result.states(),
            &catalog.snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "the immutable previous-root prefix is exact"
    );

    let mut substituted_snapshots = catalog.snapshots.clone();
    let mut snapshot_json: serde_json::Value =
        serde_json::from_slice(&substituted_snapshots[0].canonical_json().unwrap()).unwrap();
    snapshot_json["physical"]["frame"] = if snapshot_json["physical"]["frame"] == "invariant" {
        "spatial-cartesian".into()
    } else {
        "invariant".into()
    };
    substituted_snapshots[0] = FieldSnapshotEnvelopeV1::from_json(
        &serde_json::to_vec(&snapshot_json).unwrap(),
        Default::default(),
    )
    .expect("a locally valid physical-frame substitution remains untrusted");
    assert!(
        replay(
            &catalog,
            result.mesh_artifact(),
            result.trajectory(),
            &catalog.segments,
            result.states(),
            &substituted_snapshots,
            &catalog.blocks,
            result.run(),
        )
        .is_err(),
        "physical Field meaning is replayed rather than trusted"
    );

    let mut substituted_blocks = catalog.blocks.clone();
    let mut block_json: serde_json::Value =
        serde_json::from_slice(&substituted_blocks[0].canonical_json().unwrap()).unwrap();
    block_json["association"] = if block_json["association"] == "vertex" {
        "cell".into()
    } else {
        "vertex".into()
    };
    let changed_block = DiscreteFieldEnvelopeV1::from_json(
        &serde_json::to_vec(&block_json).unwrap(),
        Default::default(),
    );
    if let Ok(changed_block) = changed_block {
        substituted_blocks[0] = changed_block;
        assert!(
            replay(
                &catalog,
                result.mesh_artifact(),
                result.trajectory(),
                &catalog.segments,
                result.states(),
                &catalog.snapshots,
                &substituted_blocks,
                result.run(),
            )
            .is_err(),
            "block association and exact content identity are both closed"
        );
    }
}

fn accepted_catalog() -> AcceptedCatalog {
    let document = direct_document();
    let result = FixedReferenceFsiResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
        .expect("the existing verified 2D FSI family produces one ordinary result");
    let context = fixed_context(&result);
    let snapshot_sets = result
        .solutions()
        .iter()
        .map(|solution| {
            snapshot_fixed_reference_fsi_solution_v1(&context, solution)
                .expect("accepted solution projects through the ordinary Field path")
        })
        .collect::<Vec<_>>();
    let snapshots = unique_catalog(
        snapshot_sets
            .iter()
            .flat_map(|set| set.snapshots().iter().cloned()),
        FieldSnapshotEnvelopeV1::digest,
    );
    let blocks = unique_catalog(
        snapshot_sets.iter().flat_map(|set| {
            set.snapshots().iter().flat_map(|snapshot| {
                set.blocks(snapshot.field())
                    .expect("every snapshot retains its exact blocks")
                    .iter()
                    .cloned()
            })
        }),
        DiscreteFieldEnvelopeV1::digest,
    );
    let segments = result
        .states()
        .iter()
        .map(|state| {
            SpatialTrajectorySegmentEnvelopeV1::new(&context, std::slice::from_ref(state))
                .expect("one accepted state forms one immutable segment")
        })
        .collect::<Vec<_>>();
    let replayed_first = SpatialTrajectoryEnvelopeV1::start(&context, &segments[0]).unwrap();
    let replayed_final =
        SpatialTrajectoryEnvelopeV1::extend(&context, &replayed_first, &segments[1]).unwrap();
    assert_eq!(
        replayed_final.digest().unwrap(),
        result.trajectory().digest().unwrap(),
        "the general case consumes the existing scientific trajectory identity"
    );
    AcceptedCatalog {
        result,
        segments,
        snapshots,
        blocks,
    }
}

fn fixed_context(result: &FixedReferenceFsiResult2d) -> ValidatedFixedSpatialContextV1<'_> {
    ValidatedFixedSpatialContextV1::new(
        result.model(),
        result.realization(),
        result.geometry(),
        result.correspondence(),
        result.mesh_artifact(),
    )
    .expect("the ordinary result closes one fixed spatial context")
}

#[allow(clippy::too_many_arguments)]
fn replay<'a>(
    catalog: &'a AcceptedCatalog,
    mesh: &'a SimplicialMeshEnvelopeV1,
    trajectory: &'a SpatialTrajectoryEnvelopeV1,
    segments: &'a [SpatialTrajectorySegmentEnvelopeV1],
    states: &'a [SpatialStateEnvelopeV1],
    snapshots: &'a [FieldSnapshotEnvelopeV1],
    blocks: &'a [DiscreteFieldEnvelopeV1],
    run: &'a RunManifestV2,
) -> Result<FixedMeshFieldTrajectoryReplay2dV1<'a>, eqiora::Diagnostic> {
    FixedMeshFieldTrajectoryReplay2dV1::new(
        catalog.result.model(),
        catalog.result.realization(),
        catalog.result.geometry(),
        catalog.result.correspondence(),
        mesh,
        trajectory,
        segments,
        states,
        snapshots,
        blocks,
        run,
    )
}

fn unique_catalog<T>(
    items: impl IntoIterator<Item = T>,
    digest: impl Fn(&T) -> Result<ArtifactDigest, eqiora::Diagnostic>,
) -> Vec<T> {
    let mut unique = BTreeMap::new();
    for item in items {
        unique.insert(digest(&item).unwrap(), item);
    }
    unique.into_values().collect()
}

fn assert_unique_catalog<T>(
    items: &[T],
    digest: impl Fn(&T) -> Result<ArtifactDigest, eqiora::Diagnostic>,
) {
    let distinct = items
        .iter()
        .map(|item| digest(item).unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(items.len(), distinct.len());
}
