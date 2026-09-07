//! Closed source compilation uses the signature-directed local owner.
use crate::lower::CompiledModel;
use eqiora_core::Diagnostic;

/// Parse and type-lower every Model in one source file with no supplied bindings.
///
/// # Errors
/// Returns parser, definition, missing requirement, or occurrence diagnostics.
pub fn compile(file: &str, source: &str) -> Result<Vec<CompiledModel>, Vec<Diagnostic>> {
    crate::hierarchy::selected::local_all(file, source)
}
