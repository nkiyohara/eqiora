//! Thin application entry to the shared reference sampled execution owner.

use eqiora_core::{Diagnostic, RawId, ValueLiteral};
use eqiora_sem::{Interpreter, ReferenceConfig, SampledSession};

use crate::ModelDocument;

impl ModelDocument {
    /// Start bounded reference sampled execution on this exact immutable Model.
    ///
    /// Samples are runtime input values, separate from static signature bindings.
    /// Each table identifies the exact input Port and its exact ClockDomain.
    ///
    /// # Errors
    /// Returns reference-profile, input-table, initialization, or clock diagnostics.
    pub fn sampled_session(
        &self,
        config: ReferenceConfig,
        inputs: impl IntoIterator<Item = (RawId, RawId, Vec<ValueLiteral>)>,
    ) -> Result<SampledSession, Vec<Diagnostic>> {
        Interpreter::default().sampled_session(&self.program, config, inputs)
    }

    /// Resume an in-memory accepted sampled checkpoint on this exact Model.
    ///
    /// # Errors
    /// Rejects a checkpoint whose complete immutable Model differs.
    pub fn resume_sampled(
        &self,
        checkpoint: &SampledSession,
    ) -> Result<SampledSession, Vec<Diagnostic>> {
        Interpreter::default().resume_sampled(&self.program, checkpoint)
    }
}
