//! One failure-atomic prescribed-solid occurrence through a connected subprocess.

use std::process::Child;
use std::sync::atomic::AtomicBool;

use eqiora_artifact::{
    ArtifactDigest, GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    ModelEnvelope, PrescribedDynamicSolidProviderOccurrenceEnvelopeV1,
    PrescribedDynamicSolidRealizationEnvelopeV1, RunManifestV2, SimplicialMeshEnvelopeV1,
    SpatialStateEnvelopeV1,
};
use eqiora_assembly::AssemblyBackend;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::VertexId;
use eqiora_numerics::solid::AcceptedPrescribedDynamicSolidStep3d;
use eqiora_solver::LinearSolverBackend;

use super::PrescribedDynamicSolidStateRun3d;
use super::composition::{self, PreparedPrescribedDynamicSolid3d};
use crate::ModelDocument;

mod protocol;
mod session;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_EXTERNAL_DATA_IMPORT, message)
}

fn invalid_artifact(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

use protocol::control::{ErrorControl, ReceivedControl};
use protocol::{Exchange, Transcript};
use session::{CancellationCheckpoint, ConnectedSession, check_cancellation};

/// Complete accepted owner for one connected-subprocess provider occurrence.
pub struct PrescribedDynamicSolidExternalProviderStateRun3d {
    pub(super) direct: PrescribedDynamicSolidStateRun3d,
    pub(super) accepted_state: SpatialStateEnvelopeV1,
    pub(super) candidate_bytes: Vec<u8>,
    pub(super) candidate_bulk: Vec<u8>,
    pub(super) transcript_bytes: Vec<u8>,
    pub(super) provider_occurrence: PrescribedDynamicSolidProviderOccurrenceEnvelopeV1,
    pub(super) run: RunManifestV2,
    evidence: RetainedProviderEvidence,
}

struct RetainedProviderEvidence {
    binding_identity: ArtifactDigest,
    displacement_input_identity: ArtifactDigest,
    velocity_input_identity: ArtifactDigest,
    request_identity: ArtifactDigest,
    candidate_identity: ArtifactDigest,
    candidate: Vec<(VertexId, [f64; 3])>,
    transcript: Transcript,
}

impl PrescribedDynamicSolidExternalProviderStateRun3d {
    /// Execute one deterministic occurrence using an already-connected subprocess.
    ///
    /// The child must already own piped stdin and stdout. This operation never
    /// receives launch configuration and consumes the process on every path.
    ///
    /// # Errors
    /// Returns a structured diagnostic for cancellation, timeout, protocol,
    /// provider, numerical, admission, close, or exact-lineage failure.
    pub fn solve_reference_with_connected_subprocess(
        document: &ModelDocument,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
        connected_provider: Child,
        cancellation: &AtomicBool,
    ) -> Result<Self, Diagnostic> {
        let mut session = ConnectedSession::admit(connected_provider, cancellation)?;
        let hello = expect_hello(session.receive_control(
            cancellation,
            CancellationCheckpoint::BeforeHelloFrame,
            CancellationCheckpoint::AfterHelloFrame,
        )?)?;
        hello.validate()?;

        check_cancellation(cancellation, CancellationCheckpoint::BeforeProjection)?;
        let prepared = PreparedPrescribedDynamicSolid3d::new(document)?;
        let exchange = Exchange::new(&prepared)?;
        check_cancellation(cancellation, CancellationCheckpoint::AfterProjection)?;

        session.send_control(
            &exchange.bind,
            cancellation,
            CancellationCheckpoint::BeforeBindFrame,
            CancellationCheckpoint::AfterBindFrame,
        )?;
        let bound = expect_bound(session.receive_control(
            cancellation,
            CancellationCheckpoint::BeforeBoundFrame,
            CancellationCheckpoint::AfterBoundFrame,
        )?)?;
        exchange.validate_bound(&bound)?;
        session.send_control(
            &exchange.evaluate,
            cancellation,
            CancellationCheckpoint::BeforeEvaluateFrame,
            CancellationCheckpoint::AfterEvaluateFrame,
        )?;
        session.send_bulk(
            exchange.displacement_bulk.clone(),
            cancellation,
            CancellationCheckpoint::BeforeDisplacementBulkFrame,
            CancellationCheckpoint::AfterDisplacementBulkFrame,
        )?;
        session.send_bulk(
            exchange.velocity_bulk.clone(),
            cancellation,
            CancellationCheckpoint::BeforeVelocityBulkFrame,
            CancellationCheckpoint::AfterVelocityBulkFrame,
        )?;
        let candidate_control = expect_candidate(session.receive_control(
            cancellation,
            CancellationCheckpoint::BeforeCandidateFrame,
            CancellationCheckpoint::AfterCandidateFrame,
        )?)?;
        exchange.validate_candidate_control(&candidate_control)?;
        let candidate_bulk = session.receive_bulk(
            cancellation,
            CancellationCheckpoint::BeforeCandidateBulkFrame,
            CancellationCheckpoint::AfterCandidateBulkFrame,
        )?;
        let (candidate_identity, candidate) =
            exchange.admit_candidate(&candidate_control, &candidate_bulk)?;
        let report = expect_report(session.receive_control(
            cancellation,
            CancellationCheckpoint::BeforeReportFrame,
            CancellationCheckpoint::AfterReportFrame,
        )?)?;
        exchange.validate_report(&report, &candidate_identity)?;
        if !composition::candidate_matches_exact(&candidate) {
            return Err(invalid(
                "provider candidate differs bitwise from the frozen affine predictor",
            ));
        }

        check_cancellation(cancellation, CancellationCheckpoint::BeforeStructuralSolve)?;
        let accepted = prepared.solve_candidate(document, &candidate, assembly, solver)?;
        check_cancellation(cancellation, CancellationCheckpoint::AfterStructuralSolve)?;

        check_cancellation(cancellation, CancellationCheckpoint::BeforeComposition)?;
        let direct = prepared.compose_accepted(accepted)?;
        let close = exchange.close(&candidate_identity);
        let expected_closed = exchange.expected_closed(&candidate_identity);
        let prospective_transcript = session.transcript().prospective(&close, &expected_closed)?;
        let provider_occurrence = protocol::occurrence(
            &prepared,
            direct.accepted_state(),
            &exchange,
            candidate_identity.clone(),
            prospective_transcript.identity(),
        )?;
        let run = RunManifestV2::new(direct.realization(), direct.run().execution())?
            .with_output(direct.accepted_state().digest()?)
            .with_output(provider_occurrence.digest()?);
        let mut value = Self {
            accepted_state: direct.accepted_state().clone(),
            candidate_bytes: candidate_bulk.clone(),
            candidate_bulk: candidate_bulk.clone(),
            transcript_bytes: prospective_transcript.bytes().to_vec(),
            direct,
            provider_occurrence,
            run,
            evidence: RetainedProviderEvidence {
                binding_identity: exchange.binding_identity.clone(),
                displacement_input_identity: exchange.displacement_input_identity.clone(),
                velocity_input_identity: exchange.velocity_input_identity.clone(),
                request_identity: exchange.request_identity.clone(),
                candidate_identity: candidate_identity.clone(),
                candidate,
                transcript: prospective_transcript.clone(),
            },
        };
        value.revalidate_with_exchange(&exchange)?;
        check_cancellation(cancellation, CancellationCheckpoint::AfterComposition)?;

        session.send_control(
            &close,
            cancellation,
            CancellationCheckpoint::BeforeCloseFrame,
            CancellationCheckpoint::AfterCloseFrame,
        )?;
        let closed = expect_closed(session.receive_control(
            cancellation,
            CancellationCheckpoint::BeforeClosedFrame,
            CancellationCheckpoint::AfterClosedFrame,
        )?)?;
        if closed != expected_closed {
            return Err(invalid(
                "provider closed control differs from the accepted request and candidate",
            ));
        }
        let actual_transcript = session.finish(cancellation)?;
        if actual_transcript != prospective_transcript {
            return Err(invalid(
                "provider live transcript differs from the pre-admitted close transcript",
            ));
        }
        value.evidence.transcript = actual_transcript;
        value.transcript_bytes = value.evidence.transcript.bytes().to_vec();
        value.revalidate_with_exchange(&exchange)?;
        check_cancellation(cancellation, CancellationCheckpoint::BeforePublication)?;
        Ok(value)
    }

    /// Exact current Model used by this occurrence.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        self.direct.model()
    }

    /// Exact Geometry identity used by this occurrence.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        self.direct.geometry()
    }

    /// Exact Geometry-to-Mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        self.direct.correspondence()
    }

    /// Exact immutable reference mesh artifact.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        self.direct.mesh()
    }

    /// Exact standalone prescribed-solid Realization.
    #[must_use]
    pub const fn realization(&self) -> &PrescribedDynamicSolidRealizationEnvelopeV1 {
        self.direct.realization()
    }

    /// Nonforgeable accepted numerical evidence.
    #[must_use]
    pub const fn accepted(&self) -> &AcceptedPrescribedDynamicSolidStep3d {
        self.direct.accepted()
    }

    /// Exact retained prior State input observation.
    #[must_use]
    pub const fn prior_state(&self) -> &SpatialStateEnvelopeV1 {
        self.direct.prior_state()
    }

    /// Exact accepted-next State output.
    #[must_use]
    pub const fn accepted_state(&self) -> &SpatialStateEnvelopeV1 {
        &self.accepted_state
    }

    /// Complete role-preserving provider occurrence.
    #[must_use]
    pub const fn provider_occurrence(&self) -> &PrescribedDynamicSolidProviderOccurrenceEnvelopeV1 {
        &self.provider_occurrence
    }

    /// Two-output Run linking accepted State and provider occurrence.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Revalidate retained resources, live evidence, admission, and exact Run outputs.
    ///
    /// # Errors
    /// Returns `EQ0901` for any retained cross-substitution or role drift.
    pub fn revalidate(&self) -> Result<(), Diagnostic> {
        self.direct.revalidate()?;
        if self.direct.accepted_state() != &self.accepted_state
            || self.candidate_bytes != self.candidate_bulk
            || self.transcript_bytes != self.evidence.transcript.bytes()
        {
            return Err(invalid_artifact(
                "retained provider State, candidate bytes, or transcript bytes differ",
            ));
        }
        self.provider_occurrence.validate_against(
            self.model(),
            self.realization(),
            self.geometry(),
            self.correspondence(),
            self.mesh(),
            self.prior_state(),
            self.accepted_state(),
        )?;
        let decoded_candidate = protocol::decode_candidate(&self.candidate_bytes)
            .map_err(|error| invalid_artifact(error.message().to_owned()))?;
        if !composition::candidate_matches_exact(&self.evidence.candidate)
            || decoded_candidate != self.evidence.candidate
            || self.provider_occurrence.binding_identity() != self.evidence.binding_identity
            || self.provider_occurrence.displacement_input_identity()
                != self.evidence.displacement_input_identity
            || self.provider_occurrence.velocity_input_identity()
                != self.evidence.velocity_input_identity
            || self.provider_occurrence.request_identity() != self.evidence.request_identity
            || self.provider_occurrence.candidate_identity() != self.evidence.candidate_identity
            || self.provider_occurrence.transcript_identity() != self.evidence.transcript.identity()
        {
            return Err(invalid_artifact(
                "retained provider projection, candidate, or transcript evidence differs",
            ));
        }
        self.evidence
            .transcript
            .validate_success()
            .map_err(|error| invalid_artifact(error.message().to_owned()))?;
        self.run.validate_against(self.realization())?;
        let expected = RunManifestV2::new(self.realization(), self.direct.run().execution())?
            .with_output(self.accepted_state().digest()?)
            .with_output(self.provider_occurrence.digest()?);
        if self.run != expected {
            return Err(invalid_artifact(
                "external-provider Run differs from the exact accepted-State and occurrence outputs",
            ));
        }
        Ok(())
    }

    fn revalidate_with_exchange(&self, exchange: &Exchange) -> Result<(), Diagnostic> {
        self.revalidate()?;
        if exchange.candidate_identity_for_bulk(&self.candidate_bytes)
            != self.evidence.candidate_identity
            || exchange.binding_identity != self.evidence.binding_identity
            || exchange.request_identity != self.evidence.request_identity
            || exchange.displacement_input_identity != self.evidence.displacement_input_identity
            || exchange.velocity_input_identity != self.evidence.velocity_input_identity
        {
            return Err(invalid_artifact(
                "provider retained identities differ from the exact live exchange",
            ));
        }
        Ok(())
    }
}

fn expect_hello(control: ReceivedControl) -> Result<protocol::control::Hello, Diagnostic> {
    match control {
        ReceivedControl::Hello(value) => Ok(value),
        ReceivedControl::Error(_) => Err(invalid("provider error is invalid before bind")),
        _ => Err(invalid("provider response is invalid while awaiting hello")),
    }
}

fn expect_bound(control: ReceivedControl) -> Result<protocol::control::Bound, Diagnostic> {
    match control {
        ReceivedControl::Bound(value) => Ok(value),
        ReceivedControl::Error(error) => provider_error(error, "bind"),
        _ => Err(invalid("provider response is invalid while awaiting bound")),
    }
}

fn expect_candidate(control: ReceivedControl) -> Result<protocol::control::Candidate, Diagnostic> {
    match control {
        ReceivedControl::Candidate(value) => Ok(value),
        ReceivedControl::Error(error) => provider_error(error, "evaluate"),
        _ => Err(invalid(
            "provider response is invalid while awaiting candidate",
        )),
    }
}

fn expect_report(control: ReceivedControl) -> Result<protocol::control::Report, Diagnostic> {
    match control {
        ReceivedControl::Report(value) => Ok(value),
        ReceivedControl::Error(error) => provider_error(error, "evaluate"),
        _ => Err(invalid(
            "provider response is invalid while awaiting report",
        )),
    }
}

fn expect_closed(control: ReceivedControl) -> Result<protocol::control::Closed, Diagnostic> {
    match control {
        ReceivedControl::Closed(value) => Ok(value),
        ReceivedControl::Error(error) => provider_error(error, "close"),
        _ => Err(invalid(
            "provider response is invalid while awaiting closed",
        )),
    }
}

fn provider_error<T>(error: ErrorControl, expected_phase: &str) -> Result<T, Diagnostic> {
    if error.phase != expected_phase {
        return Err(invalid(
            "provider error phase differs from the active protocol state",
        ));
    }
    Err(invalid(format!(
        "provider rejected the {expected_phase} phase with {}: {}",
        error.code, error.message
    )))
}

#[cfg(test)]
pub(super) mod test_support {
    use std::process::Child;
    use std::sync::atomic::AtomicBool;

    use eqiora_assembly::AssemblyBackend;
    use eqiora_core::Diagnostic;
    use eqiora_solver::LinearSolverBackend;

    use super::session::set_test_cancellation_checkpoint;
    use super::{ModelDocument, PrescribedDynamicSolidExternalProviderStateRun3d};

    pub(super) use super::session::CancellationCheckpoint;

    struct ResetCheckpoint;

    impl Drop for ResetCheckpoint {
        fn drop(&mut self) {
            set_test_cancellation_checkpoint(None);
        }
    }

    pub(super) fn solve_with_injected_cancellation_checkpoint(
        document: &ModelDocument,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
        connected_provider: Child,
        cancellation: &AtomicBool,
        checkpoint: CancellationCheckpoint,
    ) -> Result<PrescribedDynamicSolidExternalProviderStateRun3d, Diagnostic> {
        set_test_cancellation_checkpoint(Some(checkpoint));
        let _reset = ResetCheckpoint;
        PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
            document,
            assembly,
            solver,
            connected_provider,
            cancellation,
        )
    }
}

#[cfg(test)]
mod tests;
