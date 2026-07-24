//! Semantic-reference execution over an immutable model document.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, RawId};
use eqiora_sem::{
    ExecutionDirective, ExecutionObserver, ExecutionOutcome, ExecutionProgress, Interpreter,
    ReferenceConfig, Trajectory,
};

use crate::{ModelDocument, single_diagnostic};

/// Stable identity of the deliberately conservative semantic reference path.
pub const REFERENCE_EXECUTION_ADAPTER: &str = "eqiora.reference";

/// Versioned, fully resolved controls for one semantic-reference run.
///
/// Clients may present this value, but they do not recreate its admission
/// rules. The key is replayed immediately before execution so an accepted
/// preview cannot silently become a different numerical request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceRunPlan {
    config: ReferenceConfig,
}

impl ReferenceRunPlan {
    /// Resolve the bounded reference configuration used by execution.
    ///
    /// # Errors
    /// Returns the same structured configuration diagnostic as the semantic
    /// interpreter when either model-time control is invalid.
    pub fn new(end_time: f64, max_step: f64) -> Result<Self, Diagnostic> {
        ReferenceConfig::new(end_time, max_step).map(|config| Self { config })
    }

    /// Stable key for exact preview-to-run replay within protocol v1.
    #[must_use]
    pub fn key(self) -> String {
        format!(
            "eqiora.reference-plan/v1:{:016x}:{:016x}",
            self.config.end_time().to_bits(),
            self.config.max_step().to_bits()
        )
    }

    /// Resolved interpreter configuration.
    #[must_use]
    pub const fn config(self) -> ReferenceConfig {
        self.config
    }

    /// Execution adapter identity, separate from the model and UI.
    #[must_use]
    pub const fn adapter(self) -> &'static str {
        REFERENCE_EXECUTION_ADAPTER
    }

    /// Adapter implementation version recorded with run evidence.
    #[must_use]
    pub const fn adapter_version(self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    /// Reference integration method.
    #[must_use]
    pub const fn integration_method(self) -> ReferenceIntegrationMethod {
        ReferenceIntegrationMethod::BackwardEuler
    }

    /// Reference nonlinear method.
    #[must_use]
    pub const fn nonlinear_method(self) -> ReferenceNonlinearMethod {
        ReferenceNonlinearMethod::DenseFiniteDifferenceNewton
    }

    /// Concrete producer placement.
    #[must_use]
    pub const fn placement(self) -> ReferenceExecutionPlacement {
        ReferenceExecutionPlacement::HostSerial
    }

    /// Acceptance meaning for the semantic oracle.
    #[must_use]
    pub const fn acceptance(self) -> ReferenceAcceptance {
        ReferenceAcceptance::SemanticOracle
    }
}

/// Time integration selected by a [`ReferenceRunPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceIntegrationMethod {
    /// Fixed backward Euler with exact activation boundaries.
    BackwardEuler,
}

/// Nonlinear solve selected by a [`ReferenceRunPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceNonlinearMethod {
    /// Small dense Newton with a finite-difference Jacobian.
    DenseFiniteDifferenceNewton,
}

/// Execution placement selected by a [`ReferenceRunPlan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceExecutionPlacement {
    /// One serial host worker; no process-global pool is touched.
    HostSerial,
}

/// Meaning of accepting one completed reference result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceAcceptance {
    /// This path defines semantic conformance and is not an optimized producer
    /// checked by a second numerical backend.
    SemanticOracle,
}

/// One accepted reference-execution boundary exposed to application clients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceRunProgress {
    model_time: f64,
    end_time: f64,
    accepted_steps: usize,
    maximum_steps: usize,
}

impl ReferenceRunProgress {
    /// Last fully accepted model time in seconds.
    #[must_use]
    pub const fn model_time(self) -> f64 {
        self.model_time
    }

    /// Requested inclusive final model time in seconds.
    #[must_use]
    pub const fn end_time(self) -> f64 {
        self.end_time
    }

    /// Number of fully accepted time/event steps.
    #[must_use]
    pub const fn accepted_steps(self) -> usize {
        self.accepted_steps
    }

    /// Configured accepted-step safety limit.
    #[must_use]
    pub const fn maximum_steps(self) -> usize {
        self.maximum_steps
    }
}

impl From<ExecutionProgress> for ReferenceRunProgress {
    fn from(progress: ExecutionProgress) -> Self {
        Self {
            model_time: progress.model_time(),
            end_time: progress.end_time(),
            accepted_steps: progress.accepted_steps(),
            maximum_steps: progress.maximum_steps(),
        }
    }
}

/// Decision returned by a reference-run observer at an accepted boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceRunDirective {
    /// Continue execution.
    Continue,
    /// Cancel without constructing partial result evidence.
    Cancel,
}

/// Bounded application observer for reference-run progress and cancellation.
pub trait ReferenceRunObserver {
    /// Inspect one accepted boundary and decide whether execution continues.
    fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective;
}

/// Evidence that cancellation was observed at an accepted boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceRunCancellation {
    plan: ReferenceRunPlan,
    elapsed: Duration,
    progress: ReferenceRunProgress,
}

impl ReferenceRunCancellation {
    /// Exact accepted plan that was cancelled.
    #[must_use]
    pub const fn plan(self) -> ReferenceRunPlan {
        self.plan
    }

    /// Wall time through cancellation observation.
    #[must_use]
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Last fully accepted semantic-execution boundary.
    #[must_use]
    pub const fn progress(self) -> ReferenceRunProgress {
        self.progress
    }
}

/// Terminal result of a controlled reference run.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceRunOutcome {
    /// Complete accepted result and evidence.
    Completed(ReferenceRunResult),
    /// Accepted cancellation boundary; no partial result is admitted.
    Cancelled(ReferenceRunCancellation),
}

struct SemanticObserver<'a, O> {
    observer: &'a mut O,
}

impl<O: ReferenceRunObserver> ExecutionObserver for SemanticObserver<'_, O> {
    fn observe(&mut self, progress: ExecutionProgress) -> ExecutionDirective {
        match self.observer.observe(progress.into()) {
            ReferenceRunDirective::Continue => ExecutionDirective::Continue,
            ReferenceRunDirective::Cancel => ExecutionDirective::Cancel,
        }
    }
}

#[derive(Debug, Default)]
struct UninterruptedReferenceRun;

impl ReferenceRunObserver for UninterruptedReferenceRun {
    fn observe(&mut self, _progress: ReferenceRunProgress) -> ReferenceRunDirective {
        ReferenceRunDirective::Continue
    }
}

impl ModelDocument {
    /// Execute the normative reference interpreter and collect owned,
    /// field-local result series.
    ///
    /// # Errors
    /// Returns configuration, semantic, or execution diagnostics.
    pub fn run_reference(
        &self,
        end_time: f64,
        max_step: f64,
    ) -> Result<ReferenceRunResult, Vec<Diagnostic>> {
        let plan = ReferenceRunPlan::new(end_time, max_step).map_err(single_diagnostic)?;
        self.run_reference_plan(plan)
    }

    /// Execute one previously resolved reference plan.
    ///
    /// # Errors
    /// Returns structured semantic or numerical diagnostics. Adapter evidence
    /// is created only after the trajectory has completed successfully.
    pub fn run_reference_plan(
        &self,
        plan: ReferenceRunPlan,
    ) -> Result<ReferenceRunResult, Vec<Diagnostic>> {
        let mut observer = UninterruptedReferenceRun;
        match self.run_reference_plan_controlled(plan, &mut observer)? {
            ReferenceRunOutcome::Completed(result) => Ok(result),
            ReferenceRunOutcome::Cancelled(_) => {
                unreachable!("the uninterrupted observer cannot request cancellation")
            }
        }
    }

    /// Execute one plan while observing only fully accepted semantic steps.
    ///
    /// Cancellation is a typed terminal outcome, not an execution diagnostic.
    /// No partial series or successful-run evidence is constructed.
    ///
    /// # Errors
    /// Returns the same structured semantic or numerical diagnostics as
    /// [`Self::run_reference_plan`].
    pub fn run_reference_plan_controlled(
        &self,
        plan: ReferenceRunPlan,
        observer: &mut impl ReferenceRunObserver,
    ) -> Result<ReferenceRunOutcome, Vec<Diagnostic>> {
        let started = Instant::now();
        let mut semantic_observer = SemanticObserver { observer };
        let outcome = Interpreter::new().run_controlled(
            &self.program,
            plan.config(),
            &mut semantic_observer,
        )?;
        match outcome {
            ExecutionOutcome::Completed(trajectory) => self
                .reference_result(plan, started, trajectory)
                .map(ReferenceRunOutcome::Completed),
            ExecutionOutcome::Cancelled(progress) => {
                Ok(ReferenceRunOutcome::Cancelled(ReferenceRunCancellation {
                    plan,
                    elapsed: started.elapsed(),
                    progress: progress.into(),
                }))
            }
        }
    }

    fn reference_result(
        &self,
        plan: ReferenceRunPlan,
        started: Instant,
        trajectory: Trajectory,
    ) -> Result<ReferenceRunResult, Vec<Diagnostic>> {
        let names = preferred_names(&self.aliases);
        let mut grouped = BTreeMap::<RawId, ReferenceSeries>::new();
        for sample in trajectory.samples() {
            let series = grouped
                .entry(sample.field())
                .or_insert_with(|| ReferenceSeries {
                    field: sample.field(),
                    name: names.get(&sample.field()).cloned(),
                    dimension: sample.value().dim(),
                    time: Vec::new(),
                    values: Vec::new(),
                });
            if series.dimension != sample.value().dim() {
                return Err(vec![Diagnostic::error(
                    codes::DIMENSION_MISMATCH,
                    format!(
                        "field {} changed physical dimension along its trajectory",
                        sample.field()
                    ),
                )]);
            }
            series.time.push(sample.time());
            series.values.push(sample.value().value());
        }
        let series: Vec<_> = grouped.into_values().collect();
        let sample_count = series.iter().map(|series| series.time.len()).sum();
        Ok(ReferenceRunResult {
            evidence: ReferenceRunEvidence {
                plan,
                elapsed: started.elapsed(),
                field_count: series.len(),
                sample_count,
            },
            series,
        })
    }
}

/// Completed reference run with independently sampled field series.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceRunResult {
    evidence: ReferenceRunEvidence,
    series: Vec<ReferenceSeries>,
}

impl ReferenceRunResult {
    /// Resolved plan and measured output evidence for this accepted run.
    #[must_use]
    pub const fn evidence(&self) -> &ReferenceRunEvidence {
        &self.evidence
    }

    /// Series in stable Field-ID order.
    #[must_use]
    pub fn series(&self) -> &[ReferenceSeries] {
        &self.series
    }

    /// Transfer ownership of all series to a binding or renderer adapter.
    #[must_use]
    pub fn into_series(self) -> Vec<ReferenceSeries> {
        self.series
    }
}

/// Auditable metadata produced only by a successful semantic-reference run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceRunEvidence {
    plan: ReferenceRunPlan,
    elapsed: Duration,
    field_count: usize,
    sample_count: usize,
}

impl ReferenceRunEvidence {
    /// Exact resolved plan used by the interpreter.
    #[must_use]
    pub const fn plan(self) -> ReferenceRunPlan {
        self.plan
    }

    /// Measured wall time covering semantic execution and owned-result
    /// projection. This is observational evidence, not model time.
    #[must_use]
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// Number of field-local result series returned.
    #[must_use]
    pub const fn field_count(self) -> usize {
        self.field_count
    }

    /// Total number of field-local samples returned across all series.
    #[must_use]
    pub const fn sample_count(self) -> usize {
        self.sample_count
    }
}

/// One owned, read-only-by-contract field series in coherent SI units.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceSeries {
    field: RawId,
    name: Option<String>,
    dimension: DimExponents,
    time: Vec<f64>,
    values: Vec<f64>,
}

impl ReferenceSeries {
    /// Stable Field ID.
    #[must_use]
    pub const fn field(&self) -> RawId {
        self.field
    }

    /// Preferred source alias, when source symbols are available.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Physical dimension shared by all values.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Field-local model times in seconds.
    #[must_use]
    pub fn time(&self) -> &[f64] {
        &self.time
    }

    /// Field-local values in coherent SI units.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Transfer the two owned buffers without copying.
    #[must_use]
    pub fn into_buffers(self) -> (Vec<f64>, Vec<f64>) {
        (self.time, self.values)
    }
}

fn preferred_names(aliases: &BTreeMap<String, RawId>) -> BTreeMap<RawId, String> {
    let mut names = BTreeMap::new();
    for (name, &id) in aliases {
        names.entry(id).or_insert_with(|| name.clone());
    }
    names
}
