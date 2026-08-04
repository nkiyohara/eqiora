use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{
    BackendId, ExecutionProvider, LinearOperatorOrientation, LinearOperatorProperties,
    LinearProblem, LinearSolution, LinearSolveRequest, LinearSolver, PreconditionerPolicy,
    ProviderLibrary, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapability,
    SolverPlan, SolverProvider,
};

const POLICY_ID: &str = "eqiora.host-serial-solver-planning/v1";

const REFERENCE_ID: &str = "eqiora.reference.bicgstab-general-jacobi-reproducible-f64";
const FAER_BICGSTAB_ID: &str = "eqiora.faer.bicgstab-general-jacobi-fast-f64";
const FAER_SPARSE_LU_ID: &str = "eqiora.faer.sparse-lu-general-identity-fast-f64";

const REFERENCE_EVIDENCE: &str = "fluid.cartesian-advection-diffusion-fvm-2d";
const FAER_EVIDENCE: &str = "numerics.linear-backends";

const EMPTY_LIBRARIES: &[ProviderLibrary] = &[];
const FAER_LIBRARIES: &[ProviderLibrary] = &[ProviderLibrary::new("faer", "0.24.4")];

const REFERENCE_PROVIDER: SolverProvider = SolverProvider::new(
    BackendId::new("eqiora.reference"),
    "0.1.0-alpha.1",
    EMPTY_LIBRARIES,
);
const FAER_PROVIDER: SolverProvider = SolverProvider::new(
    BackendId::new("eqiora.faer"),
    "0.1.0-alpha.1",
    FAER_LIBRARIES,
);

const RELATIVE_TOLERANCE_BITS: u64 = 0x3d71_9799_812d_ea11;
const ABSOLUTE_TOLERANCE_BITS: u64 = 0x3d06_849b_86a1_2b9b;
const MAXIMUM_ITERATIONS: usize = 100;

/// Deterministic preference table used by bounded host-serial solver planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverPlanningObjective {
    /// Prefer the frozen reproducible-reduction candidate.
    Robust,
    /// Prefer Fast reduction and then the frozen direct candidate.
    Fast,
    /// Prefer the frozen fixed-vector Krylov execution shape.
    LowMemory,
}

/// One untrusted member of the frozen host-serial solver catalog.
#[derive(Debug, Clone, Copy)]
pub struct HostSerialSolverCandidate<'backend> {
    id: &'static str,
    evidence_case: &'static str,
    request: LinearSolveRequest<'backend>,
}

impl<'backend> HostSerialSolverCandidate<'backend> {
    /// Bind a candidate identity and evidence claim to an executable request.
    ///
    /// Construction performs no admission. [`resolve_host_serial_solver_v1`]
    /// validates the complete catalog before ranking.
    #[must_use]
    pub const fn new(
        id: &'static str,
        evidence_case: &'static str,
        request: LinearSolveRequest<'backend>,
    ) -> Self {
        Self {
            id,
            evidence_case,
            request,
        }
    }

    /// Frozen catalog identity supplied by the caller.
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Registered evidence identity supplied by the caller.
    #[must_use]
    pub const fn evidence_case(self) -> &'static str {
        self.evidence_case
    }

    /// Exact backend and solver plan supplied by the caller.
    #[must_use]
    pub const fn request(self) -> LinearSolveRequest<'backend> {
        self.request
    }
}

/// One inspected decision bound to the exact problem against which it resolved.
#[derive(Debug)]
pub struct HostSerialSolverDecision<'problem, 'backend> {
    problem: &'problem LinearProblem<'problem>,
    objective: SolverPlanningObjective,
    selected: HostSerialSolverCandidate<'backend>,
    solver_provider: SolverProvider,
    reasons: Vec<(&'static str, &'static str)>,
}

impl<'problem, 'backend> HostSerialSolverDecision<'problem, 'backend> {
    /// Frozen objective used to rank the admitted candidates.
    #[must_use]
    pub const fn objective(&self) -> SolverPlanningObjective {
        self.objective
    }

    /// Versioned deterministic planning-policy identity.
    #[must_use]
    pub const fn policy_id(&self) -> &'static str {
        POLICY_ID
    }

    /// Exact selected catalog member.
    #[must_use]
    pub const fn selected(&self) -> HostSerialSolverCandidate<'backend> {
        self.selected
    }

    /// Exact problem borrowed during resolution.
    #[must_use]
    pub const fn problem(&self) -> &'problem LinearProblem<'problem> {
        self.problem
    }

    /// Exact selected solver provider release.
    #[must_use]
    pub const fn solver_provider(&self) -> SolverProvider {
        self.solver_provider
    }

    /// Frozen host-serial execution provider.
    #[must_use]
    pub const fn execution_provider(&self) -> ExecutionProvider {
        SERIAL_EXECUTION_PROVIDER
    }

    /// Stable candidate-ID/reason-code trace in ascending candidate-ID order.
    pub fn reasons(&self) -> impl ExactSizeIterator<Item = (&'static str, &'static str)> + '_ {
        self.reasons.iter().copied()
    }

    /// Execute exactly the selected request against the resolved problem.
    ///
    /// # Errors
    /// Returns the selected backend's capability or numerical diagnostic. No
    /// retry, fallback, plan mutation, or problem substitution is performed.
    pub fn solve(&self) -> Result<LinearSolution, Diagnostic> {
        self.selected.request.solve(self.problem)
    }
}

#[derive(Debug, Clone, Copy)]
struct CandidateEvaluation<'backend> {
    candidate: HostSerialSolverCandidate<'backend>,
    rejection: Option<&'static str>,
}

/// Resolve one exact candidate from the frozen v1 host-serial catalog.
///
/// # Errors
/// Returns `EQ0807` when inventory, common controls, catalog identity, problem
/// profile, or exact backend capability admission fails. Resolution performs
/// no numerical operator action and executes no backend.
pub fn resolve_host_serial_solver_v1<'problem, 'backend>(
    problem: &'problem LinearProblem<'problem>,
    objective: SolverPlanningObjective,
    candidates: &[HostSerialSolverCandidate<'backend>],
) -> Result<HostSerialSolverDecision<'problem, 'backend>, Diagnostic> {
    validate_inventory(candidates)?;
    validate_common_controls(candidates)?;

    let mut ordered = candidates.to_vec();
    ordered.sort_by_key(|candidate| candidate.id());
    let evaluations = ordered
        .into_iter()
        .map(|candidate| CandidateEvaluation {
            candidate,
            rejection: rejection_reason(problem, candidate),
        })
        .collect::<Vec<_>>();

    let mut admitted = evaluations
        .iter()
        .filter(|evaluation| evaluation.rejection.is_none())
        .map(|evaluation| evaluation.candidate)
        .collect::<Vec<_>>();
    if admitted.is_empty() {
        let trace = evaluations
            .iter()
            .map(|evaluation| {
                (
                    evaluation.candidate.id(),
                    evaluation
                        .rejection
                        .expect("a no-admitted trace contains only rejected candidates"),
                )
            })
            .collect::<Vec<_>>();
        return Err(no_admitted(trace.as_slice()));
    }

    admitted.sort_by_key(|candidate| rank_key(objective, *candidate));
    let selected = admitted[0];
    let solver_provider = selected.request().backend().provider();
    let selected_id = selected.id();
    let selected_reason = selected_reason(objective);
    let mut reasons = Vec::with_capacity(evaluations.len() * 2);
    for evaluation in &evaluations {
        if let Some(rejection) = evaluation.rejection {
            reasons.push((evaluation.candidate.id(), rejection));
        } else {
            reasons.push((evaluation.candidate.id(), "candidate.admitted"));
            reasons.push((
                evaluation.candidate.id(),
                if evaluation.candidate.id() == selected_id {
                    selected_reason
                } else {
                    "candidate.not-selected"
                },
            ));
        }
    }

    Ok(HostSerialSolverDecision {
        problem,
        objective,
        selected,
        solver_provider,
        reasons,
    })
}

fn validate_inventory(candidates: &[HostSerialSolverCandidate<'_>]) -> Result<(), Diagnostic> {
    let mut reference = 0_usize;
    let mut faer_bicgstab = 0_usize;
    let mut faer_sparse_lu = 0_usize;
    let mut unknown = false;
    for candidate in candidates {
        match candidate.id() {
            REFERENCE_ID => reference += 1,
            FAER_BICGSTAB_ID => faer_bicgstab += 1,
            FAER_SPARSE_LU_ID => faer_sparse_lu += 1,
            _ => unknown = true,
        }
    }
    if reference == 0 || faer_bicgstab == 0 || faer_sparse_lu == 0 {
        return Err(invalid_catalog("catalog.missing-id"));
    }
    if reference > 1 || faer_bicgstab > 1 || faer_sparse_lu > 1 {
        return Err(invalid_catalog("catalog.duplicate-id"));
    }
    if unknown {
        return Err(invalid_catalog("catalog.unknown-id"));
    }
    Ok(())
}

fn validate_common_controls(
    candidates: &[HostSerialSolverCandidate<'_>],
) -> Result<(), Diagnostic> {
    let controls_match = candidates.iter().all(|candidate| {
        let plan = candidate.request().plan();
        plan.relative_tolerance().to_bits() == RELATIVE_TOLERANCE_BITS
            && plan.absolute_tolerance().to_bits() == ABSOLUTE_TOLERANCE_BITS
            && plan.maximum_iterations().get() == MAXIMUM_ITERATIONS
    });
    if !controls_match {
        return Err(invalid_catalog("catalog.control-mismatch"));
    }
    Ok(())
}

fn rejection_reason(
    problem: &LinearProblem<'_>,
    candidate: HostSerialSolverCandidate<'_>,
) -> Option<&'static str> {
    let expected = expected_candidate(candidate.id());
    if candidate.evidence_case() != expected.evidence_case {
        return Some("catalog.evidence-mismatch");
    }
    if candidate.request().backend().provider() != expected.provider {
        return Some("catalog.provider-mismatch");
    }
    if !plan_tuple_matches(candidate.request().plan(), expected) {
        return Some("catalog.plan-mismatch");
    }
    if problem.properties() != LinearOperatorProperties::General {
        return Some("profile.general-required");
    }
    if problem.operator().orientation() != LinearOperatorOrientation::Normal {
        return Some("profile.normal-required");
    }
    let Some(system) = problem.canonical_csr_system() else {
        return Some("profile.canonical-csr-required");
    };
    if !has_complete_diagonal(system.rows(), system.row_offsets(), system.column_indices()) {
        return Some("profile.complete-diagonal-required");
    }
    let required = SolverCapability {
        algorithm: expected.algorithm,
        operator_properties: LinearOperatorProperties::General,
        preconditioner: expected.preconditioner,
        reduction: expected.reduction,
        scalar_type: ScalarType::F64,
    };
    if !candidate
        .request()
        .backend()
        .capabilities()
        .combinations()
        .contains(&required)
    {
        return Some("capability.exact-tuple-required");
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct ExpectedCandidate {
    evidence_case: &'static str,
    provider: SolverProvider,
    algorithm: LinearSolver,
    preconditioner: PreconditionerPolicy,
    reduction: ReductionPolicy,
}

fn expected_candidate(id: &str) -> ExpectedCandidate {
    match id {
        REFERENCE_ID => ExpectedCandidate {
            evidence_case: REFERENCE_EVIDENCE,
            provider: REFERENCE_PROVIDER,
            algorithm: LinearSolver::BiConjugateGradientStabilized,
            preconditioner: PreconditionerPolicy::Jacobi,
            reduction: ReductionPolicy::Reproducible,
        },
        FAER_BICGSTAB_ID => ExpectedCandidate {
            evidence_case: FAER_EVIDENCE,
            provider: FAER_PROVIDER,
            algorithm: LinearSolver::BiConjugateGradientStabilized,
            preconditioner: PreconditionerPolicy::Jacobi,
            reduction: ReductionPolicy::Fast,
        },
        FAER_SPARSE_LU_ID => ExpectedCandidate {
            evidence_case: FAER_EVIDENCE,
            provider: FAER_PROVIDER,
            algorithm: LinearSolver::SparseLu,
            preconditioner: PreconditionerPolicy::Identity,
            reduction: ReductionPolicy::Fast,
        },
        _ => unreachable!("inventory validation rejects unknown candidate IDs"),
    }
}

fn plan_tuple_matches(plan: SolverPlan, expected: ExpectedCandidate) -> bool {
    plan.algorithm() == expected.algorithm
        && plan.preconditioner() == expected.preconditioner
        && plan.reduction() == expected.reduction
}

fn has_complete_diagonal(rows: usize, row_offsets: &[usize], column_indices: &[usize]) -> bool {
    (0..rows).all(|row| {
        let start = row_offsets[row];
        let end = row_offsets[row + 1];
        column_indices[start..end].contains(&row)
    })
}

fn rank_key(
    objective: SolverPlanningObjective,
    candidate: HostSerialSolverCandidate<'_>,
) -> (u8, u8, &'static str) {
    let plan = candidate.request().plan();
    match objective {
        SolverPlanningObjective::Robust => (
            u8::from(plan.reduction() != ReductionPolicy::Reproducible),
            0,
            candidate.id(),
        ),
        SolverPlanningObjective::Fast => (
            u8::from(plan.reduction() != ReductionPolicy::Fast),
            u8::from(plan.algorithm() != LinearSolver::SparseLu),
            candidate.id(),
        ),
        SolverPlanningObjective::LowMemory => (
            u8::from(plan.algorithm() != LinearSolver::BiConjugateGradientStabilized),
            0,
            candidate.id(),
        ),
    }
}

const fn selected_reason(objective: SolverPlanningObjective) -> &'static str {
    match objective {
        SolverPlanningObjective::Robust => "candidate.selected.robust-reproducible",
        SolverPlanningObjective::Fast => "candidate.selected.fast-direct",
        SolverPlanningObjective::LowMemory => "candidate.selected.low-memory-krylov",
    }
}

fn invalid_catalog(fragment: &'static str) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_REALIZATION,
        format!("{POLICY_ID} rejected catalog: {fragment}"),
    )
}

fn no_admitted(trace: &[(&str, &str)]) -> Diagnostic {
    let rendered = trace
        .iter()
        .map(|(candidate_id, reason)| format!("{candidate_id}={reason}"))
        .collect::<Vec<_>>()
        .join(",");
    Diagnostic::error(
        codes::INVALID_REALIZATION,
        format!("{POLICY_ID} no admitted candidate; trace=[{rendered}]"),
    )
}

#[cfg(test)]
mod tests;
