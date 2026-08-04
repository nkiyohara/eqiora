use super::test_support::{
    CandidateFailurePoint, FactorStateMutant, PhaseScenario, ValidationMutant,
    candidate_failure_observation, factor_state_rejection_observation, phase_trace,
    retention_trace, validation_mutant_observation,
};

const P0_PHASES: &[&str] = &[
    "preflight-attempt",
    "preflight-success",
    "symbolic-attempt",
    "symbolic-success",
    "numeric-attempt",
    "numeric-success",
    "candidate-factor-solve",
    "solver-acceptance-attempt",
    "solver-acceptance-success",
    "execution-acceptance-attempt",
    "execution-acceptance-success",
    "state-commit",
];
const P1_PHASES: &[&str] = &[
    "preflight-attempt",
    "preflight-success",
    "retained-factor-solve",
    "solver-acceptance-attempt",
    "solver-acceptance-success",
    "execution-acceptance-attempt",
    "execution-acceptance-success",
    "state-commit",
];
const P2_PHASES: &[&str] = &[
    "preflight-attempt",
    "preflight-success",
    "numeric-attempt",
    "numeric-success",
    "candidate-factor-solve",
    "solver-acceptance-attempt",
    "solver-acceptance-success",
    "execution-acceptance-attempt",
    "execution-acceptance-success",
    "state-commit",
];
const SINGULAR_PHASES: &[&str] = &[
    "preflight-attempt",
    "preflight-success",
    "numeric-attempt",
];
const PREFLIGHT_REJECTION_PHASES: &[&str] =
    &["preflight-attempt", "preflight-rejected"];

#[test]
fn registered_state_machine_oracle_executes_all_private_falsifiers() {
    p0_p1_p2_has_the_exact_phase_and_counter_inventory();
    p0_singular_p1_retains_the_last_committed_numeric_state();
    candidate_state_never_commits_before_both_acceptance_boundaries();
    every_required_validation_component_has_a_targeted_mutant();
    stale_foreign_partial_failed_and_singular_factor_states_reject();
}

#[test]
fn p0_p1_p2_has_the_exact_phase_and_counter_inventory() {
    let trace = phase_trace(PhaseScenario::P0P1P2);
    assert_eq!(trace.operation_ids(), &["p0", "p1", "p2"]);
    assert_eq!(trace.operation_phases(0), P0_PHASES);
    assert_eq!(trace.operation_phases(1), P1_PHASES);
    assert_eq!(trace.operation_phases(2), P2_PHASES);
    assert_eq!(trace.final_counters(), [3, 3, 1, 2]);

    let identities = trace.identity_relations();
    assert!(identities.p0_p1_structure_equal());
    assert!(identities.p0_p1_coefficients_equal());
    assert!(identities.p0_p1_symbolic_equal());
    assert!(identities.p0_p1_numeric_equal());
    assert!(!identities.p0_p1_rhs_equal());
    assert!(!identities.p0_p1_full_csr_equal());
    assert!(identities.p1_p2_structure_equal());
    assert!(!identities.p1_p2_coefficients_equal());
    assert!(identities.p1_p2_symbolic_equal());
    assert!(!identities.p1_p2_numeric_equal());
}

#[test]
fn p0_singular_p1_retains_the_last_committed_numeric_state() {
    let trace = retention_trace();
    assert_eq!(trace.operation_ids(), &["p0", "singular-candidate", "p1"]);
    assert_eq!(trace.operation_phases(0), P0_PHASES);
    assert_eq!(trace.operation_phases(1), SINGULAR_PHASES);
    assert_eq!(trace.operation_phases(2), P1_PHASES);
    assert_eq!(trace.after_failure_counters(), [2, 1, 1, 1]);
    assert_eq!(trace.final_counters(), [3, 2, 1, 1]);
    assert!(trace.committed_binding_retained());
    assert!(trace.committed_symbolic_identity_retained());
    assert!(trace.committed_numeric_identity_retained());
    assert!(trace.failed_candidate_identity_was_never_visible());
    assert!(trace.p1_used_retained_p0_numeric_factor());
}

#[test]
fn candidate_state_never_commits_before_both_acceptance_boundaries() {
    for (failure, expected_phases) in [
        (
            CandidateFailurePoint::NumericFactorization,
            &[
                "preflight-attempt",
                "preflight-success",
                "numeric-attempt",
            ][..],
        ),
        (
            CandidateFailurePoint::CandidateSolve,
            &[
                "preflight-attempt",
                "preflight-success",
                "numeric-attempt",
                "numeric-success",
                "candidate-factor-solve",
            ][..],
        ),
        (
            CandidateFailurePoint::SolverAcceptance,
            &[
                "preflight-attempt",
                "preflight-success",
                "numeric-attempt",
                "numeric-success",
                "candidate-factor-solve",
                "solver-acceptance-attempt",
            ][..],
        ),
        (
            CandidateFailurePoint::ExecutionAcceptance,
            &[
                "preflight-attempt",
                "preflight-success",
                "numeric-attempt",
                "numeric-success",
                "candidate-factor-solve",
                "solver-acceptance-attempt",
                "solver-acceptance-success",
                "execution-acceptance-attempt",
            ][..],
        ),
    ] {
        let observation = candidate_failure_observation(failure);
        assert_eq!(observation.phases(), expected_phases);
        assert_eq!(observation.counters(), [2, 1, 1, 1]);
        assert!(observation.committed_binding_retained());
        assert!(observation.committed_symbolic_identity_retained());
        assert!(observation.committed_numeric_identity_retained());
        assert!(!observation.candidate_identity_visible());
        assert!(!observation.state_commit_reached());
    }
}

#[test]
fn every_required_validation_component_has_a_targeted_mutant() {
    for mutant in [
        ValidationMutant::ExistingFullCsrOmitsRightHandSide,
        ValidationMutant::ReuseOmitsStructure,
        ValidationMutant::ReuseOmitsCoefficients,
        ValidationMutant::ReuseOmitsPolicy,
        ValidationMutant::ReuseOmitsProvider,
        ValidationMutant::ReuseOmitsPortableRealizationGraph,
    ] {
        let observation = validation_mutant_observation(mutant);
        assert!(
            !observation.baseline_authorizes(),
            "the complete validation conjunction must reject the targeted mismatch"
        );
        assert!(
            observation.mutant_authorizes(),
            "omitting exactly the targeted equality must let its mutant survive"
        );
        assert!(observation.only_targeted_component_differs());
        assert!(observation.committed_state_unchanged_by_baseline_rejection());
        assert_eq!(observation.baseline_rejection_phases(), PREFLIGHT_REJECTION_PHASES);
        assert_eq!(observation.baseline_numerical_attempt_delta(), 0);
    }
}

#[test]
fn stale_foreign_partial_failed_and_singular_factor_states_reject() {
    for mutant in [
        FactorStateMutant::Stale,
        FactorStateMutant::Foreign,
        FactorStateMutant::PartiallyConstructed,
        FactorStateMutant::Failed,
        FactorStateMutant::Singular,
    ] {
        let observation = factor_state_rejection_observation(mutant);
        assert_eq!(observation.error_code(), "EQ0807");
        assert_eq!(observation.phases(), PREFLIGHT_REJECTION_PHASES);
        assert!(!observation.factor_solve_reached());
        assert_eq!(observation.numerical_attempt_delta(), 0);
        assert!(observation.committed_binding_retained());
        assert!(observation.committed_identities_retained());
        assert!(observation.public_counters_unchanged());
    }
}
