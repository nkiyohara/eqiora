//! Solver-agnostic execution telemetry emitted through `tracing`.
//!
//! These spans describe process-local execution only. They never participate in
//! Model, Plan, Result, or artifact identity.

use tracing::{Level, Span};

/// Stable tracing target for Eqiora phase spans and solver events.
pub const TARGET: &str = "eqiora::execution";

/// Root of one execution occurrence.
#[must_use]
pub fn run(family: &str) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "run", family)
}

/// Preparation performed once before a solve or step loop.
#[must_use]
pub fn setup() -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "setup")
}

/// One steady solve.
#[must_use]
pub fn solve(index: usize) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "solve", solve = index)
}

/// One accepted-action attempt in a transient execution.
#[must_use]
pub fn time_step(step: usize, time_s: f64, dt_s: f64) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "time_step", step, time_s, dt_s)
}

/// Assembly of a residual, Jacobian, or linear system.
#[must_use]
pub fn assembly() -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "assembly")
}

/// One nonlinear iteration.
#[must_use]
pub fn nonlinear_iteration(solver: &str, iteration: usize, residual_norm: f64) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "nonlinear_iteration", nonlinear_solver = solver, iteration, residual_norm)
}

/// One backend-neutral linear solve.
#[must_use]
pub fn linear_solve(solver: &str, provider: &str) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "linear_solve", linear_solver = solver, solver_provider = provider)
}

/// One backend-owned phase nested under a common Eqiora phase.
#[must_use]
pub fn backend_phase(phase: &'static str, backend: &str) -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase, backend)
}

/// Projection of native output into an application-facing result.
#[must_use]
pub fn postprocess() -> Span {
    tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "postprocess")
}

/// Emit one structured nonlinear convergence observation.
pub fn nonlinear_status(
    solver: &str,
    iteration: usize,
    residual_norm: f64,
    residual_target: f64,
    converged: bool,
) {
    tracing::event!(
        target: TARGET,
        Level::INFO,
        event = "nonlinear_status",
        nonlinear_solver = solver,
        iteration,
        residual_norm,
        residual_target,
        converged,
    );
}
