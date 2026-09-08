//! Thin application entry to the shared reference execution owner.

use eqiora_core::{Diagnostic, RawId, ValueLiteral};
use eqiora_sem::{ExecutionSession, Interpreter, ReferenceConfig};

use crate::ModelDocument;

impl ModelDocument {
    /// Start bounded reference execution on this exact immutable Model.
    ///
    /// Samples are runtime input values, separate from static signature bindings.
    /// Each table identifies the exact input Port and its exact ClockDomain.
    ///
    /// # Errors
    /// Returns reference-profile, input-table, initialization, or clock diagnostics.
    pub fn execution_session(
        &self,
        config: ReferenceConfig,
        inputs: impl IntoIterator<Item = (RawId, RawId, Vec<ValueLiteral>)>,
    ) -> Result<ExecutionSession, Vec<Diagnostic>> {
        Interpreter::default().execution_session(&self.program, config, inputs)
    }

    /// Resume an in-memory accepted execution checkpoint on this exact Model.
    ///
    /// # Errors
    /// Rejects a checkpoint whose complete immutable Model differs.
    pub fn resume_execution(
        &self,
        checkpoint: &ExecutionSession,
    ) -> Result<ExecutionSession, Vec<Diagnostic>> {
        Interpreter::default().resume_execution(&self.program, checkpoint)
    }
}
