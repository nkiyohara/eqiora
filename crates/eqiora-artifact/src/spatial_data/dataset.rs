//! Reference-only derived Dataset view.

use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DataExchangeDecoderLimits, SpatialTrajectoryEnvelopeV1,
    ValidatedFixedSpatialContextV1, check_json_limits, invalid_artifact,
};

const DATASET_VIEW_SCHEMA: &str = "eqiora.dataset-view-envelope/v1";

/// Reference-only selection over an immutable spatial trajectory.
///
/// V1 deliberately supports only identity transformation, no normalization,
/// and one unpartitioned split. It copies no numerical values; richer Dataset
/// materialization and ML semantics remain separate later contracts.
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetViewEnvelopeV1 {
    wire: WireDatasetViewV1,
}

impl DatasetViewEnvelopeV1 {
    /// Select an inclusive accepted-step window and exact Field subset.
    ///
    /// Input Field order is normalized by identity.
    ///
    /// # Errors
    /// Returns `EQ0901` for an empty/duplicate/foreign Field selection or a
    /// window outside the source trajectory.
    pub fn identity_window(
        context: &ValidatedFixedSpatialContextV1<'_>,
        trajectory: &SpatialTrajectoryEnvelopeV1,
        segments: &[crate::SpatialTrajectorySegmentEnvelopeV1],
        first_step: u64,
        last_step: u64,
        fields: impl IntoIterator<Item = Id<kinds::Field>>,
    ) -> Result<Self, Diagnostic> {
        trajectory.validate_segments(context, segments)?;
        if first_step > last_step
            || first_step < trajectory.first_step()
            || last_step > trajectory.last_step()
        {
            return Err(invalid_artifact(
                "Dataset view step window lies outside its source trajectory",
            ));
        }
        let mut fields = fields.into_iter().collect::<Vec<_>>();
        fields.sort_by_key(Id::ulid);
        if fields.is_empty() || fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid_artifact(
                "Dataset view requires a nonempty unique Field selection",
            ));
        }
        let available = trajectory.fields();
        if fields.iter().any(|field| {
            available
                .binary_search_by_key(&field.ulid(), Id::ulid)
                .is_err()
        }) {
            return Err(invalid_artifact(
                "Dataset view selects a Field outside its source trajectory",
            ));
        }
        let mut selected_states = segments
            .iter()
            .flat_map(crate::SpatialTrajectorySegmentEnvelopeV1::states)
            .filter(|(step, _, _)| *step >= first_step && *step <= last_step)
            .collect::<Vec<_>>();
        selected_states.sort_by_key(|(step, _, _)| *step);
        if selected_states.first().map(|entry| entry.0) != Some(first_step)
            || selected_states.last().map(|entry| entry.0) != Some(last_step)
        {
            return Err(invalid_artifact(
                "Dataset view window endpoints are not accepted states in the source trajectory",
            ));
        }
        let value = Self {
            wire: WireDatasetViewV1 {
                schema: DATASET_VIEW_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                trajectory_sha256: trajectory.digest()?.to_string(),
                window: WireStepWindow {
                    first_step,
                    last_step,
                },
                states: selected_states
                    .into_iter()
                    .map(|(step, time_s, state)| WireSelectedState {
                        step,
                        time_s,
                        state_sha256: state.to_string(),
                    })
                    .collect(),
                field_ulids: fields
                    .into_iter()
                    .map(|field| field.ulid().to_string())
                    .collect(),
                transformation: WireTransformation::Identity,
                normalization: WireNormalization::None,
                split: WireSplit::Unpartitioned,
            },
        };
        value.validate_local(DataExchangeDecoderLimits::default())?;
        Ok(value)
    }

    /// Decode without loading the source trajectory or any numerical values.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DataExchangeDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits.json)?;
        let wire = serde_json::from_slice(bytes)
            .map_err(|error| invalid_artifact(format!("invalid Dataset view JSON: {error}")))?;
        let value = Self { wire };
        value.validate_local(limits)?;
        Ok(value)
    }

    /// Deterministic canonical JSON bytes containing no numerical values.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire)
            .map_err(|error| invalid_artifact(format!("cannot serialize Dataset view: {error}")))
    }

    /// Domain-separated derived-view identity.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            DATASET_VIEW_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Exact immutable source trajectory root.
    #[must_use]
    pub fn trajectory(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.trajectory_sha256.clone())
    }

    /// Inclusive accepted-step window.
    #[must_use]
    pub const fn step_window(&self) -> (u64, u64) {
        (self.wire.window.first_step, self.wire.window.last_step)
    }

    /// Canonically ordered exact Field selection.
    #[must_use]
    pub fn fields(&self) -> Vec<Id<kinds::Field>> {
        self.wire
            .field_ulids
            .iter()
            .map(|field| {
                Ulid::from_str(field)
                    .map(Id::from_ulid)
                    .expect("validated Dataset view Field ULID")
            })
            .collect()
    }

    /// Exact selected accepted-state references, with no copied Field values.
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

    /// Rebuild and compare the selection against its exact source trajectory.
    ///
    /// # Errors
    /// Returns `EQ0901` for source substitution or selection drift.
    pub fn validate_against(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
        trajectory: &SpatialTrajectoryEnvelopeV1,
        segments: &[crate::SpatialTrajectorySegmentEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        let expected = Self::identity_window(
            context,
            trajectory,
            segments,
            self.wire.window.first_step,
            self.wire.window.last_step,
            self.fields(),
        )?;
        if self != &expected {
            return Err(invalid_artifact(
                "Dataset view differs from exact source-trajectory replay",
            ));
        }
        Ok(())
    }

    fn validate_local(&self, limits: DataExchangeDecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != DATASET_VIEW_SCHEMA
            || self.wire.encoding != CANONICAL_ENCODING
            || self.wire.transformation != WireTransformation::Identity
            || self.wire.normalization != WireNormalization::None
            || self.wire.split != WireSplit::Unpartitioned
        {
            return Err(invalid_artifact(
                "unsupported Dataset view schema, transform, normalization, or split",
            ));
        }
        ArtifactDigest::from_hex(self.wire.trajectory_sha256.clone())?;
        if self.wire.window.first_step > self.wire.window.last_step {
            return Err(invalid_artifact("Dataset view step window is reversed"));
        }
        if self.wire.states.is_empty() || self.wire.states.len() > limits.max_trajectory_states {
            return Err(invalid_artifact(
                "Dataset view state selection is empty or exceeds the decoder limit",
            ));
        }
        for state in &self.wire.states {
            ArtifactDigest::from_hex(state.state_sha256.clone())?;
            if !state.time_s.is_finite()
                || state.time_s < 0.0
                || (state.time_s == 0.0 && state.time_s.is_sign_negative())
            {
                return Err(invalid_artifact(
                    "Dataset view state time is not finite nonnegative canonical time",
                ));
            }
        }
        if self.wire.states.first().map(|state| state.step) != Some(self.wire.window.first_step)
            || self.wire.states.last().map(|state| state.step) != Some(self.wire.window.last_step)
            || self
                .wire
                .states
                .windows(2)
                .any(|pair| pair[0].step >= pair[1].step || pair[0].time_s >= pair[1].time_s)
        {
            return Err(invalid_artifact(
                "Dataset view selected states must cover exact endpoints in strict source order",
            ));
        }
        if self.wire.field_ulids.is_empty()
            || self.wire.field_ulids.len() > limits.max_dataset_view_fields
        {
            return Err(invalid_artifact(
                "Dataset view Field selection is empty or exceeds the decoder limit",
            ));
        }
        let mut prior = None;
        for field in &self.wire.field_ulids {
            let parsed = Ulid::from_str(field)
                .map_err(|_| invalid_artifact("Dataset view Field ULID is malformed"))?;
            if parsed.to_string() != *field || prior.is_some_and(|prior| prior >= parsed) {
                return Err(invalid_artifact(
                    "Dataset view Fields must use canonical ULIDs and be unique in identity order",
                ));
            }
            prior = Some(parsed);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDatasetViewV1 {
    schema: String,
    encoding: String,
    trajectory_sha256: String,
    window: WireStepWindow,
    states: Vec<WireSelectedState>,
    field_ulids: Vec<String>,
    transformation: WireTransformation,
    normalization: WireNormalization,
    split: WireSplit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSelectedState {
    step: u64,
    time_s: f64,
    state_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStepWindow {
    first_step: u64,
    last_step: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireTransformation {
    Identity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireNormalization {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSplit {
    Unpartitioned,
}
