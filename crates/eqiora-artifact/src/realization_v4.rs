//! Fixed-topology ALE coupled Realization artifact.

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_realization::{
    FixedTopologyAleCoupledRealizationPlan, FixedTopologyAleCoupledRealizationRequirements,
    RealizationRevision, ResolvedFixedTopologyAleCoupledRealization, SemanticRevision,
};
use eqiora_schema::Model;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::realization_v2::wire::{WireLayoutArtifacts, WireQuadrature, WireQuadratureCodec};
use crate::{
    ArtifactDigest, CANONICAL_ENCODING, CanonicalModelArtifact, LayoutArtifacts,
    RealizationDecoderLimits, SimplicialMeshEnvelopeV1, check_json_limits, invalid_artifact,
};

pub(crate) mod wire;

use self::wire::WireAleRequirements;

const REALIZATION_SCHEMA: &str = "eqiora.realization-envelope/v4";

type WireRealizationEnvelopeV4 = WireAleEnvelope<WireQuadrature>;

/// Versioned serialization of one resolved fixed-topology ALE Realization.
///
/// V4 is a distinct closed wire generation. It does not append optional ALE
/// fields to v1--v3. The payload records the complete common coupled plan and
/// one graph-shaped ALE projection: explicit fluid/reference configurations,
/// one P1 harmonic geometry action, the exact Backward Euler, elimination,
/// trace, and GCL-compatible transformations, one general monolithic system,
/// and its linear/nonlinear solve roots.
#[derive(Debug, Clone, PartialEq)]
pub struct RealizationEnvelopeV4 {
    wire: WireRealizationEnvelopeV4,
}

impl RealizationEnvelopeV4 {
    /// Encode one completely resolved fixed-topology ALE Realization.
    ///
    /// # Errors
    /// Returns `EQ0901` for Model lineage drift, contradictory layout
    /// artifacts, an invalid portable graph projection, or a value outside
    /// the closed v4 contract.
    pub fn from_resolved(
        model: &impl CanonicalModelArtifact,
        resolved: &ResolvedFixedTopologyAleCoupledRealization,
        layout_artifacts: LayoutArtifacts,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            wire: WireRealizationEnvelopeV4::from_resolved(
                REALIZATION_SCHEMA,
                model,
                resolved,
                layout_artifacts,
            )?,
        })
    }

    /// Decode and locally validate a v4 ALE realization envelope.
    ///
    /// # Errors
    /// Returns `EQ0901` for oversized, malformed, unknown-version,
    /// noncanonical, resource-excess, or graph-inconsistent data.
    pub fn from_json(bytes: &[u8], limits: RealizationDecoderLimits) -> Result<Self, Diagnostic> {
        Ok(Self {
            wire: WireRealizationEnvelopeV4::from_json(REALIZATION_SCHEMA, bytes, limits)?,
        })
    }

    /// Deterministic canonical JSON bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize ALE realization envelope: {error}"
            ))
        })
    }

    /// Domain-separated SHA-256 identity of the complete v4 bytes.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            REALIZATION_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Referenced canonical Semantic Model artifact.
    #[must_use]
    pub fn model_artifact(&self) -> ArtifactDigest {
        ArtifactDigest(self.wire.model_sha256.clone())
    }

    /// Typed Semantic Model identity.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn model(&self) -> Result<OntologyId<Model>, Diagnostic> {
        parse_ulid(&self.wire.model_ulid).map(OntologyId::from_ulid)
    }

    /// Validate the exact Model artifact selected by this Realization.
    ///
    /// # Errors
    /// Returns `EQ0901` for digest, Model identity, or revision drift.
    pub fn validate_model_artifact(
        &self,
        model: &impl CanonicalModelArtifact,
    ) -> Result<(), Diagnostic> {
        let reference = model.artifact_reference()?;
        if self.model_artifact() != *reference.artifact()
            || self.model()? != reference.model()
            || self.semantic_revision() != reference.semantic_revision()
        {
            return Err(invalid_artifact(
                "Model artifact identity or semantic revision differs from the ALE Realization",
            ));
        }
        Ok(())
    }

    /// Semantic graph revision realized by this artifact.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        SemanticRevision::new(self.wire.semantic_revision)
    }

    /// Explicit Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        match self.wire.source {
            WireAleSource::Explicit {
                realization_revision,
            } => RealizationRevision::new(realization_revision),
        }
    }

    /// Exact lowerer requirements used during resolution.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn requirements(
        &self,
    ) -> Result<FixedTopologyAleCoupledRealizationRequirements, Diagnostic> {
        self.wire.requirements.clone().decode()
    }

    /// Complete typed fixed-topology ALE plan reconstructed from v4.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn plan(&self) -> Result<FixedTopologyAleCoupledRealizationPlan, Diagnostic> {
        let requirements = self.requirements()?;
        self.wire.plan.clone().decode(&requirements)
    }

    /// Referenced replicated or distributed layout artifacts.
    #[must_use]
    pub fn layout_artifacts(&self) -> LayoutArtifacts {
        self.wire.layout_artifacts.decode()
    }

    /// Sole referenced immutable simplex mesh artifact.
    ///
    /// # Errors
    /// Returns `EQ0901` only if validated internal state was corrupted.
    pub fn mesh_artifact(&self) -> Result<ArtifactDigest, Diagnostic> {
        match self.plan()?.coupled().spatial().discretization().mesh() {
            eqiora_realization::MeshPolicy::ImportedSimplicial { artifact } => {
                Ok(ArtifactDigest::from_sha256(artifact.sha256()))
            }
            eqiora_realization::MeshPolicy::GeneratedUniform { .. } => Err(invalid_artifact(
                "fixed-topology ALE realization must reference one imported simplex mesh",
            )),
            eqiora_realization::MeshPolicy::SuppliedCartesian { .. } => Err(invalid_artifact(
                "fixed-topology ALE realization does not admit a supplied Cartesian mesh",
            )),
        }
    }

    /// Validate the immutable reference mesh against exact content, dimension,
    /// and ALE quality policy.
    ///
    /// # Errors
    /// Returns `EQ0901` for digest, admitted-dimension, or quality-gate drift.
    pub fn validate_mesh_artifact(
        &self,
        mesh: &SimplicialMeshEnvelopeV1,
    ) -> Result<(), Diagnostic> {
        if mesh.digest()? != self.mesh_artifact()? {
            return Err(invalid_artifact(
                "reference mesh digest differs from the ALE realization plan",
            ));
        }
        let dimension = self
            .requirements()?
            .coupled()
            .execution()
            .spatial_dimension()
            .get();
        if mesh.dimension() != dimension {
            return Err(invalid_artifact(format!(
                "reference mesh dimension {} differs from admitted ALE dimension {dimension}",
                mesh.dimension(),
            )));
        }
        validate_ale_mesh_quality_gate(&self.plan()?, mesh)?;
        Ok(())
    }
}

pub(crate) fn validate_ale_mesh_quality_gate(
    plan: &FixedTopologyAleCoupledRealizationPlan,
    mesh: &SimplicialMeshEnvelopeV1,
) -> Result<(), Diagnostic> {
    let mesh_gate = mesh.mesh().quality_gate().minimum_mean_ratio();
    let realization_gate = plan.mesh_motion().quality_gate().minimum_mean_ratio();
    if mesh_gate.to_bits() != realization_gate.to_bits() {
        return Err(invalid_artifact(
            "reference mesh quality gate differs from the ALE mesh-motion quality gate",
        ));
    }
    Ok(())
}

pub(crate) fn validate_requirements_plan(
    requirements: &FixedTopologyAleCoupledRealizationRequirements,
    plan: &FixedTopologyAleCoupledRealizationPlan,
) -> Result<(), Diagnostic> {
    let motion = plan.mesh_motion();
    let pullback = plan.pullback();
    if motion.fluid_domain() != requirements.fluid_domain()
        || motion.solid_domain() != requirements.solid_domain()
        || motion.solid_displacement() != requirements.solid_displacement()
        || motion.interface() != requirements.coupled().trace_quotient().connection()
        || plan.fluid_time_step().relation() != requirements.fluid_relation()
        || plan.fluid_time_step().state() != requirements.fluid_velocity()
        || pullback.relation() != requirements.fluid_relation()
        || pullback.velocity() != requirements.fluid_velocity()
        || plan.solid_kinematic_relation() != requirements.solid_kinematic_relation()
    {
        return Err(invalid_artifact(
            "ALE plan differs from the exact lowerer Domain, Field, Relation, or Connection roles",
        ));
    }
    let coupled = plan.coupled();
    let eliminated = coupled.time_step().eliminated_state().pair();
    let rate_domain = coupled
        .spatial()
        .domains()
        .iter()
        .find(|domain| {
            domain
                .field_spaces()
                .iter()
                .any(|binding| binding.field() == eliminated.rate())
        })
        .map(|domain| domain.domain())
        .ok_or_else(|| invalid_artifact("ALE eliminated rate has no selected Domain"))?;
    let selected_domains = coupled
        .spatial()
        .domains()
        .iter()
        .map(|domain| {
            let fields = domain
                .field_spaces()
                .iter()
                .map(|binding| binding.field())
                .chain((domain.domain() == rate_domain).then_some(eliminated.state()));
            eqiora_realization::DomainFieldInventory::new(domain.domain(), fields)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| invalid_artifact(error.to_string()))?;
    if requirements.coupled().domains() != selected_domains
        || requirements.coupled().trace_quotient() != coupled.spatial().trace_quotient()
        || requirements.coupled().eliminated_state() != eliminated
    {
        return Err(invalid_artifact(
            "ALE common plan does not bind the exact required Domain, Field, trace, and eliminated-state inventory",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireAleEnvelope<Q> {
    pub(crate) schema: String,
    pub(crate) encoding: String,
    pub(crate) model_sha256: String,
    pub(crate) model_ulid: String,
    pub(crate) semantic_revision: u64,
    pub(crate) source: WireAleSource,
    pub(crate) requirements: WireAleRequirements,
    pub(crate) plan: wire::WireAlePlanWith<Q>,
    pub(crate) layout_artifacts: WireLayoutArtifacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireAleSource {
    Explicit { realization_revision: u64 },
}

impl<Q> WireAleEnvelope<Q>
where
    Q: WireQuadratureCodec + Clone + PartialEq,
{
    pub(crate) fn from_resolved(
        schema: &str,
        model: &impl CanonicalModelArtifact,
        resolved: &ResolvedFixedTopologyAleCoupledRealization,
        layout_artifacts: LayoutArtifacts,
    ) -> Result<Self, Diagnostic> {
        let model = model.artifact_reference()?;
        if model.model() != resolved.model()
            || model.semantic_revision() != resolved.semantic_revision()
        {
            return Err(invalid_artifact(
                "resolved ALE realization does not identify the supplied Model artifact and source revision",
            ));
        }
        require_layout_artifacts(
            resolved
                .requirements()
                .coupled()
                .execution()
                .vector_layout(),
            &layout_artifacts,
        )?;
        resolved.portable_graph().map_err(|error| {
            invalid_artifact(format!(
                "cannot project resolved fixed-topology ALE graph: {error}",
            ))
        })?;
        let wire = Self {
            schema: schema.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            model_sha256: model.artifact().to_string(),
            model_ulid: resolved.model().ulid().to_string(),
            semantic_revision: resolved.semantic_revision().get(),
            source: WireAleSource::Explicit {
                realization_revision: resolved.realization_revision().get(),
            },
            requirements: WireAleRequirements::encode(resolved.requirements())?,
            plan: wire::WireAlePlanWith::<Q>::encode(resolved.requirements(), resolved.plan())?,
            layout_artifacts: WireLayoutArtifacts::encode(layout_artifacts),
        };
        wire.validate(schema)?;
        Ok(wire)
    }

    pub(crate) fn from_json(
        schema: &str,
        bytes: &[u8],
        limits: RealizationDecoderLimits,
    ) -> Result<Self, Diagnostic>
    where
        Q: for<'de> Deserialize<'de>,
    {
        check_json_limits(bytes, limits.json)?;
        let wire: Self = serde_json::from_slice(bytes).map_err(|error| {
            invalid_artifact(format!("invalid ALE realization envelope JSON: {error}"))
        })?;
        wire.requirements.validate_limits(limits)?;
        wire.plan.validate_limits(limits)?;
        wire.validate(schema)?;
        Ok(wire)
    }

    pub(crate) fn validate(&self, schema: &str) -> Result<(), Diagnostic> {
        if self.schema != schema || self.encoding != CANONICAL_ENCODING {
            return Err(invalid_artifact(format!(
                "unsupported {schema} schema or canonical encoding",
            )));
        }
        ArtifactDigest::from_hex(self.model_sha256.clone())?;
        parse_ulid(&self.model_ulid)?;
        let requirements = self.requirements.clone().decode()?;
        let plan = self.plan.clone().decode(&requirements)?;
        validate_requirements_plan(&requirements, &plan)?;
        if WireAleRequirements::encode(&requirements)? != self.requirements
            || wire::WireAlePlanWith::<Q>::encode(&requirements, &plan)? != self.plan
        {
            return Err(invalid_artifact(
                "ALE realization arrays or graph values are not in canonical closed form",
            ));
        }
        let artifacts = self.layout_artifacts.decode_validated()?;
        require_layout_artifacts(
            requirements.coupled().execution().vector_layout(),
            &artifacts,
        )?;
        Ok(())
    }
}

pub(crate) fn require_layout_artifacts(
    layout: eqiora_realization::VectorLayoutKind,
    artifacts: &LayoutArtifacts,
) -> Result<(), Diagnostic> {
    if matches!(
        (layout, artifacts),
        (
            eqiora_realization::VectorLayoutKind::Replicated,
            LayoutArtifacts::Replicated
        ) | (
            eqiora_realization::VectorLayoutKind::Distributed,
            LayoutArtifacts::Distributed { .. }
        )
    ) {
        Ok(())
    } else {
        Err(invalid_artifact(
            "ALE realization layout artifacts contradict the admitted vector-layout requirement",
        ))
    }
}

pub(crate) fn parse_ulid(value: &str) -> Result<Ulid, Diagnostic> {
    let ulid: Ulid = value
        .parse()
        .map_err(|_| invalid_artifact("model ULID is malformed"))?;
    if ulid.to_string() != value {
        return Err(invalid_artifact("model ULID is not in canonical spelling"));
    }
    Ok(ulid)
}
