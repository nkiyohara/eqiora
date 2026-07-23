use std::collections::BTreeSet;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DecoderLimits, SpatialStateEnvelopeV1,
    ValidatedFixedSpatialContextV1, check_wire_limits, invalid_artifact,
};

use super::common::{
    MAX_EXACT_F64_INTEGER, WireFieldIdentity, context_field_inventory, field_ids, field_inventory,
    require_fixed_step, validate_field_inventory, validate_lineage, validate_time,
};

const SEGMENT_SCHEMA: &str = "eqiora.spatial-trajectory-segment/v1";

/// Nonempty immutable sequence of accepted fixed-resource spatial states.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialTrajectorySegmentEnvelopeV1 {
    wire: WireTrajectorySegmentV1,
}

impl SpatialTrajectorySegmentEnvelopeV1 {
    /// Build one canonical segment from exact accepted states.
    ///
    /// Caller order is normalized by step identity. All states must share the
    /// same exact Model, Realization, geometry, correspondence, mesh, and Field
    /// inventory, and step/time must both increase strictly.
    ///
    /// # Errors
    /// Returns `EQ0901` for an empty, duplicate, nonmonotone, or cross-resource
    /// state sequence.
    pub fn new(
        context: &ValidatedFixedSpatialContextV1<'_>,
        states: &[SpatialStateEnvelopeV1],
    ) -> Result<Self, Diagnostic> {
        let mut states = states.iter().collect::<Vec<_>>();
        states.sort_by_key(|state| state.step());
        let first = states
            .first()
            .copied()
            .ok_or_else(|| invalid_artifact("trajectory segment must contain a state"))?;
        let inventory = field_inventory(first);
        let expected_inventory = context_field_inventory(context);
        if first.model_artifact() != *context.model_reference().artifact()
            || first.realization_artifact() != context.realization().digest()?
            || first.geometry_artifact() != context.geometry().digest()?
            || first.correspondence_artifact() != context.correspondence().digest()?
            || first.mesh_artifact() != context.mesh().digest()?
            || inventory != expected_inventory
        {
            return Err(invalid_artifact(
                "trajectory segment differs from its validated fixed-spatial context",
            ));
        }
        for pair in states.windows(2) {
            if pair[0].step() >= pair[1].step() || pair[0].time_s() >= pair[1].time_s() {
                return Err(invalid_artifact(
                    "trajectory segment accepted step and time identities must increase strictly",
                ));
            }
        }
        for state in &states {
            if state.model_artifact() != first.model_artifact()
                || state.realization_artifact() != first.realization_artifact()
                || state.geometry_artifact() != first.geometry_artifact()
                || state.correspondence_artifact() != first.correspondence_artifact()
                || state.mesh_artifact() != first.mesh_artifact()
                || field_inventory(state) != inventory
            {
                return Err(invalid_artifact(
                    "trajectory segment states do not share exact fixed resources and Field inventory",
                ));
            }
            require_fixed_step(context, state.step(), state.time_s())?;
        }
        let value = Self {
            wire: WireTrajectorySegmentV1 {
                schema: SEGMENT_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                model_sha256: context.model_reference().artifact().to_string(),
                realization_sha256: context.realization().digest()?.to_string(),
                geometry_sha256: context.geometry().digest()?.to_string(),
                correspondence_sha256: context.correspondence().digest()?.to_string(),
                mesh_sha256: context.mesh().digest()?.to_string(),
                fields: inventory,
                states: states
                    .into_iter()
                    .map(|state| {
                        Ok(WireStateReference {
                            step: state.step(),
                            time_s: state.time_s(),
                            state_sha256: state.digest()?.to_string(),
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
            },
        };
        value.validate_local(DecoderLimits::default())?;
        Ok(value)
    }

    /// Decode without resolving referenced states.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid spatial trajectory segment JSON: {error}"))
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
                "cannot serialize spatial trajectory segment: {error}"
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

    /// First accepted step.
    #[must_use]
    pub fn first_step(&self) -> u64 {
        self.wire.states[0].step
    }

    /// Last accepted step.
    #[must_use]
    pub fn last_step(&self) -> u64 {
        self.wire.states[self.wire.states.len() - 1].step
    }

    /// Exact selected Semantic Field inventory.
    #[must_use]
    pub fn fields(&self) -> Vec<Id<kinds::Field>> {
        field_ids(&self.wire.fields)
    }

    /// First accepted coherent-SI time.
    #[must_use]
    pub fn first_time_s(&self) -> f64 {
        self.wire.states[0].time_s
    }

    /// Last accepted coherent-SI time.
    #[must_use]
    pub fn last_time_s(&self) -> f64 {
        self.wire.states[self.wire.states.len() - 1].time_s
    }

    /// Number of accepted states.
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.wire.states.len()
    }

    /// Ordered exact state artifact references for partial retrieval.
    #[must_use]
    pub fn state_artifacts(&self) -> Vec<ArtifactDigest> {
        self.wire
            .states
            .iter()
            .map(|state| ArtifactDigest(state.state_sha256.clone()))
            .collect()
    }

    /// Ordered `(step, time, state artifact)` index for reference-only views.
    #[must_use]
    pub fn states(&self) -> Vec<(u64, f64, ArtifactDigest)> {
        self.wire
            .states
            .iter()
            .map(|state| {
                (
                    state.step,
                    state.time_s,
                    ArtifactDigest(state.state_sha256.clone()),
                )
            })
            .collect()
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

    /// Rebuild and compare one segment from separately loaded states.
    ///
    /// # Errors
    /// Returns `EQ0901` for missing, reordered, substituted, or stale state content.
    pub fn validate_against(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
        states: &[SpatialStateEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        let expected = Self::new(context, states)?;
        if self != &expected {
            return Err(invalid_artifact(
                "trajectory segment differs from exact accepted-state replay",
            ));
        }
        Ok(())
    }

    pub(super) fn field_inventory_wire(&self) -> &[WireFieldIdentity] {
        &self.wire.fields
    }

    pub(super) fn validate_context(
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
                "trajectory segment resources differ from its validated fixed-spatial context",
            ));
        }
        for state in &self.wire.states {
            require_fixed_step(context, state.step, state.time_s)?;
        }
        Ok(())
    }

    fn validate_local(&self, limits: DecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != SEGMENT_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported spatial-trajectory-segment schema or encoding",
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
        if self.wire.states.is_empty()
            || self.wire.states.len() > limits.max_trajectory_segment_states
        {
            return Err(invalid_artifact(
                "trajectory segment state count is empty or exceeds the decoder limit",
            ));
        }
        for state in &self.wire.states {
            ArtifactDigest::from_hex(state.state_sha256.clone())?;
            validate_time(state.time_s)?;
            if state.step > MAX_EXACT_F64_INTEGER {
                return Err(invalid_artifact(
                    "trajectory state step cannot be represented exactly as binary64",
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
                "trajectory segment contains a duplicate state artifact",
            ));
        }
        if self
            .wire
            .states
            .windows(2)
            .any(|pair| pair[0].step >= pair[1].step || pair[0].time_s >= pair[1].time_s)
        {
            return Err(invalid_artifact(
                "trajectory segment wire order must increase strictly in step and time",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTrajectorySegmentV1 {
    schema: String,
    encoding: String,
    model_sha256: String,
    realization_sha256: String,
    geometry_sha256: String,
    correspondence_sha256: String,
    mesh_sha256: String,
    fields: Vec<WireFieldIdentity>,
    states: Vec<WireStateReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStateReference {
    step: u64,
    time_s: f64,
    state_sha256: String,
}
