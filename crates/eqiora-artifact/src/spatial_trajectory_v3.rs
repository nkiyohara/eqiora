//! Immutable trajectory append chains across one V2-to-V3 remesh seam.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DataExchangeDecoderLimits,
    ReplayableCanonicalModelArtifact, SpatialStateEnvelopeV2, SpatialStateEnvelopeV3,
    SpatialStateOriginKindV3, SpatialTrajectoryEnvelopeV2, SpatialTrajectorySegmentEnvelopeV2,
    ValidatedMovingSpatialContextV2, check_json_limits, invalid_artifact,
};

const SEGMENT_SCHEMA: &str = "eqiora.spatial-trajectory-segment/v3";
const TRAJECTORY_SCHEMA: &str = "eqiora.spatial-trajectory/v3";

/// Closed origin of one target-side trajectory segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialTrajectorySegmentOriginKindV3 {
    /// First target segment paired to the finalized V2 source prefix.
    Remesh,
    /// Later target-only append after one exact V3 predecessor.
    Continuation,
}

/// Nonempty immutable target-state segment retaining the exact V2 source root.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialTrajectorySegmentEnvelopeV3 {
    wire: WireTrajectorySegmentV3,
}

impl SpatialTrajectorySegmentEnvelopeV3 {
    /// Construct the first target segment at the same coordinate as the V2 tip.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the source root is completely replayed, its last
    /// state is the exact transition source, and the first target state is the
    /// matching remesh-origin state at the same step/time.
    #[allow(clippy::too_many_arguments)]
    pub fn remesh<S: ReplayableCanonicalModelArtifact, T: ReplayableCanonicalModelArtifact>(
        source_context: &ValidatedMovingSpatialContextV2<'_, S>,
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        source_segments: &[SpatialTrajectorySegmentEnvelopeV2],
        source_state: &SpatialStateEnvelopeV2,
        target_context: &ValidatedMovingSpatialContextV2<'_, T>,
        target_states: &[SpatialStateEnvelopeV3],
    ) -> Result<Self, Diagnostic> {
        source_prefix.validate_segments(source_context, source_segments)?;
        let source_tip = source_segments
            .last()
            .and_then(|segment| segment.state_artifacts().last().cloned())
            .ok_or_else(|| invalid_artifact("remesh trajectory source prefix is empty"))?;
        if source_tip != source_state.digest()? {
            return Err(invalid_artifact(
                "remesh trajectory source state is not the exact V2 prefix tip",
            ));
        }
        let first = target_states
            .first()
            .ok_or_else(|| invalid_artifact("remesh trajectory segment is empty"))?;
        if first.origin() != SpatialStateOriginKindV3::Remesh
            || first.predecessor().is_some()
            || first.remesh_source_spatial_state() != source_tip
            || first.step() != source_state.step()
            || first.time_s() != source_state.time_s()
        {
            return Err(invalid_artifact(
                "first V3 state is not the same-coordinate representation of the V2 tip",
            ));
        }
        validate_target_states(target_context, target_states)?;
        Self::finish(
            source_prefix,
            source_state,
            target_states,
            WireSegmentOriginV3::Remesh {
                source_step: source_state.step(),
                source_time_s: source_state.time_s(),
            },
        )
    }

    /// Construct a later target-only segment after one exact predecessor.
    ///
    /// # Errors
    /// Returns `EQ0901` for a broken state edge, changed transition anchor, or
    /// target-context drift.
    pub fn continuation<M: ReplayableCanonicalModelArtifact>(
        context: &ValidatedMovingSpatialContextV2<'_, M>,
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        source_state: &SpatialStateEnvelopeV2,
        predecessor: &SpatialStateEnvelopeV3,
        states: &[SpatialStateEnvelopeV3],
    ) -> Result<Self, Diagnostic> {
        let first = states
            .first()
            .ok_or_else(|| invalid_artifact("V3 trajectory continuation is empty"))?;
        predecessor.require_adjacent(first)?;
        validate_target_states(context, states)?;
        Self::finish(
            source_prefix,
            source_state,
            states,
            WireSegmentOriginV3::Continuation {
                predecessor_spatial_state_v3_sha256: predecessor.digest()?.to_string(),
            },
        )
    }

    fn finish(
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        source_state: &SpatialStateEnvelopeV2,
        target_states: &[SpatialStateEnvelopeV3],
        origin: WireSegmentOriginV3,
    ) -> Result<Self, Diagnostic> {
        let first = &target_states[0];
        if target_states.iter().any(|state| {
            state.remesh_source_spatial_state() != first.remesh_source_spatial_state()
                || state.overlap_artifact() != first.overlap_artifact()
                || state.transfer_receipt_artifact() != first.transfer_receipt_artifact()
        }) {
            return Err(invalid_artifact(
                "V3 trajectory segment changes its immutable remesh anchor",
            ));
        }
        let value = Self {
            wire: WireTrajectorySegmentV3 {
                schema: SEGMENT_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source_prefix_v2_sha256: source_prefix.digest()?.to_string(),
                source_state_v2_sha256: source_state.digest()?.to_string(),
                target_reference: WireTargetReferenceV3::from_state(first),
                overlap_sha256: first.overlap_artifact().to_string(),
                transfer_receipt_sha256: first.transfer_receipt_artifact().to_string(),
                origin,
                states: target_states
                    .iter()
                    .map(WireTargetStateReferenceV3::from_state)
                    .collect::<Result<_, _>>()?,
            },
        };
        value.validate_local(DataExchangeDecoderLimits::default())?;
        Ok(value)
    }

    /// Decode bounded segment data without resolving state dependencies.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DataExchangeDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!(
                "invalid spatial-trajectory-segment/v3 JSON: {error}"
            ))
        })?;
        let value = Self { wire };
        value.validate_local(limits)?;
        Ok(value)
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize spatial trajectory segment v3: {error}"
            ))
        })
    }

    /// Domain-separated segment identity.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            SEGMENT_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Closed segment origin.
    #[must_use]
    pub const fn origin(&self) -> SpatialTrajectorySegmentOriginKindV3 {
        match self.wire.origin {
            WireSegmentOriginV3::Remesh { .. } => SpatialTrajectorySegmentOriginKindV3::Remesh,
            WireSegmentOriginV3::Continuation { .. } => {
                SpatialTrajectorySegmentOriginKindV3::Continuation
            }
        }
    }

    /// Exact immutable V2 source prefix.
    #[must_use]
    pub fn source_prefix(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.source_prefix_v2_sha256.clone())
    }

    /// Exact finalized V2 source state.
    #[must_use]
    pub fn source_state(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.source_state_v2_sha256.clone())
    }

    /// First target step.
    #[must_use]
    pub fn first_step(&self) -> u64 {
        self.wire.states[0].step
    }

    /// Last target step.
    #[must_use]
    pub fn last_step(&self) -> u64 {
        self.wire.states[self.wire.states.len() - 1].step
    }

    /// First target time.
    #[must_use]
    pub fn first_time_s(&self) -> f64 {
        self.wire.states[0].time_s
    }

    /// Last target time.
    #[must_use]
    pub fn last_time_s(&self) -> f64 {
        self.wire.states[self.wire.states.len() - 1].time_s
    }

    /// Last target state.
    #[must_use]
    pub fn last_state(&self) -> ArtifactDigest {
        ArtifactDigest(
            self.wire.states[self.wire.states.len() - 1]
                .state_sha256
                .clone(),
        )
    }

    /// Ordered exact target states.
    #[must_use]
    pub fn state_artifacts(&self) -> Vec<ArtifactDigest> {
        self.wire
            .states
            .iter()
            .map(|state| ArtifactDigest(state.state_sha256.clone()))
            .collect()
    }

    /// Replay exact target state objects.
    ///
    /// # Errors
    /// Returns `EQ0901` for reordered, substituted, or cross-context states.
    pub fn validate_states<M: ReplayableCanonicalModelArtifact>(
        &self,
        context: &ValidatedMovingSpatialContextV2<'_, M>,
        states: &[SpatialStateEnvelopeV3],
    ) -> Result<(), Diagnostic> {
        validate_target_states(context, states)?;
        let references = states
            .iter()
            .map(WireTargetStateReferenceV3::from_state)
            .collect::<Result<Vec<_>, _>>()?;
        if self.wire.target_reference != WireTargetReferenceV3::from_state(&states[0])
            || self.wire.states != references
        {
            return Err(invalid_artifact(
                "V3 trajectory segment differs from exact state replay",
            ));
        }
        Ok(())
    }

    fn validate_local(&self, limits: DataExchangeDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != SEGMENT_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported spatial-trajectory-segment/v3 schema or encoding",
            ));
        }
        for digest in [
            &self.wire.source_prefix_v2_sha256,
            &self.wire.source_state_v2_sha256,
            &self.wire.overlap_sha256,
            &self.wire.transfer_receipt_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        self.wire.target_reference.validate()?;
        self.wire.origin.validate()?;
        if self.wire.states.is_empty()
            || self.wire.states.len() > limits.max_remesh_trajectory_states
        {
            return Err(invalid_artifact(
                "V3 trajectory segment is empty or exceeds its state budget",
            ));
        }
        for state in &self.wire.states {
            state.validate()?;
            if state.overlap_sha256 != self.wire.overlap_sha256
                || state.transfer_receipt_sha256 != self.wire.transfer_receipt_sha256
            {
                return Err(invalid_artifact(
                    "V3 trajectory state changes the segment transition anchor",
                ));
            }
        }
        if self
            .wire
            .states
            .iter()
            .map(|state| state.state_sha256.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.wire.states.len()
        {
            return Err(invalid_artifact(
                "V3 trajectory segment contains a duplicate state",
            ));
        }
        for pair in self.wire.states.windows(2) {
            pair[0].require_adjacent(&pair[1])?;
        }
        match self.wire.origin {
            WireSegmentOriginV3::Remesh {
                source_step,
                source_time_s,
            } if self.wire.states[0].origin == WireStateOriginV3::Remesh
                && self.wire.states[0].predecessor_sha256.is_none()
                && self.wire.states[0].step == source_step
                && self.wire.states[0].time_s == source_time_s => {}
            WireSegmentOriginV3::Continuation {
                ref predecessor_spatial_state_v3_sha256,
            } if self.wire.states[0].origin == WireStateOriginV3::Continuous
                && self.wire.states[0].predecessor_sha256.as_ref()
                    == Some(predecessor_spatial_state_v3_sha256) => {}
            _ => {
                return Err(invalid_artifact(
                    "V3 trajectory segment origin does not close its first state edge",
                ));
            }
        }
        Ok(())
    }
}

/// Immutable V3 root retaining the exact V2 prefix and complete V3 segment prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialTrajectoryEnvelopeV3 {
    wire: WireSpatialTrajectoryV3,
}

impl SpatialTrajectoryEnvelopeV3 {
    /// Publish the first root containing the remesh segment.
    ///
    /// # Errors
    /// Returns `EQ0901` unless the segment retains the exact source root and
    /// begins with a remesh-origin pair.
    pub fn start(
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        segment: &SpatialTrajectorySegmentEnvelopeV3,
    ) -> Result<Self, Diagnostic> {
        if segment.origin() != SpatialTrajectorySegmentOriginKindV3::Remesh
            || segment.source_prefix() != source_prefix.digest()?
        {
            return Err(invalid_artifact(
                "V3 trajectory root must start from the exact V2 prefix remesh segment",
            ));
        }
        let value = Self::finish(0, None, source_prefix, vec![segment_reference(segment)?])?;
        Ok(value)
    }

    /// Publish a new immutable root by appending one continuation segment.
    ///
    /// # Errors
    /// Returns `EQ0901` for a changed V2 prefix, broken V3 state edge, or
    /// repeated/substituted segment.
    pub fn extend(
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        previous: &Self,
        segment: &SpatialTrajectorySegmentEnvelopeV3,
    ) -> Result<Self, Diagnostic> {
        if previous.source_prefix() != source_prefix.digest()?
            || segment.source_prefix() != previous.source_prefix()
            || segment.source_state() != previous.source_state()
            || segment.origin() != SpatialTrajectorySegmentOriginKindV3::Continuation
            || segment.wire.origin.predecessor()
                != Some(
                    previous
                        .wire
                        .segments
                        .last()
                        .expect("validated root")
                        .last_state_sha256
                        .as_str(),
                )
        {
            return Err(invalid_artifact(
                "V3 trajectory extension breaks its immutable source or target prefix",
            ));
        }
        let mut segments = previous.wire.segments.clone();
        segments.push(segment_reference(segment)?);
        Self::finish(
            previous
                .wire
                .generation
                .checked_add(1)
                .ok_or_else(|| invalid_artifact("V3 trajectory generation overflows"))?,
            Some(previous.digest()?.to_string()),
            source_prefix,
            segments,
        )
    }

    fn finish(
        generation: u64,
        previous_root_sha256: Option<String>,
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        segments: Vec<WireSegmentReferenceV3>,
    ) -> Result<Self, Diagnostic> {
        let first = &segments[0];
        let value = Self {
            wire: WireSpatialTrajectoryV3 {
                schema: TRAJECTORY_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                generation,
                previous_root_sha256,
                source_prefix_v2_sha256: source_prefix.digest()?.to_string(),
                source_state_v2_sha256: first.source_state_v2_sha256.clone(),
                target_reference: first.target_reference.clone(),
                overlap_sha256: first.overlap_sha256.clone(),
                transfer_receipt_sha256: first.transfer_receipt_sha256.clone(),
                segments,
            },
        };
        value.validate_local(DataExchangeDecoderLimits::default())?;
        Ok(value)
    }

    /// Decode a bounded immutable root.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DataExchangeDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid spatial-trajectory/v3 JSON: {error}"))
        })?;
        let value = Self { wire };
        value.validate_local(limits)?;
        Ok(value)
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!("cannot serialize spatial trajectory v3: {error}"))
        })
    }

    /// Domain-separated immutable-root identity.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            TRAJECTORY_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Root generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.wire.generation
    }

    /// Exact previous V3 root.
    #[must_use]
    pub fn previous_root(&self) -> Option<ArtifactDigest> {
        self.wire.previous_root_sha256.clone().map(ArtifactDigest)
    }

    /// Exact immutable V2 source prefix.
    #[must_use]
    pub fn source_prefix(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.source_prefix_v2_sha256.clone())
    }

    /// Exact finalized V2 source state.
    #[must_use]
    pub fn source_state(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.source_state_v2_sha256.clone())
    }

    /// Complete ordered V3 segment prefix.
    #[must_use]
    pub fn segment_artifacts(&self) -> Vec<ArtifactDigest> {
        self.wire
            .segments
            .iter()
            .map(|segment| ArtifactDigest(segment.segment_sha256.clone()))
            .collect()
    }

    /// Replay the immutable V3 segment prefix and exact V2 source root.
    ///
    /// # Errors
    /// Returns `EQ0901` for a missing, reordered, substituted, or cyclic edge.
    pub fn validate_segments(
        &self,
        source_prefix: &SpatialTrajectoryEnvelopeV2,
        segments: &[SpatialTrajectorySegmentEnvelopeV3],
    ) -> Result<(), Diagnostic> {
        if self.source_prefix() != source_prefix.digest()?
            || segments.len() != self.wire.segments.len()
        {
            return Err(invalid_artifact(
                "V3 trajectory root differs from its exact source or segment count",
            ));
        }
        for (reference, segment) in self.wire.segments.iter().zip(segments) {
            if reference != &segment_reference(segment)? {
                return Err(invalid_artifact(
                    "V3 trajectory segment differs from its exact root reference",
                ));
            }
        }
        let mut replay = Self::start(source_prefix, &segments[0])?;
        for segment in &segments[1..] {
            replay = Self::extend(source_prefix, &replay, segment)?;
        }
        if self == &replay {
            Ok(())
        } else {
            Err(invalid_artifact(
                "V3 trajectory root differs from immutable-prefix replay",
            ))
        }
    }

    fn validate_local(&self, limits: DataExchangeDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != TRAJECTORY_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported spatial-trajectory/v3 schema or encoding",
            ));
        }
        for digest in [
            &self.wire.source_prefix_v2_sha256,
            &self.wire.source_state_v2_sha256,
            &self.wire.overlap_sha256,
            &self.wire.transfer_receipt_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        self.wire.target_reference.validate()?;
        match (&self.wire.previous_root_sha256, self.wire.generation) {
            (None, 0) => {}
            (Some(digest), 1..) => {
                ArtifactDigest::from_hex(digest.clone())?;
            }
            _ => {
                return Err(invalid_artifact(
                    "V3 trajectory root generation and previous edge are inconsistent",
                ));
            }
        }
        if self.wire.segments.is_empty()
            || self.wire.segments.len() > limits.max_remesh_trajectory_segments
        {
            return Err(invalid_artifact(
                "V3 trajectory root is empty or exceeds its segment budget",
            ));
        }
        let mut total_states = 0_usize;
        let mut segment_ids = BTreeSet::new();
        for (index, segment) in self.wire.segments.iter().enumerate() {
            segment.validate()?;
            if !segment_ids.insert(&segment.segment_sha256)
                || segment.source_prefix_v2_sha256 != self.wire.source_prefix_v2_sha256
                || segment.source_state_v2_sha256 != self.wire.source_state_v2_sha256
                || segment.target_reference != self.wire.target_reference
                || segment.overlap_sha256 != self.wire.overlap_sha256
                || segment.transfer_receipt_sha256 != self.wire.transfer_receipt_sha256
                || (index == 0 && segment.origin != WireSegmentOriginTagV3::Remesh)
                || (index > 0 && segment.origin != WireSegmentOriginTagV3::Continuation)
            {
                return Err(invalid_artifact(
                    "V3 trajectory root contains a stale, duplicate, or misclassified segment",
                ));
            }
            total_states = total_states
                .checked_add(segment.state_count)
                .ok_or_else(|| invalid_artifact("V3 trajectory state count overflows"))?;
        }
        if total_states > limits.max_remesh_trajectory_states {
            return Err(invalid_artifact(
                "V3 trajectory root exceeds its aggregate state budget",
            ));
        }
        for pair in self.wire.segments.windows(2) {
            if pair[1].first_predecessor_sha256.as_ref() != Some(&pair[0].last_state_sha256)
                || pair[1].first_step
                    != pair[0]
                        .last_step
                        .checked_add(1)
                        .ok_or_else(|| invalid_artifact("V3 trajectory step overflows"))?
                || pair[1].first_time_s <= pair[0].last_time_s
            {
                return Err(invalid_artifact(
                    "V3 trajectory segments break the exact target append chain",
                ));
            }
        }
        Ok(())
    }
}

fn validate_target_states<M: ReplayableCanonicalModelArtifact>(
    context: &ValidatedMovingSpatialContextV2<'_, M>,
    states: &[SpatialStateEnvelopeV3],
) -> Result<(), Diagnostic> {
    if states.is_empty() {
        return Err(invalid_artifact("V3 trajectory segment is empty"));
    }
    for state in states {
        state.require_context(context)?;
    }
    for pair in states.windows(2) {
        pair[0].require_adjacent(&pair[1])?;
    }
    Ok(())
}

fn segment_reference(
    segment: &SpatialTrajectorySegmentEnvelopeV3,
) -> Result<WireSegmentReferenceV3, Diagnostic> {
    Ok(WireSegmentReferenceV3 {
        segment_sha256: segment.digest()?.to_string(),
        origin: WireSegmentOriginTagV3::encode(segment.origin()),
        source_prefix_v2_sha256: segment.wire.source_prefix_v2_sha256.clone(),
        source_state_v2_sha256: segment.wire.source_state_v2_sha256.clone(),
        target_reference: segment.wire.target_reference.clone(),
        overlap_sha256: segment.wire.overlap_sha256.clone(),
        transfer_receipt_sha256: segment.wire.transfer_receipt_sha256.clone(),
        first_step: segment.first_step(),
        last_step: segment.last_step(),
        first_time_s: segment.first_time_s(),
        last_time_s: segment.last_time_s(),
        first_predecessor_sha256: segment.wire.states[0].predecessor_sha256.clone(),
        last_state_sha256: segment.last_state().to_string(),
        state_count: segment.wire.states.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTrajectorySegmentV3 {
    schema: String,
    encoding: String,
    source_prefix_v2_sha256: String,
    source_state_v2_sha256: String,
    target_reference: WireTargetReferenceV3,
    overlap_sha256: String,
    transfer_receipt_sha256: String,
    origin: WireSegmentOriginV3,
    states: Vec<WireTargetStateReferenceV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireSegmentOriginV3 {
    Remesh {
        source_step: u64,
        source_time_s: f64,
    },
    Continuation {
        predecessor_spatial_state_v3_sha256: String,
    },
}

impl WireSegmentOriginV3 {
    fn predecessor(&self) -> Option<&str> {
        match self {
            Self::Remesh { .. } => None,
            Self::Continuation {
                predecessor_spatial_state_v3_sha256,
            } => Some(predecessor_spatial_state_v3_sha256),
        }
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Remesh { source_time_s, .. }
                if source_time_s.is_finite()
                    && *source_time_s >= 0.0
                    && !is_negative_zero(*source_time_s) =>
            {
                Ok(())
            }
            Self::Continuation {
                predecessor_spatial_state_v3_sha256,
            } => ArtifactDigest::from_hex(predecessor_spatial_state_v3_sha256.clone()).map(drop),
            _ => Err(invalid_artifact("V3 remesh segment source time is invalid")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTargetReferenceV3 {
    model_sha256: String,
    semantic_revision: u64,
    realization_sha256: String,
    geometry_sha256: String,
    correspondence_sha256: String,
    mesh_sha256: String,
}

impl WireTargetReferenceV3 {
    fn from_state(state: &SpatialStateEnvelopeV3) -> Self {
        Self {
            model_sha256: state.model_artifact().to_string(),
            semantic_revision: state.semantic_revision(),
            realization_sha256: state.realization_artifact().to_string(),
            geometry_sha256: state.reference_geometry_artifact().to_string(),
            correspondence_sha256: state.correspondence_artifact().to_string(),
            mesh_sha256: state.reference_mesh_artifact().to_string(),
        }
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        for digest in [
            &self.model_sha256,
            &self.realization_sha256,
            &self.geometry_sha256,
            &self.correspondence_sha256,
            &self.mesh_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        if self.semantic_revision == 0 {
            return Err(invalid_artifact(
                "V3 trajectory target semantic revision must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTargetStateReferenceV3 {
    state_sha256: String,
    origin: WireStateOriginV3,
    step: u64,
    time_s: f64,
    geometry_state_sha256: String,
    predecessor_sha256: Option<String>,
    source_state_v2_sha256: String,
    overlap_sha256: String,
    transfer_receipt_sha256: String,
}

impl WireTargetStateReferenceV3 {
    fn from_state(state: &SpatialStateEnvelopeV3) -> Result<Self, Diagnostic> {
        Ok(Self {
            state_sha256: state.digest()?.to_string(),
            origin: WireStateOriginV3::encode(state.origin()),
            step: state.step(),
            time_s: state.time_s(),
            geometry_state_sha256: state.geometry_state_artifact().to_string(),
            predecessor_sha256: state.predecessor().map(|digest| digest.to_string()),
            source_state_v2_sha256: state.remesh_source_spatial_state().to_string(),
            overlap_sha256: state.overlap_artifact().to_string(),
            transfer_receipt_sha256: state.transfer_receipt_artifact().to_string(),
        })
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        for digest in [
            &self.state_sha256,
            &self.geometry_state_sha256,
            &self.source_state_v2_sha256,
            &self.overlap_sha256,
            &self.transfer_receipt_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        if let Some(predecessor) = &self.predecessor_sha256 {
            ArtifactDigest::from_hex(predecessor.clone())?;
        }
        if !self.time_s.is_finite()
            || self.time_s < 0.0
            || is_negative_zero(self.time_s)
            || matches!(self.origin, WireStateOriginV3::Remesh) != self.predecessor_sha256.is_none()
        {
            return Err(invalid_artifact(
                "V3 trajectory state coordinate or predecessor shape is invalid",
            ));
        }
        Ok(())
    }

    fn require_adjacent(&self, next: &Self) -> Result<(), Diagnostic> {
        if next.origin != WireStateOriginV3::Continuous
            || next.predecessor_sha256.as_deref() != Some(&self.state_sha256)
            || next.step
                != self
                    .step
                    .checked_add(1)
                    .ok_or_else(|| invalid_artifact("V3 trajectory step overflows"))?
            || next.time_s <= self.time_s
            || next.source_state_v2_sha256 != self.source_state_v2_sha256
            || next.overlap_sha256 != self.overlap_sha256
            || next.transfer_receipt_sha256 != self.transfer_receipt_sha256
        {
            return Err(invalid_artifact(
                "V3 trajectory states do not form an exact acyclic append edge",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireStateOriginV3 {
    Remesh,
    Continuous,
}

impl WireStateOriginV3 {
    const fn encode(value: SpatialStateOriginKindV3) -> Self {
        match value {
            SpatialStateOriginKindV3::Remesh => Self::Remesh,
            SpatialStateOriginKindV3::Continuous => Self::Continuous,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpatialTrajectoryV3 {
    schema: String,
    encoding: String,
    generation: u64,
    previous_root_sha256: Option<String>,
    source_prefix_v2_sha256: String,
    source_state_v2_sha256: String,
    target_reference: WireTargetReferenceV3,
    overlap_sha256: String,
    transfer_receipt_sha256: String,
    segments: Vec<WireSegmentReferenceV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSegmentReferenceV3 {
    segment_sha256: String,
    origin: WireSegmentOriginTagV3,
    source_prefix_v2_sha256: String,
    source_state_v2_sha256: String,
    target_reference: WireTargetReferenceV3,
    overlap_sha256: String,
    transfer_receipt_sha256: String,
    first_step: u64,
    last_step: u64,
    first_time_s: f64,
    last_time_s: f64,
    first_predecessor_sha256: Option<String>,
    last_state_sha256: String,
    state_count: usize,
}

impl WireSegmentReferenceV3 {
    fn validate(&self) -> Result<(), Diagnostic> {
        for digest in [
            &self.segment_sha256,
            &self.source_prefix_v2_sha256,
            &self.source_state_v2_sha256,
            &self.overlap_sha256,
            &self.transfer_receipt_sha256,
            &self.last_state_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        if let Some(predecessor) = &self.first_predecessor_sha256 {
            ArtifactDigest::from_hex(predecessor.clone())?;
        }
        self.target_reference.validate()?;
        if self.state_count == 0
            || self.first_step > self.last_step
            || !self.first_time_s.is_finite()
            || !self.last_time_s.is_finite()
            || self.first_time_s < 0.0
            || self.last_time_s < self.first_time_s
            || is_negative_zero(self.first_time_s)
            || is_negative_zero(self.last_time_s)
        {
            return Err(invalid_artifact("V3 trajectory segment summary is invalid"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSegmentOriginTagV3 {
    Remesh,
    Continuation,
}

impl WireSegmentOriginTagV3 {
    const fn encode(value: SpatialTrajectorySegmentOriginKindV3) -> Self {
        match value {
            SpatialTrajectorySegmentOriginKindV3::Remesh => Self::Remesh,
            SpatialTrajectorySegmentOriginKindV3::Continuation => Self::Continuation,
        }
    }
}

fn is_negative_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_negative()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_reference() -> WireTargetReferenceV3 {
        WireTargetReferenceV3 {
            model_sha256: "00".repeat(32),
            semantic_revision: 1,
            realization_sha256: "11".repeat(32),
            geometry_sha256: "22".repeat(32),
            correspondence_sha256: "33".repeat(32),
            mesh_sha256: "44".repeat(32),
        }
    }

    fn segment() -> SpatialTrajectorySegmentEnvelopeV3 {
        SpatialTrajectorySegmentEnvelopeV3 {
            wire: WireTrajectorySegmentV3 {
                schema: SEGMENT_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source_prefix_v2_sha256: "55".repeat(32),
                source_state_v2_sha256: "66".repeat(32),
                target_reference: target_reference(),
                overlap_sha256: "77".repeat(32),
                transfer_receipt_sha256: "88".repeat(32),
                origin: WireSegmentOriginV3::Remesh {
                    source_step: 5,
                    source_time_s: 0.5,
                },
                states: vec![
                    WireTargetStateReferenceV3 {
                        state_sha256: "99".repeat(32),
                        origin: WireStateOriginV3::Remesh,
                        step: 5,
                        time_s: 0.5,
                        geometry_state_sha256: "aa".repeat(32),
                        predecessor_sha256: None,
                        source_state_v2_sha256: "66".repeat(32),
                        overlap_sha256: "77".repeat(32),
                        transfer_receipt_sha256: "88".repeat(32),
                    },
                    WireTargetStateReferenceV3 {
                        state_sha256: "bb".repeat(32),
                        origin: WireStateOriginV3::Continuous,
                        step: 6,
                        time_s: 0.6,
                        geometry_state_sha256: "cc".repeat(32),
                        predecessor_sha256: Some("99".repeat(32)),
                        source_state_v2_sha256: "66".repeat(32),
                        overlap_sha256: "77".repeat(32),
                        transfer_receipt_sha256: "88".repeat(32),
                    },
                ],
            },
        }
    }

    fn root(segment: &SpatialTrajectorySegmentEnvelopeV3) -> SpatialTrajectoryEnvelopeV3 {
        SpatialTrajectoryEnvelopeV3 {
            wire: WireSpatialTrajectoryV3 {
                schema: TRAJECTORY_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                generation: 0,
                previous_root_sha256: None,
                source_prefix_v2_sha256: "55".repeat(32),
                source_state_v2_sha256: "66".repeat(32),
                target_reference: target_reference(),
                overlap_sha256: "77".repeat(32),
                transfer_receipt_sha256: "88".repeat(32),
                segments: vec![segment_reference(segment).unwrap()],
            },
        }
    }

    #[test]
    fn trajectory_v3_wires_roundtrip_and_digests_are_frozen() {
        let segment = segment();
        segment
            .validate_local(DataExchangeDecoderLimits::default())
            .unwrap();
        let segment_bytes = segment.canonical_json().unwrap();
        assert_eq!(
            SpatialTrajectorySegmentEnvelopeV3::from_json(
                &segment_bytes,
                DataExchangeDecoderLimits::default(),
            )
            .unwrap(),
            segment
        );
        assert_eq!(
            segment.digest().unwrap().to_string(),
            "d3eecff525ef6734e4f23fba7bb85a16111d4ac4080494b1c497c7aaa75715a0"
        );

        let root = root(&segment);
        root.validate_local(DataExchangeDecoderLimits::default())
            .unwrap();
        let root_bytes = root.canonical_json().unwrap();
        assert_eq!(
            SpatialTrajectoryEnvelopeV3::from_json(
                &root_bytes,
                DataExchangeDecoderLimits::default()
            )
            .unwrap(),
            root
        );
        assert_eq!(
            root.digest().unwrap().to_string(),
            "1d054468bc22731691d6449e7e2cb05837ba78351c840a5ba602210ee8bbedc0"
        );
    }

    #[test]
    fn trajectory_v3_rejects_broken_edges_and_resource_excess() {
        let value = segment();
        let mut broken = value.clone();
        broken.wire.states[1].predecessor_sha256 = Some("dd".repeat(32));
        assert!(
            broken
                .validate_local(DataExchangeDecoderLimits::default())
                .is_err()
        );

        let limits = DataExchangeDecoderLimits {
            max_remesh_trajectory_states: 1,
            ..DataExchangeDecoderLimits::default()
        };
        assert!(SpatialTrajectorySegmentEnvelopeV3::from_json(
            &value.canonical_json().unwrap(),
            limits,
        )
        .is_err());

        let root = root(&value);
        let root_bytes = root.canonical_json().unwrap();
        let segment_limits = DataExchangeDecoderLimits {
            max_remesh_trajectory_segments: 0,
            ..DataExchangeDecoderLimits::default()
        };
        assert!(SpatialTrajectoryEnvelopeV3::from_json(&root_bytes, segment_limits).is_err());

        let aggregate_state_limits = DataExchangeDecoderLimits {
            max_remesh_trajectory_states: 1,
            ..DataExchangeDecoderLimits::default()
        };
        assert!(
            SpatialTrajectoryEnvelopeV3::from_json(&root_bytes, aggregate_state_limits).is_err()
        );
    }
}
