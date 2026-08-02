//! Complete replay of one durable fixed-mesh two-dimensional Field trajectory.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::{
    ArtifactDigest, DiscreteFieldEnvelopeV1, FieldSnapshotEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, RealizationEnvelopeV3, ReplayableCanonicalModelArtifact,
    RunManifestV2, SimplicialMeshEnvelopeV1, SpatialStateEnvelopeV1, SpatialTrajectoryEnvelopeV1,
    SpatialTrajectorySegmentEnvelopeV1, ValidatedFixedSpatialContextV1,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

#[derive(Debug)]
pub(crate) struct ReplayedFixedField<'a> {
    pub(crate) snapshot: &'a FieldSnapshotEnvelopeV1,
    pub(crate) blocks: Vec<&'a DiscreteFieldEnvelopeV1>,
}

#[derive(Debug)]
pub(crate) struct ReplayedFixedFrame<'a> {
    pub(crate) state: &'a SpatialStateEnvelopeV1,
    pub(crate) fields: Vec<ReplayedFixedField<'a>>,
}

/// Complete borrowed dependency replay for one fixed-mesh 2D trajectory root.
///
/// This proof adds no durable identity and is deliberately narrower than a
/// universal trajectory interface. It accepts only the existing fixed-step,
/// affine-simplicial V1 artifact family and requires the Run to name exactly
/// the replayed trajectory root as its complete output inventory.
#[derive(Debug)]
pub struct FixedMeshFieldTrajectoryReplay2dV1<'a> {
    trajectory: &'a SpatialTrajectoryEnvelopeV1,
    frames: Vec<ReplayedFixedFrame<'a>>,
}

impl<'a> FixedMeshFieldTrajectoryReplay2dV1<'a> {
    /// Replay every segment, state, snapshot, and numerical block edge.
    ///
    /// Catalog declaration order is irrelevant. Every declared object must be
    /// resolved, with no unused declarations, while artifact-internal state
    /// and Field ordering remains exact and canonical.
    ///
    /// # Errors
    /// Returns `EQ0901` for a non-2D mesh, any missing, stale, substituted,
    /// duplicate, reordered, cross-context, or unused dependency, fewer than
    /// two accepted states, or a Run output inventory other than this root.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &'a RealizationEnvelopeV3,
        geometry: &'a GeometryIdentityEnvelopeV1,
        correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &'a SimplicialMeshEnvelopeV1,
        trajectory: &'a SpatialTrajectoryEnvelopeV1,
        segments: &'a [SpatialTrajectorySegmentEnvelopeV1],
        states: &'a [SpatialStateEnvelopeV1],
        snapshots: &'a [FieldSnapshotEnvelopeV1],
        blocks: &'a [DiscreteFieldEnvelopeV1],
        run: &'a RunManifestV2,
    ) -> Result<Self, Diagnostic> {
        if mesh.dimension() != 2 || mesh.mesh().cells().iter().any(|cell| cell.len() != 3) {
            return Err(replay_error(
                "fixed-mesh Field trajectory replay requires an affine-triangle 2D mesh",
            ));
        }
        let context = ValidatedFixedSpatialContextV1::new(
            model,
            realization,
            geometry,
            correspondence,
            mesh,
        )?;

        let segment_index = index_catalog(
            segments,
            SpatialTrajectorySegmentEnvelopeV1::digest,
            "trajectory segment",
        )?;
        let ordered_segments = resolve_catalog(
            &segment_index,
            &trajectory.segment_artifacts(),
            "trajectory segment",
        )?;
        require_complete_catalog(
            segment_index.len(),
            ordered_segments.len(),
            "trajectory segment",
        )?;
        replay_root(&context, trajectory, &ordered_segments)?;

        let state_index = index_catalog(states, SpatialStateEnvelopeV1::digest, "spatial state")?;
        let state_identities = ordered_segments
            .iter()
            .flat_map(|segment| segment.state_artifacts())
            .collect::<Vec<_>>();
        let ordered_states = resolve_catalog(&state_index, &state_identities, "spatial state")?;
        require_complete_catalog(state_index.len(), ordered_states.len(), "spatial state")?;
        if ordered_states.len() < 2 {
            return Err(replay_error(
                "fixed-mesh Field trajectory replay requires at least two accepted states",
            ));
        }

        let snapshot_index =
            index_catalog(snapshots, FieldSnapshotEnvelopeV1::digest, "Field snapshot")?;
        let block_index = index_catalog(
            blocks,
            DiscreteFieldEnvelopeV1::digest,
            "DiscreteField block",
        )?;
        let mut used_snapshots = BTreeSet::new();
        let mut used_blocks = BTreeSet::new();
        let frames = ordered_states
            .into_iter()
            .map(|state| {
                Ok(ReplayedFixedFrame {
                    state,
                    fields: replay_fields(
                        &state.fields(),
                        &snapshot_index,
                        &block_index,
                        &mut used_snapshots,
                        &mut used_blocks,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        require_complete_catalog(snapshot_index.len(), used_snapshots.len(), "Field snapshot")?;
        require_complete_catalog(block_index.len(), used_blocks.len(), "DiscreteField block")?;

        validate_segments(&context, &ordered_segments, &frames)?;
        validate_frames(&context, &frames)?;
        run.validate_against(realization)?;
        if run.outputs() != vec![trajectory.digest()?] {
            return Err(replay_error(
                "fixed-mesh Field trajectory Run must output exactly the replayed root",
            ));
        }

        Ok(Self { trajectory, frames })
    }

    /// Exact immutable trajectory root whose complete dependency DAG replayed.
    #[must_use]
    pub const fn trajectory(&self) -> &'a SpatialTrajectoryEnvelopeV1 {
        self.trajectory
    }

    /// Accepted states in exact trajectory-edge order.
    ///
    /// This is a borrowed projection of dependencies already admitted by the
    /// complete replay. It assigns no new identity and performs no catalog
    /// lookup or digest-based reordering.
    pub fn states(
        &self,
    ) -> impl ExactSizeIterator<Item = &'a SpatialStateEnvelopeV1> + DoubleEndedIterator + '_ {
        self.frames.iter().map(|frame| frame.state)
    }

    /// Exact Field snapshots for one accepted state, in state-edge order.
    ///
    /// Returns `None` when `state_index` is outside the accepted trajectory.
    pub fn fields(
        &self,
        state_index: usize,
    ) -> Option<impl ExactSizeIterator<Item = &'a FieldSnapshotEnvelopeV1> + DoubleEndedIterator + '_>
    {
        self.frames
            .get(state_index)
            .map(|frame| frame.fields.iter().map(|field| field.snapshot))
    }

    /// Exact numerical blocks for one accepted Field occurrence.
    ///
    /// State and Field indices address the accepted edge order exposed by
    /// [`Self::states`] and [`Self::fields`]. Returns `None` for either
    /// out-of-range index.
    pub fn blocks(
        &self,
        state_index: usize,
        field_index: usize,
    ) -> Option<impl ExactSizeIterator<Item = &'a DiscreteFieldEnvelopeV1> + DoubleEndedIterator + '_>
    {
        self.frames
            .get(state_index)?
            .fields
            .get(field_index)
            .map(|field| field.blocks.iter().copied())
    }
}

fn replay_root(
    context: &ValidatedFixedSpatialContextV1<'_>,
    trajectory: &SpatialTrajectoryEnvelopeV1,
    segments: &[&SpatialTrajectorySegmentEnvelopeV1],
) -> Result<(), Diagnostic> {
    let first = segments
        .first()
        .copied()
        .ok_or_else(|| replay_error("fixed-mesh Field trajectory segment set is empty"))?;
    let mut replay = SpatialTrajectoryEnvelopeV1::start(context, first)?;
    for segment in &segments[1..] {
        replay = SpatialTrajectoryEnvelopeV1::extend(context, &replay, segment)?;
    }
    if &replay != trajectory {
        return Err(replay_error(
            "fixed-mesh Field trajectory root differs from complete immutable-prefix replay",
        ));
    }
    Ok(())
}

fn validate_segments(
    context: &ValidatedFixedSpatialContextV1<'_>,
    segments: &[&SpatialTrajectorySegmentEnvelopeV1],
    frames: &[ReplayedFixedFrame<'_>],
) -> Result<(), Diagnostic> {
    let mut offset = 0_usize;
    for segment in segments {
        let end = offset
            .checked_add(segment.state_count())
            .ok_or_else(|| replay_error("fixed-mesh trajectory state count overflows usize"))?;
        let states = frames
            .get(offset..end)
            .ok_or_else(|| replay_error("fixed-mesh trajectory omits exact frame states"))?
            .iter()
            .map(|frame| frame.state.clone())
            .collect::<Vec<_>>();
        segment.validate_against(context, &states)?;
        offset = end;
    }
    if offset != frames.len() {
        return Err(replay_error(
            "fixed-mesh frame inventory exceeds the exact trajectory segments",
        ));
    }
    Ok(())
}

fn validate_frames(
    context: &ValidatedFixedSpatialContextV1<'_>,
    frames: &[ReplayedFixedFrame<'_>],
) -> Result<(), Diagnostic> {
    for frame in frames {
        let snapshots = frame
            .fields
            .iter()
            .map(|field| field.snapshot.clone())
            .collect::<Vec<_>>();
        frame.state.validate_against(context, &snapshots)?;
        for field in &frame.fields {
            let blocks = field
                .blocks
                .iter()
                .map(|block| (*block).clone())
                .collect::<Vec<_>>();
            field.snapshot.validate_against(context, &blocks)?;
        }
    }
    Ok(())
}

fn replay_fields<'a>(
    references: &[(
        eqiora_core::Id<eqiora_core::entity::kinds::Domain>,
        eqiora_core::Id<eqiora_core::entity::kinds::Field>,
        ArtifactDigest,
    )],
    snapshots: &BTreeMap<ArtifactDigest, &'a FieldSnapshotEnvelopeV1>,
    blocks: &BTreeMap<ArtifactDigest, &'a DiscreteFieldEnvelopeV1>,
    used_snapshots: &mut BTreeSet<ArtifactDigest>,
    used_blocks: &mut BTreeSet<ArtifactDigest>,
) -> Result<Vec<ReplayedFixedField<'a>>, Diagnostic> {
    references
        .iter()
        .map(|(support, field, snapshot_identity)| {
            let snapshot = resolve_one(snapshots, snapshot_identity, "Field snapshot")?;
            if snapshot.support_domain() != *support
                || snapshot.field() != *field
                || snapshot.digest()? != *snapshot_identity
            {
                return Err(replay_error(
                    "fixed-mesh snapshot differs from its exact SpatialState Field edge",
                ));
            }
            used_snapshots.insert(snapshot_identity.clone());
            let mut resolved_blocks = Vec::new();
            for (association, block_identity) in snapshot.block_artifacts() {
                let block = resolve_one(blocks, &block_identity, "DiscreteField block")?;
                if block.association() != association || block.digest()? != block_identity {
                    return Err(replay_error(
                        "fixed-mesh block differs from its exact FieldSnapshot edge",
                    ));
                }
                used_blocks.insert(block_identity);
                resolved_blocks.push(block);
            }
            Ok(ReplayedFixedField {
                snapshot,
                blocks: resolved_blocks,
            })
        })
        .collect()
}

fn index_catalog<'a, T>(
    items: &'a [T],
    digest: impl Fn(&T) -> Result<ArtifactDigest, Diagnostic>,
    label: &str,
) -> Result<BTreeMap<ArtifactDigest, &'a T>, Diagnostic> {
    let mut index = BTreeMap::new();
    for item in items {
        let identity = digest(item)?;
        if index.insert(identity, item).is_some() {
            return Err(replay_error(format!(
                "fixed-mesh Field trajectory {label} catalog contains a duplicate identity"
            )));
        }
    }
    Ok(index)
}

fn resolve_catalog<'a, T>(
    index: &BTreeMap<ArtifactDigest, &'a T>,
    identities: &[ArtifactDigest],
    label: &str,
) -> Result<Vec<&'a T>, Diagnostic> {
    identities
        .iter()
        .map(|identity| resolve_one(index, identity, label))
        .collect()
}

fn resolve_one<'a, T>(
    index: &BTreeMap<ArtifactDigest, &'a T>,
    identity: &ArtifactDigest,
    label: &str,
) -> Result<&'a T, Diagnostic> {
    index.get(identity).copied().ok_or_else(|| {
        replay_error(format!(
            "fixed-mesh Field trajectory omits exact {label} `{identity}`"
        ))
    })
}

fn require_complete_catalog(declared: usize, used: usize, label: &str) -> Result<(), Diagnostic> {
    if declared == used {
        Ok(())
    } else {
        Err(replay_error(format!(
            "fixed-mesh Field trajectory {label} catalog has {declared} declarations but exact replay uses {used}"
        )))
    }
}

fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
