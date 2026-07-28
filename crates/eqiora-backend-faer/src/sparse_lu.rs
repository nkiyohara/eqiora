use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::{
    ConvergenceReason, FixedOrderInnerProduct, LinearOperatorOrientation, LinearProblem,
    LinearSolution, ReplicatedLinearExecution, SERIAL_LINEAR_EXECUTION, SolverPlan, SolverProvider,
    accept_linear_solution,
};
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{NumericLu, factorize_symbolic_lu};
use faer::sparse::{SparseRowMat, SymbolicSparseRowMat};
use faer::{Conj, Mat, Par};

pub(super) fn solve_sparse_lu(
    provider: SolverProvider,
    problem: &LinearProblem<'_>,
    plan: SolverPlan,
) -> Result<LinearSolution, Diagnostic> {
    if problem.operator().orientation() != LinearOperatorOrientation::Normal {
        return Err(invalid_realization(
            "faer sparse LU requires a normal-orientation canonical CSR problem",
        ));
    }
    let system = problem.canonical_csr_system().ok_or_else(|| {
        invalid_realization(
            "faer sparse LU requires a LinearProblem created from CanonicalCsrSystemView",
        )
    })?;

    let initial = problem
        .initial_guess()
        .map_or_else(|| vec![0.0; system.columns()], <[f64]>::to_vec);
    let initial_residual_norm = fixed_residual_norm(problem, &initial)?;
    let right_hand_side_norm = fixed_norm(problem.right_hand_side())?;
    let residual_target = plan.residual_target(right_hand_side_norm)?;
    if initial_residual_norm <= residual_target {
        return accept_linear_solution(
            problem,
            plan,
            provider,
            ConvergenceReason::InitialResidualSatisfied,
            0,
            initial_residual_norm,
            initial,
        );
    }

    let values = factor_and_solve(system)?;
    let reported_residual_norm = fixed_residual_norm(problem, &values)?;
    accept_linear_solution(
        problem,
        plan,
        provider,
        ConvergenceReason::ResidualToleranceSatisfied,
        1,
        reported_residual_norm,
        values,
    )
}

fn factor_and_solve(
    system: &eqiora_solver::CanonicalCsrSystemView,
) -> Result<Vec<f64>, Diagnostic> {
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
    let symbolic_lu = factorize_symbolic_lu(column_matrix.symbolic(), Default::default())
        .map_err(|error| solve_failed(format!("faer symbolic LU failed: {error}")))?;

    let parallelism = Par::Seq;
    let mut numeric_lu = NumericLu::<usize, f64>::new();
    let factor_scratch =
        symbolic_lu.factorize_numeric_lu_scratch::<f64>(parallelism, Default::default());
    let mut factor_buffer = MemBuffer::try_new(factor_scratch)
        .map_err(|error| solve_failed(format!("faer numeric LU workspace failed: {error}")))?;
    let factor_stack = MemStack::new(&mut factor_buffer);
    let lu = symbolic_lu
        .factorize_numeric_lu(
            &mut numeric_lu,
            column_matrix.as_ref(),
            parallelism,
            factor_stack,
            Default::default(),
        )
        .map_err(|error| solve_failed(format!("faer numeric LU failed: {error}")))?;

    let mut output = Mat::from_fn(system.rows(), 1, |row, _| system.right_hand_side()[row]);
    let solve_scratch = symbolic_lu.solve_in_place_scratch::<f64>(1, parallelism);
    let mut solve_buffer = MemBuffer::try_new(solve_scratch)
        .map_err(|error| solve_failed(format!("faer sparse LU solve workspace failed: {error}")))?;
    let solve_stack = MemStack::new(&mut solve_buffer);
    lu.solve_in_place_with_conj(Conj::No, output.as_mut(), parallelism, solve_stack);
    Ok(output.col_as_slice(0).to_vec())
}

fn fixed_residual_norm(problem: &LinearProblem<'_>, values: &[f64]) -> Result<f64, Diagnostic> {
    let mut residual = vec![0.0; problem.operator().rows()];
    SERIAL_LINEAR_EXECUTION.apply(problem.operator(), values, &mut residual)?;
    for (applied, right_hand_side) in residual.iter_mut().zip(problem.right_hand_side()) {
        *applied = right_hand_side - *applied;
    }
    fixed_norm(&residual)
}

fn fixed_norm(values: &[f64]) -> Result<f64, Diagnostic> {
    let squared =
        SERIAL_LINEAR_EXECUTION.inner_product(FixedOrderInnerProduct::new(values, values)?)?;
    Ok(squared.sqrt())
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}
