//! Stable diagnostic construction for time contracts.

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};

pub(crate) fn invalid_lowering(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_TIME_LOWERING, message)
        .with_graph_path(GraphPath::new(["time", "lowering"]))
}

pub(crate) fn invalid_plan(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_EXECUTION_CONFIG, message)
        .with_graph_path(GraphPath::new(["time", "plan"]))
}

pub(crate) fn time_solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
        .with_graph_path(GraphPath::new(["time", "solution"]))
}

pub(crate) fn invalid_sensitivity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
        .with_graph_path(GraphPath::new(["time", "sensitivity"]))
}
