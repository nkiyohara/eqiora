use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use eqiora::api::{
    ModelDocument, PrescribedDynamicSolidExternalProviderStateRun3d,
    PrescribedDynamicSolidStateRun3d,
};
use eqiora::artifact::{
    ArtifactDigest, JsonDecoderLimits, PrescribedDynamicSolidProviderOccurrenceEnvelopeV1,
};
use eqiora::assembly::{
    AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork, REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora::solver::{
    LinearProblem, LinearSolution, LinearSolverBackend, REFERENCE_LINEAR_SOLVER,
    ReplicatedLinearExecution, SolverCapabilities, SolverPlan, SolverProvider,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const DIRECT_SOURCE: &str = include_str!(
    "../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/models/direct.eqi"
);
const EXPECTED_OCCURRENCE: &[u8] = include_bytes!(
    "../../../verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/provider-occurrence.json"
);
const EXPECTED_RUN: &[u8] = include_bytes!(
    "../../../verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/run.json"
);
const EXPECTED_CANDIDATE: &[u8] = include_bytes!(
    "../../../verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/candidate.bin"
);
const EXPECTED_TRANSCRIPT: &[u8] = include_bytes!(
    "../../../verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/transcript.bin"
);
const LINEAGE_KEY: &str = "model_sha256";
const TRANSCRIPT_DOMAIN: &str = "eqiora.prescribed-dynamic-solid-provider-transcript/v1";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document() -> ModelDocument {
    ModelDocument::compile("external-provider-oracle.eqi", DIRECT_SOURCE)
        .expect("the accepted direct source compiles")
}

fn positive_child(launch_salt: &str, working_directory: &Path) -> Child {
    let script = repository_root().join("examples/python/prescribed_dynamic_solid_provider.py");
    let mut command = Command::new("uv");
    command
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
        .current_dir(working_directory)
        .env("EQIORA_ORACLE_LAUNCH_SALT", launch_salt)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .spawn()
        .expect("uv must provision the exact positive provider profile")
}

fn hostile_child(mode: &str) -> Child {
    let script = repository_root().join(
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/mutants/hostile_provider.py",
    );
    let mut command = Command::new("uv");
    command
        .args(["run", "--isolated", "--python", "3.12", "python"])
        .arg(script)
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
        .spawn()
        .unwrap_or_else(|error| panic!("cannot launch hostile mode {mode}: {error}"))
}

fn solve(
    child: Child,
    cancellation: &AtomicBool,
) -> Result<PrescribedDynamicSolidExternalProviderStateRun3d, eqiora::Diagnostic> {
    PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
        &document(),
        &REFERENCE_ASSEMBLY_BACKEND,
        &REFERENCE_LINEAR_SOLVER,
        child,
        cancellation,
    )
}

fn compact_fixture(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_suffix(b"\n")
        .expect("canonical JSON fixtures carry one repository newline")
}

fn fixture_value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("the precommitted fixture is valid JSON")
}

fn fixture_digest(value: &Value, pointer: &str) -> ArtifactDigest {
    ArtifactDigest::from_hex(
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("missing digest at {pointer}"))
            .to_owned(),
    )
    .expect("the precommitted digest is canonical")
}

fn transcript_digest(bytes: &[u8]) -> ArtifactDigest {
    let mut hasher = Sha256::new();
    hasher.update(TRANSCRIPT_DOMAIN.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    ArtifactDigest::from_sha256(hasher.finalize().into())
}

fn assert_reaped(pid: u32, mode: &str) {
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "hostile mode {mode} returned before consuming and waiting for its child"
    );
    #[cfg(not(target_os = "linux"))]
    let _ = (pid, mode);
}

fn assert_hostile_rejected(mode: &str) {
    let child = hostile_child(mode);
    let pid = child.id();
    assert!(
        solve(child, &AtomicBool::new(false)).is_err(),
        "hostile mode {mode} unexpectedly published an owner"
    );
    assert_reaped(pid, mode);
}

#[allow(dead_code)]
fn compile_frozen_public_surface(
    document: &ModelDocument,
    child: Child,
    cancellation: &AtomicBool,
) -> Result<PrescribedDynamicSolidExternalProviderStateRun3d, eqiora::Diagnostic> {
    let owner =
        PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
            document,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
            child,
            cancellation,
        )?;
    let _ = (
        owner.model(),
        owner.geometry(),
        owner.correspondence(),
        owner.mesh(),
        owner.realization(),
        owner.accepted(),
        owner.prior_state(),
        owner.accepted_state(),
        owner.provider_occurrence(),
        owner.run(),
    );
    owner.revalidate()?;
    Ok(owner)
}

#[test]
fn exact_positive_sessions_publish_the_frozen_candidate_occurrence_and_run() {
    let root = repository_root();
    let case = root.join("verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d");
    let first = solve(
        positive_child("first-launch", &root),
        &AtomicBool::new(false),
    )
    .expect("the first exact provider session is admitted");
    let second = solve(
        positive_child("different-launch", &case),
        &AtomicBool::new(false),
    )
    .expect("launch metadata cannot alter the second admitted session");

    for owner in [&first, &second] {
        owner.revalidate().expect("the complete owner revalidates");
        assert_eq!(
            owner.provider_occurrence().canonical_json().unwrap(),
            compact_fixture(EXPECTED_OCCURRENCE)
        );
        assert_eq!(
            owner.run().canonical_json().unwrap(),
            compact_fixture(EXPECTED_RUN)
        );
        assert_eq!(owner.accepted().generation(), 1);
        for (position, vertex) in [1, 3, 5, 7].into_iter().enumerate() {
            let value = owner.accepted().displacement()[vertex].1;
            let start = position * 24;
            let expected = [
                u64::from_le_bytes(EXPECTED_CANDIDATE[start..start + 8].try_into().unwrap()),
                u64::from_le_bytes(
                    EXPECTED_CANDIDATE[start + 8..start + 16]
                        .try_into()
                        .unwrap(),
                ),
                u64::from_le_bytes(
                    EXPECTED_CANDIDATE[start + 16..start + 24]
                        .try_into()
                        .unwrap(),
                ),
            ];
            assert_eq!(expected, [0x3f8eb851eb851eb8, 0, 0]);
            assert_eq!(value.map(f64::to_bits), expected);
        }
        owner
            .provider_occurrence()
            .validate_against(
                owner.model(),
                owner.realization(),
                owner.geometry(),
                owner.correspondence(),
                owner.mesh(),
                owner.prior_state(),
                owner.accepted_state(),
            )
            .expect("the exact occurrence replays all retained resources and States");
    }

    assert_eq!(
        first.accepted_state().canonical_json().unwrap(),
        second.accepted_state().canonical_json().unwrap()
    );
    assert_eq!(
        first.provider_occurrence().canonical_json().unwrap(),
        second.provider_occurrence().canonical_json().unwrap()
    );
    assert_eq!(
        first.run().canonical_json().unwrap(),
        second.run().canonical_json().unwrap()
    );

    let occurrence = fixture_value(EXPECTED_OCCURRENCE);
    assert_eq!(
        first.provider_occurrence().model_artifact(),
        fixture_digest(&occurrence, &format!("/{LINEAGE_KEY}"))
    );
    assert_eq!(first.provider_occurrence().semantic_revision(), 1);
    assert_eq!(first.provider_occurrence().contract_generation(), 1);
    assert_eq!(
        first.provider_occurrence().provider_id(),
        "eqiora.python.prescribed-dynamic-solid-affine"
    );
    assert_eq!(first.provider_occurrence().provider_release(), "1.0.0");
    assert_eq!(
        first.provider_occurrence().provider_dependencies(),
        &BTreeMap::from([
            ("cpython".to_owned(), "3.12".to_owned()),
            ("numpy".to_owned(), "2.1.0".to_owned()),
        ])
    );
    assert_eq!(
        first.provider_occurrence().transcript_identity(),
        transcript_digest(EXPECTED_TRANSCRIPT)
    );
    assert_eq!(
        first.provider_occurrence().candidate_identity(),
        fixture_digest(&occurrence, "/candidate/candidate_sha256")
    );
    assert_eq!(
        first.provider_occurrence().binding_identity(),
        fixture_digest(&occurrence, "/request/binding_sha256")
    );
    assert_eq!(
        first.provider_occurrence().request_identity(),
        fixture_digest(&occurrence, "/request/request_sha256")
    );

    let occurrence_identity = first.provider_occurrence().digest().unwrap();
    let accepted_identity = first.accepted_state().digest().unwrap();
    let mut expected_outputs = vec![accepted_identity.clone(), occurrence_identity.clone()];
    expected_outputs.sort();
    assert_eq!(first.run().outputs(), expected_outputs);

    let direct = PrescribedDynamicSolidStateRun3d::solve_reference(
        &document(),
        &REFERENCE_ASSEMBLY_BACKEND,
        &REFERENCE_LINEAR_SOLVER,
    )
    .expect("the unchanged direct owner remains valid");
    assert_eq!(direct.run().outputs(), vec![accepted_identity]);
    assert_ne!(
        direct.run().canonical_json().unwrap(),
        first.run().canonical_json().unwrap()
    );
}

#[test]
fn transcript_fixture_has_the_exact_frame_sequence_and_candidate_payload() {
    let directions = [1_u8, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1];
    let kinds = [1_u8, 1, 1, 1, 2, 2, 1, 2, 1, 1, 1];
    let mut offset = 0;
    let mut aggregate_bulk = 0;
    for (index, (&direction, &kind)) in directions.iter().zip(&kinds).enumerate() {
        assert_eq!(EXPECTED_TRANSCRIPT[offset], direction);
        offset += 1;
        let prefix = &EXPECTED_TRANSCRIPT[offset..offset + 16];
        assert_eq!(&prefix[..4], b"EQP1");
        assert_eq!(prefix[4], kind);
        assert_eq!(&prefix[5..8], &[0, 0, 0]);
        let length = u64::from_le_bytes(prefix[8..16].try_into().unwrap()) as usize;
        offset += 16;
        let payload = &EXPECTED_TRANSCRIPT[offset..offset + length];
        offset += length;
        if kind == 1 {
            let value: Value = serde_json::from_slice(payload).unwrap();
            assert_eq!(serde_json::to_vec(&value).unwrap(), payload);
        } else {
            assert_eq!(length, 96);
            aggregate_bulk += length;
            if index == 7 {
                assert_eq!(payload, EXPECTED_CANDIDATE);
            }
        }
    }
    assert_eq!(offset, EXPECTED_TRANSCRIPT.len());
    assert_eq!(aggregate_bulk, 288);
}

#[test]
fn occurrence_decoder_is_closed_canonical_and_intrinsically_bounded() {
    let bytes = compact_fixture(EXPECTED_OCCURRENCE);
    let decoded = PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
        bytes,
        JsonDecoderLimits::default(),
    )
    .expect("the detached frozen occurrence is locally canonical");
    assert_eq!(decoded.canonical_json().unwrap(), bytes);

    for raw in [b"{".as_slice(), b"[]", b" null"] {
        assert!(
            PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
                raw,
                JsonDecoderLimits::default(),
            )
            .is_err()
        );
    }
    let reordered = move_first_member_to_end(bytes);
    assert!(
        PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
            &reordered,
            JsonDecoderLimits::default(),
        )
        .is_err()
    );

    let mut mutations = vec![
        replace_first(
            bytes,
            br#""encoding":"eqiora.canonical-json/v1""#,
            br#""encoding":"json""#,
        ),
        replace_first(bytes, br#""generation":1"#, br#""generation":2"#),
        replace_first(
            bytes,
            br#""statefulness":"stateless""#,
            br#""statefulness":"stateful""#,
        ),
        replace_first(bytes, br#""model_time_s":0.0"#, br#""model_time_s":-0.0"#),
        replace_first(
            bytes,
            br#""vertex_indices":[1,3,5,7]"#,
            br#""vertex_indices":[1,5,3,7]"#,
        ),
        replace_first(bytes, br#""unit":"m""#, br#""unit":"mm""#),
        replace_first(
            bytes,
            br#""convention":"total-reference-configuration""#,
            br#""convention":"increment""#,
        ),
        replace_first(bytes, br#""frame_count":11"#, br#""frame_count":10"#),
        replace_first(
            bytes,
            br#""admission":{"status":"accepted""#,
            br#""admission":{"status":"rejected""#,
        ),
        replace_first(
            bytes,
            br#"{"name":"numpy","release":"2.1.0"}"#,
            br#"{"name":"cpython","release":"3.12"}"#,
        ),
    ];
    let mut unknown = bytes[..bytes.len() - 1].to_vec();
    unknown.extend_from_slice(br#","unexpected":true}"#);
    mutations.push(unknown);

    for mutant in mutations {
        assert!(
            PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::from_json(
                &mutant,
                JsonDecoderLimits::default(),
            )
            .is_err(),
            "locally invalid occurrence mutation was accepted"
        );
    }
}

#[test]
fn prefix_control_policy_and_dependency_mutants_publish_no_owner() {
    for mode in [
        "wrong-magic",
        "wrong-frame-kind",
        "nonzero-reserved",
        "big-endian-length",
        "nonportable-length",
        "truncated-prefix",
        "truncated-control",
        "declared-length-mismatch",
        "control-budget-breach",
        "malformed-json",
        "malformed-utf8",
        "duplicate-field",
        "missing-field",
        "unknown-field",
        "reordered-field",
        "whitespace-drift",
        "number-spelling-drift",
        "excessive-nesting",
        "dependency-omission",
        "dependency-duplication",
        "dependency-reordering",
        "wrong-protocol",
        "wrong-contract",
        "wrong-determinism",
        "wrong-statefulness",
        "wrong-scalar",
        "wrong-target",
        "wrong-association",
        "wrong-layout",
        "wrong-input-count",
        "wrong-output-count",
        "wrong-coefficient-limit",
        "wrong-aggregate-limit",
        "wrong-provider-id",
        "wrong-provider-release",
        "wrong-python-policy",
        "wrong-numpy-release",
    ] {
        assert_hostile_rejected(mode);
    }
}

#[test]
fn state_candidate_identity_and_terminal_mutants_publish_no_owner() {
    for mode in [
        "bound-before-bind",
        "duplicate-bound",
        "candidate-before-evaluate",
        "report-before-candidate-bulk",
        "duplicate-report",
        "extra-response-before-close",
        "duplicate-closed",
        "response-after-close",
        "extra-bytes-after-closed",
        "error-bind",
        "error-evaluate",
        "error-close",
        "exit-before-hello",
        "exit-before-bound",
        "exit-before-candidate",
        "exit-before-report",
        "exit-before-closed",
        "wrong-binding-identity",
        "wrong-request-identity",
        "wrong-candidate-identity",
        "wrong-report-request",
        "wrong-report-candidate",
        "wrong-success-code",
        "wrong-success-message",
        "truncated-bulk",
        "bulk-length-mismatch",
        "bulk-budget-breach",
        "wrong-bulk-kind",
        "wrong-endian-binary64",
        "negative-zero",
        "nan",
        "infinity",
        "candidate-as-increment",
        "stale-prior-velocity",
        "changed-time-step",
        "ignored-input",
        "wrong-candidate-bits",
        "stderr-overflow",
        "nonzero-exit",
    ] {
        assert_hostile_rejected(mode);
    }
}

#[test]
fn every_awaited_boundary_has_a_deadline_and_poisoned_child() {
    for mode in [
        "timeout-before-hello",
        "timeout-before-bound",
        "timeout-before-candidate",
        "timeout-before-report",
        "timeout-before-closed",
        "dirty-eof-delay",
    ] {
        assert_hostile_rejected(mode);
    }
}

#[test]
fn cancellation_before_admission_and_while_waiting_returns_no_owner() {
    let pre_cancelled = AtomicBool::new(true);
    let child = hostile_child("cancel-before-hello");
    let pid = child.id();
    let error = match solve(child, &pre_cancelled) {
        Err(error) => error,
        Ok(_) => panic!("pre-session cancellation unexpectedly published an owner"),
    };
    assert_eq!(
        error.code(),
        eqiora::diagnostic::codes::EXECUTION_CANCELLED,
        "pre-session cancellation precedes provider admission"
    );
    assert_reaped(pid, "pre-cancelled");

    for mode in [
        "timeout-before-hello",
        "timeout-before-bound",
        "timeout-before-candidate",
        "timeout-before-report",
        "timeout-before-closed",
        "dirty-eof-delay",
    ] {
        let cancellation = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&cancellation);
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            setter.store(true, Ordering::Release);
        });
        let child = hostile_child(mode);
        let pid = child.id();
        assert!(solve(child, &cancellation).is_err());
        trigger.join().unwrap();
        assert_reaped(pid, mode);
    }

    let cancellation = AtomicBool::new(false);
    let child = hostile_child("honest");
    let pid = child.id();
    assert!(
        PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
            &document(),
            &CancelDuringStructuralSolve(&cancellation),
            &REFERENCE_LINEAR_SOLVER,
            child,
            &cancellation,
        )
        .is_err()
    );
    assert!(cancellation.load(Ordering::Acquire));
    assert_reaped(pid, "cancel-during-structural-solve");
}

#[test]
fn missing_pipe_ends_and_backend_failures_are_failure_atomic() {
    let script = repository_root().join(
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/mutants/hostile_provider.py",
    );
    for missing_stdout in [false, true] {
        let mut command = Command::new("uv");
        command
            .args(["run", "--isolated", "--python", "3.12", "python"])
            .arg(&script)
            .arg("honest")
            .stdin(if missing_stdout {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if missing_stdout {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .stderr(Stdio::piped());
        let child = command.spawn().unwrap();
        let pid = child.id();
        assert!(solve(child, &AtomicBool::new(false)).is_err());
        assert_reaped(pid, "missing-pipe");
    }

    let child = hostile_child("honest");
    let pid = child.id();
    assert!(
        PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
            &document(),
            &RejectAssembly,
            &REFERENCE_LINEAR_SOLVER,
            child,
            &AtomicBool::new(false),
        )
        .is_err()
    );
    assert_reaped(pid, "assembly-failure");

    let child = hostile_child("honest");
    let pid = child.id();
    assert!(
        PrescribedDynamicSolidExternalProviderStateRun3d::solve_reference_with_connected_subprocess(
            &document(),
            &REFERENCE_ASSEMBLY_BACKEND,
            &RejectSolver,
            child,
            &AtomicBool::new(false),
        )
        .is_err()
    );
    assert_reaped(pid, "solver-failure");
}

#[derive(Debug)]
struct RejectAssembly;

impl AssemblyBackend for RejectAssembly {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, eqiora::Diagnostic> {
        Err(eqiora::Diagnostic::error(
            eqiora::diagnostic::codes::ASSEMBLY_FAILED,
            "oracle-injected external-provider assembly failure",
        ))
    }
}

#[derive(Debug)]
struct CancelDuringStructuralSolve<'a>(&'a AtomicBool);

impl AssemblyBackend for CancelDuringStructuralSolve<'_> {
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, eqiora::Diagnostic> {
        let result = REFERENCE_ASSEMBLY_BACKEND.assemble(plan, work)?;
        self.0.store(true, Ordering::Release);
        Ok(result)
    }
}

#[derive(Debug)]
struct RejectSolver;

impl LinearSolverBackend for RejectSolver {
    fn provider(&self) -> SolverProvider {
        REFERENCE_LINEAR_SOLVER.provider()
    }

    fn capabilities(&self) -> SolverCapabilities {
        REFERENCE_LINEAR_SOLVER.capabilities()
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, eqiora::Diagnostic> {
        Err(eqiora::Diagnostic::error(
            eqiora::diagnostic::codes::NUMERICAL_SOLVE_FAILED,
            "oracle-injected external-provider solver failure",
        ))
    }
}

fn replace_first(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap_or_else(|| {
            panic!(
                "mutation source is absent: {}",
                String::from_utf8_lossy(from)
            )
        });
    let mut output = Vec::with_capacity(bytes.len() - from.len() + to.len());
    output.extend_from_slice(&bytes[..start]);
    output.extend_from_slice(to);
    output.extend_from_slice(&bytes[start + from.len()..]);
    output
}

fn move_first_member_to_end(bytes: &[u8]) -> Vec<u8> {
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut separator = None;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b'{' || byte == b'[' {
            depth += 1;
        } else if byte == b'}' || byte == b']' {
            depth -= 1;
        } else if byte == b',' && depth == 1 {
            separator = Some(index);
            break;
        }
    }
    let separator = separator.expect("the occurrence has multiple top-level members");
    let mut reordered = Vec::with_capacity(bytes.len());
    reordered.push(b'{');
    reordered.extend_from_slice(&bytes[separator + 1..bytes.len() - 1]);
    reordered.push(b',');
    reordered.extend_from_slice(&bytes[1..separator]);
    reordered.push(b'}');
    reordered
}
