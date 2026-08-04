use std::cell::Cell;
use std::num::NonZeroUsize;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_execution::{AcceptedLinearExecution, AdmittedExecution, DeploymentBinding};
use eqiora_solver::{
    ConvergenceReason, ExecutionReport, LinearOperator, LinearOperatorOrientation, LinearSolution,
    LinearSolver, PreconditionerPolicy, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, SolverPlan,
    accept_linear_solution,
};

use crate::FAER_SOLVER_PROVIDER;
use crate::sparse_lu::fixed_residual_norm;
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
        self.execute_core(LiveExecution::new(admitted))
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

    pub(crate) fn execute_core<E: ReuseExecution>(
        &mut self,
        mut execution: E,
    ) -> Result<E::Accepted, Diagnostic> {
        self.last_phases.restart();
        self.last_phases.record(Phase::PreflightAttempt);
        let input = match execution.preflight_input(self.plan) {
            Ok(input) => input,
            Err(error) => {
                self.last_phases.record(Phase::PreflightRejected);
                return Err(error);
            }
        };
        let preflight = match self.preflight(
            input,
            execution.validation_omission(),
            execution.requires_numeric_reuse_validation(),
        ) {
            Ok(preflight) => preflight,
            Err(error) => {
                self.last_phases.record(Phase::PreflightRejected);
                return Err(error);
            }
        };
        self.last_phases.record(Phase::PreflightSuccess);
        self.counters.begin_attempt();

        let (candidate_solution, candidate_factors) = match preflight.action {
            ReuseAction::BuildSymbolicAndNumeric => {
                self.last_phases.record(Phase::SymbolicAttempt);
                let symbolic = execution.factor_symbolic()?;
                self.last_phases.record(Phase::SymbolicSuccess);
                self.last_phases.record(Phase::NumericAttempt);
                let numeric = execution.factor_numeric(&symbolic)?;
                self.last_phases.record(Phase::NumericSuccess);
                self.last_phases.record(Phase::CandidateFactorSolve);
                let solution = execution.solve(&symbolic, &numeric)?;
                (
                    solution,
                    CandidateFactors::SymbolicAndNumeric(symbolic, numeric),
                )
            }
            ReuseAction::ReuseNumeric => {
                self.last_phases.record(Phase::RetainedFactorSolve);
                let ready = self.ready_state()?;
                let solution = execution.solve(&ready.symbolic_factor, &ready.numeric_factor)?;
                (solution, CandidateFactors::Retained)
            }
            ReuseAction::RebuildNumeric => {
                self.last_phases.record(Phase::NumericAttempt);
                let numeric = {
                    let ready = self.ready_state()?;
                    execution.factor_numeric(&ready.symbolic_factor)?
                };
                self.last_phases.record(Phase::NumericSuccess);
                self.last_phases.record(Phase::CandidateFactorSolve);
                let solution = {
                    let ready = self.ready_state()?;
                    execution.solve(&ready.symbolic_factor, &numeric)?
                };
                (solution, CandidateFactors::Numeric(numeric))
            }
        };

        self.last_phases.record(Phase::SolverAcceptanceAttempt);
        let solver_accepted = execution.accept_solver(candidate_solution)?;
        self.last_phases.record(Phase::SolverAcceptanceSuccess);
        self.last_phases.record(Phase::ExecutionAcceptanceAttempt);
        let accepted = execution.accept_execution(solver_accepted)?;
        self.last_phases.record(Phase::ExecutionAcceptanceSuccess);
        self.commit(preflight, candidate_factors)?;
        self.last_phases.record(Phase::StateCommit);
        Ok(accepted)
    }

    fn preflight(
        &self,
        input: PreflightInput,
        omission: Option<ValidationComponent>,
        requires_numeric_reuse_validation: bool,
    ) -> Result<Preflight, Diagnostic> {
        if self.counters.attempted >= self.maximum_attempts.get() {
            return Err(invalid_realization(
                "faer sparse LU reuse owner exhausted its numerical attempt capacity",
            ));
        }
        let action = match &self.state {
            ReuseState::Empty => ReuseAction::BuildSymbolicAndNumeric,
            ReuseState::Ready(ready) => {
                validate_factor_state(
                    ready.factor_state,
                    ready.symbolic_identity,
                    ready.numeric_identity,
                )?;
                let validation = ValidationComponents::between(ready, &input);
                if requires_numeric_reuse_validation
                    && !validation.authorizes_numeric(omission)
                {
                    return Err(invalid_realization(
                        "faer sparse LU numeric reuse validation rejected a changed component",
                    ));
                }
                validation.classify(omission)?
            }
        };
        Ok(Preflight {
            binding: input.binding,
            identities: input.identities,
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

    #[cfg(test)]
    pub(crate) fn phase_names(&self) -> Vec<&'static str> {
        self.last_phases.names()
    }

    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> OwnerSnapshot {
        let mut snapshot = OwnerSnapshot {
            counters: self.counters.as_array(),
            binding: None,
            symbolic_identity: self.symbolic_reuse_identity(),
            numeric_identity: self.numeric_reuse_identity(),
            symbolic_factor: None,
            numeric_factor: None,
        };
        if let ReuseState::Ready(ready) = &self.state {
            if let ReuseBinding::Synthetic(binding) = ready.binding {
                snapshot.binding = Some(binding);
            }
            snapshot.symbolic_factor = ready.symbolic_factor.synthetic_token();
            snapshot.numeric_factor = ready.numeric_factor.synthetic_token();
        }
        snapshot
    }

    #[cfg(test)]
    pub(crate) fn validation_for(&self, input: &PreflightInput) -> ValidationComponents {
        let ReuseState::Ready(ready) = &self.state else {
            panic!("validation observation requires one committed state");
        };
        ValidationComponents::between(ready, input)
    }

    #[cfg(test)]
    pub(crate) fn inject_factor_state(&mut self, mutant: InjectedFactorState) {
        let ReuseState::Ready(ready) = &mut self.state else {
            panic!("factor-state injection requires one committed state");
        };
        match mutant {
            InjectedFactorState::Stale => ready.factor_state.status = FactorStateStatus::Stale,
            InjectedFactorState::Foreign => {
                ready.factor_state.symbolic_identity = Some([0xF0; 32]);
            }
            InjectedFactorState::PartiallyConstructed => {
                ready.factor_state.numeric_identity = None;
            }
            InjectedFactorState::Failed => ready.factor_state.status = FactorStateStatus::Failed,
            InjectedFactorState::Singular => {
                ready.factor_state.status = FactorStateStatus::Singular;
            }
        }
    }
}

struct LiveExecution<'system> {
    admitted: Option<AdmittedExecution<'system>>,
}

impl<'system> LiveExecution<'system> {
    fn new(admitted: AdmittedExecution<'system>) -> Self {
        Self {
            admitted: Some(admitted),
        }
    }

    fn admitted(&self) -> &AdmittedExecution<'system> {
        self.admitted
            .as_ref()
            .expect("live execution remains available until execution acceptance")
    }
}

impl ReuseExecution for LiveExecution<'_> {
    type CandidateSolution = Vec<f64>;
    type SolverAccepted = LinearSolution;
    type Accepted = AcceptedLinearExecution;

    fn preflight_input(&self, owner_plan: SolverPlan) -> Result<PreflightInput, Diagnostic> {
        let admitted = self.admitted();
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
        if binding.solver_plan() != owner_plan || admitted.solver_plan() != owner_plan {
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
        Ok(PreflightInput {
            binding: ReuseBinding::Live(binding.clone()),
            identities: identities(system, owner_plan, binding.solver_provider())?,
        })
    }

    fn factor_symbolic(&mut self) -> Result<StoredSymbolicFactor, Diagnostic> {
        Ok(StoredSymbolicFactor::Live(factor_symbolic(
            self.admitted().system(),
        )?))
    }

    fn factor_numeric(
        &mut self,
        symbolic: &StoredSymbolicFactor,
    ) -> Result<StoredNumericFactor, Diagnostic> {
        let symbolic = match symbolic {
            StoredSymbolicFactor::Live(symbolic) => symbolic,
            #[cfg(test)]
            StoredSymbolicFactor::Synthetic(_) => {
                return Err(invalid_realization(
                    "live execution received synthetic symbolic state",
                ));
            }
        };
        Ok(StoredNumericFactor::Live(factor_numeric(
            symbolic,
            self.admitted().system(),
        )?))
    }

    fn solve(
        &mut self,
        symbolic: &StoredSymbolicFactor,
        numeric: &StoredNumericFactor,
    ) -> Result<Self::CandidateSolution, Diagnostic> {
        let symbolic = match symbolic {
            StoredSymbolicFactor::Live(symbolic) => symbolic,
            #[cfg(test)]
            StoredSymbolicFactor::Synthetic(_) => {
                return Err(invalid_realization(
                    "live execution received synthetic factor state",
                ));
            }
        };
        let numeric = match numeric {
            StoredNumericFactor::Live(numeric) => numeric,
            #[cfg(test)]
            StoredNumericFactor::Synthetic(_) => {
                return Err(invalid_realization(
                    "live execution received synthetic factor state",
                ));
            }
        };
        solve_factored(
            symbolic,
            numeric,
            self.admitted().system().right_hand_side(),
        )
    }

    fn accept_solver(
        &mut self,
        values: Self::CandidateSolution,
    ) -> Result<Self::SolverAccepted, Diagnostic> {
        let system = self.admitted().system();
        let problem = system.linear_problem()?;
        let reported_residual_norm = fixed_residual_norm(&problem, &values)?;
        accept_linear_solution(
            &problem,
            self.admitted().solver_plan(),
            FAER_SOLVER_PROVIDER,
            ConvergenceReason::ResidualToleranceSatisfied,
            1,
            reported_residual_norm,
            values,
        )
    }

    fn accept_execution(
        &mut self,
        solution: Self::SolverAccepted,
    ) -> Result<Self::Accepted, Diagnostic> {
        self.admitted
            .take()
            .expect("live execution accepts exactly once")
            .accept(solution)
    }
}

pub(crate) trait ReuseExecution {
    type CandidateSolution;
    type SolverAccepted;
    type Accepted;

    fn preflight_input(&self, owner_plan: SolverPlan) -> Result<PreflightInput, Diagnostic>;
    fn factor_symbolic(&mut self) -> Result<StoredSymbolicFactor, Diagnostic>;
    fn factor_numeric(
        &mut self,
        symbolic: &StoredSymbolicFactor,
    ) -> Result<StoredNumericFactor, Diagnostic>;
    fn solve(
        &mut self,
        symbolic: &StoredSymbolicFactor,
        numeric: &StoredNumericFactor,
    ) -> Result<Self::CandidateSolution, Diagnostic>;
    fn accept_solver(
        &mut self,
        candidate: Self::CandidateSolution,
    ) -> Result<Self::SolverAccepted, Diagnostic>;
    fn accept_execution(
        &mut self,
        accepted: Self::SolverAccepted,
    ) -> Result<Self::Accepted, Diagnostic>;
    fn validation_omission(&self) -> Option<ValidationComponent> {
        None
    }
    fn requires_numeric_reuse_validation(&self) -> bool {
        false
    }
}

#[derive(Debug)]
enum ReuseState {
    Empty,
    Ready(ReadyState),
}

#[derive(Debug)]
struct ReadyState {
    binding: ReuseBinding,
    structure_identity: [u8; 32],
    coefficient_identity: [u8; 32],
    policy_identity: [u8; 32],
    symbolic_identity: [u8; 32],
    numeric_identity: [u8; 32],
    symbolic_factor: StoredSymbolicFactor,
    numeric_factor: StoredNumericFactor,
    factor_state: FactorStateMarker,
}

#[derive(Debug, Clone)]
pub(crate) enum ReuseBinding {
    Live(DeploymentBinding),
    #[cfg(test)]
    Synthetic(SyntheticBinding),
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SyntheticBinding {
    pub(crate) shell: u64,
    pub(crate) provider: u64,
    pub(crate) graph: u64,
}

#[derive(Debug)]
pub(crate) enum StoredSymbolicFactor {
    Live(SparseLuSymbolicFactor),
    #[cfg(test)]
    Synthetic(u64),
}

impl StoredSymbolicFactor {
    #[cfg(test)]
    fn synthetic_token(&self) -> Option<u64> {
        match self {
            Self::Synthetic(token) => Some(*token),
            Self::Live(_) => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum StoredNumericFactor {
    Live(SparseLuNumericFactor),
    #[cfg(test)]
    Synthetic(u64),
}

impl StoredNumericFactor {
    #[cfg(test)]
    fn synthetic_token(&self) -> Option<u64> {
        match self {
            Self::Synthetic(token) => Some(*token),
            Self::Live(_) => None,
        }
    }
}

#[derive(Debug)]
enum CandidateFactors {
    SymbolicAndNumeric(StoredSymbolicFactor, StoredNumericFactor),
    Numeric(StoredNumericFactor),
    Retained,
}

#[derive(Debug)]
struct Preflight {
    binding: ReuseBinding,
    identities: IdentitySet,
    action: ReuseAction,
}

#[derive(Debug, Clone)]
pub(crate) struct PreflightInput {
    pub(crate) binding: ReuseBinding,
    pub(crate) identities: IdentitySet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReuseAction {
    BuildSymbolicAndNumeric,
    ReuseNumeric,
    RebuildNumeric,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ValidationComponents {
    accepted_binding: bool,
    structure: bool,
    coefficients: bool,
    policy: bool,
    provider: bool,
    graph: bool,
}

impl ValidationComponents {
    fn between(ready: &ReadyState, input: &PreflightInput) -> Self {
        let (accepted_binding, provider, graph) = match (&ready.binding, &input.binding) {
            (ReuseBinding::Live(left), ReuseBinding::Live(right)) => (
                binding_shell_equal(left, right),
                left.solver_provider() == right.solver_provider(),
                left.realization() == right.realization(),
            ),
            #[cfg(test)]
            (ReuseBinding::Synthetic(left), ReuseBinding::Synthetic(right)) => (
                left.shell == right.shell,
                left.provider == right.provider,
                left.graph == right.graph,
            ),
            #[cfg(test)]
            (ReuseBinding::Live(_), ReuseBinding::Synthetic(_))
            | (ReuseBinding::Synthetic(_), ReuseBinding::Live(_)) => (false, false, false),
        };
        Self {
            accepted_binding,
            structure: ready.structure_identity == input.identities.structure,
            coefficients: ready.coefficient_identity == input.identities.coefficients,
            policy: ready.policy_identity == input.identities.policy,
            provider,
            graph,
        }
    }

    fn classify(
        self,
        omission: Option<ValidationComponent>,
    ) -> Result<ReuseAction, Diagnostic> {
        let includes = |component, value| omission == Some(component) || value;
        if !(includes(ValidationComponent::AcceptedBinding, self.accepted_binding)
            && includes(ValidationComponent::Structure, self.structure)
            && includes(ValidationComponent::Policy, self.policy)
            && includes(ValidationComponent::Provider, self.provider)
            && includes(ValidationComponent::Graph, self.graph))
        {
            return Err(invalid_realization(
                "faer sparse LU reuse binding, structure, policy, provider, or graph changed",
            ));
        }
        Ok(if includes(ValidationComponent::Coefficients, self.coefficients) {
            ReuseAction::ReuseNumeric
        } else {
            ReuseAction::RebuildNumeric
        })
    }

    pub(crate) fn authorizes_numeric(self, omission: Option<ValidationComponent>) -> bool {
        let includes = |component, value| omission == Some(component) || value;
        includes(ValidationComponent::AcceptedBinding, self.accepted_binding)
            && includes(ValidationComponent::Structure, self.structure)
            && includes(ValidationComponent::Coefficients, self.coefficients)
            && includes(ValidationComponent::Policy, self.policy)
            && includes(ValidationComponent::Provider, self.provider)
            && includes(ValidationComponent::Graph, self.graph)
    }

    #[cfg(test)]
    pub(crate) fn difference_count(self) -> usize {
        [
            self.accepted_binding,
            self.structure,
            self.coefficients,
            self.policy,
            self.provider,
            self.graph,
        ]
        .into_iter()
        .filter(|equal| !equal)
        .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationComponent {
    AcceptedBinding,
    Structure,
    Coefficients,
    Policy,
    Provider,
    Graph,
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
        self.phases
            .iter()
            .map(|phase| match phase {
                Phase::PreflightAttempt => "preflight-attempt",
                Phase::PreflightSuccess => "preflight-success",
                Phase::PreflightRejected => "preflight-rejected",
                Phase::SymbolicAttempt => "symbolic-attempt",
                Phase::SymbolicSuccess => "symbolic-success",
                Phase::NumericAttempt => "numeric-attempt",
                Phase::NumericSuccess => "numeric-success",
                Phase::RetainedFactorSolve => "retained-factor-solve",
                Phase::CandidateFactorSolve => "candidate-factor-solve",
                Phase::SolverAcceptanceAttempt => "solver-acceptance-attempt",
                Phase::SolverAcceptanceSuccess => "solver-acceptance-success",
                Phase::ExecutionAcceptanceAttempt => "execution-acceptance-attempt",
                Phase::ExecutionAcceptanceSuccess => "execution-acceptance-success",
                Phase::StateCommit => "state-commit",
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactorStateStatus {
    Ready,
    #[cfg(test)]
    Stale,
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

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum InjectedFactorState {
    Stale,
    Foreign,
    PartiallyConstructed,
    Failed,
    Singular,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OwnerSnapshot {
    pub(crate) counters: [usize; 4],
    pub(crate) binding: Option<SyntheticBinding>,
    pub(crate) symbolic_identity: Option<[u8; 32]>,
    pub(crate) numeric_identity: Option<[u8; 32]>,
    pub(crate) symbolic_factor: Option<u64>,
    pub(crate) numeric_factor: Option<u64>,
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
pub(crate) use crate::sparse_lu_factor::test_support;

#[cfg(test)]
#[path = "sparse_lu_reuse/tests.rs"]
mod tests;
