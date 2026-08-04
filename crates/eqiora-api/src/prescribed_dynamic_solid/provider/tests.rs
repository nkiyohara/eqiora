use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;

use eqiora_artifact::{
    ArtifactDigest, PrescribedDynamicSolidProviderOccurrenceEnvelopeV1, RunManifestV2,
};
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_solver::REFERENCE_LINEAR_SOLVER;
use serde_json::{Value, json};

use crate::ModelDocument;

use super::super::PrescribedDynamicSolidExternalProviderStateRun3d;
use super::test_support::{
    CancellationCheckpoint, solve_with_injected_cancellation_checkpoint,
};

const DIRECT_SOURCE: &str = include_str!(
    "../../../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/models/direct.eqi"
);
const EXPECTED_OCCURRENCE: &[u8] = include_bytes!(
    "../../../../../verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/provider-occurrence.json"
);

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document() -> ModelDocument {
    ModelDocument::compile("private-provider-owner-oracle.eqi", DIRECT_SOURCE).unwrap()
}

fn positive_child() -> Child {
    let script = repository_root().join("examples/python/prescribed_dynamic_solid_provider.py");
    Command::new("uv")
        .args([
            "run",
            "--isolated",
            "--python",
            "3.12",
            "--with",
            "numpy==2.1.0",
            "python",
        ])
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the private owner oracle requires the exact provider profile")
}

fn hostile_child() -> Child {
    let script = repository_root().join(
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/mutants/hostile_provider.py",
    );
    Command::new("uv")
        .args(["run", "--isolated", "--python", "3.12", "python"])
        .arg(script)
        .arg("honest")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the checkpoint oracle launches its deterministic test peer")
}

fn assert_reaped(pid: u32, checkpoint: CancellationCheckpoint) {
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "{checkpoint:?} returned before waiting for the poisoned child"
    );
    #[cfg(not(target_os = "linux"))]
    let _ = (pid, checkpoint);
}

fn owner() -> PrescribedDynamicSolidExternalProviderStateRun3d {
    PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
        &document(),
        &REFERENCE_ASSEMBLY_BACKEND,
        &REFERENCE_LINEAR_SOLVER,
        positive_child(),
        &AtomicBool::new(false),
    )
    .expect("the retained-field oracle starts from one valid complete owner")
}

fn assert_revalidation_rejected(
    owner: &PrescribedDynamicSolidExternalProviderStateRun3d,
    label: &str,
) {
    let error = match owner.revalidate() {
        Err(error) => error,
        Ok(()) => panic!("{label} unexpectedly revalidated"),
    };
    assert_eq!(
        error.code(),
        eqiora_core::diagnostic::codes::INVALID_ARTIFACT,
        "{label} returned the wrong diagnostic"
    );
}

fn dependencies(numpy_release: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("cpython".to_owned(), "3.12".to_owned()),
        ("numpy".to_owned(), numpy_release.to_owned()),
    ])
}

fn rebuilt_occurrence(
    owner: &PrescribedDynamicSolidExternalProviderStateRun3d,
    provider_id: &str,
    provider_dependencies: &BTreeMap<String, String>,
    displacement_input: ArtifactDigest,
    velocity_input: ArtifactDigest,
) -> PrescribedDynamicSolidProviderOccurrenceEnvelopeV1 {
    let retained = &owner.provider_occurrence;
    PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::new(
        &owner.realization,
        &owner.prior_state,
        &owner.accepted_state,
        provider_id,
        "1.0.0",
        provider_dependencies,
        "provider.success",
        "affine predictor completed",
        retained.binding_identity(),
        displacement_input,
        velocity_input,
        retained.request_identity(),
        retained.candidate_identity(),
        retained.transcript_identity(),
    )
    .expect("the cross-substituted occurrence remains locally well formed")
}

fn occurrence_from_wire(
    wire: &[u8],
    label: &str,
) -> PrescribedDynamicSolidProviderOccurrenceEnvelopeV1 {
    PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
        wire,
        Default::default(),
    )
    .unwrap_or_else(|error| panic!("{label} must remain locally valid occurrence data: {error}"))
}

fn assert_occurrence_wire_rejected(wire: &[u8], label: &str) {
    let error = match PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
        wire,
        Default::default(),
    ) {
        Err(error) => error,
        Ok(_) => panic!("{label} unexpectedly decoded"),
    };
    assert_eq!(
        error.code(),
        eqiora_core::diagnostic::codes::INVALID_ARTIFACT,
        "{label} returned the wrong diagnostic"
    );
}

fn replace_first(bytes: &[u8], before: &[u8], after: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(before.len())
        .position(|window| window == before)
        .expect("the frozen occurrence contains the cross-substitution target");
    let mut output = Vec::with_capacity(bytes.len() - before.len() + after.len());
    output.extend_from_slice(&bytes[..start]);
    output.extend_from_slice(after);
    output.extend_from_slice(&bytes[start + before.len()..]);
    output
}

fn run_with_outputs(
    owner: &PrescribedDynamicSolidExternalProviderStateRun3d,
    mut outputs: Vec<String>,
) -> RunManifestV2 {
    outputs.sort();
    let mut wire: Value = serde_json::from_slice(&owner.run.canonical_json().unwrap()).unwrap();
    wire["output_sha256"] = json!(outputs);
    RunManifestV2::from_json(&serde_json::to_vec(&wire).unwrap(), Default::default())
        .expect("the cross-substituted output vector remains locally valid Run data")
}

#[test]
fn revalidate_rejects_accepted_state_candidate_and_transcript_substitution() {
    let mut wrong_state = owner();
    wrong_state.accepted_state = wrong_state.prior_state.clone();
    assert_revalidation_rejected(&wrong_state, "accepted State substitution");

    let mut wrong_candidate = owner();
    wrong_candidate.candidate_bytes[0] ^= 1;
    assert_revalidation_rejected(&wrong_candidate, "candidate byte substitution");

    let mut wrong_transcript = owner();
    let last = wrong_transcript.transcript_bytes.len() - 1;
    wrong_transcript.transcript_bytes[last] ^= 1;
    assert_revalidation_rejected(&wrong_transcript, "transcript byte substitution");
}

#[test]
fn revalidate_rejects_every_occurrence_cross_substitution() {
    let retained_dependencies = dependencies("2.1.0");
    let foreign_input = ArtifactDigest::from_sha256([0xa5; 32]);

    let mut provider = owner();
    provider.provider_occurrence = rebuilt_occurrence(
        &provider,
        "eqiora.python.foreign-affine",
        &retained_dependencies,
        provider
            .provider_occurrence
            .displacement_input_identity(),
        provider.provider_occurrence.velocity_input_identity(),
    );
    assert_revalidation_rejected(&provider, "provider and occurrence linkage substitution");

    let mut dependency = owner();
    dependency.provider_occurrence = rebuilt_occurrence(
        &dependency,
        "eqiora.python.prescribed-dynamic-solid-affine",
        &dependencies("2.1.1"),
        dependency
            .provider_occurrence
            .displacement_input_identity(),
        dependency.provider_occurrence.velocity_input_identity(),
    );
    assert_revalidation_rejected(&dependency, "provider dependency substitution");

    let mut displacement = owner();
    displacement.provider_occurrence = rebuilt_occurrence(
        &displacement,
        "eqiora.python.prescribed-dynamic-solid-affine",
        &retained_dependencies,
        foreign_input.clone(),
        displacement.provider_occurrence.velocity_input_identity(),
    );
    assert_revalidation_rejected(&displacement, "displacement input-block substitution");

    let mut velocity = owner();
    velocity.provider_occurrence = rebuilt_occurrence(
        &velocity,
        "eqiora.python.prescribed-dynamic-solid-affine",
        &retained_dependencies,
        velocity
            .provider_occurrence
            .displacement_input_identity(),
        foreign_input,
    );
    assert_revalidation_rejected(&velocity, "velocity input-block substitution");

    let wire = replace_first(
        EXPECTED_OCCURRENCE,
        b"eqiora.subprocess.external-boundary-provider",
        b"eqiora.subprocess.foreign-boundary-provider",
    );
    assert_occurrence_wire_rejected(&wire, "fixed adapter substitution");

    let mut accepted_state = owner();
    let retained = accepted_state.accepted_state.digest().unwrap().to_string();
    let foreign = accepted_state.prior_state.digest().unwrap().to_string();
    let wire = replace_first(EXPECTED_OCCURRENCE, retained.as_bytes(), foreign.as_bytes());
    accepted_state.provider_occurrence = occurrence_from_wire(&wire, "accepted State substitution");
    assert_revalidation_rejected(&accepted_state, "occurrence accepted State substitution");
}

#[test]
fn revalidate_rejects_prior_missing_additional_and_unlinked_run_outputs() {
    let mut prior = owner();
    let accepted_identity = prior.accepted_state.digest().unwrap().to_string();
    let occurrence_identity = prior.provider_occurrence.digest().unwrap().to_string();
    let prior_identity = prior.prior_state.digest().unwrap().to_string();
    prior.run = run_with_outputs(
        &prior,
        vec![occurrence_identity.clone(), prior_identity],
    );
    assert_revalidation_rejected(&prior, "prior State Run output");

    let mut missing_accepted = owner();
    missing_accepted.run = run_with_outputs(&missing_accepted, vec![occurrence_identity.clone()]);
    assert_revalidation_rejected(&missing_accepted, "missing accepted State Run output");

    let mut missing_occurrence = owner();
    missing_occurrence.run = run_with_outputs(&missing_occurrence, vec![accepted_identity.clone()]);
    assert_revalidation_rejected(&missing_occurrence, "missing occurrence Run output");

    let mut additional = owner();
    additional.run = run_with_outputs(
        &additional,
        vec![
            accepted_identity.clone(),
            occurrence_identity,
            ArtifactDigest::from_sha256([0x5a; 32]).to_string(),
        ],
    );
    assert_revalidation_rejected(&additional, "additional Run output");

    let mut unlinked = owner();
    unlinked.run = run_with_outputs(
        &unlinked,
        vec![
            accepted_identity,
            ArtifactDigest::from_sha256([0xc3; 32]).to_string(),
        ],
    );
    assert_revalidation_rejected(&unlinked, "foreign occurrence Run linkage");
}

#[test]
fn cancellation_is_observed_at_every_frozen_safe_boundary() {
    let checkpoints = [
        CancellationCheckpoint::BeforeSessionAdmission,
        CancellationCheckpoint::BeforeHelloFrame,
        CancellationCheckpoint::AfterHelloFrame,
        CancellationCheckpoint::BeforeBindFrame,
        CancellationCheckpoint::AfterBindFrame,
        CancellationCheckpoint::BeforeBoundFrame,
        CancellationCheckpoint::AfterBoundFrame,
        CancellationCheckpoint::BeforeEvaluateFrame,
        CancellationCheckpoint::AfterEvaluateFrame,
        CancellationCheckpoint::BeforeDisplacementBulkFrame,
        CancellationCheckpoint::AfterDisplacementBulkFrame,
        CancellationCheckpoint::BeforeVelocityBulkFrame,
        CancellationCheckpoint::AfterVelocityBulkFrame,
        CancellationCheckpoint::BeforeCandidateFrame,
        CancellationCheckpoint::AfterCandidateFrame,
        CancellationCheckpoint::BeforeCandidateBulkFrame,
        CancellationCheckpoint::AfterCandidateBulkFrame,
        CancellationCheckpoint::BeforeReportFrame,
        CancellationCheckpoint::AfterReportFrame,
        CancellationCheckpoint::BeforeProjection,
        CancellationCheckpoint::AfterProjection,
        CancellationCheckpoint::BeforeStructuralSolve,
        CancellationCheckpoint::AfterStructuralSolve,
        CancellationCheckpoint::BeforeComposition,
        CancellationCheckpoint::AfterComposition,
        CancellationCheckpoint::BeforeCloseFrame,
        CancellationCheckpoint::AfterCloseFrame,
        CancellationCheckpoint::BeforeClosedFrame,
        CancellationCheckpoint::AfterClosedFrame,
        CancellationCheckpoint::BeforeFinalEofWait,
        CancellationCheckpoint::AfterFinalEof,
        CancellationCheckpoint::BeforeProcessExitWait,
        CancellationCheckpoint::AfterProcessExit,
        CancellationCheckpoint::BeforePublication,
    ];
    for checkpoint in checkpoints {
        let cancellation = AtomicBool::new(false);
        let child = hostile_child();
        let pid = child.id();
        let error = match solve_with_injected_cancellation_checkpoint(
            &document(),
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
            child,
            &cancellation,
            checkpoint,
        ) {
            Err(error) => error,
            Ok(_) => panic!("{checkpoint:?} published a partial or complete owner"),
        };
        assert_eq!(
            error.code(),
            eqiora_core::diagnostic::codes::EXECUTION_CANCELLED,
            "{checkpoint:?} did not retain cancellation precedence"
        );
        assert!(
            error.message().to_ascii_lowercase().contains("cancel"),
            "{checkpoint:?} returned a non-cancellation explanation: {error}"
        );
        assert!(cancellation.load(std::sync::atomic::Ordering::Acquire));
        assert_reaped(pid, checkpoint);
    }
}
