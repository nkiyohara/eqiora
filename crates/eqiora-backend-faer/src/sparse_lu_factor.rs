#[cfg(test)]
pub(super) use crate::sparse_lu_identity::{
    COEFFICIENT_DOMAIN, STRUCTURE_DOMAIN, normalized_bits, numeric_identity,
    rhs_omission_mutant_observation, symbolic_identity,
};
pub(super) use crate::sparse_lu_identity::{IdentitySet, binding_shell_equal, identities};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::{CanonicalCsrSystemView, LinearOperatorOrientation};
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{LuRef, NumericLu, SymbolicLu, factorize_symbolic_lu};
use faer::sparse::{SparseRowMat, SymbolicSparseRowMat};
use faer::{Conj, Mat, Par};

/// Owned faer symbolic state for one exact canonical CSR structure.
#[derive(Debug)]
pub(super) struct SparseLuSymbolicFactor {
    factor: SymbolicLu<usize>,
}

/// Owned faer numeric state produced under one compatible symbolic factor.
#[derive(Debug)]
pub(super) struct SparseLuNumericFactor {
    factor: NumericLu<usize, f64>,
}

pub(super) fn factor_symbolic(
    system: &CanonicalCsrSystemView,
) -> Result<SparseLuSymbolicFactor, Diagnostic> {
    let _phase = eqiora_execution::telemetry::phase(eqiora_execution::telemetry::Phase::Backend {
        name: "symbolic_factorization",
        backend: "faer",
    })
    .entered();
    let symbolic_row = SymbolicSparseRowMat::<usize>::new_checked(
        system.rows(),
        system.columns(),
        system.row_offsets().to_vec(),
        None,
        system.column_indices().to_vec(),
    );
    let symbolic_column = symbolic_row
        .to_col_major()
        .map_err(|error| solve_failed(format!("faer CSR structure conversion failed: {error}")))?;
    let factor = factorize_symbolic_lu(symbolic_column.as_ref(), Default::default())
        .map_err(|error| solve_failed(format!("faer symbolic LU failed: {error}")))?;
    Ok(SparseLuSymbolicFactor { factor })
}

pub(super) fn factor_numeric(
    symbolic: &SparseLuSymbolicFactor,
    system: &CanonicalCsrSystemView,
) -> Result<SparseLuNumericFactor, Diagnostic> {
    let _phase = eqiora_execution::telemetry::phase(eqiora_execution::telemetry::Phase::Backend {
        name: "numeric_factorization",
        backend: "faer",
    })
    .entered();
    let symbolic_row = SymbolicSparseRowMat::<usize>::new_checked(
        system.rows(),
        system.columns(),
        system.row_offsets().to_vec(),
        None,
        system.column_indices().to_vec(),
    );
    let row_matrix = SparseRowMat::<usize, f64>::new(symbolic_row, system.values().to_vec());
    let column_matrix = row_matrix
        .to_col_major()
        .map_err(|error| solve_failed(format!("faer CSR conversion failed: {error}")))?;

    let parallelism = Par::Seq;
    let mut factor = NumericLu::<usize, f64>::new();
    let scratch = symbolic
        .factor
        .factorize_numeric_lu_scratch::<f64>(parallelism, Default::default());
    let mut buffer = MemBuffer::try_new(scratch)
        .map_err(|error| solve_failed(format!("faer numeric LU workspace failed: {error}")))?;
    symbolic
        .factor
        .factorize_numeric_lu(
            &mut factor,
            column_matrix.as_ref(),
            parallelism,
            MemStack::new(&mut buffer),
            Default::default(),
        )
        .map_err(|error| solve_failed(format!("faer numeric LU failed: {error}")))?;
    Ok(SparseLuNumericFactor { factor })
}

pub(super) fn solve_factored(
    symbolic: &SparseLuSymbolicFactor,
    numeric: &SparseLuNumericFactor,
    right_hand_side: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let orientation = LinearOperatorOrientation::Normal;
    solve_factored_oriented(symbolic, numeric, right_hand_side, orientation)
}
pub(super) fn solve_factored_oriented(
    symbolic: &SparseLuSymbolicFactor,
    numeric: &SparseLuNumericFactor,
    right_hand_side: &[f64],
    orientation: LinearOperatorOrientation,
) -> Result<Vec<f64>, Diagnostic> {
    let _phase = eqiora_execution::telemetry::phase(eqiora_execution::telemetry::Phase::Backend {
        name: "backsolve",
        backend: "faer",
    })
    .entered();
    if right_hand_side.len() != symbolic.factor.nrows()
        || symbolic.factor.nrows() != symbolic.factor.ncols()
    {
        return Err(solve_failed(
            "faer sparse LU factors and right-hand side have incompatible dimensions",
        ));
    }
    let parallelism = Par::Seq;
    let mut output = Mat::from_fn(right_hand_side.len(), 1, |row, _| right_hand_side[row]);
    let scratch = match orientation {
        LinearOperatorOrientation::Normal => symbolic
            .factor
            .solve_in_place_scratch::<f64>(1, parallelism),
        LinearOperatorOrientation::Transposed => symbolic
            .factor
            .solve_transpose_in_place_scratch::<f64>(1, parallelism),
    };
    let mut buffer = MemBuffer::try_new(scratch)
        .map_err(|error| solve_failed(format!("faer sparse LU solve workspace failed: {error}")))?;
    match orientation {
        LinearOperatorOrientation::Normal => {
            LuRef::new_unchecked(&symbolic.factor, &numeric.factor).solve_in_place_with_conj(
                Conj::No,
                output.as_mut(),
                parallelism,
                MemStack::new(&mut buffer),
            );
        }
        LinearOperatorOrientation::Transposed => {
            LuRef::new_unchecked(&symbolic.factor, &numeric.factor)
                .solve_transpose_in_place_with_conj(
                    Conj::No,
                    output.as_mut(),
                    parallelism,
                    MemStack::new(&mut buffer),
                );
        }
    }
    let values = output.col_as_slice(0).to_vec();
    if values.iter().any(|value| !value.is_finite()) {
        return Err(solve_failed(
            "faer sparse LU produced a non-finite solution",
        ));
    }
    Ok(values)
}
#[cfg(test)]
fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::num::NonZeroUsize;

    use super::*;
    use crate::sparse_lu_reuse::{
        FaerSparseLuPreparedSession, InjectedFactorState, OwnerSnapshot, PreflightInput,
        ReuseBinding, ReuseExecution, StoredNumericFactor, StoredSymbolicFactor, SyntheticBinding,
        ValidationComponent,
    };
    use eqiora_solver::{LinearSolver, ReductionPolicy, SolverPlan};
    use sha2::{Digest, Sha256};

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum PhaseScenario {
        P0P1P2,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum CandidateFailurePoint {
        NumericFactorization,
        CandidateSolve,
        SolverAcceptance,
        ExecutionAcceptance,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum ValidationMutant {
        ExistingFullCsrOmitsRightHandSide,
        ReuseOmitsStructure,
        ReuseOmitsCoefficients,
        ReuseOmitsPolicy,
        ReuseOmitsProvider,
        ReuseOmitsPortableRealizationGraph,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum FactorStateMutant {
        Stale,
        Foreign,
        PartiallyConstructed,
        Failed,
        Singular,
    }

    struct SyntheticExecution {
        input: PreflightInput,
        failure: Option<CandidateFailurePoint>,
        omission: Option<ValidationComponent>,
        reject_input: bool,
    }

    impl SyntheticExecution {
        fn new(input: PreflightInput) -> Self {
            Self {
                input,
                failure: None,
                omission: None,
                reject_input: false,
            }
        }

        fn failing(mut self, failure: CandidateFailurePoint) -> Self {
            self.failure = Some(failure);
            self
        }

        fn omitting(mut self, component: ValidationComponent) -> Self {
            self.omission = Some(component);
            self
        }

        fn rejecting_input(mut self) -> Self {
            self.reject_input = true;
            self
        }
    }

    impl ReuseExecution for SyntheticExecution {
        type CandidateSolution = u64;
        type SolverAccepted = u64;
        type Accepted = u64;

        fn preflight_input(&self, _owner_plan: SolverPlan) -> Result<PreflightInput, Diagnostic> {
            if self.reject_input {
                return Err(invalid_realization(
                    "synthetic complete-CSR validation rejected the changed right-hand side",
                ));
            }
            Ok(self.input.clone())
        }

        fn factor_symbolic(&mut self) -> Result<StoredSymbolicFactor, Diagnostic> {
            Ok(StoredSymbolicFactor::Synthetic(token(
                self.input.identities.symbolic,
            )))
        }

        fn factor_numeric(
            &mut self,
            _symbolic: &StoredSymbolicFactor,
        ) -> Result<StoredNumericFactor, Diagnostic> {
            if self.failure == Some(CandidateFailurePoint::NumericFactorization) {
                return Err(solve_failed("injected numeric factorization failure"));
            }
            Ok(StoredNumericFactor::Synthetic(token(
                self.input.identities.numeric,
            )))
        }

        fn solve(
            &mut self,
            _symbolic: &StoredSymbolicFactor,
            _numeric: &StoredNumericFactor,
        ) -> Result<Self::CandidateSolution, Diagnostic> {
            if self.failure == Some(CandidateFailurePoint::CandidateSolve) {
                return Err(solve_failed("injected candidate factor solve failure"));
            }
            Ok(token(self.input.identities.numeric))
        }

        fn accept_solver(
            &mut self,
            candidate: Self::CandidateSolution,
        ) -> Result<Self::SolverAccepted, Diagnostic> {
            if self.failure == Some(CandidateFailurePoint::SolverAcceptance) {
                return Err(solve_failed("injected solver acceptance failure"));
            }
            Ok(candidate)
        }

        fn accept_execution(
            &mut self,
            accepted: Self::SolverAccepted,
        ) -> Result<Self::Accepted, Diagnostic> {
            if self.failure == Some(CandidateFailurePoint::ExecutionAcceptance) {
                return Err(invalid_realization("injected execution acceptance failure"));
            }
            Ok(accepted)
        }

        fn validation_omission(&self) -> Option<ValidationComponent> {
            self.omission
        }
    }

    #[derive(Debug)]
    pub(crate) struct PhaseTrace {
        ids: [&'static str; 3],
        phases: [Vec<&'static str>; 3],
        counters: [usize; 4],
        identities: IdentityRelations,
    }

    impl PhaseTrace {
        pub(crate) fn operation_ids(&self) -> &[&'static str] {
            &self.ids
        }
        pub(crate) fn operation_phases(&self, index: usize) -> &[&'static str] {
            &self.phases[index]
        }
        pub(crate) const fn final_counters(&self) -> [usize; 4] {
            self.counters
        }
        pub(crate) const fn identity_relations(&self) -> IdentityRelations {
            self.identities
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct IdentityRelations {
        values: [bool; 10],
    }

    macro_rules! identity_getter {
        ($name:ident, $index:expr) => {
            pub(crate) const fn $name(self) -> bool {
                self.values[$index]
            }
        };
    }

    impl IdentityRelations {
        identity_getter!(p0_p1_structure_equal, 0);
        identity_getter!(p0_p1_coefficients_equal, 1);
        identity_getter!(p0_p1_symbolic_equal, 2);
        identity_getter!(p0_p1_numeric_equal, 3);
        identity_getter!(p0_p1_rhs_equal, 4);
        identity_getter!(p0_p1_full_csr_equal, 5);
        identity_getter!(p1_p2_structure_equal, 6);
        identity_getter!(p1_p2_coefficients_equal, 7);
        identity_getter!(p1_p2_symbolic_equal, 8);
        identity_getter!(p1_p2_numeric_equal, 9);
    }

    pub(crate) fn phase_trace(_scenario: PhaseScenario) -> PhaseTrace {
        let mut owner = owner();
        let p0 = fixture(0);
        let p1 = fixture(1);
        let p2 = fixture(2);
        owner
            .execute_core(SyntheticExecution::new(p0.clone()))
            .expect("p0 synthetic execution succeeds");
        let p0_phases = owner.phase_names();
        owner
            .execute_core(SyntheticExecution::new(p1.clone()))
            .expect("p1 synthetic execution succeeds");
        let p1_phases = owner.phase_names();
        owner
            .execute_core(SyntheticExecution::new(p2.clone()))
            .expect("p2 synthetic execution succeeds");
        let p2_phases = owner.phase_names();
        let (full_csr, rhs_omitting) = rhs_omission_mutant_observation();
        PhaseTrace {
            ids: ["p0", "p1", "p2"],
            phases: [p0_phases, p1_phases, p2_phases],
            counters: owner.test_snapshot().counters,
            identities: IdentityRelations {
                values: [
                    p0.identities.structure == p1.identities.structure,
                    p0.identities.coefficients == p1.identities.coefficients,
                    p0.identities.symbolic == p1.identities.symbolic,
                    p0.identities.numeric == p1.identities.numeric,
                    !rhs_omitting,
                    full_csr,
                    p1.identities.structure == p2.identities.structure,
                    p1.identities.coefficients == p2.identities.coefficients,
                    p1.identities.symbolic == p2.identities.symbolic,
                    p1.identities.numeric == p2.identities.numeric,
                ],
            },
        }
    }

    #[derive(Debug)]
    pub(crate) struct RetentionTrace {
        phases: [Vec<&'static str>; 3],
        after_failure: [usize; 4],
        final_counters: [usize; 4],
        retained: [bool; 5],
    }

    impl RetentionTrace {
        pub(crate) const fn operation_ids(&self) -> &[&'static str] {
            &["p0", "singular-candidate", "p1"]
        }
        pub(crate) fn operation_phases(&self, index: usize) -> &[&'static str] {
            &self.phases[index]
        }
        pub(crate) const fn after_failure_counters(&self) -> [usize; 4] {
            self.after_failure
        }
        pub(crate) const fn final_counters(&self) -> [usize; 4] {
            self.final_counters
        }
        pub(crate) const fn committed_binding_retained(&self) -> bool {
            self.retained[0]
        }
        pub(crate) const fn committed_symbolic_identity_retained(&self) -> bool {
            self.retained[1]
        }
        pub(crate) const fn committed_numeric_identity_retained(&self) -> bool {
            self.retained[2]
        }
        pub(crate) const fn failed_candidate_identity_was_never_visible(&self) -> bool {
            self.retained[3]
        }
        pub(crate) const fn p1_used_retained_p0_numeric_factor(&self) -> bool {
            self.retained[4]
        }
    }

    pub(crate) fn retention_trace() -> RetentionTrace {
        let mut owner = owner();
        let p0 = fixture(0);
        let p1 = fixture(1);
        let candidate = fixture(2);
        owner
            .execute_core(SyntheticExecution::new(p0))
            .expect("p0 commits");
        let p0_phases = owner.phase_names();
        let committed = owner.test_snapshot();
        owner
            .execute_core(
                SyntheticExecution::new(candidate.clone())
                    .failing(CandidateFailurePoint::NumericFactorization),
            )
            .expect_err("singular candidate fails");
        let failed_phases = owner.phase_names();
        let after_failure = owner.test_snapshot();
        owner
            .execute_core(SyntheticExecution::new(p1))
            .expect("p1 commits by reuse");
        let p1_phases = owner.phase_names();
        let final_state = owner.test_snapshot();
        RetentionTrace {
            phases: [p0_phases, failed_phases, p1_phases.clone()],
            after_failure: after_failure.counters,
            final_counters: final_state.counters,
            retained: [
                committed.binding == after_failure.binding,
                committed.symbolic_identity == after_failure.symbolic_identity,
                committed.numeric_identity == after_failure.numeric_identity,
                after_failure.numeric_identity != Some(candidate.identities.numeric)
                    && after_failure.numeric_identity == committed.numeric_identity,
                final_state.numeric_factor == committed.numeric_factor
                    && p1_phases.contains(&"retained-factor-solve"),
            ],
        }
    }

    #[derive(Debug)]
    pub(crate) struct CandidateFailureObservation {
        phases: Vec<&'static str>,
        counters: [usize; 4],
        retained: [bool; 3],
        candidate_visible: bool,
        state_commit: bool,
    }

    impl CandidateFailureObservation {
        pub(crate) fn phases(&self) -> &[&'static str] {
            &self.phases
        }
        pub(crate) const fn counters(&self) -> [usize; 4] {
            self.counters
        }
        pub(crate) const fn committed_binding_retained(&self) -> bool {
            self.retained[0]
        }
        pub(crate) const fn committed_symbolic_identity_retained(&self) -> bool {
            self.retained[1]
        }
        pub(crate) const fn committed_numeric_identity_retained(&self) -> bool {
            self.retained[2]
        }
        pub(crate) const fn candidate_identity_visible(&self) -> bool {
            self.candidate_visible
        }
        pub(crate) const fn state_commit_reached(&self) -> bool {
            self.state_commit
        }
    }

    pub(crate) fn candidate_failure_observation(
        failure: CandidateFailurePoint,
    ) -> CandidateFailureObservation {
        let mut owner = owner();
        owner
            .execute_core(SyntheticExecution::new(fixture(0)))
            .expect("p0 commits");
        let committed = owner.test_snapshot();
        let candidate = fixture(2);
        owner
            .execute_core(SyntheticExecution::new(candidate.clone()).failing(failure))
            .expect_err("candidate failure is injected");
        let phases = owner.phase_names();
        let after = owner.test_snapshot();
        CandidateFailureObservation {
            counters: after.counters,
            retained: retained_fields(committed, after),
            candidate_visible: after.symbolic_identity == Some(candidate.identities.symbolic)
                && after.numeric_identity == Some(candidate.identities.numeric),
            state_commit: phases.contains(&"state-commit"),
            phases,
        }
    }

    #[derive(Debug)]
    pub(crate) struct ValidationMutantObservation {
        baseline: bool,
        mutant: bool,
        targeted: bool,
        unchanged: bool,
        phases: Vec<&'static str>,
        attempt_delta: usize,
    }

    impl ValidationMutantObservation {
        pub(crate) const fn baseline_authorizes(&self) -> bool {
            self.baseline
        }
        pub(crate) const fn mutant_authorizes(&self) -> bool {
            self.mutant
        }
        pub(crate) const fn only_targeted_component_differs(&self) -> bool {
            self.targeted
        }
        pub(crate) const fn committed_state_unchanged_by_baseline_rejection(&self) -> bool {
            self.unchanged
        }
        pub(crate) fn baseline_rejection_phases(&self) -> &[&'static str] {
            &self.phases
        }
        pub(crate) const fn baseline_numerical_attempt_delta(&self) -> usize {
            self.attempt_delta
        }
    }

    pub(crate) fn validation_mutant_observation(
        mutant: ValidationMutant,
    ) -> ValidationMutantObservation {
        if matches!(mutant, ValidationMutant::ExistingFullCsrOmitsRightHandSide) {
            return full_csr_mutant_observation();
        }
        let component = validation_component(mutant);
        let mut owner = owner();
        owner
            .execute_core(SyntheticExecution::new(fixture(0)))
            .expect("baseline state commits");
        let before = owner.test_snapshot();
        let candidate = mutated_fixture(component);
        let validation = owner.validation_for(&candidate);
        let baseline_result = owner.execute_core(SyntheticExecution::new(candidate.clone()));
        let phases = owner.phase_names();
        let after = owner.test_snapshot();

        let mut mutant_owner = self::owner();
        mutant_owner
            .execute_core(SyntheticExecution::new(fixture(0)))
            .expect("mutant baseline state commits");
        let mutant_before = mutant_owner.test_snapshot();
        let mutant_result = mutant_owner
            .execute_core(SyntheticExecution::new(candidate.clone()).omitting(component));
        let mutant_phases = mutant_owner.phase_names();
        let mutant_after = mutant_owner.test_snapshot();
        let baseline_authorizes = baseline_result.is_ok()
            && phases.contains(&"retained-factor-solve")
            && !phases.contains(&"numeric-attempt");
        let mutant_authorizes = mutant_result.is_ok()
            && mutant_phases.contains(&"retained-factor-solve")
            && !mutant_phases.contains(&"numeric-attempt")
            && (component != ValidationComponent::Coefficients
                || (mutant_after.numeric_factor == mutant_before.numeric_factor
                    && mutant_after.numeric_identity == mutant_before.numeric_identity
                    && mutant_after.numeric_identity != Some(candidate.identities.numeric)));
        ValidationMutantObservation {
            baseline: baseline_authorizes,
            mutant: mutant_authorizes,
            targeted: validation.difference_count() == 1,
            unchanged: before == after,
            phases,
            attempt_delta: after.counters[0] - before.counters[0],
        }
    }

    fn full_csr_mutant_observation() -> ValidationMutantObservation {
        let (baseline, mutant) = rhs_omission_mutant_observation();
        let mut owner = owner();
        let before = owner.test_snapshot();
        owner
            .execute_core(SyntheticExecution::new(fixture(0)).rejecting_input())
            .expect_err("complete CSR equality rejects changed RHS");
        let phases = owner.phase_names();
        let after = owner.test_snapshot();
        ValidationMutantObservation {
            baseline,
            mutant,
            targeted: baseline != mutant,
            unchanged: before == after,
            phases,
            attempt_delta: after.counters[0] - before.counters[0],
        }
    }

    #[derive(Debug)]
    pub(crate) struct FactorStateRejectionObservation {
        error_code: &'static str,
        phases: Vec<&'static str>,
        factor_solve: bool,
        attempt_delta: usize,
        retained_binding: bool,
        retained_identities: bool,
        counters_unchanged: bool,
    }

    impl FactorStateRejectionObservation {
        pub(crate) const fn error_code(&self) -> &'static str {
            self.error_code
        }
        pub(crate) fn phases(&self) -> &[&'static str] {
            &self.phases
        }
        pub(crate) const fn factor_solve_reached(&self) -> bool {
            self.factor_solve
        }
        pub(crate) const fn numerical_attempt_delta(&self) -> usize {
            self.attempt_delta
        }
        pub(crate) const fn committed_binding_retained(&self) -> bool {
            self.retained_binding
        }
        pub(crate) const fn committed_identities_retained(&self) -> bool {
            self.retained_identities
        }
        pub(crate) const fn public_counters_unchanged(&self) -> bool {
            self.counters_unchanged
        }
    }

    pub(crate) fn factor_state_rejection_observation(
        mutant: FactorStateMutant,
    ) -> FactorStateRejectionObservation {
        let mut owner = owner();
        owner
            .execute_core(SyntheticExecution::new(fixture(0)))
            .expect("p0 commits");
        owner.inject_factor_state(match mutant {
            FactorStateMutant::Stale => InjectedFactorState::Stale,
            FactorStateMutant::Foreign => InjectedFactorState::Foreign,
            FactorStateMutant::PartiallyConstructed => InjectedFactorState::PartiallyConstructed,
            FactorStateMutant::Failed => InjectedFactorState::Failed,
            FactorStateMutant::Singular => InjectedFactorState::Singular,
        });
        let before = owner.test_snapshot();
        let error = owner
            .execute_core(SyntheticExecution::new(fixture(1)))
            .expect_err("invalid retained factor state rejects in preflight");
        let phases = owner.phase_names();
        let after = owner.test_snapshot();
        FactorStateRejectionObservation {
            error_code: error.code().0,
            factor_solve: phases
                .iter()
                .any(|phase| matches!(*phase, "retained-factor-solve" | "candidate-factor-solve")),
            attempt_delta: after.counters[0] - before.counters[0],
            retained_binding: before.binding == after.binding,
            retained_identities: before.symbolic_identity == after.symbolic_identity
                && before.numeric_identity == after.numeric_identity,
            counters_unchanged: before.counters == after.counters,
            phases,
        }
    }

    fn owner() -> FaerSparseLuPreparedSession {
        FaerSparseLuPreparedSession::new_bounded(test_plan(), NonZeroUsize::new(64).unwrap())
            .expect("synthetic owner policy is valid")
    }

    fn test_plan() -> SolverPlan {
        SolverPlan::new(
            LinearSolver::SparseLu,
            0.0,
            f64::from_bits(0x3e10_0000_0000_0000),
            NonZeroUsize::MIN,
        )
        .expect("synthetic solver plan is valid")
        .with_reduction(ReductionPolicy::Fast)
    }

    fn fixture(index: usize) -> PreflightInput {
        let structure = synthetic_structure();
        let coefficients = synthetic_coefficients(structure, if index == 2 { 5.0 } else { 4.0 });
        let policy = [7; 32];
        let symbolic = symbolic_identity(structure, policy);
        let numeric = numeric_identity(symbolic, coefficients);
        PreflightInput {
            binding: ReuseBinding::Synthetic(SyntheticBinding {
                shell: 11,
                provider: 13,
                graph: 17,
            }),
            identities: IdentitySet {
                structure,
                coefficients,
                policy,
                symbolic,
                numeric,
            },
            coefficients_reusable: None,
        }
    }

    fn mutated_fixture(component: ValidationComponent) -> PreflightInput {
        let mut input = if component == ValidationComponent::Coefficients {
            fixture(2)
        } else {
            fixture(1)
        };
        match component {
            ValidationComponent::AcceptedBinding => match &mut input.binding {
                ReuseBinding::Synthetic(binding) => binding.shell += 1,
                ReuseBinding::Live(_) => unreachable!(),
            },
            ValidationComponent::Structure => input.identities.structure[0] ^= 1,
            ValidationComponent::Coefficients => {}
            ValidationComponent::Policy => input.identities.policy[0] ^= 1,
            ValidationComponent::Provider => match &mut input.binding {
                ReuseBinding::Synthetic(binding) => binding.provider += 1,
                ReuseBinding::Live(_) => unreachable!(),
            },
            ValidationComponent::Graph => match &mut input.binding {
                ReuseBinding::Synthetic(binding) => binding.graph += 1,
                ReuseBinding::Live(_) => unreachable!(),
            },
        }
        input
    }

    fn validation_component(mutant: ValidationMutant) -> ValidationComponent {
        match mutant {
            ValidationMutant::ExistingFullCsrOmitsRightHandSide => unreachable!(),
            ValidationMutant::ReuseOmitsStructure => ValidationComponent::Structure,
            ValidationMutant::ReuseOmitsCoefficients => ValidationComponent::Coefficients,
            ValidationMutant::ReuseOmitsPolicy => ValidationComponent::Policy,
            ValidationMutant::ReuseOmitsProvider => ValidationComponent::Provider,
            ValidationMutant::ReuseOmitsPortableRealizationGraph => ValidationComponent::Graph,
        }
    }

    fn retained_fields(before: OwnerSnapshot, after: OwnerSnapshot) -> [bool; 3] {
        [
            before.binding == after.binding,
            before.symbolic_identity == after.symbolic_identity,
            before.numeric_identity == after.numeric_identity,
        ]
    }

    fn token(identity: [u8; 32]) -> u64 {
        u64::from_be_bytes(identity[..8].try_into().unwrap())
    }

    fn synthetic_structure() -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(STRUCTURE_DOMAIN);
        for value in [1_u64, 1, 2, 0, 1, 1, 0] {
            hash.update(value.to_be_bytes());
        }
        hash.finalize().into()
    }

    fn synthetic_coefficients(structure: [u8; 32], value: f64) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(COEFFICIENT_DOMAIN);
        hash.update(structure);
        hash.update(1_u64.to_be_bytes());
        hash.update(normalized_bits(value).to_be_bytes());
        hash.finalize().into()
    }
}
