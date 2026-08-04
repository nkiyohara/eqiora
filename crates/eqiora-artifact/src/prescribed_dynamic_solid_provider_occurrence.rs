//! Durable identity of one admitted prescribed-solid external-provider occurrence.

use std::collections::BTreeMap;
use std::str::FromStr;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, JsonDecoderLimits,
    PrescribedDynamicSolidRealizationEnvelopeV1, ReplayableCanonicalModelArtifact,
    SimplicialMeshEnvelopeV1, SpatialStateEnvelopeV1, check_json_limits, invalid_artifact,
};

const SCHEMA: &str = "eqiora.prescribed-dynamic-solid-provider-occurrence-envelope/v1";
const ADAPTER_ID: &str = "eqiora.subprocess.external-boundary-provider";
const ADAPTER_RELEASE: &str = "0.1.0-alpha.1";
const PROTOCOL: &str = "eqiora.external-boundary-provider-subprocess/v1";
const PROVIDER_ID: &str = "eqiora.python.prescribed-dynamic-solid-affine";
const PROVIDER_RELEASE: &str = "1.0.0";
const PRODUCER_CODE: &str = "provider.success";
const PRODUCER_MESSAGE: &str = "affine predictor completed";
const MAX_ARTIFACT_BYTES: usize = 8192;
const MAX_NESTING_DEPTH: usize = 8;
const MAX_DEPENDENCIES: usize = 16;
const VERTEX_INDICES: [u64; 4] = [1, 3, 5, 7];

/// Closed, role-preserving record of one accepted provider occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidProviderOccurrenceEnvelopeV1 {
    wire: WireEnvelope,
    provider_dependencies: BTreeMap<String, String>,
}

impl PrescribedDynamicSolidProviderOccurrenceEnvelopeV1 {
    /// Construct the exact E1 provider occurrence from admitted resources and identities.
    ///
    /// # Errors
    /// Returns `EQ0901` for any resource, provider, projection, report, or identity drift.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        prior_state: &SpatialStateEnvelopeV1,
        accepted_state: &SpatialStateEnvelopeV1,
        provider_id: &str,
        provider_release: &str,
        provider_dependencies: &BTreeMap<String, String>,
        producer_code: &str,
        producer_message: &str,
        binding_identity: ArtifactDigest,
        displacement_input_identity: ArtifactDigest,
        velocity_input_identity: ArtifactDigest,
        request_identity: ArtifactDigest,
        candidate_identity: ArtifactDigest,
        transcript_identity: ArtifactDigest,
    ) -> Result<Self, Diagnostic> {
        validate_state_lineage(realization, prior_state, accepted_state)?;
        let value = Self {
            wire: WireEnvelope {
                schema: SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                model_sha256: realization.model_artifact().to_string(),
                semantic_revision: realization.semantic_revision().get(),
                realization_sha256: realization.digest()?.to_string(),
                prior_state_sha256: prior_state.digest()?.to_string(),
                contract: WireContract::exact(),
                provider: WireProvider {
                    id: provider_id.to_owned(),
                    release: provider_release.to_owned(),
                    dependencies: provider_dependencies
                        .iter()
                        .map(|(name, release)| WireDependency {
                            name: name.clone(),
                            release: release.clone(),
                        })
                        .collect(),
                },
                adapter: WireAdapter::exact(),
                projection: WireProjection::new(
                    realization,
                    displacement_input_identity,
                    velocity_input_identity,
                ),
                request: WireRequest {
                    binding_sha256: binding_identity.to_string(),
                    request_sha256: request_identity.to_string(),
                },
                candidate: WireCandidate {
                    candidate_sha256: candidate_identity.to_string(),
                    producer_report: WireProducerReport {
                        status: "success".to_owned(),
                        code: producer_code.to_owned(),
                        message: producer_message.to_owned(),
                    },
                },
                transcript: WireTranscript {
                    transcript_sha256: transcript_identity.to_string(),
                    frame_count: 11,
                    control_frame_count: 8,
                    bulk_frame_count: 3,
                    aggregate_bulk_bytes: 288,
                },
                admission: WireAdmission {
                    status: "accepted".to_owned(),
                    accepted_generation: 1,
                    accepted_state_sha256: accepted_state.digest()?.to_string(),
                },
            },
            provider_dependencies: provider_dependencies.clone(),
        };
        value.validate_local()?;
        Ok(value)
    }

    /// Decode locally canonical bytes without resolving referenced resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for malformed, reordered, unknown, unsupported, or over-budget bytes.
    pub fn from_json(bytes: &[u8], limits: JsonDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(
            bytes,
            JsonDecoderLimits {
                max_bytes: limits.max_bytes.min(MAX_ARTIFACT_BYTES),
                max_nesting_depth: limits.max_nesting_depth.min(MAX_NESTING_DEPTH),
            },
        )?;
        let wire: WireEnvelope = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!(
                "invalid prescribed dynamic-solid provider occurrence JSON: {error}"
            ))
        })?;
        let provider_dependencies = wire
            .provider
            .dependencies
            .iter()
            .map(|entry| (entry.name.clone(), entry.release.clone()))
            .collect();
        let value = Self {
            wire,
            provider_dependencies,
        };
        value.validate_local()?;
        if value.canonical_json()?.as_slice() != bytes {
            return Err(invalid_artifact(
                "prescribed dynamic-solid provider occurrence JSON is not canonical",
            ));
        }
        Ok(value)
    }

    /// Deterministic compact canonical JSON.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize prescribed dynamic-solid provider occurrence: {error}"
            ))
        })
    }

    /// Domain-separated identity of the complete occurrence.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Exact current Model artifact.
    #[must_use]
    pub fn model_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.model_sha256)
    }

    /// Semantic revision of the retained Model.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.wire.semantic_revision
    }

    /// Exact prescribed-solid Realization artifact.
    #[must_use]
    pub fn realization_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.realization_sha256)
    }

    /// Exact prior State artifact.
    #[must_use]
    pub fn prior_state_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.prior_state_sha256)
    }

    /// Exact admitted accepted-next State artifact.
    #[must_use]
    pub fn accepted_state_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.admission.accepted_state_sha256)
    }

    /// Frozen provider contract generation.
    #[must_use]
    pub const fn contract_generation(&self) -> u64 {
        self.wire.contract.generation
    }

    /// Provider identity.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.wire.provider.id
    }

    /// Provider release.
    #[must_use]
    pub fn provider_release(&self) -> &str {
        &self.wire.provider.release
    }

    /// Complete normalized dependency inventory.
    #[must_use]
    pub const fn provider_dependencies(&self) -> &BTreeMap<String, String> {
        &self.provider_dependencies
    }

    /// Adapter identity.
    #[must_use]
    pub fn adapter_id(&self) -> &str {
        &self.wire.adapter.id
    }

    /// Adapter release.
    #[must_use]
    pub fn adapter_release(&self) -> &str {
        &self.wire.adapter.release
    }

    /// Connected-subprocess protocol identity.
    #[must_use]
    pub fn protocol(&self) -> &str {
        &self.wire.adapter.protocol
    }

    /// Exact solid body Domain.
    #[must_use]
    pub fn solid_domain(&self) -> Id<kinds::Domain> {
        admitted_id(&self.wire.projection.solid_domain_ulid, "solid Domain")
    }

    /// Exact driven boundary Domain.
    #[must_use]
    pub fn boundary(&self) -> Id<kinds::Domain> {
        admitted_id(&self.wire.projection.boundary_ulid, "driven boundary")
    }

    /// Exact displacement Field.
    #[must_use]
    pub fn displacement_field(&self) -> Id<kinds::Field> {
        admitted_id(
            &self.wire.projection.inputs[0].field_ulid,
            "displacement Field",
        )
    }

    /// Exact velocity Field.
    #[must_use]
    pub fn velocity_field(&self) -> Id<kinds::Field> {
        admitted_id(&self.wire.projection.inputs[1].field_ulid, "velocity Field")
    }

    /// Canonical boundary vertex order.
    #[must_use]
    pub const fn vertex_indices(&self) -> &[u64] {
        self.wire.projection.vertex_indices.as_slice()
    }

    /// Exact bind-control identity.
    #[must_use]
    pub fn binding_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.request.binding_sha256)
    }

    /// Exact displacement input-block identity.
    #[must_use]
    pub fn displacement_input_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.projection.inputs[0].block_sha256)
    }

    /// Exact velocity input-block identity.
    #[must_use]
    pub fn velocity_input_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.projection.inputs[1].block_sha256)
    }

    /// Exact evaluate-control identity.
    #[must_use]
    pub fn request_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.request.request_sha256)
    }

    /// Exact returned-candidate identity.
    #[must_use]
    pub fn candidate_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.candidate.candidate_sha256)
    }

    /// Exact successful framed transcript identity.
    #[must_use]
    pub fn transcript_identity(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.transcript.transcript_sha256)
    }

    /// Provider success code.
    #[must_use]
    pub fn producer_code(&self) -> &str {
        &self.wire.candidate.producer_report.code
    }

    /// Provider success message.
    #[must_use]
    pub fn producer_message(&self) -> &str {
        &self.wire.candidate.producer_report.message
    }

    /// Replay every retained durable role against exact external resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for any stale identity, role, coordinate, or resource.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_against(
        &self,
        model: &impl ReplayableCanonicalModelArtifact,
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        geometry: &GeometryIdentityEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        prior_state: &SpatialStateEnvelopeV1,
        accepted_state: &SpatialStateEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        self.validate_local()?;
        realization.validate_against(model, geometry, correspondence, mesh)?;
        geometry.validate_against(model)?;
        correspondence.validate_against(geometry, model, mesh)?;
        validate_state_lineage(realization, prior_state, accepted_state)?;
        let reference = model.replay_model()?;
        if self.model_artifact() != *reference.artifact_reference().artifact()
            || self.semantic_revision() != reference.artifact_reference().semantic_revision().get()
            || self.realization_artifact() != realization.digest()?
            || self.prior_state_artifact() != prior_state.digest()?
            || self.accepted_state_artifact() != accepted_state.digest()?
            || admitted_digest(&self.wire.projection.geometry_sha256) != geometry.digest()?
            || admitted_digest(&self.wire.projection.correspondence_sha256)
                != correspondence.digest()?
            || admitted_digest(&self.wire.projection.mesh_sha256) != mesh.digest()?
            || self.solid_domain() != realization.solid_domain()
            || self.boundary() != realization.driven_boundary()
            || self.displacement_field() != realization.displacement_field()
            || self.velocity_field() != realization.velocity_field()
        {
            return Err(invalid_artifact(
                "provider occurrence differs from exact Model, resource, State, or role replay",
            ));
        }
        Ok(())
    }

    fn validate_local(&self) -> Result<(), Diagnostic> {
        if self.wire.schema != SCHEMA
            || self.wire.encoding != CANONICAL_ENCODING
            || self.wire.semantic_revision != 1
            || self.wire.contract != WireContract::exact()
            || self.wire.adapter != WireAdapter::exact()
        {
            return Err(invalid_artifact(
                "unsupported provider occurrence schema, encoding, contract, or adapter",
            ));
        }
        validate_provider(&self.wire.provider, &self.provider_dependencies)?;
        self.wire.projection.validate()?;
        for digest in self.wire.digests() {
            ArtifactDigest::from_hex(digest.to_owned())?;
        }
        if self.wire.candidate.producer_report.status != "success"
            || self.wire.candidate.producer_report.code != PRODUCER_CODE
            || self.wire.candidate.producer_report.message != PRODUCER_MESSAGE
            || self.wire.transcript
                != (WireTranscript {
                    transcript_sha256: self.wire.transcript.transcript_sha256.clone(),
                    frame_count: 11,
                    control_frame_count: 8,
                    bulk_frame_count: 3,
                    aggregate_bulk_bytes: 288,
                })
            || self.wire.admission.status != "accepted"
            || self.wire.admission.accepted_generation != 1
        {
            return Err(invalid_artifact(
                "provider occurrence report, transcript summary, or admission differs",
            ));
        }
        validate_key(PRODUCER_CODE, 64, "producer code")?;
        validate_message(PRODUCER_MESSAGE)?;
        Ok(())
    }
}

impl WireProjection {
    fn new(
        realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
        displacement_input_identity: ArtifactDigest,
        velocity_input_identity: ArtifactDigest,
    ) -> Self {
        let common =
            |role: &str, field: Id<kinds::Field>, unit: &str, block: ArtifactDigest| WireInput {
                role: role.to_owned(),
                field_ulid: field.ulid().to_string(),
                unit: unit.to_owned(),
                value_shape: [3],
                frame: "spatial-cartesian".to_owned(),
                representation: "continuous-lagrange-p1-trace".to_owned(),
                association: "vertex".to_owned(),
                coefficient_count: 12,
                byte_length: 96,
                block_sha256: block.to_string(),
            };
        Self {
            geometry_sha256: realization.geometry_artifact().to_string(),
            correspondence_sha256: realization.correspondence_artifact().to_string(),
            mesh_sha256: realization.mesh_artifact().to_string(),
            solid_domain_ulid: realization.solid_domain().ulid().to_string(),
            boundary_ulid: realization.driven_boundary().ulid().to_string(),
            model_time_s: 0.0,
            next_time_s: 0.25,
            delta_time_s: 0.25,
            vertex_indices: VERTEX_INDICES.to_vec(),
            coefficient_order: "vertex-index-ascending-component-x-y-z".to_owned(),
            inputs: vec![
                common(
                    "prior-displacement-trace",
                    realization.displacement_field(),
                    "m",
                    displacement_input_identity,
                ),
                common(
                    "prior-velocity-trace",
                    realization.velocity_field(),
                    "m/s",
                    velocity_input_identity,
                ),
            ],
            output: WireOutput {
                role: "next-total-displacement".to_owned(),
                field_ulid: realization.displacement_field().ulid().to_string(),
                unit: "m".to_owned(),
                value_shape: [3],
                frame: "spatial-cartesian".to_owned(),
                representation: "continuous-lagrange-p1-trace".to_owned(),
                association: "vertex".to_owned(),
                convention: "total-reference-configuration".to_owned(),
                coefficient_count: 12,
                byte_length: 96,
            },
        }
    }

    fn validate(&self) -> Result<(), Diagnostic> {
        parse_id::<kinds::Domain>(&self.solid_domain_ulid, "solid Domain")?;
        parse_id::<kinds::Domain>(&self.boundary_ulid, "driven boundary")?;
        if self.model_time_s.to_bits() != 0.0_f64.to_bits()
            || self.next_time_s.to_bits() != 0.25_f64.to_bits()
            || self.delta_time_s.to_bits() != 0.25_f64.to_bits()
            || self.vertex_indices != VERTEX_INDICES
            || self.coefficient_order != "vertex-index-ascending-component-x-y-z"
            || self.inputs.len() != 2
        {
            return Err(invalid_artifact(
                "provider projection coordinate, vertex order, or input count differs",
            ));
        }
        let expected_inputs = [
            ("prior-displacement-trace", "m"),
            ("prior-velocity-trace", "m/s"),
        ];
        for (input, (role, unit)) in self.inputs.iter().zip(expected_inputs) {
            parse_id::<kinds::Field>(&input.field_ulid, "projection Field")?;
            if input.role != role
                || input.unit != unit
                || input.value_shape != [3]
                || input.frame != "spatial-cartesian"
                || input.representation != "continuous-lagrange-p1-trace"
                || input.association != "vertex"
                || input.coefficient_count != 12
                || input.byte_length != 96
            {
                return Err(invalid_artifact(
                    "provider projection input descriptor differs from the exact trace contract",
                ));
            }
        }
        parse_id::<kinds::Field>(&self.output.field_ulid, "output Field")?;
        if self.inputs[0].field_ulid != self.output.field_ulid
            || self.inputs[0].field_ulid == self.inputs[1].field_ulid
            || self.output.role != "next-total-displacement"
            || self.output.unit != "m"
            || self.output.value_shape != [3]
            || self.output.frame != "spatial-cartesian"
            || self.output.representation != "continuous-lagrange-p1-trace"
            || self.output.association != "vertex"
            || self.output.convention != "total-reference-configuration"
            || self.output.coefficient_count != 12
            || self.output.byte_length != 96
        {
            return Err(invalid_artifact(
                "provider projection output descriptor differs from the exact total-displacement contract",
            ));
        }
        Ok(())
    }
}

fn validate_state_lineage(
    realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
    prior_state: &SpatialStateEnvelopeV1,
    accepted_state: &SpatialStateEnvelopeV1,
) -> Result<(), Diagnostic> {
    let realization_digest = realization.digest()?;
    let exact_coordinate = prior_state.step() == 0
        && prior_state.time_s().to_bits() == 0.0_f64.to_bits()
        && accepted_state.step() == 1
        && accepted_state.time_s().to_bits() == 0.25_f64.to_bits();
    let exact_lineage = [prior_state, accepted_state].iter().all(|state| {
        state.model_artifact() == realization.model_artifact()
            && state.realization_artifact() == realization_digest
            && state.geometry_artifact() == realization.geometry_artifact()
            && state.correspondence_artifact() == realization.correspondence_artifact()
            && state.mesh_artifact() == realization.mesh_artifact()
            && state.fields().len() == 2
            && state.fields().iter().all(|(domain, field, _)| {
                *domain == realization.solid_domain()
                    && (*field == realization.displacement_field()
                        || *field == realization.velocity_field())
            })
    });
    if !exact_coordinate || !exact_lineage {
        return Err(invalid_artifact(
            "provider occurrence prior or accepted State role differs from the exact Realization",
        ));
    }
    Ok(())
}

fn validate_provider(
    provider: &WireProvider,
    dependencies: &BTreeMap<String, String>,
) -> Result<(), Diagnostic> {
    validate_key(&provider.id, 128, "provider id")?;
    validate_visible_ascii(&provider.release, 128, "provider release")?;
    if provider.id != PROVIDER_ID
        || provider.release != PROVIDER_RELEASE
        || provider.dependencies.len() > MAX_DEPENDENCIES
        || provider.dependencies.len() != dependencies.len()
    {
        return Err(invalid_artifact(
            "provider identity or dependency inventory differs from the frozen E1 provider",
        ));
    }
    let mut prior = None;
    for dependency in &provider.dependencies {
        validate_key(&dependency.name, 64, "dependency name")?;
        validate_visible_ascii(&dependency.release, 128, "dependency release")?;
        if prior.is_some_and(|name: &str| name >= dependency.name.as_str())
            || dependencies.get(&dependency.name) != Some(&dependency.release)
        {
            return Err(invalid_artifact(
                "provider dependencies must be unique and in ascending name order",
            ));
        }
        prior = Some(dependency.name.as_str());
    }
    if dependencies.get("cpython").map(String::as_str) != Some("3.12")
        || dependencies.get("numpy").map(String::as_str) != Some("2.1.0")
        || dependencies.len() != 2
    {
        return Err(invalid_artifact(
            "provider dependency inventory differs from normalized CPython 3.12 and NumPy 2.1.0",
        ));
    }
    Ok(())
}

fn validate_key(value: &str, maximum: usize, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.len() > maximum
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return Err(invalid_artifact(format!(
            "{label} must be bounded lowercase dotted/kebab/snake ASCII"
        )));
    }
    Ok(())
}

fn validate_visible_ascii(value: &str, maximum: usize, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || value.len() > maximum
        || value.trim() != value
        || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err(invalid_artifact(format!(
            "{label} must be bounded, trimmed, visible ASCII"
        )));
    }
    Ok(())
}

fn validate_message(value: &str) -> Result<(), Diagnostic> {
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(invalid_artifact(
            "producer message must be bounded UTF-8 without control characters",
        ));
    }
    Ok(())
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

fn admitted_id<E: eqiora_core::Entity>(value: &str, label: &str) -> Id<E> {
    parse_id(value, label).expect("locally validated provider occurrence ULID")
}

fn admitted_digest(value: &str) -> ArtifactDigest {
    ArtifactDigest::from_hex(value.to_owned())
        .expect("locally validated provider occurrence digest")
}

impl WireContract {
    fn exact() -> Self {
        Self {
            generation: 1,
            approximation: "lagged-accepted-state".to_owned(),
            statefulness: "stateless".to_owned(),
            determinism: "required".to_owned(),
            scalar: "ieee754-binary64".to_owned(),
            target: "host-cpu".to_owned(),
        }
    }
}

impl WireAdapter {
    fn exact() -> Self {
        Self {
            id: ADAPTER_ID.to_owned(),
            release: ADAPTER_RELEASE.to_owned(),
            protocol: PROTOCOL.to_owned(),
        }
    }
}

impl WireEnvelope {
    fn digests(&self) -> [&str; 13] {
        [
            &self.model_sha256,
            &self.realization_sha256,
            &self.prior_state_sha256,
            &self.projection.geometry_sha256,
            &self.projection.correspondence_sha256,
            &self.projection.mesh_sha256,
            &self.projection.inputs[0].block_sha256,
            &self.projection.inputs[1].block_sha256,
            &self.request.binding_sha256,
            &self.request.request_sha256,
            &self.candidate.candidate_sha256,
            &self.transcript.transcript_sha256,
            &self.admission.accepted_state_sha256,
        ]
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEnvelope { schema: String, encoding: String, model_sha256: String, semantic_revision: u64, realization_sha256: String, prior_state_sha256: String, contract: WireContract, provider: WireProvider, adapter: WireAdapter, projection: WireProjection, request: WireRequest, candidate: WireCandidate, transcript: WireTranscript, admission: WireAdmission }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContract { generation: u64, approximation: String, statefulness: String, determinism: String, scalar: String, target: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProvider { id: String, release: String, dependencies: Vec<WireDependency> }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDependency { name: String, release: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAdapter { id: String, release: String, protocol: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProjection { geometry_sha256: String, correspondence_sha256: String, mesh_sha256: String, solid_domain_ulid: String, boundary_ulid: String, model_time_s: f64, next_time_s: f64, delta_time_s: f64, vertex_indices: Vec<u64>, coefficient_order: String, inputs: Vec<WireInput>, output: WireOutput }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireInput { role: String, field_ulid: String, unit: String, value_shape: [u64; 1], frame: String, representation: String, association: String, coefficient_count: u64, byte_length: u64, block_sha256: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOutput { role: String, field_ulid: String, unit: String, value_shape: [u64; 1], frame: String, representation: String, association: String, convention: String, coefficient_count: u64, byte_length: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest { binding_sha256: String, request_sha256: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCandidate { candidate_sha256: String, producer_report: WireProducerReport }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProducerReport { status: String, code: String, message: String }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTranscript { transcript_sha256: String, frame_count: u64, control_frame_count: u64, bulk_frame_count: u64, aggregate_bulk_bytes: u64 }

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAdmission { status: String, accepted_generation: u64, accepted_state_sha256: String }
