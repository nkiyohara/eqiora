//! Durable binding from one exact circular-hole source to chordal resources.
//!
//! This artifact owns only the closed binding relation. Exact geometry,
//! chordal realization semantics, realized geometry, mesh topology, and
//! authored-region correspondence remain owned by their existing contracts.

use eqiora_core::Diagnostic;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_meshing::MeshQualityGate;
use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDigest, CANONICAL_ENCODING, GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1,
    JsonDecoderLimits, SimplicialMeshEnvelopeV1, check_json_limits, invalid_artifact,
};

use super::circular_hole_chordal_reference::ChordalReference;

const CIRCULAR_HOLE_CHORDAL_REALIZATION_SCHEMA: &str =
    "eqiora.circular-hole-chordal-realization-envelope/v1";
const MINIMUM_CIRCLE_SEGMENTS: u64 = 8;

/// Canonical binding of an exact circular-hole source to chordal resources.
///
/// [`AcceptedCircularHoleChordalRealizationV1::from_reference`] constructs an
/// accepted reference binding. [`Self::from_json`] admits only the local wire;
/// a decoded value is not an accepted realization until
/// [`Self::replay_against`] succeeds with all four independently admitted
/// external resources.
///
/// The supplied mesh may be any conforming affine-simplex mesh. It need not be
/// the source-owned reference mesh, because its own content identity and its
/// authored-region correspondence are bound separately.
#[derive(Clone, Debug, PartialEq)]
pub struct CircularHoleChordalRealizationEnvelopeV1 {
    wire: WireCircularHoleChordalRealizationEnvelopeV1,
}

/// Replay-accepted exact-source chordal resources.
///
/// This type-state owner is the only runtime bundle for the bounded reference
/// realization. The bare envelope remains a pure decoded wire value; accepted
/// Geometry, Mesh, correspondence, and regenerated reference observations are
/// available only after construction or complete replay succeeds.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptedCircularHoleChordalRealizationV1 {
    envelope: CircularHoleChordalRealizationEnvelopeV1,
    source: CanonicalGeometryV1,
    realized_geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    reference: ChordalReference,
}

impl CircularHoleChordalRealizationEnvelopeV1 {
    fn capture_reference(
        source: &CanonicalGeometryV1,
        reference: &ChordalReference,
        realized_geometry: &GeometryDefinitionV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        if reference.source().digest_bytes() != source.digest_bytes() {
            return Err(invalid_artifact(
                "chordal reference does not name the supplied exact source",
            ));
        }

        let circle_segments = u64::try_from(reference.circle_segments()).map_err(|_| {
            invalid_artifact("chordal realization segment count exceeds portable u64")
        })?;
        let envelope = Self {
            wire: WireCircularHoleChordalRealizationEnvelopeV1 {
                schema: CIRCULAR_HOLE_CHORDAL_REALIZATION_SCHEMA.to_owned(),
                encoding: CANONICAL_ENCODING.to_owned(),
                source_geometry_sha256: ArtifactDigest::from_sha256(source.digest_bytes())
                    .to_string(),
                realized_geometry_sha256: realized_geometry.digest()?.to_string(),
                mesh_sha256: mesh.digest()?.to_string(),
                correspondence_sha256: correspondence.digest()?.to_string(),
                requested_max_boundary_error_m: reference.requested_max_boundary_error_m(),
                boundary_evaluation_allowance_m: reference.boundary_evaluation_allowance_m(),
                boundary_error_bound_m: reference.boundary_error_bound_m(),
                circle_segments,
                circle_area_deficit_m2: reference.circle_area_deficit_m2(),
                circle_perimeter_deficit_m: reference.circle_perimeter_deficit_m(),
                required_minimum_mean_ratio: reference.mesh().quality_gate().minimum_mean_ratio(),
            },
        };
        envelope.validate_local()?;
        envelope.validate_replay_against(source, realized_geometry, mesh, correspondence)?;
        Ok(envelope)
    }

    /// Decode and locally admit one canonical binding envelope.
    ///
    /// This operation checks only the binding envelope's closed vocabulary,
    /// byte and nesting budgets, canonical spelling, digest syntax, and local
    /// scalar relations. It does not load or accept any referenced resource.
    /// The returned value remains unaccepted until [`Self::replay_against`]
    /// succeeds with independently bounded source, geometry, mesh, and
    /// correspondence resources.
    ///
    /// # Errors
    /// Returns `EQ0901` for oversized, over-nested, malformed, unknown,
    /// missing, reordered, noncanonical, non-finite, non-positive, or locally
    /// inconsistent wire data.
    pub fn from_json(bytes: &[u8], limits: JsonDecoderLimits) -> Result<Self, Diagnostic> {
        check_json_limits(bytes, limits)?;
        let wire: WireCircularHoleChordalRealizationEnvelopeV1 = serde_json::from_slice(bytes)
            .map_err(|error| {
                invalid_artifact(format!(
                    "invalid circular-hole chordal realization JSON: {error}"
                ))
            })?;
        let envelope = Self { wire };
        envelope.validate_local()?;
        if envelope.canonical_json()?.as_slice() != bytes {
            return Err(invalid_artifact(
                "circular-hole chordal realization JSON is not the canonical encoding",
            ));
        }
        Ok(envelope)
    }

    /// Deterministic canonical JSON bytes in the frozen thirteen-field order.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization unexpectedly fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        serde_json::to_vec(&self.wire).map_err(|error| {
            invalid_artifact(format!(
                "cannot serialize circular-hole chordal realization: {error}"
            ))
        })
    }

    /// Domain-separated identity of the complete binding envelope.
    ///
    /// # Errors
    /// Returns `EQ0901` if canonical serialization fails.
    pub fn digest(&self) -> Result<ArtifactDigest, Diagnostic> {
        Ok(ArtifactDigest::compute(
            CIRCULAR_HOLE_CHORDAL_REALIZATION_SCHEMA.as_bytes(),
            &self.canonical_json()?,
        ))
    }

    /// Regenerate and validate this binding against all external resources.
    ///
    /// The stored segment count is a maximum work limit, not a trusted answer.
    /// The accepted owner retains the supplied, digest-bound conforming Mesh.
    /// Its private reference mesh is freshly regenerated from exact source
    /// meaning and may differ from that bound resource.
    ///
    /// # Errors
    /// Returns `EQ0901` for source substitution, regeneration failure,
    /// observation drift, region mismatch, non-authored correspondence,
    /// correspondence replay failure, or any bound resource digest mismatch.
    pub fn replay_against(
        &self,
        source: &CanonicalGeometryV1,
        realized_geometry: &GeometryDefinitionV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<AcceptedCircularHoleChordalRealizationV1, Diagnostic> {
        let reference =
            self.validate_replay_against(source, realized_geometry, mesh, correspondence)?;
        Ok(AcceptedCircularHoleChordalRealizationV1 {
            envelope: self.clone(),
            source: source.clone(),
            realized_geometry: realized_geometry.clone(),
            mesh: mesh.clone(),
            correspondence: correspondence.clone(),
            reference,
        })
    }

    fn validate_replay_against(
        &self,
        source: &CanonicalGeometryV1,
        realized_geometry: &GeometryDefinitionV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<ChordalReference, Diagnostic> {
        self.validate_local()?;

        let source_digest = ArtifactDigest::from_sha256(source.digest_bytes());
        if self.source_geometry_artifact() != source_digest {
            return Err(invalid_artifact(
                "chordal realization exact source digest does not match",
            ));
        }

        let max_segments = usize::try_from(self.wire.circle_segments).map_err(|_| {
            invalid_artifact("chordal realization segment work limit exceeds the local usize range")
        })?;
        let quality_gate =
            MeshQualityGate::new(self.wire.required_minimum_mean_ratio).map_err(|error| {
                invalid_artifact(format!(
                    "chordal realization quality policy cannot be replayed: {}",
                    error.message()
                ))
            })?;
        let reference = ChordalReference::from_exact(
            source,
            self.wire.requested_max_boundary_error_m,
            max_segments,
            quality_gate,
        )
        .map_err(|error| {
            invalid_artifact(format!(
                "chordal realization reference cannot be regenerated: {}",
                error.message()
            ))
        })?;

        if !self.matches_reference_observations(&reference) {
            return Err(invalid_artifact(
                "chordal realization observations differ from deterministic reference replay",
            ));
        }

        let replayed_region = realized_geometry.region()?;
        if &replayed_region != reference.region() {
            return Err(invalid_artifact(
                "chordal realization geometry differs from deterministic reference replay",
            ));
        }

        correspondence
            .validate_against_region(realized_geometry, mesh)
            .map_err(|error| {
                invalid_artifact(format!(
                    "chordal realization authored-region correspondence cannot be replayed: {}",
                    error.message()
                ))
            })?;

        let actual_resource_digests = [
            source_digest,
            realized_geometry.digest()?,
            mesh.digest()?,
            correspondence.digest()?,
        ];
        let bound_resource_digests = [
            self.source_geometry_artifact(),
            self.realized_geometry_artifact(),
            self.mesh_artifact(),
            self.correspondence_artifact(),
        ];
        if actual_resource_digests != bound_resource_digests {
            return Err(invalid_artifact(
                "chordal realization bound resource digests do not match",
            ));
        }

        Ok(reference)
    }

    /// Exact circular-hole source geometry artifact.
    #[must_use]
    pub fn source_geometry_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.source_geometry_sha256)
    }

    /// Exact realized straight-region artifact.
    #[must_use]
    pub fn realized_geometry_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.realized_geometry_sha256)
    }

    /// Exact conforming affine-simplex mesh artifact.
    #[must_use]
    pub fn mesh_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.mesh_sha256)
    }

    /// Exact Model-free authored-region correspondence artifact.
    #[must_use]
    pub fn correspondence_artifact(&self) -> ArtifactDigest {
        admitted_digest(&self.wire.correspondence_sha256)
    }

    /// Requested maximum symmetric circular-boundary error in metres.
    #[must_use]
    pub const fn requested_max_boundary_error_m(&self) -> f64 {
        self.wire.requested_max_boundary_error_m
    }

    /// Scale-aware binary64 boundary-evaluation allowance in metres.
    #[must_use]
    pub const fn boundary_evaluation_allowance_m(&self) -> f64 {
        self.wire.boundary_evaluation_allowance_m
    }

    /// Measured accepted circular-boundary error bound in metres.
    #[must_use]
    pub const fn boundary_error_bound_m(&self) -> f64 {
        self.wire.boundary_error_bound_m
    }

    /// Number of straight segments realizing the exact circular boundary.
    #[must_use]
    pub const fn circle_segments(&self) -> u64 {
        self.wire.circle_segments
    }

    /// Exact-circle minus chordal-loop area in square metres.
    #[must_use]
    pub const fn circle_area_deficit_m2(&self) -> f64 {
        self.wire.circle_area_deficit_m2
    }

    /// Exact-circle minus chordal-loop perimeter in metres.
    #[must_use]
    pub const fn circle_perimeter_deficit_m(&self) -> f64 {
        self.wire.circle_perimeter_deficit_m
    }

    /// Mean-ratio threshold used to construct the source-owned reference mesh.
    #[must_use]
    pub const fn required_minimum_mean_ratio(&self) -> f64 {
        self.wire.required_minimum_mean_ratio
    }

    fn validate_local(&self) -> Result<(), Diagnostic> {
        if self.wire.schema != CIRCULAR_HOLE_CHORDAL_REALIZATION_SCHEMA
            || self.wire.encoding != CANONICAL_ENCODING
        {
            return Err(invalid_artifact(
                "unsupported circular-hole chordal realization schema or canonical encoding",
            ));
        }
        for digest in [
            &self.wire.source_geometry_sha256,
            &self.wire.realized_geometry_sha256,
            &self.wire.mesh_sha256,
            &self.wire.correspondence_sha256,
        ] {
            ArtifactDigest::from_hex(digest.clone())?;
        }
        for (label, value) in [
            (
                "requested maximum boundary error",
                self.wire.requested_max_boundary_error_m,
            ),
            (
                "boundary evaluation allowance",
                self.wire.boundary_evaluation_allowance_m,
            ),
            ("boundary error bound", self.wire.boundary_error_bound_m),
            ("circle area deficit", self.wire.circle_area_deficit_m2),
            (
                "circle perimeter deficit",
                self.wire.circle_perimeter_deficit_m,
            ),
            (
                "required minimum mean ratio",
                self.wire.required_minimum_mean_ratio,
            ),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(invalid_artifact(format!(
                    "chordal realization {label} must be finite and positive"
                )));
            }
        }
        if self.wire.circle_segments < MINIMUM_CIRCLE_SEGMENTS {
            return Err(invalid_artifact(format!(
                "chordal realization requires at least {MINIMUM_CIRCLE_SEGMENTS} circle segments",
            )));
        }
        if self.wire.boundary_evaluation_allowance_m >= self.wire.requested_max_boundary_error_m
            || self.wire.boundary_error_bound_m < self.wire.boundary_evaluation_allowance_m
            || self.wire.boundary_error_bound_m > self.wire.requested_max_boundary_error_m
            || self.wire.required_minimum_mean_ratio > 1.0
        {
            return Err(invalid_artifact(
                "chordal realization boundary-error or quality policy is inconsistent",
            ));
        }
        Ok(())
    }

    fn matches_reference_observations(&self, reference: &ChordalReference) -> bool {
        self.wire.requested_max_boundary_error_m.to_bits()
            == reference.requested_max_boundary_error_m().to_bits()
            && self.wire.boundary_evaluation_allowance_m.to_bits()
                == reference.boundary_evaluation_allowance_m().to_bits()
            && self.wire.boundary_error_bound_m.to_bits()
                == reference.boundary_error_bound_m().to_bits()
            && usize::try_from(self.wire.circle_segments)
                .is_ok_and(|segments| segments == reference.circle_segments())
            && self.wire.circle_area_deficit_m2.to_bits()
                == reference.circle_area_deficit_m2().to_bits()
            && self.wire.circle_perimeter_deficit_m.to_bits()
                == reference.circle_perimeter_deficit_m().to_bits()
            && self.wire.required_minimum_mean_ratio.to_bits()
                == reference
                    .mesh()
                    .quality_gate()
                    .minimum_mean_ratio()
                    .to_bits()
    }
}

impl AcceptedCircularHoleChordalRealizationV1 {
    /// Construct the bounded source-owned reference realization.
    ///
    /// # Errors
    /// Preserves exact-source, approximation-policy, work-limit, topology,
    /// quality, artifact, and authored-correspondence diagnostics.
    pub fn from_reference(
        source: &CanonicalGeometryV1,
        requested_max_boundary_error_m: f64,
        max_segments: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        let reference = ChordalReference::from_exact(
            source,
            requested_max_boundary_error_m,
            max_segments,
            quality_gate,
        )?;
        let realized_geometry = GeometryDefinitionV1::from_region(reference.region());
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(reference.mesh())?;
        let correspondence =
            GeometryMeshCorrespondenceEnvelopeV1::from_region(&realized_geometry, &mesh)?;
        let envelope = CircularHoleChordalRealizationEnvelopeV1::capture_reference(
            source,
            &reference,
            &realized_geometry,
            &mesh,
            &correspondence,
        )?;
        Ok(Self {
            envelope,
            source: source.clone(),
            realized_geometry,
            mesh,
            correspondence,
            reference,
        })
    }

    /// Bind a distinct conforming Mesh to the same exact source and reference
    /// observations.
    ///
    /// The realized Geometry remains the deterministic chordal region. The
    /// supplied correspondence is the only bridge to the independently
    /// admitted Mesh index space.
    ///
    /// # Errors
    /// Returns `EQ0901` for any geometry, mesh, correspondence, digest, source,
    /// policy, or deterministic-observation mismatch.
    pub fn bind_conforming_mesh(
        &self,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<Self, Diagnostic> {
        self.revalidate()?;
        let envelope = CircularHoleChordalRealizationEnvelopeV1::capture_reference(
            &self.source,
            &self.reference,
            &self.realized_geometry,
            mesh,
            correspondence,
        )?;
        envelope.replay_against(&self.source, &self.realized_geometry, mesh, correspondence)
    }

    /// Re-run deterministic source and resource admission at the use site.
    ///
    /// # Errors
    /// Preserves all envelope replay and referenced-resource diagnostics.
    pub fn revalidate(&self) -> Result<(), Diagnostic> {
        let regenerated = self.envelope.validate_replay_against(
            &self.source,
            &self.realized_geometry,
            &self.mesh,
            &self.correspondence,
        )?;
        if regenerated != self.reference {
            return Err(invalid_artifact(
                "accepted chordal reference differs from deterministic replay",
            ));
        }
        Ok(())
    }

    /// Pure durable binding envelope retained by this accepted owner.
    #[must_use]
    pub const fn envelope(&self) -> &CircularHoleChordalRealizationEnvelopeV1 {
        &self.envelope
    }

    /// Exact common Geometry source.
    #[must_use]
    pub const fn source(&self) -> &CanonicalGeometryV1 {
        &self.source
    }

    /// Deterministic straight-edged Geometry resource.
    #[must_use]
    pub const fn realized_geometry(&self) -> &GeometryDefinitionV1 {
        &self.realized_geometry
    }

    /// Exact bound conforming Mesh resource.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }

    /// Exact authored Geometry-to-Mesh correspondence resource.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Caller-requested maximum symmetric circular-boundary error in metres.
    #[must_use]
    pub const fn requested_max_boundary_error_m(&self) -> f64 {
        self.envelope.requested_max_boundary_error_m()
    }

    /// Precommitted scale-aware binary64 evaluation allowance in metres.
    #[must_use]
    pub const fn boundary_evaluation_allowance_m(&self) -> f64 {
        self.envelope.boundary_evaluation_allowance_m()
    }

    /// Accepted measured circular-boundary error bound in metres.
    #[must_use]
    pub const fn boundary_error_bound_m(&self) -> f64 {
        self.envelope.boundary_error_bound_m()
    }

    /// Number of straight segments in the regenerated reference boundary.
    #[must_use]
    pub fn circle_segments(&self) -> usize {
        usize::try_from(self.envelope.circle_segments())
            .expect("accepted chordal segment count fits the local usize range")
    }

    /// Exact-circle minus chordal-loop area in square metres.
    #[must_use]
    pub const fn circle_area_deficit_m2(&self) -> f64 {
        self.envelope.circle_area_deficit_m2()
    }

    /// Exact-circle minus chordal-loop perimeter in metres.
    #[must_use]
    pub const fn circle_perimeter_deficit_m(&self) -> f64 {
        self.envelope.circle_perimeter_deficit_m()
    }
}

fn admitted_digest(value: &str) -> ArtifactDigest {
    ArtifactDigest::from_hex(value.to_owned())
        .expect("constructed and decoded chordal realization digests are locally admitted")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCircularHoleChordalRealizationEnvelopeV1 {
    schema: String,
    encoding: String,
    source_geometry_sha256: String,
    realized_geometry_sha256: String,
    mesh_sha256: String,
    correspondence_sha256: String,
    requested_max_boundary_error_m: f64,
    boundary_evaluation_allowance_m: f64,
    boundary_error_bound_m: f64,
    circle_segments: u64,
    circle_area_deficit_m2: f64,
    circle_perimeter_deficit_m: f64,
    required_minimum_mean_ratio: f64,
}
