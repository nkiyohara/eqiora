//! The two diagnostic codes this workflow can report, and nothing else.
//!
//! A rejected Realization and a rejected artifact are different failures and
//! carry different codes. Naming both here keeps every module in this workflow
//! reporting the same code for the same kind of refusal.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

pub(super) fn capability_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

pub(super) fn run_manifest_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

pub(super) fn single(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    vec![diagnostic]
}
