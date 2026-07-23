use serde::{Deserialize, Serialize};

/// Closed, privacy-safe public evidence schema.
///
/// The source identity is limited to a public commit plus its clean-worktree
/// status. The environment is an allowlist of selected-device and library
/// properties; host identity and filesystem paths have no representation.
pub(crate) const SCHEMA: &str = "eqiora.fixed-reference-fsi-cuda-observation/v2";
pub(crate) const MAX_OBSERVATION_BYTES: usize = 128 * 1024;
pub(crate) const MAX_ARTIFACT_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_VALUE_COUNT: usize = 4096;
pub(crate) const CPU_CUDA_ABSOLUTE: f64 = 2.0e-10;
pub(crate) const CPU_CUDA_RELATIVE: f64 = 2.0e-10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Observation {
    pub(crate) schema: String,
    pub(crate) source_commit: String,
    pub(crate) source_clean: bool,
    pub(crate) environment: Environment,
    pub(crate) operator_sha256: String,
    pub(crate) output_sha256: String,
    pub(crate) values: Vec<f64>,
    pub(crate) producer: Producer,
    pub(crate) receipt: Receipt,
    pub(crate) physics: Physics,
    pub(crate) conformance: Conformance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Environment {
    pub(crate) runtime: String,
    pub(crate) device_ordinal: u16,
    pub(crate) device_name: String,
    pub(crate) total_memory_bytes: u64,
    pub(crate) compute_capability_major: u16,
    pub(crate) compute_capability_minor: u16,
    pub(crate) driver: i32,
    pub(crate) cusparse: i32,
    pub(crate) cublas: i32,
    pub(crate) cudarc: String,
    pub(crate) binding_toolkit: String,
    pub(crate) adapter_version: String,
    pub(crate) observation_kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Producer {
    pub(crate) reason: String,
    pub(crate) completed_iterations: usize,
    pub(crate) reported_residual_norm: f64,
    pub(crate) true_residual_norm: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub(crate) dimension: usize,
    pub(crate) minimum_device_payload_bytes: usize,
    pub(crate) external_sparse_workspace_bytes: usize,
    pub(crate) dag: Vec<String>,
    pub(crate) transfer_count: usize,
    pub(crate) inverse_diagonal_present: bool,
    pub(crate) inputs_ready_sequence: u64,
    pub(crate) solve_visible_sequence: u64,
    pub(crate) output_transfer_sequence: u64,
    pub(crate) solution_visible_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Physics {
    pub(crate) residual_norm: f64,
    pub(crate) continuity_residual_norm: f64,
    pub(crate) kinematic_residual_norm: f64,
    pub(crate) interface_velocity_jump_norm: f64,
    pub(crate) interface_action_imbalance_norm: f64,
    pub(crate) energy_defect: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Conformance {
    pub(crate) algebraic: ErrorPair,
    pub(crate) vertex_velocity_over_u: ErrorPair,
    pub(crate) bubble_velocity_over_u: ErrorPair,
    pub(crate) pressure_over_p: ErrorPair,
    pub(crate) displacement_over_l: ErrorPair,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ErrorPair {
    pub(crate) maximum_absolute_error: f64,
    pub(crate) maximum_scaled_error: f64,
}

impl Observation {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || self.source_commit.len() != 40
            || !self.source_commit.bytes().all(is_lower_hex)
            || !self.source_clean
            || self.operator_sha256.len() != 64
            || self.output_sha256.len() != 64
            || !self.operator_sha256.bytes().all(is_lower_hex)
            || !self.output_sha256.bytes().all(is_lower_hex)
            || self.values.is_empty()
            || self.values.len() > MAX_VALUE_COUNT
            || self.receipt.dimension != self.values.len()
        {
            return Err("observation identity or bounded solution shape differs".to_owned());
        }
        self.environment.validate()?;
        if self.producer.reason != "residual-tolerance-satisfied"
            || self.producer.completed_iterations == 0
            || self.receipt.minimum_device_payload_bytes == 0
            || self.receipt.transfer_count != 6
            || self.receipt.inverse_diagonal_present
            || self.receipt.dag.iter().map(String::as_str).ne(cuda_dag())
        {
            return Err("solver or generic CUDA receipt contract differs".to_owned());
        }
        let sequences = [
            self.receipt.inputs_ready_sequence,
            self.receipt.solve_visible_sequence,
            self.receipt.output_transfer_sequence,
            self.receipt.solution_visible_sequence,
        ];
        if sequences.contains(&0) || !sequences.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err("recorded CUDA completion order is invalid".to_owned());
        }
        for value in self.values.iter().copied().chain([
            self.producer.reported_residual_norm,
            self.producer.true_residual_norm,
            self.physics.residual_norm,
            self.physics.continuity_residual_norm,
            self.physics.kinematic_residual_norm,
            self.physics.interface_velocity_jump_norm,
            self.physics.interface_action_imbalance_norm,
            self.physics.energy_defect,
        ]) {
            require_canonical_float(value)?;
        }
        for pair in self.conformance.pairs() {
            require_canonical_float(pair.maximum_absolute_error)?;
            require_canonical_float(pair.maximum_scaled_error)?;
            if pair.maximum_scaled_error > 1.0 {
                return Err("recorded CPU/CUDA conformance exceeds its tolerance".to_owned());
            }
        }
        Ok(())
    }
}

impl Environment {
    fn validate(&self) -> Result<(), String> {
        if self.runtime != "eqiora.cuda.cudarc"
            || self.device_ordinal != 0
            || self.device_name.trim().is_empty()
            || self.total_memory_bytes == 0
            || self.compute_capability_major == 0
            || self.driver <= 0
            || self.cusparse <= 0
            || self.cublas <= 0
            || self.cudarc.trim().is_empty()
            || self.binding_toolkit.trim().is_empty()
            || self.adapter_version.trim().is_empty()
            || self.observation_kind
                != "public-source-selected-device-run; no-host-identity; not-hardware-attestation"
        {
            return Err("selected CUDA environment is incomplete or contradictory".to_owned());
        }
        Ok(())
    }
}

impl Conformance {
    fn pairs(&self) -> [ErrorPair; 5] {
        [
            self.algebraic,
            self.vertex_velocity_over_u,
            self.bubble_velocity_over_u,
            self.pressure_over_p,
            self.displacement_over_l,
        ]
    }
}

pub(crate) fn error_pair(
    reference: impl IntoIterator<Item = f64>,
    candidate: impl IntoIterator<Item = f64>,
) -> Result<ErrorPair, String> {
    let mut reference = reference.into_iter();
    let mut candidate = candidate.into_iter();
    let mut maximum_absolute_error: f64 = 0.0;
    let mut maximum_scaled_error: f64 = 0.0;
    loop {
        match (reference.next(), candidate.next()) {
            (Some(reference), Some(candidate)) => {
                require_canonical_float(reference)?;
                require_canonical_float(candidate)?;
                let error = (candidate - reference).abs();
                let tolerance =
                    CPU_CUDA_ABSOLUTE + CPU_CUDA_RELATIVE * reference.abs().max(candidate.abs());
                maximum_absolute_error = maximum_absolute_error.max(error);
                maximum_scaled_error = maximum_scaled_error.max(error / tolerance);
            }
            (None, None) => break,
            _ => return Err("CPU and CUDA observation shapes differ".to_owned()),
        }
    }
    Ok(ErrorPair {
        maximum_absolute_error,
        maximum_scaled_error,
    })
}

pub(crate) fn require_same_float(label: &str, actual: f64, expected: f64) -> Result<(), String> {
    require_canonical_float(actual)?;
    require_canonical_float(expected)?;
    let actual = if actual == 0.0 { 0.0 } else { actual };
    let expected = if expected == 0.0 { 0.0 } else { expected };
    if actual.to_bits() != expected.to_bits() {
        return Err(format!(
            "{label} differs: expected {expected:.17e}, found {actual:.17e}"
        ));
    }
    Ok(())
}

pub(crate) fn cuda_dag() -> impl Iterator<Item = &'static str> {
    [
        "transfer-inputs-to-cuda",
        "await-cuda-inputs-ready",
        "solve-on-cuda",
        "await-cuda-solve-completion",
        "transfer-candidate-to-host",
        "await-host-visibility",
        "accept-with-native-host-verification",
        "replay-true-residual-on-host",
        "accept-host-complete",
    ]
    .into_iter()
}

fn require_canonical_float(value: f64) -> Result<(), String> {
    if !value.is_finite() || (value == 0.0 && value.is_sign_negative()) {
        return Err("observation contains a non-finite or negative-zero float".to_owned());
    }
    Ok(())
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}
