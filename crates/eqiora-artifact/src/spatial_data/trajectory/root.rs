use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, TrajectoryDecoderLimits, ValidatedFixedSpatialContextV1,
    check_json_limits, invalid_artifact,
};

use super::common::{
    WireFieldIdentity, context_field_inventory, field_ids, validate_field_inventory,
    validate_lineage, validate_time,
};
use super::segment::SpatialTrajectorySegmentEnvelopeV1;

const TRAJECTORY_SCHEMA: &str = "eqiora.spatial-trajectory/v1";

/// Immutable root over a complete prefix of trajectory segments.
///
/// Extending a trajectory creates a new root whose segment list retains the
/// prior list byte-for-byte as a prefix. Run provenance is deliberately
/// separate so the final root can be named as a Run output without a digest
/// cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialTrajectoryEnvelopeV1 {
    wire: WireSpatialTrajectoryV1,
}

impl SpatialTrajectoryEnvelopeV1 {
    /// Publish the first immutable trajectory root.
    ///
    /// # Errors
    /// Returns `EQ0901` only if the segment is internally invalid.
    pub fn start(
        context: &ValidatedFixedSpatialContextV1<'_>,
        segment: &SpatialTrajectorySegmentEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        segment.validate_context(context)?;
        let value = Self {
            wire: WireSpatialTrajectoryV1 {
                schema: TRAJECTORY_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                generation: 0,
                previous_root_sha256: None,
                model_sha256: segment.model_artifact().to_string(),
                realization_sha256: segment.realization_artifact().to_string(),
                geometry_sha256: segment.geometry_artifact().to_string(),
                correspondence_sha256: segment.correspondence_artifact().to_string(),
                mesh_sha256: segment.mesh_artifact().to_string(),
                fields: segment.field_inventory_wire().to_vec(),
                segments: vec![WireSegmentReference::from_segment(segment)?],
            },
        };
        value.validate_local(TrajectoryDecoderLimits::default())?;
        Ok(value)
    }

    /// Publish a new root retaining the complete previous segment prefix.
    ///
    /// # Errors
    /// Returns `EQ0901` for resource/Field drift, duplicate/nonmonotone range,
    /// generation overflow, or any attempt to remesh without an explicit
    /// future transition contract.
    pub fn extend(
        context: &ValidatedFixedSpatialContextV1<'_>,
        previous: &Self,
        segment: &SpatialTrajectorySegmentEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        previous.validate_context(context)?;
        segment.validate_context(context)?;
        if segment.model_artifact() != previous.model_artifact()
            || segment.realization_artifact() != previous.realization_artifact()
            || segment.geometry_artifact() != previous.geometry_artifact()
            || segment.correspondence_artifact() != previous.correspondence_artifact()
            || segment.mesh_artifact() != previous.mesh_artifact()
            || segment.field_inventory_wire() != previous.wire.fields
        {
            return Err(invalid_artifact(
                "trajectory extension changes fixed resources or the exact Field inventory",
            ));
        }
        let prior = previous
            .wire
            .segments
            .last()
            .expect("validated trajectory owns a segment");
        if segment.first_step() <= prior.last_step || segment.first_time_s() <= prior.last_time_s {
            return Err(invalid_artifact(
                "trajectory extension must begin after the previous accepted step and time",
            ));
        }
        let mut segments = previous.wire.segments.clone();
        segments.push(WireSegmentReference::from_segment(segment)?);
        let value = Self {
            wire: WireSpatialTrajectoryV1 {
                schema: TRAJECTORY_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                generation: previous
                    .wire
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| invalid_artifact("trajectory generation overflows u64"))?,
                previous_root_sha256: Some(previous.digest()?.to_string()),
                model_sha256: previous.wire.model_sha256.clone(),
                realization_sha256: previous.wire.realization_sha256.clone(),
                geometry_sha256: previous.wire.geometry_sha256.clone(),
                correspondence_sha256: previous.wire.correspondence_sha256.clone(),
                mesh_sha256: previous.wire.mesh_sha256.clone(),
                fields: previous.wire.fields.clone(),
                segments,
            },
        };
        value.validate_local(TrajectoryDecoderLimits::default())?;
        Ok(value)
    }

    /// Decode without resolving prior roots or segments.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: TrajectoryDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid spatial trajectory JSON: {error}"))
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
            invalid_artifact(format!("cannot serialize spatial trajectory: {error}"))
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

    /// Root generation, starting at zero.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.wire.generation
    }

    /// Exact previous immutable root, when this is an extension.
    #[must_use]
    pub fn previous_root(&self) -> Option<ArtifactDigest> {
        self.wire.previous_root_sha256.clone().map(ArtifactDigest)
    }

    /// Ordered complete segment prefix for bounded partial retrieval.
    #[must_use]
    pub fn segment_artifacts(&self) -> Vec<ArtifactDigest> {
        self.wire
            .segments
            .iter()
            .map(|segment| ArtifactDigest(segment.segment_sha256.clone()))
            .collect()
    }

    /// First accepted step represented by this root.
    #[must_use]
    pub fn first_step(&self) -> u64 {
        self.wire.segments[0].first_step
    }

    /// Last accepted step represented by this root.
    #[must_use]
    pub fn last_step(&self) -> u64 {
        self.wire.segments[self.wire.segments.len() - 1].last_step
    }

    /// Exact selected Semantic Field inventory.
    #[must_use]
    pub fn fields(&self) -> Vec<Id<kinds::Field>> {
        field_ids(&self.wire.fields)
    }

    /// Exact Model artifact.
    #[must_use]
    pub fn model_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.model_sha256.clone())
    }

    /// Exact Realization artifact.
    #[must_use]
    pub fn realization_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.realization_sha256.clone())
    }

    /// Exact geometry revision.
    #[must_use]
    pub fn geometry_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.geometry_sha256.clone())
    }

    /// Exact correspondence artifact.
    #[must_use]
    pub fn correspondence_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.correspondence_sha256.clone())
    }

    /// Exact mesh revision.
    #[must_use]
    pub fn mesh_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.mesh_sha256.clone())
    }

    /// Validate the complete immutable segment list and optional prior root.
    ///
    /// # Errors
    /// Returns `EQ0901` for a missing/substituted segment, broken prefix, or
    /// lineage/range drift.
    pub fn validate_against(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
        previous: Option<&Self>,
        segments: &[SpatialTrajectorySegmentEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        if segments.len() != self.wire.segments.len() {
            return Err(invalid_artifact(
                "trajectory root is missing one or more referenced segments",
            ));
        }
        let mut ordered = segments.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|segment| segment.first_step());
        for segment in &ordered {
            segment.validate_context(context)?;
        }
        let mut replay = Self::start(context, ordered[0])?;
        for segment in ordered.iter().skip(1) {
            replay = Self::extend(context, &replay, segment)?;
        }
        if self != &replay {
            return Err(invalid_artifact(
                "trajectory root differs from exact immutable-segment replay",
            ));
        }
        match (previous, self.previous_root()) {
            (None, None) => Ok(()),
            (Some(previous), Some(digest))
                if previous.wire.segments.len() <= self.wire.segments.len()
                    && previous.digest()? == digest
                    && previous.wire.segments
                        == self.wire.segments[..previous.wire.segments.len()] =>
            {
                Ok(())
            }
            _ => Err(invalid_artifact(
                "trajectory previous-root identity or immutable segment prefix differs",
            )),
        }
    }

    /// Validate the complete ordered segment inventory without requiring the
    /// previous root object.
    ///
    /// This is the bounded dependency check used by reference-only consumers
    /// such as Dataset views. Prefix provenance remains the responsibility of
    /// [`Self::validate_against`].
    ///
    /// # Errors
    /// Returns `EQ0901` for a missing, substituted, reordered, or cross-lineage
    /// segment.
    pub fn validate_segments(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
        segments: &[SpatialTrajectorySegmentEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        if segments.len() != self.wire.segments.len() {
            return Err(invalid_artifact(
                "trajectory root is missing one or more referenced segments",
            ));
        }
        let mut ordered = segments.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|segment| segment.first_step());
        for (expected, segment) in self.wire.segments.iter().zip(ordered) {
            segment.validate_context(context)?;
            if expected != &WireSegmentReference::from_segment(segment)?
                || segment.model_artifact() != self.model_artifact()
                || segment.realization_artifact() != self.realization_artifact()
                || segment.geometry_artifact() != self.geometry_artifact()
                || segment.correspondence_artifact() != self.correspondence_artifact()
                || segment.mesh_artifact() != self.mesh_artifact()
                || segment.field_inventory_wire() != self.wire.fields
            {
                return Err(invalid_artifact(
                    "trajectory segment identity, summary, resources, or Field inventory differs",
                ));
            }
        }
        Ok(())
    }

    fn validate_context(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
    ) -> Result<(), Diagnostic> {
        let expected_fields = context_field_inventory(context);
        if self.model_artifact() != *context.model_reference().artifact()
            || self.realization_artifact() != context.realization().digest()?
            || self.geometry_artifact() != context.geometry().digest()?
            || self.correspondence_artifact() != context.correspondence().digest()?
            || self.mesh_artifact() != context.mesh().digest()?
            || self.wire.fields != expected_fields
        {
            return Err(invalid_artifact(
                "trajectory resources differ from its validated fixed-spatial context",
            ));
        }
        Ok(())
    }

    fn validate_local(&self, limits: TrajectoryDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != TRAJECTORY_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported spatial-trajectory schema or canonical encoding",
            ));
        }
        validate_lineage(
            &self.wire.model_sha256,
            &self.wire.realization_sha256,
            &self.wire.geometry_sha256,
            &self.wire.correspondence_sha256,
            &self.wire.mesh_sha256,
        )?;
        validate_field_inventory(&self.wire.fields, limits.max_spatial_state_fields)?;
        if let Some(previous) = &self.wire.previous_root_sha256 {
            ArtifactDigest::from_hex(previous.clone())?;
        }
        if (self.wire.generation == 0) != self.wire.previous_root_sha256.is_none() {
            return Err(invalid_artifact(
                "trajectory generation zero must be exactly the root without a predecessor",
            ));
        }
        if self.wire.segments.is_empty()
            || self.wire.segments.len() > limits.max_trajectory_segments
            || u64::try_from(self.wire.segments.len() - 1).ok() != Some(self.wire.generation)
        {
            return Err(invalid_artifact(
                "trajectory segment count is empty, excessive, or inconsistent with generation",
            ));
        }
        for segment in &self.wire.segments {
            segment.validate(limits.max_trajectory_segment_states)?;
        }
        let total_states = self
            .wire
            .segments
            .iter()
            .try_fold(0_usize, |total, segment| {
                let state_count = usize::try_from(segment.state_count).map_err(|_| {
                    invalid_artifact("trajectory segment state count exceeds local usize")
                })?;
                total
                    .checked_add(state_count)
                    .ok_or_else(|| invalid_artifact("trajectory aggregate state count overflows"))
            })?;
        if total_states > limits.max_trajectory_states {
            return Err(invalid_artifact(
                "trajectory aggregate state count exceeds the decoder limit",
            ));
        }
        if self.wire.segments.windows(2).any(|pair| {
            pair[0].last_step >= pair[1].first_step || pair[0].last_time_s >= pair[1].first_time_s
        }) {
            return Err(invalid_artifact(
                "trajectory segment ranges must increase strictly without overlap",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpatialTrajectoryV1 {
    schema: String,
    encoding: String,
    generation: u64,
    previous_root_sha256: Option<String>,
    model_sha256: String,
    realization_sha256: String,
    geometry_sha256: String,
    correspondence_sha256: String,
    mesh_sha256: String,
    fields: Vec<WireFieldIdentity>,
    segments: Vec<WireSegmentReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSegmentReference {
    first_step: u64,
    last_step: u64,
    first_time_s: f64,
    last_time_s: f64,
    state_count: u64,
    segment_sha256: String,
}

impl WireSegmentReference {
    fn from_segment(segment: &SpatialTrajectorySegmentEnvelopeV1) -> Result<Self, Diagnostic> {
        Ok(Self {
            first_step: segment.first_step(),
            last_step: segment.last_step(),
            first_time_s: segment.first_time_s(),
            last_time_s: segment.last_time_s(),
            state_count: u64::try_from(segment.state_count())
                .map_err(|_| invalid_artifact("trajectory segment state count exceeds u64"))?,
            segment_sha256: segment.digest()?.to_string(),
        })
    }

    fn validate(&self, state_limit: usize) -> Result<(), Diagnostic> {
        ArtifactDigest::from_hex(self.segment_sha256.clone())?;
        validate_time(self.first_time_s)?;
        validate_time(self.last_time_s)?;
        let state_count = usize::try_from(self.state_count)
            .map_err(|_| invalid_artifact("trajectory segment state count exceeds local usize"))?;
        if self.state_count == 0
            || state_count > state_limit
            || self.first_step > self.last_step
            || self.first_time_s > self.last_time_s
        {
            return Err(invalid_artifact(
                "trajectory segment summary has an invalid range or empty state count",
            ));
        }
        Ok(())
    }
}
