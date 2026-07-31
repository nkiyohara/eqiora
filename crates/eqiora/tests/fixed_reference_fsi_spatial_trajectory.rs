use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora::api::{FixedReferenceFsiSnapshotSetV1, snapshot_fixed_reference_fsi_solution_v1};
use eqiora::artifact::{
    ArtifactDigest, DatasetViewEnvelopeV1, DiscreteFieldEnvelopeV1, DiscreteFieldStorageEnvelopeV1,
    FieldSnapshotEnvelopeV1, SpatialStateEnvelopeV1, SpatialTrajectoryEnvelopeV1,
    SpatialTrajectorySegmentEnvelopeV1, StorageChunkV1, TrajectoryDecoderLimits,
    ValidatedFixedSpatialContextV1,
};
use eqiora::meshing::{DiscreteFieldAssociation, DiscreteFieldPayload};
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use eqiora_numerics::{
    fsi::ResolvedFixedReferenceFsiSolution2d, fsi::lower_fixed_reference_fsi_cartesian_2d,
};
use support::fixed_reference_fsi::{
    ExecutionContext, SpatialContext, direct_document, execution_context, packaged_document,
    prestrained_state, solve_step, spatial_context, state_from_solution,
};

mod support;

struct AcceptedTrajectory {
    spatial: SpatialContext,
    execution: ExecutionContext,
    first_solution: ResolvedFixedReferenceFsiSolution2d,
    second_solution: ResolvedFixedReferenceFsiSolution2d,
    first_snapshots: FixedReferenceFsiSnapshotSetV1,
    second_snapshots: FixedReferenceFsiSnapshotSetV1,
    first_state: SpatialStateEnvelopeV1,
    second_state: SpatialStateEnvelopeV1,
    first_segment: SpatialTrajectorySegmentEnvelopeV1,
    second_segment: SpatialTrajectorySegmentEnvelopeV1,
    first_root: SpatialTrajectoryEnvelopeV1,
    final_root: SpatialTrajectoryEnvelopeV1,
}

#[test]
fn two_accepted_fsi_steps_publish_one_immutable_reference_only_trajectory() {
    let document = direct_document();
    let accepted = accepted_trajectory_from(&document);
    let shared = eqiora::api::FixedReferenceFsiResult2d::solve_reference(
        &document,
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("production application result consumes the oracle-owned direct Model");
    let context = fixed_context(&accepted);
    let fields = accepted.first_solution.fields();

    assert_eq!(
        shared.solutions()[0].vertex_velocity_coefficients(),
        accepted.first_solution.vertex_velocity_coefficients()
    );
    assert_eq!(
        shared.solutions()[1].vertex_velocity_coefficients(),
        accepted.second_solution.vertex_velocity_coefficients()
    );
    assert_eq!(
        shared.states()[0].digest().unwrap(),
        accepted.first_state.digest().unwrap()
    );
    assert_eq!(
        shared.states()[1].digest().unwrap(),
        accepted.second_state.digest().unwrap()
    );
    assert_eq!(
        shared.trajectory().digest().unwrap(),
        accepted.final_root.digest().unwrap()
    );

    assert!(accepted.first_solution.numerical_evidence().residual_norm() < 1.0e-9);
    assert!(
        accepted
            .second_solution
            .numerical_evidence()
            .residual_norm()
            < 1.0e-9
    );
    assert_ne!(
        accepted.first_solution.solid_displacement_coefficients(),
        accepted.second_solution.solid_displacement_coefficients(),
        "the second accepted state must not be a duplicated first-step observation"
    );

    for snapshots in [&accepted.first_snapshots, &accepted.second_snapshots] {
        assert_eq!(snapshots.snapshots().len(), 4);
        for snapshot in snapshots.snapshots() {
            snapshot
                .validate_against(
                    &context,
                    snapshots
                        .blocks(snapshot.field())
                        .expect("every logical snapshot retains its normalized leaves"),
                )
                .expect("snapshot meaning and normalized content replay exactly");
        }
    }

    let fluid_velocity = snapshot(&accepted.first_snapshots, fields.fluid_velocity());
    assert_eq!(fluid_velocity.block_artifacts().len(), 2);
    let fluid_blocks = accepted
        .first_snapshots
        .blocks(fields.fluid_velocity())
        .expect("fluid velocity blocks");
    assert!(
        FieldSnapshotEnvelopeV1::new(&context, fields.fluid_velocity(), &fluid_blocks[..1])
            .is_err(),
        "dropping the MINI bubble block must not publish a lossy Field snapshot"
    );
    assert!(
        FieldSnapshotEnvelopeV1::new(
            &context,
            fields.fluid_velocity(),
            &[fluid_blocks[0].clone(), fluid_blocks[0].clone()],
        )
        .is_err(),
        "duplicate coefficient-block roles must fail"
    );
    let outside_vertex = accepted
        .first_solution
        .solid_velocity_vertices()
        .iter()
        .find(|vertex| {
            accepted
                .first_solution
                .fluid_velocity_vertices()
                .binary_search(vertex)
                .is_err()
        })
        .expect("the solid owns a vertex outside the fluid closure");
    let mut outside_values = fluid_blocks[0].values().to_vec();
    outside_values[2 * outside_vertex.index()] = 1.0;
    let outside_payload = DiscreteFieldPayload::new(
        context.mesh().mesh(),
        DiscreteFieldAssociation::Vertex,
        fluid_blocks[0].component_shape(),
        outside_values,
    )
    .expect("shape-valid but support-invalid payload");
    let outside_block = DiscreteFieldEnvelopeV1::from_payload(context.mesh(), &outside_payload)
        .expect("mesh-bound support-invalid leaf");
    assert!(
        FieldSnapshotEnvelopeV1::new(
            &context,
            fields.fluid_velocity(),
            &[outside_block, fluid_blocks[1].clone()],
        )
        .is_err(),
        "a nonzero coefficient outside semantic support must fail"
    );
    let mut reversed_blocks = accepted
        .first_snapshots
        .blocks(fields.fluid_velocity())
        .expect("fluid velocity blocks")
        .to_vec();
    reversed_blocks.reverse();
    assert_eq!(
        FieldSnapshotEnvelopeV1::new(&context, fields.fluid_velocity(), &reversed_blocks,)
            .expect("block declaration order is not logical identity"),
        fluid_velocity.clone()
    );

    let mut reversed_snapshots = accepted.first_snapshots.snapshots().to_vec();
    reversed_snapshots.reverse();
    assert_eq!(
        SpatialStateEnvelopeV1::new(
            &context,
            accepted.first_state.step(),
            accepted.first_state.time_s(),
            &reversed_snapshots,
        )
        .expect("snapshot declaration order is not state identity"),
        accepted.first_state
    );

    let combined_forward = SpatialTrajectorySegmentEnvelopeV1::new(
        &context,
        &[accepted.first_state.clone(), accepted.second_state.clone()],
    )
    .expect("two accepted states form one valid segment");
    let combined_reverse = SpatialTrajectorySegmentEnvelopeV1::new(
        &context,
        &[accepted.second_state.clone(), accepted.first_state.clone()],
    )
    .expect("caller state order is canonicalized by accepted step identity");
    assert_eq!(combined_forward, combined_reverse);

    let first_root_bytes = accepted.first_root.canonical_json().unwrap();
    let first_root_digest = accepted.first_root.digest().unwrap();
    assert_eq!(accepted.final_root.generation(), 1);
    assert_eq!(
        accepted.final_root.previous_root(),
        Some(first_root_digest.clone())
    );
    assert_eq!(
        accepted.final_root.segment_artifacts(),
        vec![
            accepted.first_segment.digest().unwrap(),
            accepted.second_segment.digest().unwrap(),
        ]
    );
    assert_eq!(
        accepted.first_root.canonical_json().unwrap(),
        first_root_bytes
    );
    assert_eq!(accepted.first_root.digest().unwrap(), first_root_digest);
    accepted
        .final_root
        .validate_against(
            &context,
            Some(&accepted.first_root),
            &[
                accepted.second_segment.clone(),
                accepted.first_segment.clone(),
            ],
        )
        .expect("root replay preserves the exact immutable prefix");

    let final_run = accepted
        .execution
        .run
        .clone()
        .with_output(accepted.final_root.digest().unwrap());
    final_run
        .validate_against(&accepted.execution.realization)
        .expect("the final Run remains valid against its exact Realization");
    assert_eq!(shared.run().digest().unwrap(), final_run.digest().unwrap());
    let root_digest = accepted.final_root.digest().unwrap();
    assert!(final_run.outputs().contains(&root_digest));
    assert!(
        !accepted.execution.run.outputs().contains(&root_digest),
        "only the final Run may claim the completed trajectory root"
    );

    let segments = [
        accepted.first_segment.clone(),
        accepted.second_segment.clone(),
    ];
    let view = DatasetViewEnvelopeV1::identity_window(
        &context,
        &accepted.final_root,
        &segments,
        1,
        2,
        [fields.solid_displacement(), fields.fluid_pressure()],
    )
    .expect("identity Dataset view selects two exact accepted states");
    let reordered_view = DatasetViewEnvelopeV1::identity_window(
        &context,
        &accepted.final_root,
        &segments,
        1,
        2,
        [fields.fluid_pressure(), fields.solid_displacement()],
    )
    .expect("Field selection order is canonicalized");
    assert_eq!(view, reordered_view);
    assert_eq!(view.trajectory(), accepted.final_root.digest().unwrap());
    assert_eq!(
        view.states(),
        vec![
            (
                1,
                accepted.first_state.time_s(),
                accepted.first_state.digest().unwrap()
            ),
            (
                2,
                accepted.second_state.time_s(),
                accepted.second_state.digest().unwrap()
            ),
        ]
    );
    assert!(
        !String::from_utf8(view.canonical_json().unwrap())
            .unwrap()
            .contains("\"values\""),
        "a reference-only view must not copy numerical values"
    );
    view.validate_against(&context, &accepted.final_root, &segments)
        .expect("Dataset selection replays from exact segment indices");
}

#[test]
fn storage_layout_and_partial_traversal_never_change_logical_identity() {
    let accepted = accepted_trajectory();
    let context = fixed_context(&accepted);
    let pressure = accepted.second_solution.fields().fluid_pressure();
    let pressure_snapshot = snapshot(&accepted.second_snapshots, pressure);
    let pressure_field = accepted
        .second_snapshots
        .blocks(pressure)
        .expect("pressure block")[0]
        .clone();

    let (small_storage, small_chunks) =
        DiscreteFieldStorageEnvelopeV1::pack_raw(&pressure_field, NonZeroUsize::new(17).unwrap())
            .expect("small raw chunks");
    let (large_storage, large_chunks) =
        DiscreteFieldStorageEnvelopeV1::pack_raw(&pressure_field, NonZeroUsize::new(71).unwrap())
            .expect("large raw chunks");
    assert_eq!(
        small_storage.logical_field(),
        pressure_field.digest().unwrap()
    );
    assert_eq!(
        large_storage.logical_field(),
        pressure_field.digest().unwrap()
    );
    assert_ne!(
        small_storage.digest().unwrap(),
        large_storage.digest().unwrap()
    );
    assert_eq!(
        small_storage
            .restore(&small_chunks, Default::default())
            .unwrap(),
        pressure_field
    );
    assert_eq!(
        large_storage
            .restore(&large_chunks, Default::default())
            .unwrap(),
        pressure_field
    );
    assert!(
        small_storage
            .restore(&small_chunks[..small_chunks.len() - 1], Default::default(),)
            .is_err(),
        "a missing chunk must not produce a logical Field"
    );
    let mut substituted_chunks = small_chunks.clone();
    let mut substituted_bytes = substituted_chunks[0].bytes().to_vec();
    substituted_bytes[0] ^= 1;
    substituted_chunks[0] = StorageChunkV1::from_bytes(substituted_bytes);
    assert!(
        small_storage
            .restore(&substituted_chunks, Default::default())
            .is_err(),
        "content-valid replacement storage identity cannot satisfy the original reference"
    );
    let mut reversed_chunks = small_chunks.clone();
    reversed_chunks.reverse();
    assert!(
        small_storage
            .restore(&reversed_chunks, Default::default())
            .is_err(),
        "storage chunk order is identity and cannot be repaired"
    );
    let mut truncated_chunks = small_chunks.clone();
    truncated_chunks[0] = StorageChunkV1::from_bytes(
        truncated_chunks[0].bytes()[..truncated_chunks[0].bytes().len() - 1].to_vec(),
    );
    assert!(
        small_storage
            .restore(&truncated_chunks, Default::default())
            .is_err(),
        "a truncated chunk cannot reproduce the logical Field"
    );
    let mut overlapping_manifest: serde_json::Value =
        serde_json::from_slice(&small_storage.canonical_json().unwrap()).unwrap();
    overlapping_manifest["chunks"][1]["offset"] = 0.into();
    assert!(
        DiscreteFieldStorageEnvelopeV1::from_json(
            &serde_json::to_vec(&overlapping_manifest).unwrap(),
            Default::default(),
        )
        .is_err(),
        "overlapping storage extents must fail during bounded decode"
    );

    let mut store = ExactByteStore::default();
    store.insert(
        accepted.final_root.digest().unwrap(),
        accepted.final_root.canonical_json().unwrap(),
    );
    store.insert(
        accepted.second_segment.digest().unwrap(),
        accepted.second_segment.canonical_json().unwrap(),
    );
    store.insert(
        accepted.second_state.digest().unwrap(),
        accepted.second_state.canonical_json().unwrap(),
    );
    store.insert(
        pressure_snapshot.digest().unwrap(),
        pressure_snapshot.canonical_json().unwrap(),
    );
    store.insert(
        pressure_field.digest().unwrap(),
        pressure_field.canonical_json().unwrap(),
    );

    let loaded_root = load_exact(
        &store,
        &accepted.final_root.digest().unwrap(),
        |bytes| SpatialTrajectoryEnvelopeV1::from_json(bytes, Default::default()),
        SpatialTrajectoryEnvelopeV1::digest,
    )
    .expect("load only the accepted root");
    assert!(
        store.get(&loaded_root.segment_artifacts()[0]).is_none(),
        "the sparse replica deliberately omits the unrelated first segment"
    );
    let loaded_segment = load_exact(
        &store,
        &loaded_root.segment_artifacts()[1],
        |bytes| SpatialTrajectorySegmentEnvelopeV1::from_json(bytes, Default::default()),
        SpatialTrajectorySegmentEnvelopeV1::digest,
    )
    .expect("load only the selected segment");
    let loaded_state = load_exact(
        &store,
        &loaded_segment.state_artifacts()[0],
        |bytes| SpatialStateEnvelopeV1::from_json(bytes, Default::default()),
        SpatialStateEnvelopeV1::digest,
    )
    .expect("load only the selected state");
    let loaded_snapshot = load_exact(
        &store,
        &loaded_state
            .field_snapshot(pressure)
            .expect("state indexes exact pressure snapshot"),
        |bytes| FieldSnapshotEnvelopeV1::from_json(bytes, Default::default()),
        FieldSnapshotEnvelopeV1::digest,
    )
    .expect("load only pressure meaning");
    let loaded_field = load_exact(
        &store,
        &loaded_snapshot.block_artifacts()[0].1,
        |bytes| DiscreteFieldEnvelopeV1::from_json(bytes, Default::default()),
        DiscreteFieldEnvelopeV1::digest,
    )
    .expect("load only the pressure values");
    loaded_snapshot
        .validate_against(&context, &[loaded_field])
        .expect("partial traversal still closes the selected snapshot contract");

    let other_snapshot = snapshot(
        &accepted.second_snapshots,
        accepted.second_solution.fields().solid_displacement(),
    );
    store.insert(
        pressure_snapshot.digest().unwrap(),
        other_snapshot.canonical_json().unwrap(),
    );
    assert!(
        load_exact(
            &store,
            &pressure_snapshot.digest().unwrap(),
            |bytes| FieldSnapshotEnvelopeV1::from_json(bytes, Default::default()),
            FieldSnapshotEnvelopeV1::digest,
        )
        .is_err(),
        "valid bytes stored under another logical identity are rejected"
    );
}

#[test]
fn stale_incomplete_nonmonotone_and_missing_content_fail_closed() {
    let accepted = accepted_trajectory();
    let context = fixed_context(&accepted);

    assert!(
        SpatialStateEnvelopeV1::new(
            &context,
            3,
            0.15,
            &accepted.second_snapshots.snapshots()[..3],
        )
        .is_err(),
        "a partial multiphysics inventory is not a SpatialState"
    );

    let mut stale_json: serde_json::Value = serde_json::from_slice(
        &accepted.second_snapshots.snapshots()[0]
            .canonical_json()
            .unwrap(),
    )
    .unwrap();
    stale_json["mesh_sha256"] = accepted.spatial.model.digest().unwrap().to_string().into();
    let stale_snapshot = FieldSnapshotEnvelopeV1::from_json(
        &serde_json::to_vec(&stale_json).unwrap(),
        Default::default(),
    )
    .expect("a locally well-formed reference remains untrusted until linked replay");
    let mut stale_inventory = accepted.second_snapshots.snapshots().to_vec();
    stale_inventory[0] = stale_snapshot;
    assert!(
        SpatialStateEnvelopeV1::new(&context, 3, 0.15, &stale_inventory,).is_err(),
        "a stale mesh reference fails before state acceptance"
    );

    let velocity_field = accepted.second_solution.fields().fluid_velocity();
    let velocity_snapshot = snapshot(&accepted.second_snapshots, velocity_field);
    let mut wrong_frame_json: serde_json::Value =
        serde_json::from_slice(&velocity_snapshot.canonical_json().unwrap()).unwrap();
    wrong_frame_json["physical"]["frame"] = "invariant".into();
    let wrong_frame = FieldSnapshotEnvelopeV1::from_json(
        &serde_json::to_vec(&wrong_frame_json).unwrap(),
        Default::default(),
    )
    .expect("invariant is a locally valid frame variant");
    assert!(
        wrong_frame
            .validate_against(
                &context,
                accepted
                    .second_snapshots
                    .blocks(velocity_field)
                    .expect("velocity blocks"),
            )
            .is_err(),
        "physical type drift must fail exact semantic replay"
    );

    let mut early_json: serde_json::Value =
        serde_json::from_slice(&accepted.second_state.canonical_json().unwrap()).unwrap();
    early_json["accepted"]["time_s"] = 0.04.into();
    let early_state = SpatialStateEnvelopeV1::from_json(
        &serde_json::to_vec(&early_json).unwrap(),
        Default::default(),
    )
    .expect("one isolated state cannot establish trajectory monotonicity");
    assert!(
        SpatialTrajectorySegmentEnvelopeV1::new(
            &context,
            &[accepted.first_state.clone(), early_state],
        )
        .is_err(),
        "accepted time must increase with step identity"
    );

    let mut off_grid_json: serde_json::Value =
        serde_json::from_slice(&accepted.second_state.canonical_json().unwrap()).unwrap();
    off_grid_json["accepted"]["time_s"] = 0.09.into();
    let off_grid_state = SpatialStateEnvelopeV1::from_json(
        &serde_json::to_vec(&off_grid_json).unwrap(),
        Default::default(),
    )
    .expect("an isolated monotone coordinate remains untrusted until context replay");
    assert!(
        SpatialTrajectorySegmentEnvelopeV1::new(
            &context,
            &[accepted.first_state.clone(), off_grid_state],
        )
        .is_err(),
        "monotone accepted time must still equal step times the exact Realization duration"
    );

    let mut duplicate_json: serde_json::Value =
        serde_json::from_slice(&accepted.second_state.canonical_json().unwrap()).unwrap();
    duplicate_json["accepted"]["step"] = 1.into();
    let duplicate_state = SpatialStateEnvelopeV1::from_json(
        &serde_json::to_vec(&duplicate_json).unwrap(),
        Default::default(),
    )
    .expect("one isolated state cannot establish step uniqueness");
    assert!(
        SpatialTrajectorySegmentEnvelopeV1::new(
            &context,
            &[accepted.first_state.clone(), duplicate_state],
        )
        .is_err(),
        "duplicate accepted step identity must fail"
    );

    let combined = SpatialTrajectorySegmentEnvelopeV1::new(
        &context,
        &[accepted.first_state.clone(), accepted.second_state.clone()],
    )
    .unwrap();
    let mut reversed_wire: serde_json::Value =
        serde_json::from_slice(&combined.canonical_json().unwrap()).unwrap();
    reversed_wire["states"].as_array_mut().unwrap().swap(0, 1);
    assert!(
        SpatialTrajectorySegmentEnvelopeV1::from_json(
            &serde_json::to_vec(&reversed_wire).unwrap(),
            Default::default(),
        )
        .is_err(),
        "wire order is canonical and cannot be silently repaired"
    );

    assert!(
        accepted
            .final_root
            .validate_against(
                &context,
                Some(&accepted.first_root),
                std::slice::from_ref(&accepted.first_segment),
            )
            .is_err(),
        "a final root with a missing segment cannot be accepted"
    );
    assert!(
        DatasetViewEnvelopeV1::identity_window(
            &context,
            &accepted.final_root,
            std::slice::from_ref(&accepted.first_segment),
            1,
            2,
            [accepted.first_solution.fields().fluid_pressure()],
        )
        .is_err(),
        "a Dataset view cannot hide a missing source segment"
    );

    let root_bytes = accepted.final_root.canonical_json().unwrap();
    let mut substituted_segment: serde_json::Value = serde_json::from_slice(&root_bytes).unwrap();
    substituted_segment["segments"][1]["segment_sha256"] =
        accepted.spatial.model.digest().unwrap().to_string().into();
    let substituted_root = SpatialTrajectoryEnvelopeV1::from_json(
        &serde_json::to_vec(&substituted_segment).unwrap(),
        Default::default(),
    )
    .expect("a local digest reference remains untrusted until dependency replay");
    assert!(
        substituted_root
            .validate_segments(
                &context,
                &[
                    accepted.first_segment.clone(),
                    accepted.second_segment.clone(),
                ],
            )
            .is_err(),
        "a substituted segment digest must fail exact trajectory replay"
    );
    let mut overlapping_ranges: serde_json::Value = serde_json::from_slice(&root_bytes).unwrap();
    overlapping_ranges["segments"][1]["first_step"] = 1.into();
    assert!(
        SpatialTrajectoryEnvelopeV1::from_json(
            &serde_json::to_vec(&overlapping_ranges).unwrap(),
            Default::default(),
        )
        .is_err(),
        "overlapping segment summaries must fail local decode"
    );
    let mut malformed_ulid: serde_json::Value = serde_json::from_slice(&root_bytes).unwrap();
    malformed_ulid["fields"][0]["field_ulid"] = "not-a-ulid".into();
    assert!(
        SpatialTrajectoryEnvelopeV1::from_json(
            &serde_json::to_vec(&malformed_ulid).unwrap(),
            Default::default(),
        )
        .is_err(),
        "malformed trajectory Field identity must fail during bounded decode"
    );
    let mut noncanonical_ulid: serde_json::Value = serde_json::from_slice(&root_bytes).unwrap();
    let canonical_ulid = noncanonical_ulid["fields"][0]["field_ulid"]
        .as_str()
        .unwrap()
        .to_owned();
    let lowercase_ulid = canonical_ulid.to_ascii_lowercase();
    assert_ne!(lowercase_ulid, canonical_ulid);
    noncanonical_ulid["fields"][0]["field_ulid"] = lowercase_ulid.into();
    assert!(
        SpatialTrajectoryEnvelopeV1::from_json(
            &serde_json::to_vec(&noncanonical_ulid).unwrap(),
            Default::default(),
        )
        .is_err(),
        "a parseable but noncanonical trajectory ULID spelling must fail"
    );

    let mut forged_count: serde_json::Value = serde_json::from_slice(&root_bytes).unwrap();
    for segment in forged_count["segments"].as_array_mut().unwrap() {
        segment["state_count"] = 2.into();
    }
    let aggregate_limits = TrajectoryDecoderLimits {
        max_trajectory_segment_states: 2,
        max_trajectory_states: 3,
        ..Default::default()
    };
    assert!(
        SpatialTrajectoryEnvelopeV1::from_json(
            &serde_json::to_vec(&forged_count).unwrap(),
            aggregate_limits,
        )
        .is_err(),
        "individually admissible summaries cannot forge an excessive aggregate state count"
    );

    let packaged = packaged_document();
    let packaged_canonical = lower_fixed_reference_fsi_cartesian_2d(packaged.model().program())
        .expect("packaged FSI meaning lowers independently");
    let packaged_spatial = spatial_context(packaged.model().program(), &packaged_canonical);
    let packaged_execution = execution_context(
        packaged.model().program(),
        &packaged_canonical,
        &packaged_spatial,
    );
    let foreign_context = ValidatedFixedSpatialContextV1::new(
        &packaged_spatial.model,
        &packaged_execution.realization,
        &packaged_spatial.geometry,
        &packaged_spatial.correspondence,
        &packaged_spatial.mesh_artifact,
    )
    .expect("the packaged lineage is valid in its own right");
    assert!(
        snapshot_fixed_reference_fsi_solution_v1(&foreign_context, &accepted.first_solution)
            .is_err(),
        "a finalized solution cannot be projected through a foreign Model and Realization plan"
    );

    let segments = [
        accepted.first_segment.clone(),
        accepted.second_segment.clone(),
    ];
    let view = DatasetViewEnvelopeV1::identity_window(
        &context,
        &accepted.final_root,
        &segments,
        1,
        2,
        [accepted.first_solution.fields().fluid_pressure()],
    )
    .unwrap();
    let view_bytes = view.canonical_json().unwrap();
    let mut stale_view: serde_json::Value = serde_json::from_slice(&view_bytes).unwrap();
    stale_view["trajectory_sha256"] = accepted.first_root.digest().unwrap().to_string().into();
    let stale_view = DatasetViewEnvelopeV1::from_json(
        &serde_json::to_vec(&stale_view).unwrap(),
        Default::default(),
    )
    .expect("a local Dataset source reference remains untrusted until replay");
    assert!(
        stale_view
            .validate_against(&context, &accepted.final_root, &segments)
            .is_err(),
        "a stale Dataset trajectory reference must fail"
    );
    let mut unknown_transform: serde_json::Value = serde_json::from_slice(&view_bytes).unwrap();
    unknown_transform["transformation"] = "normalize".into();
    assert!(
        DatasetViewEnvelopeV1::from_json(
            &serde_json::to_vec(&unknown_transform).unwrap(),
            Default::default(),
        )
        .is_err(),
        "Dataset transformation semantics cannot enter an identity-only view"
    );
}

fn accepted_trajectory() -> AcceptedTrajectory {
    let document = direct_document();
    accepted_trajectory_from(&document)
}

fn accepted_trajectory_from(document: &eqiora::api::ModelDocument) -> AcceptedTrajectory {
    let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program())
        .expect("fixed-reference FSI meaning lowers");
    let spatial = spatial_context(document.program(), &canonical);
    let execution = execution_context(document.program(), &canonical, &spatial);
    let context = ValidatedFixedSpatialContextV1::new(
        &spatial.model,
        &execution.realization,
        &spatial.geometry,
        &spatial.correspondence,
        &spatial.mesh_artifact,
    )
    .expect("one fixed spatial lineage validates before any observation is published");
    let first = solve_step(
        &canonical,
        &spatial,
        &execution,
        &prestrained_state(&spatial),
    );
    let second = solve_step(
        &canonical,
        &spatial,
        &execution,
        &state_from_solution(&spatial, &first.solution),
    );
    let first_snapshots = snapshot_fixed_reference_fsi_solution_v1(&context, &first.solution)
        .expect("first accepted solution projects to exact snapshots");
    let second_snapshots = snapshot_fixed_reference_fsi_solution_v1(&context, &second.solution)
        .expect("second accepted solution projects to exact snapshots");
    let dt = execution
        .realization
        .plan()
        .unwrap()
        .time_step()
        .duration()
        .value();
    let first_state = SpatialStateEnvelopeV1::new(&context, 1, dt, first_snapshots.snapshots())
        .expect("first complete accepted state");
    let second_state =
        SpatialStateEnvelopeV1::new(&context, 2, 2.0 * dt, second_snapshots.snapshots())
            .expect("second complete accepted state");
    let first_segment =
        SpatialTrajectorySegmentEnvelopeV1::new(&context, std::slice::from_ref(&first_state))
            .expect("first immutable one-state segment");
    let second_segment =
        SpatialTrajectorySegmentEnvelopeV1::new(&context, std::slice::from_ref(&second_state))
            .expect("second immutable one-state segment");
    let first_root =
        SpatialTrajectoryEnvelopeV1::start(&context, &first_segment).expect("first immutable root");
    let final_root = SpatialTrajectoryEnvelopeV1::extend(&context, &first_root, &second_segment)
        .expect("append publishes a new immutable root");
    AcceptedTrajectory {
        spatial,
        execution,
        first_solution: first.solution,
        second_solution: second.solution,
        first_snapshots,
        second_snapshots,
        first_state,
        second_state,
        first_segment,
        second_segment,
        first_root,
        final_root,
    }
}

fn fixed_context(accepted: &AcceptedTrajectory) -> ValidatedFixedSpatialContextV1<'_> {
    ValidatedFixedSpatialContextV1::new(
        &accepted.spatial.model,
        &accepted.execution.realization,
        &accepted.spatial.geometry,
        &accepted.spatial.correspondence,
        &accepted.spatial.mesh_artifact,
    )
    .expect("accepted trajectory retains one valid fixed-spatial lineage")
}

fn snapshot(
    snapshots: &FixedReferenceFsiSnapshotSetV1,
    field: eqiora::Id<eqiora::kinds::Field>,
) -> &FieldSnapshotEnvelopeV1 {
    snapshots
        .snapshots()
        .iter()
        .find(|snapshot| snapshot.field() == field)
        .expect("snapshot set contains every exact FSI Field")
}

#[derive(Default)]
struct ExactByteStore {
    objects: BTreeMap<ArtifactDigest, Vec<u8>>,
}

impl ExactByteStore {
    fn insert(&mut self, key: ArtifactDigest, bytes: Vec<u8>) {
        self.objects.insert(key, bytes);
    }

    fn get(&self, key: &ArtifactDigest) -> Option<&[u8]> {
        self.objects.get(key).map(Vec::as_slice)
    }
}

fn load_exact<T>(
    store: &ExactByteStore,
    expected: &ArtifactDigest,
    decode: impl FnOnce(&[u8]) -> Result<T, eqiora::Diagnostic>,
    digest: impl FnOnce(&T) -> Result<ArtifactDigest, eqiora::Diagnostic>,
) -> Result<T, String> {
    let bytes = store
        .get(expected)
        .ok_or_else(|| format!("missing exact artifact {expected}"))?;
    let value = decode(bytes).map_err(|error| error.to_string())?;
    let actual = digest(&value).map_err(|error| error.to_string())?;
    if &actual != expected {
        return Err(format!(
            "artifact substitution: requested {expected}, decoded {actual}"
        ));
    }
    Ok(value)
}
