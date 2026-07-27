use eqiora::Diagnostic;
use eqiora::api::{
    ModelDocument, ReferenceRunCancellation, ReferenceRunPlan, ReferenceRunProgress,
    ScalarEllipticRunCancellation, ScalarEllipticRunPlan, ScalarEllipticRunProgress,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pyo3::prelude::*;

/// Monotone public state of one native execution occurrence.
#[pyclass(
    name = "RunStatus",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyRunStatus {
    Created,
    Validating,
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Completed,
    Failed,
}

impl PyRunStatus {
    pub(super) const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }
}

/// Last coalesced fully accepted semantic-execution boundary.
#[pyclass(
    name = "RunProgress",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PyRunProgress {
    model_time: f64,
    end_time: f64,
    accepted_steps: usize,
    maximum_steps: usize,
}

impl Hash for PyRunProgress {
    /// Consistent with the derived `PartialEq`, which is what makes this type
    /// usable as a dict key rather than silently unhashable.
    ///
    /// Two float subtleties decide the implementation. `0.0 == -0.0` is true
    /// while their bit patterns differ, so hashing raw bits would give equal
    /// values different hashes and lose keys; each zero is therefore normalized
    /// before folding. `NaN` is equal to nothing including itself, so whatever
    /// its bits hash to is unobservable through equality.
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        for value in [self.model_time, self.end_time] {
            let normalized = if value == 0.0 { 0.0 } else { value };
            normalized.to_bits().hash(hasher);
        }
        self.accepted_steps.hash(hasher);
        self.maximum_steps.hash(hasher);
    }
}

impl From<ReferenceRunProgress> for PyRunProgress {
    fn from(progress: ReferenceRunProgress) -> Self {
        Self {
            model_time: progress.model_time(),
            end_time: progress.end_time(),
            accepted_steps: progress.accepted_steps(),
            maximum_steps: progress.maximum_steps(),
        }
    }
}

#[pymethods]
impl PyRunProgress {
    /// Consistent with `__eq__`, which pyo3 generates from the derived
    /// `PartialEq`. Without this, Python sets `__hash__` to `None` and the
    /// type silently leaves every dict and set.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    #[getter]
    const fn model_time(&self) -> f64 {
        self.model_time
    }

    #[getter]
    const fn end_time(&self) -> f64 {
        self.end_time
    }

    #[getter]
    const fn accepted_steps(&self) -> usize {
        self.accepted_steps
    }

    #[getter]
    const fn maximum_steps(&self) -> usize {
        self.maximum_steps
    }

    fn __repr__(&self) -> String {
        format!(
            "RunProgress(model_time={}, end_time={}, accepted_steps={})",
            self.model_time, self.end_time, self.accepted_steps
        )
    }
}

/// Last fully accepted scalar-elliptic application phase.
#[pyclass(
    name = "ScalarEllipticRunProgress",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyScalarEllipticRunProgress {
    PlanReplayed,
    SystemFinalized,
    SolutionAccepted,
}

impl From<ScalarEllipticRunProgress> for PyScalarEllipticRunProgress {
    fn from(progress: ScalarEllipticRunProgress) -> Self {
        match progress {
            ScalarEllipticRunProgress::PlanReplayed => Self::PlanReplayed,
            ScalarEllipticRunProgress::SystemFinalized => Self::SystemFinalized,
            ScalarEllipticRunProgress::SolutionAccepted => Self::SolutionAccepted,
        }
    }
}

/// Exact accepted boundary at which cooperative cancellation terminated.
#[pyclass(
    name = "RunCancellation",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyRunCancellation {
    progress: PyRunProgress,
    elapsed_seconds: f64,
    plan_key: String,
}

impl From<ReferenceRunCancellation> for PyRunCancellation {
    fn from(cancellation: ReferenceRunCancellation) -> Self {
        Self {
            progress: cancellation.progress().into(),
            elapsed_seconds: cancellation.elapsed().as_secs_f64(),
            plan_key: cancellation.plan().key(),
        }
    }
}

#[pymethods]
impl PyRunCancellation {
    #[getter]
    const fn progress(&self) -> PyRunProgress {
        self.progress
    }

    #[getter]
    const fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    #[getter]
    fn plan_key(&self) -> &str {
        &self.plan_key
    }

    fn __repr__(&self) -> String {
        format!(
            "RunCancellation(model_time={}, accepted_steps={}, plan_key={:?})",
            self.progress.model_time, self.progress.accepted_steps, self.plan_key
        )
    }
}

/// Exact scalar-elliptic phase at which cooperative cancellation terminated.
#[pyclass(
    name = "ScalarEllipticRunCancellation",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyScalarEllipticRunCancellation {
    progress: PyScalarEllipticRunProgress,
    elapsed_seconds: f64,
    plan_key: String,
}

impl From<ScalarEllipticRunCancellation> for PyScalarEllipticRunCancellation {
    fn from(cancellation: ScalarEllipticRunCancellation) -> Self {
        Self {
            progress: cancellation.progress().into(),
            elapsed_seconds: cancellation.elapsed().as_secs_f64(),
            plan_key: cancellation.plan().key().to_owned(),
        }
    }
}

#[pymethods]
impl PyScalarEllipticRunCancellation {
    #[getter]
    const fn progress(&self) -> PyScalarEllipticRunProgress {
        self.progress
    }

    #[getter]
    const fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    #[getter]
    fn plan_key(&self) -> &str {
        &self.plan_key
    }

    fn __repr__(&self) -> String {
        format!(
            "ScalarEllipticRunCancellation(progress={:?}, plan_key={:?})",
            self.progress, self.plan_key
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RunIdentity {
    model_id: String,
    model_digest: String,
    model_revision: u64,
    plan_key: String,
    adapter: &'static str,
    adapter_version: &'static str,
}

impl RunIdentity {
    pub(super) fn from_reference(
        document: &ModelDocument,
        plan: &ReferenceRunPlan,
    ) -> Result<Self, Diagnostic> {
        let reference = document.artifact_reference()?;
        Ok(Self {
            model_id: reference.model().ulid().to_string(),
            model_digest: reference.artifact().to_string(),
            model_revision: reference.semantic_revision().get(),
            plan_key: plan.key(),
            adapter: plan.adapter(),
            adapter_version: plan.adapter_version(),
        })
    }

    pub(super) fn from_scalar_elliptic(
        document: &ModelDocument,
        plan: &ScalarEllipticRunPlan,
    ) -> Result<Self, Diagnostic> {
        let reference = document.artifact_reference()?;
        Ok(Self {
            model_id: reference.model().ulid().to_string(),
            model_digest: reference.artifact().to_string(),
            model_revision: reference.semantic_revision().get(),
            plan_key: plan.key().to_owned(),
            adapter: plan.adapter(),
            adapter_version: plan.adapter_version(),
        })
    }

    pub(crate) fn model_id(&self) -> &str {
        &self.model_id
    }

    pub(crate) fn model_digest(&self) -> &str {
        &self.model_digest
    }

    pub(crate) const fn model_revision(&self) -> u64 {
        self.model_revision
    }

    pub(crate) fn plan_key(&self) -> &str {
        &self.plan_key
    }

    pub(crate) fn adapter(&self) -> &'static str {
        self.adapter
    }

    pub(crate) fn adapter_version(&self) -> &'static str {
        self.adapter_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(model_time: f64) -> PyRunProgress {
        PyRunProgress {
            model_time,
            end_time: 1.0,
            accepted_steps: 1,
            maximum_steps: 2,
        }
    }

    fn digest(value: PyRunProgress) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn negative_zero_hashes_with_positive_zero_because_they_compare_equal() {
        // Hashing raw bits would pass every other test and lose the key.
        assert_eq!(progress(0.0), progress(-0.0));
        assert_eq!(digest(progress(0.0)), digest(progress(-0.0)));
    }

    #[test]
    fn distinct_progress_values_are_still_distinguished() {
        assert_ne!(progress(0.0), progress(0.5));
        assert_ne!(digest(progress(0.0)), digest(progress(0.5)));
    }
}
