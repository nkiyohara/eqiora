//! Solver-agnostic execution telemetry emitted through `tracing`.
//!
//! These spans describe process-local execution only. They never participate in
//! Model, Plan, Result, or artifact identity.

use tracing::{Level, Span};

/// Stable tracing target for Eqiora phase spans and solver events.
pub const TARGET: &str = "eqiora::execution";

/// One typed process-local execution phase.
#[derive(Debug, Clone, Copy)]
pub enum Phase<'a> {
    /// Root of one execution occurrence.
    Run { family: &'a str },
    /// Preparation performed once before a solve or step loop.
    Setup,
    /// One steady solve.
    Solve { index: usize },
    /// One accepted-action attempt in a transient execution.
    TimeStep { step: usize, time_s: f64, dt_s: f64 },
    /// Assembly of a residual, Jacobian, or linear system.
    Assembly,
    /// One nonlinear iteration.
    NonlinearIteration {
        solver: &'a str,
        iteration: usize,
        residual_norm: f64,
    },
    /// One backend-neutral linear solve.
    LinearSolve { solver: &'a str, provider: &'a str },
    /// One backend-owned phase nested under a common Eqiora phase.
    Backend {
        name: &'static str,
        backend: &'a str,
    },
    /// Projection of native output into an application-facing result.
    Postprocess,
}

/// Create a structured span for one execution phase.
#[must_use]
pub fn phase(boundary: Phase<'_>) -> Span {
    match boundary {
        Phase::Run { family } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "run", family)
        }
        Phase::Setup => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "setup")
        }
        Phase::Solve { index } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "solve", solve = index)
        }
        Phase::TimeStep { step, time_s, dt_s } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "time_step", step, time_s, dt_s)
        }
        Phase::Assembly => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "assembly")
        }
        Phase::NonlinearIteration {
            solver,
            iteration,
            residual_norm,
        } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "nonlinear_iteration", nonlinear_solver = solver, iteration, residual_norm)
        }
        Phase::LinearSolve { solver, provider } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "linear_solve", linear_solver = solver, solver_provider = provider)
        }
        Phase::Backend { name, backend } => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = name, backend)
        }
        Phase::Postprocess => {
            tracing::span!(target: TARGET, Level::INFO, "eqiora_phase", phase = "postprocess")
        }
    }
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
