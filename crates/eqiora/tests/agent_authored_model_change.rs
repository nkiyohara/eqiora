use std::num::NonZeroUsize;

use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
    ScalarEllipticRunResult,
};
use eqiora::artifact::{ArtifactDigest, DecoderLimits, ExecutionProvenanceV1, RunManifestV2};
use eqiora::control::{CompileOutcomeV1, CompileRequestV1, execute_compile_v1};
use eqiora::diagnostic::codes;
use eqiora::realization::RealizationRevision;
use serde::Deserialize;

const SOURCE: &str =
    include_str!("../../../verify/interfaces/agent-authored-model-change/models/poisson.eqi");
const OBJECTIVE: &[u8] =
    include_bytes!("../../../verify/interfaces/agent-authored-model-change/expected/oracle.json");
const ACCEPTED_PROPOSAL: &[u8] = include_bytes!(
    "../../../verify/interfaces/agent-authored-model-change/proposals/accepted.json"
);
const REJECTED_PROPOSAL: &[u8] = include_bytes!(
    "../../../verify/interfaces/agent-authored-model-change/proposals/rejected.json"
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificObjective {
    schema: String,
    target_alias: String,
    expected_field_values: Vec<f64>,
    absolute_tolerance: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedEdit {
    proposed_value_si: f64,
}

fn objective() -> ScientificObjective {
    let objective: ScientificObjective = serde_json::from_slice(OBJECTIVE).unwrap();
    assert_eq!(
        objective.schema,
        "eqiora.verify.agent-authored-model-change-oracle/v1"
    );
    assert!(objective.absolute_tolerance.is_finite());
    assert!(objective.absolute_tolerance >= 0.0);
    objective
}

fn proposal(bytes: &[u8]) -> ProposedEdit {
    let proposal: ProposedEdit = serde_json::from_slice(bytes).unwrap();
    assert!(proposal.proposed_value_si.is_finite());
    proposal
}

fn compile_base() -> ModelDocument {
    let request =
        CompileRequestV1::new_current("verify.agent-authored-model-change", "poisson.eqi", SOURCE)
            .unwrap();
    let (response, document) = execute_compile_v1(&request).into_parts();
    assert!(matches!(
        response.outcome(),
        CompileOutcomeV1::Accepted { .. }
    ));
    document.expect("accepted compilation must expose one immutable Model")
}

fn intent(workers: usize) -> ScalarEllipticIntent {
    ScalarEllipticIntent::new(
        RealizationRevision::new(1),
        ScalarEllipticMethod::FiniteElement,
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(workers).unwrap(),
    )
}

fn execute(
    document: &ModelDocument,
    workers: usize,
) -> Result<ScalarEllipticRunResult, Vec<eqiora::Diagnostic>> {
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let plan = document.preview_scalar_elliptic_run(intent(workers), environment)?;
    document.run_scalar_elliptic_plan(plan, environment)
}

fn verify_scientific_objective(
    result: &ScalarEllipticRunResult,
    objective: &ScientificObjective,
) -> Result<(), String> {
    let observed = result.field_values();
    if observed.len() != objective.expected_field_values.len() {
        return Err(format!(
            "oracle expected {} values, execution produced {}",
            objective.expected_field_values.len(),
            observed.len()
        ));
    }
    for (index, (&actual, &expected)) in observed
        .iter()
        .zip(&objective.expected_field_values)
        .enumerate()
    {
        if (actual - expected).abs() > objective.absolute_tolerance {
            return Err(format!(
                "oracle mismatch at field value {index}: expected {expected}, observed {actual}"
            ));
        }
    }
    Ok(())
}

#[test]
fn offline_agent_proposal_uses_the_ordinary_exact_edit_and_execution_path() {
    let base = compile_base();
    let objective = objective();
    let proposal = proposal(ACCEPTED_PROPOSAL);
    let base_bytes = base.canonical_json().unwrap();
    let base_digest = base.digest().unwrap();
    let base_revision = base.program().revision();
    let target = base.aliases()[&objective.target_alias];

    let agent_plan = base
        .preview_value_edit(target, proposal.proposed_value_si)
        .unwrap();
    let ordinary_client_plan = base
        .preview_value_edit(target, proposal.proposed_value_si)
        .unwrap();

    assert_eq!(agent_plan, ordinary_client_plan);
    assert_eq!(
        agent_plan.transaction_json().unwrap(),
        ordinary_client_plan.transaction_json().unwrap()
    );
    assert_eq!(agent_plan.base_digest(), base_digest);
    assert_eq!(agent_plan.base_revision(), base_revision);
    assert_eq!(agent_plan.before().value(), 1.0);
    assert_eq!(agent_plan.after().value(), 2.0);
    assert!(
        String::from_utf8(agent_plan.transaction_json().unwrap())
            .unwrap()
            .contains("eqiora.model-transaction-envelope/v6")
    );
    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
    assert_eq!(base.program().revision(), base_revision);

    let agent_change = base.commit_value_edit(agent_plan).unwrap();
    let ordinary_change = base.commit_value_edit(ordinary_client_plan).unwrap();
    assert_eq!(
        agent_change.document().canonical_json().unwrap(),
        ordinary_change.document().canonical_json().unwrap()
    );
    assert_eq!(
        agent_change.result_digest(),
        ordinary_change.result_digest()
    );
    assert_ne!(agent_change.result_digest(), base_digest);
    assert_eq!(agent_change.result_revision().0, base_revision.0 + 1);
    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
    assert_eq!(base.program().revision(), base_revision);
    assert_eq!(base.program().value(target).unwrap().value(), 1.0);

    let accepted = execute(agent_change.document(), 1).unwrap();
    verify_scientific_objective(&accepted, &objective).unwrap();
    assert!(accepted.balance().relative_imbalance() < 1.0e-12);
    assert!(accepted.solve().true_residual_norm() <= accepted.solve().residual_target());

    let child_bytes = agent_change.document().canonical_json().unwrap();
    let replayed_child = agent_change
        .document()
        .exact_codec()
        .replay(&child_bytes)
        .unwrap();
    assert_eq!(replayed_child.canonical_json().unwrap(), child_bytes);
    assert_eq!(
        replayed_child.digest().unwrap(),
        agent_change.result_digest()
    );

    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let replayed_plan = replayed_child
        .preview_scalar_elliptic_run(intent(1), environment)
        .unwrap();
    assert_eq!(
        replayed_plan.artifact().canonical_json().unwrap(),
        accepted.plan().artifact().canonical_json().unwrap()
    );
    assert_eq!(
        replayed_plan.artifact().digest().unwrap(),
        accepted.plan().artifact().digest().unwrap()
    );
    let replayed_result = replayed_child
        .run_scalar_elliptic_plan(replayed_plan, environment)
        .unwrap();
    assert_eq!(
        replayed_result.field_values(),
        accepted.field_values(),
        "exact replay must reproduce the complete accepted data plane"
    );
    assert_eq!(
        replayed_result.receipt().output(),
        accepted.receipt().output()
    );
    assert_eq!(
        replayed_result.run_manifest().canonical_json().unwrap(),
        accepted.run_manifest().canonical_json().unwrap()
    );

    let run_bytes = accepted.run_manifest().canonical_json().unwrap();
    let replayed_run = accepted
        .plan()
        .replay_run_manifest(&run_bytes, DecoderLimits::default())
        .unwrap();
    assert_eq!(replayed_run.canonical_json().unwrap(), run_bytes);
    assert_eq!(
        replayed_run.digest().unwrap(),
        accepted.run_manifest().digest().unwrap()
    );
}

#[test]
fn independent_evidence_rejects_a_valid_but_scientifically_wrong_proposal() {
    let base = compile_base();
    let objective = objective();
    let proposal = proposal(REJECTED_PROPOSAL);
    let base_bytes = base.canonical_json().unwrap();
    let base_digest = base.digest().unwrap();
    let base_revision = base.program().revision();
    let target = base.aliases()[&objective.target_alias];

    let plan = base
        .preview_value_edit(target, proposal.proposed_value_si)
        .unwrap();
    let candidate = base.commit_value_edit(plan).unwrap();
    let result = execute(candidate.document(), 1).unwrap();

    assert!(result.balance().relative_imbalance() < 1.0e-12);
    assert!(result.solve().true_residual_norm() <= result.solve().residual_target());
    assert!(
        verify_scientific_objective(&result, &objective).is_err(),
        "solver acceptance cannot substitute for an independent scientific oracle"
    );
    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
    assert_eq!(base.program().revision(), base_revision);
    assert_eq!(base.program().value(target).unwrap().value(), 1.0);
}

#[test]
fn stale_foreign_forged_and_unsupported_inputs_fail_closed() {
    let base = compile_base();
    let objective = objective();
    let target = base.aliases()[&objective.target_alias];
    let field = base.aliases()["potential"];
    let relation = base.aliases()["balance"];
    let base_bytes = base.canonical_json().unwrap();
    let base_digest = base.digest().unwrap();
    let base_revision = base.program().revision();

    let stale = base.preview_value_edit(target, 2.0).unwrap();
    let child = base
        .commit_value_edit(base.preview_value_edit(target, 3.0).unwrap())
        .unwrap();
    assert_eq!(
        child.document().commit_value_edit(stale).unwrap_err()[0].code(),
        codes::PRECONDITION_FAILED
    );

    let left = base
        .commit_value_edit(base.preview_value_edit(target, 2.0).unwrap())
        .unwrap();
    let right = base
        .commit_value_edit(base.preview_value_edit(target, 3.0).unwrap())
        .unwrap();
    let left_plan = left.document().preview_value_edit(field, 1.0).unwrap();
    let right_plan = right.document().preview_value_edit(field, 1.0).unwrap();
    assert_eq!(left_plan.base_revision(), right_plan.base_revision());
    assert_eq!(
        left_plan.transaction_digest(),
        right_plan.transaction_digest()
    );
    assert_ne!(left_plan.base_digest(), right_plan.base_digest());
    assert_eq!(
        left.document().commit_value_edit(right_plan).unwrap_err()[0].code(),
        codes::PRECONDITION_FAILED
    );

    assert_eq!(
        left.document()
            .preview_scalar_elliptic_run(
                intent(2),
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .unwrap_err()[0]
            .code(),
        codes::INVALID_REALIZATION
    );

    let accepted = execute(left.document(), 1).unwrap();
    assert_eq!(
        accepted
            .plan()
            .artifact()
            .validate_model_artifact(&base.artifact_reference().unwrap())
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    let actual = accepted.run_manifest().execution();
    let forged_execution = ExecutionProvenanceV1::new(
        "example.forged-agent-adapter",
        actual.adapter_version(),
        actual.solver_backend(),
        actual.solver_backend_version(),
        actual.topology().unwrap(),
        actual.reduction(),
    )
    .unwrap();
    let forged_run = RunManifestV2::new(accepted.plan().artifact(), forged_execution).unwrap();
    assert_eq!(
        accepted
            .plan()
            .validate_run_manifest(&forged_run)
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    let forged_output = accepted
        .run_manifest()
        .clone()
        .with_output(ArtifactDigest::from_hex("00".repeat(32)).unwrap());
    assert_eq!(
        accepted
            .plan()
            .validate_run_manifest(&forged_output)
            .unwrap_err()
            .code(),
        codes::INVALID_ARTIFACT
    );

    assert_eq!(
        base.preview_value_edit(target, 1.0).unwrap_err().code(),
        codes::INVALID_OPERATION
    );
    assert_eq!(
        base.preview_value_edit(target, f64::NAN)
            .unwrap_err()
            .code(),
        codes::INVALID_OPERATION
    );
    assert_eq!(
        base.preview_value_edit(relation, 2.0).unwrap_err().code(),
        codes::INVALID_OPERATION
    );
    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
    assert_eq!(base.program().revision(), base_revision);
}
