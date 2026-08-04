use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;

use eqiora_artifact::PrescribedDynamicSolidProviderOccurrenceEnvelopeV1;
use eqiora_assembly::REFERENCE_ASSEMBLY_BACKEND;
use eqiora_solver::REFERENCE_LINEAR_SOLVER;

use crate::ModelDocument;

use super::super::{
    PrescribedDynamicSolidExternalProviderStateRun3d, PrescribedDynamicSolidStateRun3d,
};

const DIRECT_SOURCE: &str = include_str!(
    "../../../../../verify/artifacts/prescribed-dynamic-solid-state-run-3d/models/direct.eqi"
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

#[test]
fn revalidate_rejects_accepted_state_candidate_and_transcript_substitution() {
    let mut wrong_state = owner();
    wrong_state.accepted_state = wrong_state.prior_state.clone();
    assert!(wrong_state.revalidate().is_err());

    let mut wrong_candidate = owner();
    wrong_candidate.candidate_bytes[0] ^= 1;
    assert!(wrong_candidate.revalidate().is_err());

    let mut wrong_transcript = owner();
    let last = wrong_transcript.transcript_bytes.len() - 1;
    wrong_transcript.transcript_bytes[last] ^= 1;
    assert!(wrong_transcript.revalidate().is_err());
}

#[test]
fn revalidate_rejects_locally_valid_occurrence_and_run_cross_substitution() {
    let mut wrong_occurrence = owner();
    let retained = wrong_occurrence.provider_occurrence.clone();
    let dependencies = BTreeMap::from([
        ("cpython".to_owned(), "3.12".to_owned()),
        ("numpy".to_owned(), "2.1.0".to_owned()),
    ]);
    wrong_occurrence.provider_occurrence = PrescribedDynamicSolidProviderOccurrenceEnvelopeV1::new(
        &wrong_occurrence.realization,
        &wrong_occurrence.prior_state,
        &wrong_occurrence.accepted_state,
        "eqiora.python.foreign-affine",
        "1.0.0",
        &dependencies,
        "provider.success",
        "affine predictor completed",
        retained.binding_identity(),
        retained.displacement_input_identity(),
        retained.velocity_input_identity(),
        retained.request_identity(),
        retained.candidate_identity(),
        retained.transcript_identity(),
    )
    .expect("a different well-formed provider occurrence is locally valid");
    assert_ne!(
        wrong_occurrence.provider_occurrence.digest().unwrap(),
        retained.digest().unwrap()
    );
    assert!(wrong_occurrence.revalidate().is_err());

    let mut singleton_run = owner();
    let direct = PrescribedDynamicSolidStateRun3d::solve_reference(
        &document(),
        &REFERENCE_ASSEMBLY_BACKEND,
        &REFERENCE_LINEAR_SOLVER,
    )
    .unwrap();
    singleton_run.run = direct.run().clone();
    assert!(singleton_run.revalidate().is_err());
}
