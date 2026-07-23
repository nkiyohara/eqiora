use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    ArtifactDigest, SpatialStateEnvelopeV1, ValidatedFixedSpatialContextV1, invalid_artifact,
};

pub(super) const MAX_EXACT_F64_INTEGER: u64 = 1_u64 << 53;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireFieldIdentity {
    pub(super) field_ulid: String,
    pub(super) support_domain_ulid: String,
}

pub(super) fn field_inventory(state: &SpatialStateEnvelopeV1) -> Vec<WireFieldIdentity> {
    state
        .fields()
        .into_iter()
        .map(|(domain, field, _)| WireFieldIdentity {
            support_domain_ulid: domain.ulid().to_string(),
            field_ulid: field.ulid().to_string(),
        })
        .collect()
}

pub(super) fn field_ids(fields: &[WireFieldIdentity]) -> Vec<Id<kinds::Field>> {
    fields
        .iter()
        .map(|field| {
            Ulid::from_str(&field.field_ulid)
                .map(Id::from_ulid)
                .expect("validated trajectory Field ULID")
        })
        .collect()
}

pub(super) fn context_field_inventory(
    context: &ValidatedFixedSpatialContextV1<'_>,
) -> Vec<WireFieldIdentity> {
    context
        .represented_fields()
        .iter()
        .map(|entry| WireFieldIdentity {
            field_ulid: entry.field().ulid().to_string(),
            support_domain_ulid: entry.domain().ulid().to_string(),
        })
        .collect()
}

pub(super) fn validate_lineage(
    model: &str,
    realization: &str,
    geometry: &str,
    correspondence: &str,
    mesh: &str,
) -> Result<(), Diagnostic> {
    for digest in [model, realization, geometry, correspondence, mesh] {
        ArtifactDigest::from_hex(digest.to_owned())?;
    }
    Ok(())
}

pub(super) fn validate_field_inventory(
    fields: &[WireFieldIdentity],
    limit: usize,
) -> Result<(), Diagnostic> {
    if fields.is_empty() || fields.len() > limit {
        return Err(invalid_artifact(
            "trajectory Field inventory is empty or exceeds the decoder limit",
        ));
    }
    let mut prior_field = None;
    for entry in fields {
        let domain = Ulid::from_str(&entry.support_domain_ulid)
            .map_err(|_| invalid_artifact("trajectory support Domain ULID is malformed"))?;
        let field = Ulid::from_str(&entry.field_ulid)
            .map_err(|_| invalid_artifact("trajectory Field ULID is malformed"))?;
        if domain.to_string() != entry.support_domain_ulid
            || field.to_string() != entry.field_ulid
            || prior_field.is_some_and(|prior| prior >= field)
        {
            return Err(invalid_artifact(
                "trajectory Fields must use canonical ULIDs and be unique in Field identity order",
            ));
        }
        prior_field = Some(field);
    }
    Ok(())
}

pub(super) fn validate_time(time: f64) -> Result<(), Diagnostic> {
    if !time.is_finite() || time < 0.0 || (time == 0.0 && time.is_sign_negative()) {
        Err(invalid_artifact(
            "trajectory accepted time must be finite, nonnegative, and canonical",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn require_fixed_step(
    context: &ValidatedFixedSpatialContextV1<'_>,
    step: u64,
    time_s: f64,
) -> Result<(), Diagnostic> {
    if step > MAX_EXACT_F64_INTEGER {
        return Err(invalid_artifact(
            "trajectory step cannot be represented exactly as binary64",
        ));
    }
    let duration = context.realization().plan()?.time_step().duration().value();
    let expected = if step == 0 {
        0.0
    } else {
        (step as f64) * duration
    };
    if time_s != expected {
        return Err(invalid_artifact(
            "trajectory accepted time differs from step times the exact fixed-step duration",
        ));
    }
    Ok(())
}
