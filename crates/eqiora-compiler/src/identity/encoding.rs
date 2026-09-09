//! Bounded canonical path encoding and stable entity tags.

use super::*;

pub(super) fn collect_path<I, S>(
    segments: I,
    max_segments: usize,
    limits: ElaborationIdentityLimits,
    label: &'static str,
) -> Result<Path, Diagnostic>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut collected = Vec::new();
    let mut byte_len = 0_usize;
    for segment in segments {
        if collected.len() >= max_segments || collected.len() >= usize::from(u16::MAX) {
            return Err(identity_error(format!(
                "{label} exceeds the {max_segments} segment limit"
            )));
        }
        let segment = segment.into();
        if segment.is_empty() {
            return Err(identity_error(format!("{label} contains an empty segment")));
        }
        if segment.len() > limits.max_segment_bytes {
            return Err(identity_error(format!(
                "{label} segment requires {} bytes, exceeding the {} byte limit",
                segment.len(),
                limits.max_segment_bytes
            )));
        }
        byte_len = checked_add(byte_len, segment.len(), "path segment bytes")?;
        if byte_len > limits.max_path_bytes {
            return Err(identity_error(format!(
                "{label} exceeds the {} total byte limit",
                limits.max_path_bytes
            )));
        }
        collected
            .try_reserve(1)
            .map_err(|_| identity_error(format!("cannot reserve {label}")))?;
        collected.push(segment);
    }
    if collected.is_empty() {
        return Err(identity_error(format!("{label} must not be empty")));
    }
    Ok(Path {
        segments: collected,
        byte_len,
    })
}

pub(super) fn encoded_path_len(path: &Path) -> Result<usize, Diagnostic> {
    let prefix_bytes = checked_mul(path.segments.len(), 4, "path length prefixes")?;
    checked_add(
        2,
        checked_add(prefix_bytes, path.byte_len, "encoded path bytes")?,
        "encoded path bytes",
    )
}

pub(super) fn validate_path_against_limits(
    path: &Path,
    max_segments: usize,
    limits: ElaborationIdentityLimits,
    label: &'static str,
) -> Result<(), Diagnostic> {
    if path.segments.len() > max_segments {
        return Err(identity_error(format!(
            "{label} exceeds the {max_segments} segment limit"
        )));
    }
    if path.byte_len > limits.max_path_bytes {
        return Err(identity_error(format!(
            "{label} exceeds the {} total byte limit",
            limits.max_path_bytes
        )));
    }
    if let Some(segment) = path
        .segments
        .iter()
        .find(|segment| segment.len() > limits.max_segment_bytes)
    {
        return Err(identity_error(format!(
            "{label} segment requires {} bytes, exceeding the {} byte limit",
            segment.len(),
            limits.max_segment_bytes
        )));
    }
    Ok(())
}

pub(super) fn canonical_total_len(field_lengths: [usize; 4]) -> Result<usize, Diagnostic> {
    canonical_total_len_for(MAGIC.len(), field_lengths)
}

pub(super) fn canonical_total_len_for<const N: usize>(
    magic_len: usize,
    field_lengths: [usize; N],
) -> Result<usize, Diagnostic> {
    let mut total = checked_add(magic_len, 2, "canonical header bytes")?;
    for length in field_lengths {
        total = checked_add(total, 5, "canonical field header bytes")?;
        total = checked_add(total, length, "canonical field payload bytes")?;
    }
    Ok(total)
}

pub(super) fn write_path_field(
    bytes: &mut Vec<u8>,
    tag: u8,
    path: &Path,
) -> Result<(), Diagnostic> {
    let payload_len = encoded_path_len(path)?;
    bytes.push(tag);
    bytes.extend_from_slice(&as_u32(payload_len, "path payload length")?.to_be_bytes());
    bytes.extend_from_slice(&as_u16(path.segments.len(), "path segment count")?.to_be_bytes());
    for segment in &path.segments {
        bytes.extend_from_slice(&as_u32(segment.len(), "path segment length")?.to_be_bytes());
        bytes.extend_from_slice(segment.as_bytes());
    }
    Ok(())
}

pub(super) fn entity_code(kind: EntityKind) -> Result<u16, Diagnostic> {
    let code = match kind {
        EntityKind::Domain => 1,
        EntityKind::Representation => 2,
        EntityKind::Field => 3,
        EntityKind::Parameter => 4,
        EntityKind::Port => 5,
        EntityKind::Relation => 6,
        EntityKind::Activation => 7,
        EntityKind::Connection => 8,
        EntityKind::ClockDomain => 9,
        EntityKind::Space => 10,
        EntityKind::Discretization => 11,
        EntityKind::SolverPlan => 12,
        EntityKind::Partition => 13,
        EntityKind::Target => 14,
        EntityKind::ExecutionSchedule => 15,
        EntityKind::Experiment => 16,
        EntityKind::Observation => 17,
        EntityKind::Dataset => 18,
        EntityKind::Run => 19,
        EntityKind::Artifact => 20,
        EntityKind::ValidityDomain => 21,
        EntityKind::Evidence => 22,
        EntityKind::Transaction => 23,
        EntityKind::Actor => 24,
        EntityKind::Action => 25,
        EntityKind::Review => 26,
        EntityKind::Approval => 27,
        EntityKind::Policy => 28,
        EntityKind::FiniteSpace => 29,
        EntityKind::IndexSet => 30,
        EntityKind::Enum => 31,
        EntityKind::Record => 32,
        EntityKind::RecordInstance => 33,
        EntityKind::Observable => 34,
        _ => {
            return Err(identity_error(
                "entity kind has no canonical elaboration identity code",
            ));
        }
    };
    Ok(code)
}

pub(super) fn as_u16(value: usize, label: &'static str) -> Result<u16, Diagnostic> {
    u16::try_from(value).map_err(|_| identity_error(format!("{label} exceeds u16")))
}

pub(super) fn as_u32(value: usize, label: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| identity_error(format!("{label} exceeds u32")))
}

pub(super) fn checked_add(
    left: usize,
    right: usize,
    label: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| identity_error(format!("{label} overflows usize")))
}

pub(super) fn checked_mul(
    left: usize,
    right: usize,
    label: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| identity_error(format!("{label} overflows usize")))
}
