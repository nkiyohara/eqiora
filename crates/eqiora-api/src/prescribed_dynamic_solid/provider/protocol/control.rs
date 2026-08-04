//! Closed canonical control payloads for the E1 provider state machine.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use eqiora_core::Diagnostic;

use super::super::super::invalid;

pub(in super::super) const PROTOCOL: &str = "eqiora.external-boundary-provider-subprocess/v1";
pub(in super::super) const CONTRACT: &str = "eqiora.prescribed-dynamic-solid-state-boundary/v1";
pub(in super::super) const PROVIDER_ID: &str = "eqiora.python.prescribed-dynamic-solid-affine";
pub(in super::super) const PROVIDER_RELEASE: &str = "1.0.0";
pub(in super::super) const SUCCESS_CODE: &str = "provider.success";
pub(in super::super) const SUCCESS_MESSAGE: &str = "affine predictor completed";
pub(in super::super) const MAX_CONTROL_BYTES: usize = 4096;
const MAX_CONTROL_NESTING: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Dependency {
    pub(in super::super) name: String,
    pub(in super::super) release: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Provider {
    pub(in super::super) id: String,
    pub(in super::super) release: String,
    pub(in super::super) dependencies: Vec<Dependency>,
}

impl Provider {
    pub(in super::super) fn exact() -> Self {
        Self {
            id: PROVIDER_ID.to_owned(),
            release: PROVIDER_RELEASE.to_owned(),
            dependencies: vec![
                Dependency {
                    name: "cpython".to_owned(),
                    release: "3.12".to_owned(),
                },
                Dependency {
                    name: "numpy".to_owned(),
                    release: "2.1.0".to_owned(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Capability {
    pub(in super::super) deterministic: bool,
    pub(in super::super) stateful: bool,
    pub(in super::super) scalar: String,
    pub(in super::super) target: String,
    pub(in super::super) association: String,
    pub(in super::super) layout: String,
    pub(in super::super) maximum_input_fields: u64,
    pub(in super::super) maximum_output_fields: u64,
    pub(in super::super) maximum_coefficients_per_field: u64,
    pub(in super::super) maximum_aggregate_bulk_bytes: u64,
}

impl Capability {
    fn is_exact(&self) -> bool {
        self.deterministic
            && !self.stateful
            && self.scalar == "f64"
            && self.target == "host-cpu"
            && self.association == "vertex"
            && self.layout == "entity-major-spatial-cartesian"
            && self.maximum_input_fields == 2
            && self.maximum_output_fields == 1
            && self.maximum_coefficients_per_field == 12
            && self.maximum_aggregate_bulk_bytes == 288
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Hello {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) protocol: String,
    pub(in super::super) contract: String,
    pub(in super::super) provider: Provider,
    pub(in super::super) capability: Capability,
}

impl Hello {
    pub(in super::super) fn validate(&self) -> Result<(), Diagnostic> {
        if self.kind != "hello"
            || self.protocol != PROTOCOL
            || self.contract != CONTRACT
            || self.provider != Provider::exact()
            || !self.capability.is_exact()
        {
            return Err(invalid(
                "provider hello differs from the frozen protocol capability",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct InputDescriptor {
    pub(in super::super) role: String,
    pub(in super::super) field_ulid: String,
    pub(in super::super) unit: String,
    pub(in super::super) value_shape: [u64; 1],
    pub(in super::super) frame: String,
    pub(in super::super) representation: String,
    pub(in super::super) association: String,
    pub(in super::super) coefficient_count: u64,
    pub(in super::super) byte_length: u64,
    pub(in super::super) block_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct OutputDescriptor {
    pub(in super::super) role: String,
    pub(in super::super) field_ulid: String,
    pub(in super::super) unit: String,
    pub(in super::super) value_shape: [u64; 1],
    pub(in super::super) frame: String,
    pub(in super::super) representation: String,
    pub(in super::super) association: String,
    pub(in super::super) convention: String,
    pub(in super::super) coefficient_count: u64,
    pub(in super::super) byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Bind {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) protocol: String,
    pub(in super::super) contract: String,
    pub(in super::super) model_sha256: String,
    pub(in super::super) semantic_revision: u64,
    pub(in super::super) realization_sha256: String,
    pub(in super::super) geometry_sha256: String,
    pub(in super::super) correspondence_sha256: String,
    pub(in super::super) mesh_sha256: String,
    pub(in super::super) prior_state_sha256: String,
    pub(in super::super) provider: Provider,
    pub(in super::super) model_time_s: f64,
    pub(in super::super) next_time_s: f64,
    pub(in super::super) delta_time_s: f64,
    pub(in super::super) solid_domain_ulid: String,
    pub(in super::super) boundary_ulid: String,
    pub(in super::super) vertex_indices: Vec<u64>,
    pub(in super::super) coefficient_order: String,
    pub(in super::super) inputs: Vec<InputDescriptor>,
    pub(in super::super) output: OutputDescriptor,
}

impl Bind {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn exact(
        model: String,
        realization: String,
        geometry: String,
        correspondence: String,
        mesh: String,
        prior_state: String,
        solid_domain: String,
        boundary: String,
        inputs: Vec<InputDescriptor>,
        output: OutputDescriptor,
    ) -> Self {
        Self {
            kind: "bind".to_owned(),
            protocol: PROTOCOL.to_owned(),
            contract: CONTRACT.to_owned(),
            model_sha256: model,
            semantic_revision: 1,
            realization_sha256: realization,
            geometry_sha256: geometry,
            correspondence_sha256: correspondence,
            mesh_sha256: mesh,
            prior_state_sha256: prior_state,
            provider: Provider::exact(),
            model_time_s: 0.0,
            next_time_s: 0.25,
            delta_time_s: 0.25,
            solid_domain_ulid: solid_domain,
            boundary_ulid: boundary,
            vertex_indices: vec![1, 3, 5, 7],
            coefficient_order: "vertex-index-ascending-component-x-y-z".to_owned(),
            inputs,
            output,
        }
    }
}

#[derive(Serialize)]
struct InputHeader {
    model_sha256: String,
    realization_sha256: String,
    prior_state_sha256: String,
    model_time_s: f64,
    boundary_ulid: String,
    field_ulid: String,
    role: String,
    unit: String,
    value_shape: [u64; 1],
    frame: String,
    representation: String,
    association: String,
    vertex_indices: Vec<u64>,
    coefficient_count: u64,
    byte_length: u64,
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn encode_input_header(
    model: String,
    realization: String,
    prior_state: String,
    boundary: String,
    field: String,
    role: &str,
    unit: &str,
) -> Result<Vec<u8>, Diagnostic> {
    encode(&InputHeader {
        model_sha256: model,
        realization_sha256: realization,
        prior_state_sha256: prior_state,
        model_time_s: 0.0,
        boundary_ulid: boundary,
        field_ulid: field,
        role: role.to_owned(),
        unit: unit.to_owned(),
        value_shape: [3],
        frame: "spatial-cartesian".to_owned(),
        representation: "continuous-lagrange-p1-trace".to_owned(),
        association: "vertex".to_owned(),
        vertex_indices: vec![1, 3, 5, 7],
        coefficient_count: 12,
        byte_length: 96,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Bound {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) binding_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Evaluate {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) binding_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Candidate {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) request_sha256: String,
    pub(in super::super) candidate_sha256: String,
    pub(in super::super) byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Report {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) request_sha256: String,
    pub(in super::super) candidate_sha256: String,
    pub(in super::super) status: String,
    pub(in super::super) code: String,
    pub(in super::super) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Close {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) request_sha256: String,
    pub(in super::super) candidate_sha256: String,
    pub(in super::super) outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Closed {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) request_sha256: String,
    pub(in super::super) candidate_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct ErrorControl {
    #[serde(rename = "type")]
    pub(in super::super) kind: String,
    pub(in super::super) phase: String,
    pub(in super::super) code: String,
    pub(in super::super) message: String,
}

impl ErrorControl {
    pub(in super::super) fn validate(&self) -> Result<(), Diagnostic> {
        if self.kind != "error"
            || !matches!(self.phase.as_str(), "bind" | "evaluate" | "close")
            || !valid_key(&self.code, 64)
            || self.message.len() > 512
            || self.message.chars().any(char::is_control)
        {
            return Err(invalid(
                "provider error control is malformed or over budget",
            ));
        }
        Ok(())
    }
}

pub(in super::super) enum ReceivedControl {
    Hello(Hello),
    Bound(Bound),
    Candidate(Candidate),
    Report(Report),
    Closed(Closed),
    Error(ErrorControl),
}

#[derive(Deserialize)]
struct Prelude {
    #[serde(rename = "type")]
    kind: String,
}

pub(in super::super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Diagnostic> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| invalid(format!("cannot encode provider control: {error}")))?;
    preflight(&bytes)?;
    Ok(bytes)
}

pub(in super::super) fn decode(bytes: &[u8]) -> Result<ReceivedControl, Diagnostic> {
    preflight(bytes)?;
    let prelude: Prelude = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid provider control JSON: {error}")))?;
    match prelude.kind.as_str() {
        "hello" => decode_exact(bytes).map(ReceivedControl::Hello),
        "bound" => decode_exact(bytes).map(ReceivedControl::Bound),
        "candidate" => decode_exact(bytes).map(ReceivedControl::Candidate),
        "report" => decode_exact(bytes).map(ReceivedControl::Report),
        "closed" => decode_exact(bytes).map(ReceivedControl::Closed),
        "error" => {
            let value: ErrorControl = decode_exact(bytes)?;
            value.validate()?;
            Ok(ReceivedControl::Error(value))
        }
        _ => Err(invalid("provider returned an unknown control type")),
    }
}

fn decode_exact<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, Diagnostic> {
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid provider control JSON: {error}")))?;
    if encode(&value)? != bytes {
        return Err(invalid("provider control is not compact canonical JSON"));
    }
    Ok(value)
}

fn preflight(bytes: &[u8]) -> Result<(), Diagnostic> {
    if bytes.len() > MAX_CONTROL_BYTES {
        return Err(invalid("provider control exceeds the 4096-byte budget"));
    }
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("provider control nesting overflowed usize"))?;
                if depth > MAX_CONTROL_NESTING {
                    return Err(invalid("provider control exceeds the nesting budget"));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn valid_key(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}
