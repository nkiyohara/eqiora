//! Identity-bearing projection and transcript helpers for the fixed protocol.

use std::collections::BTreeMap;

use eqiora_artifact::{ArtifactDigest, PrescribedDynamicSolidProviderOccurrenceEnvelopeV1};
use eqiora_core::Diagnostic;
use eqiora_meshing::VertexId;
use sha2::{Digest, Sha256};

use super::super::composition::PreparedPrescribedDynamicSolid3d;
use super::invalid;

pub(super) mod control;
pub(super) mod frame;

use control::{
    Bind, Bound, Candidate, Close, Closed, Evaluate, InputDescriptor, OutputDescriptor, Report,
};
use frame::{Frame, FrameKind};

const INPUT_DOMAIN: &[u8] = b"eqiora.prescribed-dynamic-solid-provider-input-block/v1";
const BINDING_DOMAIN: &[u8] = b"eqiora.prescribed-dynamic-solid-provider-binding/v1";
const REQUEST_DOMAIN: &[u8] = b"eqiora.prescribed-dynamic-solid-provider-request/v1";
const CANDIDATE_DOMAIN: &[u8] = b"eqiora.prescribed-dynamic-solid-provider-candidate/v1";
const TRANSCRIPT_DOMAIN: &[u8] = b"eqiora.prescribed-dynamic-solid-provider-transcript/v1";
const MAX_TRANSCRIPT_BYTES: usize = 36_864;
const VERTEX_INDICES: [usize; 4] = [1, 3, 5, 7];
type CandidateTrace = Vec<(VertexId, [f64; 3])>;

pub(super) struct Exchange {
    pub(super) bind: Bind,
    pub(super) binding_identity: ArtifactDigest,
    pub(super) evaluate: Evaluate,
    pub(super) request_identity: ArtifactDigest,
    pub(super) displacement_bulk: Vec<u8>,
    pub(super) velocity_bulk: Vec<u8>,
    pub(super) displacement_input_identity: ArtifactDigest,
    pub(super) velocity_input_identity: ArtifactDigest,
    output_header: Vec<u8>,
}

impl Exchange {
    pub(super) fn new(prepared: &PreparedPrescribedDynamicSolid3d) -> Result<Self, Diagnostic> {
        let displacement_bulk = trace_bytes(&prepared.prior_displacement)?;
        let velocity_bulk = trace_bytes(&prepared.prior_velocity)?;
        let output = output_descriptor(prepared);
        let model = prepared.realization.model_artifact().to_string();
        let realization = prepared.realization.digest()?.to_string();
        let prior_state = prepared.prior_state.digest()?.to_string();
        let boundary = prepared.realization.driven_boundary().ulid().to_string();
        let displacement_header = control::encode_input_header(
            model.clone(),
            realization.clone(),
            prior_state.clone(),
            boundary.clone(),
            prepared.realization.displacement_field().ulid().to_string(),
            "prior-displacement-trace",
            "m",
        )?;
        let velocity_header = control::encode_input_header(
            model.clone(),
            realization.clone(),
            prior_state.clone(),
            boundary.clone(),
            prepared.realization.velocity_field().ulid().to_string(),
            "prior-velocity-trace",
            "m/s",
        )?;
        let displacement_input_identity = block_identity(&displacement_header, &displacement_bulk);
        let velocity_input_identity = block_identity(&velocity_header, &velocity_bulk);
        let bind = Bind::exact(
            model,
            realization,
            prepared.geometry.digest()?.to_string(),
            prepared.correspondence.digest()?.to_string(),
            prepared.mesh.digest()?.to_string(),
            prior_state,
            prepared.realization.solid_domain().ulid().to_string(),
            boundary,
            vec![
                input_descriptor(
                    "prior-displacement-trace",
                    prepared.realization.displacement_field().ulid().to_string(),
                    "m",
                    &displacement_input_identity,
                ),
                input_descriptor(
                    "prior-velocity-trace",
                    prepared.realization.velocity_field().ulid().to_string(),
                    "m/s",
                    &velocity_input_identity,
                ),
            ],
            output.clone(),
        );
        let bind_bytes = control::encode(&bind)?;
        let binding_identity = single_payload_identity(BINDING_DOMAIN, &bind_bytes);
        let evaluate = Evaluate {
            kind: "evaluate".to_owned(),
            binding_sha256: binding_identity.to_string(),
        };
        let evaluate_bytes = control::encode(&evaluate)?;
        let request_identity = single_payload_identity(REQUEST_DOMAIN, &evaluate_bytes);
        let output_header = control::encode(&output)?;
        Ok(Self {
            bind,
            binding_identity,
            evaluate,
            request_identity,
            displacement_bulk,
            velocity_bulk,
            displacement_input_identity,
            velocity_input_identity,
            output_header,
        })
    }

    pub(super) fn validate_bound(&self, bound: &Bound) -> Result<(), Diagnostic> {
        if bound.kind != "bound" || bound.binding_sha256 != self.binding_identity.as_str() {
            return Err(invalid(
                "provider bound control differs from the exact binding identity",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_candidate_control(
        &self,
        candidate: &Candidate,
    ) -> Result<(), Diagnostic> {
        if candidate.kind != "candidate"
            || candidate.request_sha256 != self.request_identity.as_str()
            || candidate.byte_length != frame::BULK_BYTES as u64
        {
            return Err(invalid(
                "provider candidate control differs from the active request or output length",
            ));
        }
        ArtifactDigest::from_hex(candidate.candidate_sha256.clone())
            .map_err(|_| invalid("provider candidate identity is not a canonical digest"))?;
        Ok(())
    }

    pub(super) fn admit_candidate(
        &self,
        candidate_control: &Candidate,
        bulk: &[u8],
    ) -> Result<(ArtifactDigest, CandidateTrace), Diagnostic> {
        self.validate_candidate_control(candidate_control)?;
        let identity = candidate_identity(&self.request_identity, &self.output_header, bulk);
        if candidate_control.candidate_sha256 != identity.as_str() {
            return Err(invalid(
                "provider candidate identity differs from request, output header, or bulk bytes",
            ));
        }
        Ok((identity, decode_candidate(bulk)?))
    }

    pub(super) fn candidate_identity_for_bulk(&self, bulk: &[u8]) -> ArtifactDigest {
        candidate_identity(&self.request_identity, &self.output_header, bulk)
    }

    pub(super) fn validate_report(
        &self,
        report: &Report,
        candidate_identity: &ArtifactDigest,
    ) -> Result<(), Diagnostic> {
        if report.kind != "report"
            || report.request_sha256 != self.request_identity.as_str()
            || report.candidate_sha256 != candidate_identity.as_str()
            || report.status != "success"
            || report.code != control::SUCCESS_CODE
            || report.message != control::SUCCESS_MESSAGE
        {
            return Err(invalid(
                "provider report differs from the frozen successful response",
            ));
        }
        Ok(())
    }

    pub(super) fn close(&self, candidate_identity: &ArtifactDigest) -> Close {
        Close {
            kind: "close".to_owned(),
            request_sha256: self.request_identity.to_string(),
            candidate_sha256: candidate_identity.to_string(),
            outcome: "accepted".to_owned(),
        }
    }

    pub(super) fn expected_closed(&self, candidate_identity: &ArtifactDigest) -> Closed {
        Closed {
            kind: "closed".to_owned(),
            request_sha256: self.request_identity.to_string(),
            candidate_sha256: candidate_identity.to_string(),
        }
    }

    pub(super) fn provider_dependencies(&self) -> BTreeMap<String, String> {
        self.bind
            .provider
            .dependencies
            .iter()
            .map(|entry| (entry.name.clone(), entry.release.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Transcript {
    bytes: Vec<u8>,
    frame_count: usize,
    control_count: usize,
    bulk_count: usize,
    aggregate_bulk_bytes: usize,
}

impl Transcript {
    pub(super) fn record(&mut self, direction: Direction, frame: &Frame) -> Result<(), Diagnostic> {
        let added = 1_usize
            .checked_add(frame::PREFIX_BYTES)
            .and_then(|size| size.checked_add(frame.payload().len()))
            .ok_or_else(|| invalid("provider transcript size overflowed usize"))?;
        if self.bytes.len().saturating_add(added) > MAX_TRANSCRIPT_BYTES {
            return Err(invalid(
                "provider transcript exceeds the successful byte budget",
            ));
        }
        self.bytes.push(direction as u8);
        self.bytes.extend_from_slice(&frame.prefix());
        self.bytes.extend_from_slice(frame.payload());
        self.frame_count += 1;
        match frame.kind() {
            FrameKind::Control => self.control_count += 1,
            FrameKind::Bulk => {
                self.bulk_count += 1;
                self.aggregate_bulk_bytes = self
                    .aggregate_bulk_bytes
                    .checked_add(frame.payload().len())
                    .ok_or_else(|| invalid("provider aggregate bulk size overflowed usize"))?;
                if self.aggregate_bulk_bytes > 288 {
                    return Err(invalid("provider aggregate bulk exceeds 288 bytes"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn prospective(&self, close: &Close, closed: &Closed) -> Result<Self, Diagnostic> {
        let mut prospective = self.clone();
        prospective.record(
            Direction::Outgoing,
            &Frame::control(control::encode(close)?)?,
        )?;
        prospective.record(
            Direction::Incoming,
            &Frame::control(control::encode(closed)?)?,
        )?;
        prospective.validate_success()?;
        Ok(prospective)
    }

    pub(super) fn validate_success(&self) -> Result<(), Diagnostic> {
        if self.frame_count != 11
            || self.control_count != 8
            || self.bulk_count != 3
            || self.aggregate_bulk_bytes != 288
        {
            return Err(invalid(
                "provider transcript frame inventory differs from the successful state machine",
            ));
        }
        Ok(())
    }

    pub(super) fn identity(&self) -> ArtifactDigest {
        single_payload_identity(TRANSCRIPT_DOMAIN, &self.bytes)
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub(super) enum Direction {
    Outgoing = 0x00,
    Incoming = 0x01,
}

fn input_descriptor(
    role: &str,
    field_ulid: String,
    unit: &str,
    identity: &ArtifactDigest,
) -> InputDescriptor {
    InputDescriptor {
        role: role.to_owned(),
        field_ulid,
        unit: unit.to_owned(),
        value_shape: [3],
        frame: "spatial-cartesian".to_owned(),
        representation: "continuous-lagrange-p1-trace".to_owned(),
        association: "vertex".to_owned(),
        coefficient_count: 12,
        byte_length: 96,
        block_sha256: identity.to_string(),
    }
}

fn output_descriptor(prepared: &PreparedPrescribedDynamicSolid3d) -> OutputDescriptor {
    OutputDescriptor {
        role: "next-total-displacement".to_owned(),
        field_ulid: prepared.realization.displacement_field().ulid().to_string(),
        unit: "m".to_owned(),
        value_shape: [3],
        frame: "spatial-cartesian".to_owned(),
        representation: "continuous-lagrange-p1-trace".to_owned(),
        association: "vertex".to_owned(),
        convention: "total-reference-configuration".to_owned(),
        coefficient_count: 12,
        byte_length: 96,
    }
}

fn trace_bytes(values: &[(VertexId, [f64; 3])]) -> Result<Vec<u8>, Diagnostic> {
    if values.len() != 9
        || values.iter().enumerate().any(|(index, (vertex, value))| {
            vertex.index() != index
                || value
                    .iter()
                    .any(|component| !component.is_finite() || is_negative_zero(*component))
        })
    {
        return Err(invalid(
            "provider projection source is not a complete canonical finite vertex field",
        ));
    }
    let mut bytes = Vec::with_capacity(frame::BULK_BYTES);
    for index in VERTEX_INDICES {
        for component in values[index].1 {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    Ok(bytes)
}

pub(super) fn decode_candidate(bytes: &[u8]) -> Result<Vec<(VertexId, [f64; 3])>, Diagnostic> {
    if bytes.len() != frame::BULK_BYTES {
        return Err(invalid(
            "provider candidate bulk must contain exactly 96 bytes",
        ));
    }
    let mut values = Vec::with_capacity(12);
    for chunk in bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| chunk.as_slice())
    {
        let value = f64::from_le_bytes(chunk.try_into().expect("f64 chunk has eight bytes"));
        if !value.is_finite() || is_negative_zero(value) {
            return Err(invalid(
                "provider candidate contains non-finite or negative-zero binary64",
            ));
        }
        values.push(value);
    }
    Ok(VERTEX_INDICES
        .into_iter()
        .enumerate()
        .map(|(position, vertex)| {
            let offset = position * 3;
            (
                VertexId::new(vertex),
                [values[offset], values[offset + 1], values[offset + 2]],
            )
        })
        .collect())
}

fn block_identity(header: &[u8], bulk: &[u8]) -> ArtifactDigest {
    separated_identity(INPUT_DOMAIN, &[header, bulk])
}

fn candidate_identity(
    request: &ArtifactDigest,
    output_header: &[u8],
    bulk: &[u8],
) -> ArtifactDigest {
    separated_identity(
        CANDIDATE_DOMAIN,
        &[&request.sha256_bytes(), output_header, bulk],
    )
}

fn single_payload_identity(domain: &[u8], payload: &[u8]) -> ArtifactDigest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(payload);
    ArtifactDigest::from_sha256(hasher.finalize().into())
}

fn separated_identity(domain: &[u8], pieces: &[&[u8]]) -> ArtifactDigest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            hasher.update([0]);
        }
        hasher.update(piece);
    }
    ArtifactDigest::from_sha256(hasher.finalize().into())
}

fn is_negative_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_negative()
}

pub(super) fn occurrence(
    prepared: &PreparedPrescribedDynamicSolid3d,
    accepted_state: &eqiora_artifact::SpatialStateEnvelopeV1,
    exchange: &Exchange,
    candidate_identity: ArtifactDigest,
    transcript_identity: ArtifactDigest,
) -> Result<PrescribedDynamicSolidProviderOccurrenceEnvelopeV1, Diagnostic> {
    PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::new(
        &prepared.realization,
        &prepared.prior_state,
        accepted_state,
        control::PROVIDER_ID,
        control::PROVIDER_RELEASE,
        &exchange.provider_dependencies(),
        control::SUCCESS_CODE,
        control::SUCCESS_MESSAGE,
        exchange.binding_identity.clone(),
        exchange.displacement_input_identity.clone(),
        exchange.velocity_input_identity.clone(),
        exchange.request_identity.clone(),
        candidate_identity,
        transcript_identity,
    )
}
