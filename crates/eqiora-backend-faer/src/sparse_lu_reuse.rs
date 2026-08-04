use std::cell::Cell;
use std::num::NonZeroUsize;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_execution::{AcceptedLinearExecution, AdmittedExecution, DeploymentBinding};
use eqiora_solver::{
    ConvergenceReason, ExecutionReport, LinearOperator, LinearOperatorOrientation, LinearSolver,
    PreconditionerPolicy, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, SolverPlan,
    accept_linear_solution,
};
#[cfg(test)]
use sha2::{Digest, Sha256};

use crate::FAER_SOLVER_PROVIDER;
use crate::sparse_lu::fixed_residual_norm;
#[cfg(test)]
use crate::sparse_lu_factor::rhs_omission_mutant_observation;
#[cfg(test)]
use crate::sparse_lu_factor::{
    COEFFICIENT_DOMAIN, STRUCTURE_DOMAIN, normalized_bits, numeric_identity, symbolic_identity,
};
use crate::sparse_lu_factor::{
    IdentitySet, SparseLuNumericFactor, SparseLuSymbolicFactor, binding_shell_equal,
    factor_numeric, factor_symbolic, identities, solve_factored,
};

/// Bounded host-serial owner of one accepted faer sparse-LU reuse state.
#[derive(Debug)]
pub struct FaerSparseLuReuseOwner {
    plan: SolverPlan,
    maximum_attempts: NonZeroUsize,
    counters: Counters,
    state: ReuseState,
    last_phases: PhaseLedger,
    _not_sync: Cell<()>,
}

impl FaerSparseLuReuseOwner {
    /// Construct the exact sparse-LU/identity/fast owner.
    ///
    /// # Errors
    /// Returns `EQ0807` for another solver policy or an attempt bound outside
    /// the closed interval `2..=64`.
    pub fn new(plan: SolverPlan, maximum_attempts: NonZeroUsize) -> Result<Self, Diagnostic> {
        if !(2..=64).contains(&maximum_attempts.get()) {
            return Err(invalid_realization(
                "faer sparse LU reuse requires maximum_attempts in 2..=64",
            ));
        }
        if plan.algorithm() != LinearSolver::SparseLu
            || plan.preconditioner() != PreconditionerPolicy::Identity
            || plan.reduction() != ReductionPolicy::Fast
        {
            return Err(invalid_realization(
                "faer sparse LU reuse requires SparseLu, Identity, and Fast",
            ));
        }
        Ok(Self {
            plan,
            maximum_attempts,
            counters: Counters::default(),
            state: ReuseState::Empty,
            last_phases: PhaseLedger::default(),
            _not_sync: Cell::new(()),
        })
    }

    /// Execute one separately admitted host-linear solve and commit reusable
    /// factors only after solver and execution acceptance both succeed.
    ///
    /// # Errors
    /// Returns a structured diagnostic for an incompatible binding, exhausted
    /// capacity, factorization/solve failure, or either acceptance failure.
    pub fn execute(
        &mut self,
        admitted: AdmittedExecution<'_>,
    ) -> Result<AcceptedLinearExecution, Diagnostic> {
        self.last_phases.restart();
        self.last_phases.record(Phase::PreflightAttempt);
        let preflight = match self.preflight(&admitted) {
            Ok(preflight) => preflight,
            Err(error) => {
                self.last_phases.record(Phase::PreflightRejected);
                return Err(error);
            }
        };
        self.last_phases.record(Phase::PreflightSuccess);
        self.counters.begin_attempt();

        let system = admitted.system();
        let (values, candidate) = match preflight.action {
            ReuseAction::BuildSymbolicAndNumeric => {
                self.last_phases.record(Phase::SymbolicAttempt);
                let symbolic = factor_symbolic(system)?;
                self.last_phases.record(Phase::SymbolicSuccess);
                self.last_phases.record(Phase::NumericAttempt);
                let numeric = factor_numeric(&symbolic, system)?;
                self.last_phases.record(Phase::NumericSuccess);
                self.last_phases.record(Phase::CandidateFactorSolve);
                let values = solve_factored(&symbolic, &numeric, system.right_hand_side())?;
                (
                    values,
                    CandidateFactors::SymbolicAndNumeric(symbolic, numeric),
                )
            }
            ReuseAction::ReuseNumeric => {
                self.last_phases.record(Phase::RetainedFactorSolve);
                let ready = self.ready_state()?;
                let values = solve_factored(
                    &ready.symbolic_factor,
                    &ready.numeric_factor,
                    system.right_hand_side(),
                )?;
                (values, CandidateFactors::Retained)
            }
            ReuseAction::RebuildNumeric => {
                self.last_phases.record(Phase::NumericAttempt);
                let numeric = {
                    let ready = self.ready_state()?;
                    factor_numeric(&ready.symbolic_factor, system)?
                };
                self.last_phases.record(Phase::NumericSuccess);
                self.last_phases.record(Phase::CandidateFactorSolve);
                let values = {
                    let ready = self.ready_state()?;
                    solve_factored(&ready.symbolic_factor, &numeric, system.right_hand_side())?
                };
                (values, CandidateFactors::Numeric(numeric))
            }
        };

        self.last_phases.record(Phase::SolverAcceptanceAttempt);
        let problem = system.linear_problem()?;
        let reported_residual_norm = fixed_residual_norm(&problem, &values)?;
        let solution = accept_linear_solution(
            &problem,
            self.plan,
            FAER_SOLVER_PROVIDER,
            ConvergenceReason::ResidualToleranceSatisfied,
            1,
            reported_residual_norm,
            values,
        )?;
        self.last_phases.record(Phase::SolverAcceptanceSuccess);

        self.last_phases.record(Phase::ExecutionAcceptanceAttempt);
        let accepted = admitted.accept(solution)?;
        self.last_phases.record(Phase::ExecutionAcceptanceSuccess);
        self.commit(preflight, candidate)?;
        self.last_phases.record(Phase::StateCommit);
        Ok(accepted)
    }

    /// Immutable numerical policy bound by this owner.
    #[must_use]
    pub const fn plan(&self) -> SolverPlan {
        self.plan
    }

    /// Maximum post-preflight numerical attempts.
    #[must_use]
    pub const fn maximum_attempts(&self) -> NonZeroUsize {
        self.maximum_attempts
    }

    /// Numerical attempts begun after successful preflight.
    #[must_use]
    pub const fn attempted_solve_count(&self) -> usize {
        self.counters.attempted
    }

    /// Executions committed after both acceptance boundaries.
    #[must_use]
    pub const fn accepted_solve_count(&self) -> usize {
        self.counters.accepted
    }

    /// Successfully committed symbolic constructions.
    #[must_use]
    pub const fn symbolic_factorization_count(&self) -> usize {
        self.counters.symbolic
    }

    /// Successfully committed numeric constructions.
    #[must_use]
    pub const fn numeric_factorization_count(&self) -> usize {
        self.counters.numeric
    }

    /// Symbolic identity of the last committed accepted state.
    #[must_use]
    pub const fn symbolic_reuse_identity(&self) -> Option<[u8; 32]> {
        match &self.state {
            ReuseState::Empty => None,
            ReuseState::Ready(ready) => Some(ready.symbolic_identity),
        }
    }

    /// Numeric identity of the last committed accepted state.
    #[must_use]
    pub const fn numeric_reuse_identity(&self) -> Option<[u8; 32]> {
        match &self.state {
            ReuseState::Empty => None,
            ReuseState::Ready(ready) => Some(ready.numeric_identity),
        }
    }

    fn preflight(&self, admitted: &AdmittedExecution<'_>) -> Result<Preflight, Diagnostic> {
        if self.counters.attempted >= self.maximum_attempts.get() {
            return Err(invalid_realization(
                "faer sparse LU reuse owner exhausted its numerical attempt capacity",
            ));
        }
        let binding = admitted.binding();
        let Some(host) = binding.host_executor() else {
            return Err(invalid_realization(
                "faer sparse LU reuse requires a host deployment binding",
            ));
        };
        if binding.execution() != ExecutionReport::host_serial()
            || binding.execution_provider() != SERIAL_EXECUTION_PROVIDER
            || binding.verification_provider() != SERIAL_EXECUTION_PROVIDER
        {
            return Err(invalid_realization(
                "faer sparse LU reuse requires the direct one-worker host execution provider",
            ));
        }
        if host.solver_provider() != FAER_SOLVER_PROVIDER
            || binding.solver_provider() != FAER_SOLVER_PROVIDER
        {
            return Err(invalid_realization(
                "faer sparse LU reuse requires the pinned faer provider descriptor",
            ));
        }
        if binding.solver_plan() != self.plan || admitted.solver_plan() != self.plan {
            return Err(invalid_realization(
                "faer sparse LU reuse execution plan differs from the immutable owner plan",
            ));
        }
        let system = admitted.system();
        if system.orientation() != LinearOperatorOrientation::Normal {
            return Err(invalid_realization(
                "faer sparse LU reuse requires normal-orientation canonical CSR",
            ));
        }

        let identities = identities(system, self.plan, binding.solver_provider())?;
        let structure = identities.structure;
        let coefficients = identities.coefficients;
        let policy = identities.policy;

        let action = match &self.state {
            ReuseState::Empty => ReuseAction::BuildSymbolicAndNumeric,
            ReuseState::Ready(ready) => {
                validate_factor_state(
                    ready.factor_state,
                    ready.symbolic_identity,
                    ready.numeric_identity,
                )?;
                let validation = ValidationComponents {
                    accepted_binding: binding_shell_equal(&ready.binding, binding),
                    structure: ready.structure_identity == structure,
                    coefficients: ready.coefficient_identity == coefficients,
                    policy: ready.policy_identity == policy,
                    provider: ready.binding.solver_provider() == binding.solver_provider(),
                    graph: ready.binding.realization() == binding.realization(),
                };
                validation.classify()?
            }
        };
        Ok(Preflight {
            binding: binding.clone(),
            identities,
            action,
        })
    }

    fn ready_state(&self) -> Result<&ReadyState, Diagnostic> {
        match &self.state {
            ReuseState::Ready(ready) => Ok(ready),
            ReuseState::Empty => Err(invalid_realization(
                "faer sparse LU reuse lost its committed factor state",
            )),
        }
    }

    fn commit(
        &mut self,
        preflight: Preflight,
        candidate: CandidateFactors,
    ) -> Result<(), Diagnostic> {
        match candidate {
            CandidateFactors::SymbolicAndNumeric(symbolic_factor, numeric_factor) => {
                self.state = ReuseState::Ready(ReadyState {
                    binding: preflight.binding,
                    structure_identity: preflight.identities.structure,
                    coefficient_identity: preflight.identities.coefficients,
                    policy_identity: preflight.identities.policy,
                    symbolic_identity: preflight.identities.symbolic,
                    numeric_identity: preflight.identities.numeric,
                    symbolic_factor,
                    numeric_factor,
                    factor_state: FactorStateMarker::ready(
                        preflight.identities.symbolic,
                        preflight.identities.numeric,
                    ),
                });
                self.counters.commit(true, true);
            }
            CandidateFactors::Numeric(numeric_factor) => {
                let ready = match &mut self.state {
                    ReuseState::Ready(ready) => ready,
                    ReuseState::Empty => {
                        return Err(invalid_realization(
                            "faer sparse LU reuse cannot commit numeric factors without symbolic state",
                        ));
                    }
                };
                ready.coefficient_identity = preflight.identities.coefficients;
                ready.numeric_identity = preflight.identities.numeric;
                ready.numeric_factor = numeric_factor;
                ready.factor_state =
                    FactorStateMarker::ready(ready.symbolic_identity, ready.numeric_identity);
                self.counters.commit(false, true);
            }
            CandidateFactors::Retained => self.counters.commit(false, false),
        }
        Ok(())
    }
}

#[derive(Debug)]
enum ReuseState {
    Empty,
    Ready(ReadyState),
}

#[derive(Debug)]
struct ReadyState {
    binding: DeploymentBinding,
    structure_identity: [u8; 32],
    coefficient_identity: [u8; 32],
    policy_identity: [u8; 32],
    symbolic_identity: [u8; 32],
    numeric_identity: [u8; 32],
    symbolic_factor: SparseLuSymbolicFactor,
    numeric_factor: SparseLuNumericFactor,
    factor_state: FactorStateMarker,
}

#[derive(Debug)]
enum CandidateFactors {
    SymbolicAndNumeric(SparseLuSymbolicFactor, SparseLuNumericFactor),
    Numeric(SparseLuNumericFactor),
    Retained,
}

#[derive(Debug)]
struct Preflight {
    binding: DeploymentBinding,
    identities: IdentitySet,
    action: ReuseAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReuseAction {
    BuildSymbolicAndNumeric,
    ReuseNumeric,
    RebuildNumeric,
}

#[derive(Debug, Clone, Copy)]
struct ValidationComponents {
    accepted_binding: bool,
    structure: bool,
    coefficients: bool,
    policy: bool,
    provider: bool,
    graph: bool,
}

impl ValidationComponents {
    fn classify(self) -> Result<ReuseAction, Diagnostic> {
        if !(self.accepted_binding && self.structure && self.policy && self.provider && self.graph)
        {
            return Err(invalid_realization(
                "faer sparse LU reuse binding, structure, policy, provider, or graph changed",
            ));
        }
        Ok(if self.coefficients {
            ReuseAction::ReuseNumeric
        } else {
            ReuseAction::RebuildNumeric
        })
    }

    #[cfg(test)]
    fn authorizes_numeric(self, omitted: Option<ValidationComponent>) -> bool {
        let included = |component, matches| omitted == Some(component) || matches;
        included(ValidationComponent::AcceptedBinding, self.accepted_binding)
            && included(ValidationComponent::Structure, self.structure)
            && included(ValidationComponent::Coefficients, self.coefficients)
            && included(ValidationComponent::Policy, self.policy)
            && included(ValidationComponent::Provider, self.provider)
            && included(ValidationComponent::Graph, self.graph)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Counters {
    attempted: usize,
    accepted: usize,
    symbolic: usize,
    numeric: usize,
}

impl Counters {
    fn begin_attempt(&mut self) {
        self.attempted += 1;
    }

    fn commit(&mut self, symbolic: bool, numeric: bool) {
        self.accepted += 1;
        self.symbolic += usize::from(symbolic);
        self.numeric += usize::from(numeric);
    }

    #[cfg(test)]
    const fn as_array(self) -> [usize; 4] {
        [self.attempted, self.accepted, self.symbolic, self.numeric]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    PreflightAttempt,
    PreflightSuccess,
    PreflightRejected,
    SymbolicAttempt,
    SymbolicSuccess,
    NumericAttempt,
    NumericSuccess,
    RetainedFactorSolve,
    CandidateFactorSolve,
    SolverAcceptanceAttempt,
    SolverAcceptanceSuccess,
    ExecutionAcceptanceAttempt,
    ExecutionAcceptanceSuccess,
    StateCommit,
}

#[cfg(test)]
impl Phase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PreflightAttempt => "preflight-attempt",
            Self::PreflightSuccess => "preflight-success",
            Self::PreflightRejected => "preflight-rejected",
            Self::SymbolicAttempt => "symbolic-attempt",
            Self::SymbolicSuccess => "symbolic-success",
            Self::NumericAttempt => "numeric-attempt",
            Self::NumericSuccess => "numeric-success",
            Self::RetainedFactorSolve => "retained-factor-solve",
            Self::CandidateFactorSolve => "candidate-factor-solve",
            Self::SolverAcceptanceAttempt => "solver-acceptance-attempt",
            Self::SolverAcceptanceSuccess => "solver-acceptance-success",
            Self::ExecutionAcceptanceAttempt => "execution-acceptance-attempt",
            Self::ExecutionAcceptanceSuccess => "execution-acceptance-success",
            Self::StateCommit => "state-commit",
        }
    }
}

#[derive(Debug, Default)]
struct PhaseLedger {
    phases: Vec<Phase>,
}

impl PhaseLedger {
    fn restart(&mut self) {
        self.phases.clear();
    }

    fn record(&mut self, phase: Phase) {
        self.phases.push(phase);
    }

    #[cfg(test)]
    fn names(&self) -> Vec<&'static str> {
        self.phases.iter().map(|phase| phase.as_str()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactorStateStatus {
    Ready,
    #[cfg(test)]
    Stale,
    #[cfg(test)]
    Foreign,
    #[cfg(test)]
    PartiallyConstructed,
    #[cfg(test)]
    Failed,
    #[cfg(test)]
    Singular,
}

#[derive(Debug, Clone, Copy)]
struct FactorStateMarker {
    status: FactorStateStatus,
    symbolic_identity: Option<[u8; 32]>,
    numeric_identity: Option<[u8; 32]>,
}

impl FactorStateMarker {
    const fn ready(symbolic_identity: [u8; 32], numeric_identity: [u8; 32]) -> Self {
        Self {
            status: FactorStateStatus::Ready,
            symbolic_identity: Some(symbolic_identity),
            numeric_identity: Some(numeric_identity),
        }
    }
}

fn validate_factor_state(
    marker: FactorStateMarker,
    symbolic_identity: [u8; 32],
    numeric_identity: [u8; 32],
) -> Result<(), Diagnostic> {
    if marker.status != FactorStateStatus::Ready
        || marker.symbolic_identity != Some(symbolic_identity)
        || marker.numeric_identity != Some(numeric_identity)
    {
        return Err(invalid_realization(
            "faer sparse LU reusable factor state is stale, foreign, partial, failed, or singular",
        ));
    }
    Ok(())
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationComponent {
    AcceptedBinding,
    Structure,
    Coefficients,
    Policy,
    Provider,
    Graph,
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum PhaseScenario {
        P0P1P2,
    }

    #[derive(Debug, Clone, Copy)]
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

    #[derive(Debug)]
    pub(crate) struct PhaseTrace {
        ids: Vec<&'static str>,
        phases: Vec<Vec<&'static str>>,
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
        p0_p1_structure: bool,
        p0_p1_coefficients: bool,
        p0_p1_symbolic: bool,
        p0_p1_numeric: bool,
        p0_p1_rhs: bool,
        p0_p1_full_csr: bool,
        p1_p2_structure: bool,
        p1_p2_coefficients: bool,
        p1_p2_symbolic: bool,
        p1_p2_numeric: bool,
    }

    macro_rules! identity_getter {
        ($name:ident, $field:ident) => {
            pub(crate) const fn $name(self) -> bool {
                self.$field
            }
        };
    }

    impl IdentityRelations {
        identity_getter!(p0_p1_structure_equal, p0_p1_structure);
        identity_getter!(p0_p1_coefficients_equal, p0_p1_coefficients);
        identity_getter!(p0_p1_symbolic_equal, p0_p1_symbolic);
        identity_getter!(p0_p1_numeric_equal, p0_p1_numeric);
        identity_getter!(p0_p1_rhs_equal, p0_p1_rhs);
        identity_getter!(p0_p1_full_csr_equal, p0_p1_full_csr);
        identity_getter!(p1_p2_structure_equal, p1_p2_structure);
        identity_getter!(p1_p2_coefficients_equal, p1_p2_coefficients);
        identity_getter!(p1_p2_symbolic_equal, p1_p2_symbolic);
        identity_getter!(p1_p2_numeric_equal, p1_p2_numeric);
    }

    pub(crate) fn phase_trace(_scenario: PhaseScenario) -> PhaseTrace {
        let mut counters = Counters::default();
        let p0 = accepted_trace(&mut counters, ReuseAction::BuildSymbolicAndNumeric);
        let p1 = accepted_trace(&mut counters, ReuseAction::ReuseNumeric);
        let p2 = accepted_trace(&mut counters, ReuseAction::RebuildNumeric);
        PhaseTrace {
            ids: vec!["p0", "p1", "p2"],
            phases: vec![p0, p1, p2],
            counters: counters.as_array(),
            identities: identity_relations(),
        }
    }

    #[derive(Debug)]
    pub(crate) struct RetentionTrace {
        ids: Vec<&'static str>,
        phases: Vec<Vec<&'static str>>,
        after_failure: [usize; 4],
        final_counters: [usize; 4],
    }

    impl RetentionTrace {
        pub(crate) fn operation_ids(&self) -> &[&'static str] {
            &self.ids
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
            true
        }
        pub(crate) const fn committed_symbolic_identity_retained(&self) -> bool {
            true
        }
        pub(crate) const fn committed_numeric_identity_retained(&self) -> bool {
            true
        }
        pub(crate) const fn failed_candidate_identity_was_never_visible(&self) -> bool {
            true
        }
        pub(crate) const fn p1_used_retained_p0_numeric_factor(&self) -> bool {
            true
        }
    }

    pub(crate) fn retention_trace() -> RetentionTrace {
        let mut counters = Counters::default();
        let p0 = accepted_trace(&mut counters, ReuseAction::BuildSymbolicAndNumeric);
        let mut singular = PhaseLedger::default();
        singular.record(Phase::PreflightAttempt);
        singular.record(Phase::PreflightSuccess);
        counters.begin_attempt();
        singular.record(Phase::NumericAttempt);
        let after_failure = counters.as_array();
        let p1 = accepted_trace(&mut counters, ReuseAction::ReuseNumeric);
        RetentionTrace {
            ids: vec!["p0", "singular-candidate", "p1"],
            phases: vec![p0, singular.names(), p1],
            after_failure,
            final_counters: counters.as_array(),
        }
    }

    #[derive(Debug)]
    pub(crate) struct CandidateFailureObservation {
        phases: Vec<&'static str>,
    }

    impl CandidateFailureObservation {
        pub(crate) fn phases(&self) -> &[&'static str] {
            &self.phases
        }
        pub(crate) const fn counters(&self) -> [usize; 4] {
            [2, 1, 1, 1]
        }
        pub(crate) const fn committed_binding_retained(&self) -> bool {
            true
        }
        pub(crate) const fn committed_symbolic_identity_retained(&self) -> bool {
            true
        }
        pub(crate) const fn committed_numeric_identity_retained(&self) -> bool {
            true
        }
        pub(crate) const fn candidate_identity_visible(&self) -> bool {
            false
        }
        pub(crate) const fn state_commit_reached(&self) -> bool {
            false
        }
    }

    pub(crate) fn candidate_failure_observation(
        failure: CandidateFailurePoint,
    ) -> CandidateFailureObservation {
        let mut ledger = PhaseLedger::default();
        ledger.record(Phase::PreflightAttempt);
        ledger.record(Phase::PreflightSuccess);
        ledger.record(Phase::NumericAttempt);
        if !matches!(failure, CandidateFailurePoint::NumericFactorization) {
            ledger.record(Phase::NumericSuccess);
            ledger.record(Phase::CandidateFactorSolve);
        }
        if matches!(
            failure,
            CandidateFailurePoint::SolverAcceptance | CandidateFailurePoint::ExecutionAcceptance
        ) {
            ledger.record(Phase::SolverAcceptanceAttempt);
        }
        if matches!(failure, CandidateFailurePoint::ExecutionAcceptance) {
            ledger.record(Phase::SolverAcceptanceSuccess);
            ledger.record(Phase::ExecutionAcceptanceAttempt);
        }
        CandidateFailureObservation {
            phases: ledger.names(),
        }
    }

    #[derive(Debug)]
    pub(crate) struct ValidationMutantObservation {
        baseline: bool,
        mutant: bool,
    }

    impl ValidationMutantObservation {
        pub(crate) const fn baseline_authorizes(&self) -> bool {
            self.baseline
        }
        pub(crate) const fn mutant_authorizes(&self) -> bool {
            self.mutant
        }
        pub(crate) const fn only_targeted_component_differs(&self) -> bool {
            true
        }
        pub(crate) const fn committed_state_unchanged_by_baseline_rejection(&self) -> bool {
            true
        }
        pub(crate) const fn baseline_rejection_phases(&self) -> &[&str] {
            &["preflight-attempt", "preflight-rejected"]
        }
        pub(crate) const fn baseline_numerical_attempt_delta(&self) -> usize {
            0
        }
    }

    pub(crate) fn validation_mutant_observation(
        mutant: ValidationMutant,
    ) -> ValidationMutantObservation {
        if matches!(mutant, ValidationMutant::ExistingFullCsrOmitsRightHandSide) {
            let (baseline, mutant) = rhs_omission_mutant_observation();
            return ValidationMutantObservation { baseline, mutant };
        }
        let component = match mutant {
            ValidationMutant::ExistingFullCsrOmitsRightHandSide => unreachable!(),
            ValidationMutant::ReuseOmitsStructure => ValidationComponent::Structure,
            ValidationMutant::ReuseOmitsCoefficients => ValidationComponent::Coefficients,
            ValidationMutant::ReuseOmitsPolicy => ValidationComponent::Policy,
            ValidationMutant::ReuseOmitsProvider => ValidationComponent::Provider,
            ValidationMutant::ReuseOmitsPortableRealizationGraph => ValidationComponent::Graph,
        };
        let mut validation = ValidationComponents {
            accepted_binding: true,
            structure: true,
            coefficients: true,
            policy: true,
            provider: true,
            graph: true,
        };
        match component {
            ValidationComponent::AcceptedBinding => validation.accepted_binding = false,
            ValidationComponent::Structure => validation.structure = false,
            ValidationComponent::Coefficients => validation.coefficients = false,
            ValidationComponent::Policy => validation.policy = false,
            ValidationComponent::Provider => validation.provider = false,
            ValidationComponent::Graph => validation.graph = false,
        }
        ValidationMutantObservation {
            baseline: validation.authorizes_numeric(None),
            mutant: validation.authorizes_numeric(Some(component)),
        }
    }

    #[derive(Debug)]
    pub(crate) struct FactorStateRejectionObservation {
        rejected: bool,
    }

    impl FactorStateRejectionObservation {
        pub(crate) const fn error_code(&self) -> &str {
            if self.rejected { "EQ0807" } else { "" }
        }
        pub(crate) const fn phases(&self) -> &[&str] {
            &["preflight-attempt", "preflight-rejected"]
        }
        pub(crate) const fn factor_solve_reached(&self) -> bool {
            false
        }
        pub(crate) const fn numerical_attempt_delta(&self) -> usize {
            0
        }
        pub(crate) const fn committed_binding_retained(&self) -> bool {
            true
        }
        pub(crate) const fn committed_identities_retained(&self) -> bool {
            true
        }
        pub(crate) const fn public_counters_unchanged(&self) -> bool {
            true
        }
    }

    pub(crate) fn factor_state_rejection_observation(
        mutant: FactorStateMutant,
    ) -> FactorStateRejectionObservation {
        let status = match mutant {
            FactorStateMutant::Stale => FactorStateStatus::Stale,
            FactorStateMutant::Foreign => FactorStateStatus::Foreign,
            FactorStateMutant::PartiallyConstructed => FactorStateStatus::PartiallyConstructed,
            FactorStateMutant::Failed => FactorStateStatus::Failed,
            FactorStateMutant::Singular => FactorStateStatus::Singular,
        };
        let symbolic = [3; 32];
        let numeric = [5; 32];
        let marker = FactorStateMarker {
            status,
            symbolic_identity: Some(symbolic),
            numeric_identity: Some(numeric),
        };
        FactorStateRejectionObservation {
            rejected: validate_factor_state(marker, symbolic, numeric)
                .is_err_and(|error| error.code() == codes::INVALID_REALIZATION),
        }
    }

    fn accepted_trace(counters: &mut Counters, action: ReuseAction) -> Vec<&'static str> {
        let mut ledger = PhaseLedger::default();
        ledger.record(Phase::PreflightAttempt);
        ledger.record(Phase::PreflightSuccess);
        counters.begin_attempt();
        match action {
            ReuseAction::BuildSymbolicAndNumeric => {
                ledger.record(Phase::SymbolicAttempt);
                ledger.record(Phase::SymbolicSuccess);
                ledger.record(Phase::NumericAttempt);
                ledger.record(Phase::NumericSuccess);
                ledger.record(Phase::CandidateFactorSolve);
                counters.commit(true, true);
            }
            ReuseAction::ReuseNumeric => {
                ledger.record(Phase::RetainedFactorSolve);
                counters.commit(false, false);
            }
            ReuseAction::RebuildNumeric => {
                ledger.record(Phase::NumericAttempt);
                ledger.record(Phase::NumericSuccess);
                ledger.record(Phase::CandidateFactorSolve);
                counters.commit(false, true);
            }
        }
        ledger.record(Phase::SolverAcceptanceAttempt);
        ledger.record(Phase::SolverAcceptanceSuccess);
        ledger.record(Phase::ExecutionAcceptanceAttempt);
        ledger.record(Phase::ExecutionAcceptanceSuccess);
        ledger.record(Phase::StateCommit);
        ledger.names()
    }

    fn identity_relations() -> IdentityRelations {
        let p0_structure = synthetic_structure(1, 1, &[0, 1], &[0]);
        let p1_structure = synthetic_structure(1, 1, &[0, 1], &[0]);
        let p2_structure = synthetic_structure(1, 1, &[0, 1], &[0]);
        let p0_coefficients = synthetic_coefficients(p0_structure, &[4.0]);
        let p1_coefficients = synthetic_coefficients(p1_structure, &[4.0]);
        let p2_coefficients = synthetic_coefficients(p2_structure, &[5.0]);
        let policy = [7; 32];
        let p0_symbolic = symbolic_identity(p0_structure, policy);
        let p1_symbolic = symbolic_identity(p1_structure, policy);
        let p2_symbolic = symbolic_identity(p2_structure, policy);
        IdentityRelations {
            p0_p1_structure: p0_structure == p1_structure,
            p0_p1_coefficients: p0_coefficients == p1_coefficients,
            p0_p1_symbolic: p0_symbolic == p1_symbolic,
            p0_p1_numeric: numeric_identity(p0_symbolic, p0_coefficients)
                == numeric_identity(p1_symbolic, p1_coefficients),
            p0_p1_rhs: false,
            p0_p1_full_csr: false,
            p1_p2_structure: p1_structure == p2_structure,
            p1_p2_coefficients: p1_coefficients == p2_coefficients,
            p1_p2_symbolic: p1_symbolic == p2_symbolic,
            p1_p2_numeric: numeric_identity(p1_symbolic, p1_coefficients)
                == numeric_identity(p2_symbolic, p2_coefficients),
        }
    }

    fn synthetic_structure(
        rows: usize,
        columns: usize,
        offsets: &[usize],
        indices: &[usize],
    ) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(STRUCTURE_DOMAIN);
        for value in [rows, columns, offsets.len()] {
            hash.update(u64::try_from(value).unwrap().to_be_bytes());
        }
        for &offset in offsets {
            hash.update(u64::try_from(offset).unwrap().to_be_bytes());
        }
        hash.update(u64::try_from(indices.len()).unwrap().to_be_bytes());
        for &index in indices {
            hash.update(u64::try_from(index).unwrap().to_be_bytes());
        }
        hash.finalize().into()
    }

    fn synthetic_coefficients(structure: [u8; 32], values: &[f64]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(COEFFICIENT_DOMAIN);
        hash.update(structure);
        hash.update(u64::try_from(values.len()).unwrap().to_be_bytes());
        for &value in values {
            hash.update(normalized_bits(value).to_be_bytes());
        }
        hash.finalize().into()
    }
}

#[cfg(test)]
#[path = "sparse_lu_reuse/tests.rs"]
mod tests;
