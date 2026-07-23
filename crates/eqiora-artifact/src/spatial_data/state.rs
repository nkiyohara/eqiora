//! Accepted spatial-state manifest.

use std::collections::BTreeMap;
use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, DecoderLimits, FieldSnapshotEnvelopeV1,
    ValidatedFixedSpatialContextV1, check_wire_limits, invalid_artifact,
};

const SPATIAL_STATE_SCHEMA: &str = "eqiora.spatial-state-envelope/v1";
const MAX_EXACT_F64_INTEGER: u64 = 1_u64 << 53;

/// One accepted physical observation at an exact step and coherent-SI time.
///
/// The Field inventory is the exact inventory of the selected coupled
/// Realization, including an eliminated state Field. Checkpoint-only solver or
/// backend state is intentionally absent.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialStateEnvelopeV1 {
    wire: WireSpatialStateV1,
}

impl SpatialStateEnvelopeV1 {
    /// Construct one complete fixed-mesh accepted state.
    ///
    /// Snapshot input order is not identity: references are canonicalized by
    /// exact Field identity after complete-inventory validation.
    ///
    /// # Errors
    /// Returns `EQ0901` for stale lineage, invalid time, duplicate/missing
    /// Fields, wrong Domain support, or a foreign snapshot.
    pub fn new(
        context: &ValidatedFixedSpatialContextV1<'_>,
        step: u64,
        time_s: f64,
        snapshots: &[FieldSnapshotEnvelopeV1],
    ) -> Result<Self, Diagnostic> {
        if !time_s.is_finite() || time_s < 0.0 || step > MAX_EXACT_F64_INTEGER {
            return Err(invalid_artifact(
                "spatial-state time must be finite and nonnegative and its step exactly representable as binary64",
            ));
        }
        let time_s = normalize_zero(time_s);
        let realization = context.realization();
        let duration = realization.plan()?.time_step().duration().value();
        let expected_time = normalize_zero((step as f64) * duration);
        if time_s != expected_time {
            return Err(invalid_artifact(
                "spatial-state accepted time differs from step times the exact fixed-step Realization duration",
            ));
        }
        let expected = context.represented_fields();
        if snapshots.len() != expected.len() {
            return Err(invalid_artifact(
                "spatial state does not contain the complete Realization Field inventory",
            ));
        }
        let expected_by_field = expected
            .iter()
            .map(|entry| (entry.field().ulid(), entry.domain()))
            .collect::<BTreeMap<_, _>>();
        let mut ordered = snapshots.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|snapshot| snapshot.field().ulid());
        if ordered
            .windows(2)
            .any(|pair| pair[0].field() == pair[1].field())
        {
            return Err(invalid_artifact(
                "spatial state contains a duplicate Semantic Field snapshot",
            ));
        }
        for snapshot in &ordered {
            let Some(domain) = expected_by_field.get(&snapshot.field().ulid()) else {
                return Err(invalid_artifact(
                    "spatial state contains a Field outside its Realization inventory",
                ));
            };
            if snapshot.model_artifact() != *context.model_reference().artifact()
                || snapshot.realization_artifact() != realization.digest()?
                || snapshot.geometry_artifact() != context.geometry().digest()?
                || snapshot.correspondence_artifact() != context.correspondence().digest()?
                || snapshot.mesh_artifact() != context.mesh().digest()?
                || snapshot.support_domain() != *domain
            {
                return Err(invalid_artifact(
                    "spatial state snapshot lineage or exact Domain support differs",
                ));
            }
        }
        let value = Self {
            wire: WireSpatialStateV1 {
                schema: SPATIAL_STATE_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                model_sha256: context.model_reference().artifact().to_string(),
                semantic_revision: context.model_reference().semantic_revision().get(),
                realization_sha256: realization.digest()?.to_string(),
                geometry_sha256: context.geometry().digest()?.to_string(),
                correspondence_sha256: context.correspondence().digest()?.to_string(),
                mesh_sha256: context.mesh().digest()?.to_string(),
                accepted: WireAcceptedStep { step, time_s },
                fields: ordered
                    .into_iter()
                    .map(|snapshot| {
                        Ok(WireStateField {
                            support_domain_ulid: snapshot.support_domain().ulid().to_string(),
                            field_ulid: snapshot.field().ulid().to_string(),
                            snapshot_sha256: snapshot.digest()?.to_string(),
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
            },
        };
        value.validate_local(DecoderLimits::default())?;
        Ok(value)
    }

    /// Decode the closed state manifest without resolving snapshots.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, oversized, unknown, or noncanonical data.
    pub fn from_json(bytes: &[u8], limits: DecoderLimits) -> Result<Self, Diagnostic> {
        check_wire_limits(bytes, limits)?;
        let wire = serde_json::from_slice(bytes)
            .map_err(|error| invalid_artifact(format!("invalid spatial state JSON: {error}")))?;
        let value = Self { wire };
        value.validate_local(limits)?;
        Ok(value)
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire)
            .map_err(|error| invalid_artifact(format!("cannot serialize spatial state: {error}")))
    }

    /// Domain-separated identity of the accepted state and exact snapshot set.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            SPATIAL_STATE_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Accepted step ordinal.
    #[must_use]
    pub const fn step(&self) -> u64 {
        self.wire.accepted.step
    }

    /// Accepted coherent-SI time in seconds.
    #[must_use]
    pub const fn time_s(&self) -> f64 {
        self.wire.accepted.time_s
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

    /// Canonically ordered `(Domain, Field, snapshot)` references.
    #[must_use]
    pub fn fields(&self) -> Vec<(Id<kinds::Domain>, Id<kinds::Field>, ArtifactDigest)> {
        self.wire
            .fields
            .iter()
            .map(|field| {
                (
                    parse_id(&field.support_domain_ulid, "support Domain")
                        .expect("validated state Domain ULID"),
                    parse_id(&field.field_ulid, "Field").expect("validated state Field ULID"),
                    ArtifactDigest(field.snapshot_sha256.clone()),
                )
            })
            .collect()
    }

    /// Look up one exact Field without loading unrelated snapshots.
    #[must_use]
    pub fn field_snapshot(&self, field: Id<kinds::Field>) -> Option<ArtifactDigest> {
        self.wire
            .fields
            .binary_search_by_key(&field.ulid(), |entry| {
                Ulid::from_str(&entry.field_ulid).expect("validated state Field ULID")
            })
            .ok()
            .map(|index| ArtifactDigest(self.wire.fields[index].snapshot_sha256.clone()))
    }

    /// Rebuild and compare this state from exact dependencies.
    ///
    /// # Errors
    /// Returns `EQ0901` for missing, substituted, incomplete, or stale content.
    pub fn validate_against(
        &self,
        context: &ValidatedFixedSpatialContextV1<'_>,
        snapshots: &[FieldSnapshotEnvelopeV1],
    ) -> Result<(), Diagnostic> {
        let expected = Self::new(context, self.step(), self.time_s(), snapshots)?;
        if self != &expected {
            return Err(invalid_artifact(
                "spatial state differs from exact Field snapshot replay",
            ));
        }
        Ok(())
    }

    fn validate_local(&self, limits: DecoderLimits) -> Result<(), Diagnostic> {
        if self.wire.schema != SPATIAL_STATE_SCHEMA || self.wire.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(
                "unsupported spatial-state schema or canonical encoding",
            ));
        }
        for digest in [
            &self.wire.model_sha256,
            &self.wire.realization_sha256,
            &self.wire.geometry_sha256,
            &self.wire.correspondence_sha256,
            &self.wire.mesh_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        if !self.wire.accepted.time_s.is_finite()
            || self.wire.accepted.time_s < 0.0
            || is_negative_zero(self.wire.accepted.time_s)
            || self.wire.accepted.step > MAX_EXACT_F64_INTEGER
        {
            return Err(invalid_artifact(
                "spatial-state coordinate must be finite, nonnegative, canonical, and exactly representable",
            ));
        }
        if self.wire.fields.is_empty() || self.wire.fields.len() > limits.max_spatial_state_fields {
            return Err(invalid_artifact(
                "spatial-state Field inventory is empty or exceeds the decoder limit",
            ));
        }
        let mut prior = None;
        for field in &self.wire.fields {
            parse_id::<kinds::Domain>(&field.support_domain_ulid, "support Domain")?;
            let id = parse_id::<kinds::Field>(&field.field_ulid, "Field")?;
            ArtifactDigest::from_hex(field.snapshot_sha256.clone())?;
            if prior.is_some_and(|prior| prior >= id.ulid()) {
                return Err(invalid_artifact(
                    "spatial-state Fields must be unique and in canonical identity order",
                ));
            }
            prior = Some(id.ulid());
        }
        Ok(())
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn is_negative_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_negative()
}

fn parse_id<E: eqiora_core::Entity>(value: &str, label: &str) -> Result<Id<E>, Diagnostic> {
    let parsed = Ulid::from_str(value)
        .map_err(|_| invalid_artifact(format!("{label} ULID is malformed")))?;
    if parsed.to_string() != value {
        return Err(invalid_artifact(format!(
            "{label} ULID is not in canonical spelling"
        )));
    }
    Ok(Id::from_ulid(parsed))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSpatialStateV1 {
    schema: String,
    encoding: String,
    model_sha256: String,
    semantic_revision: u64,
    realization_sha256: String,
    geometry_sha256: String,
    correspondence_sha256: String,
    mesh_sha256: String,
    accepted: WireAcceptedStep,
    fields: Vec<WireStateField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAcceptedStep {
    step: u64,
    time_s: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStateField {
    support_domain_ulid: String,
    field_ulid: String,
    snapshot_sha256: String,
}
