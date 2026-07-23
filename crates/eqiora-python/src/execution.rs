//! Bounded Python lifecycle over the shared semantic reference executor.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use eqiora::api::{
    ModelDocument, ReferenceRunCancellation, ReferenceRunDirective, ReferenceRunObserver,
    ReferenceRunOutcome, ReferenceRunPlan, ReferenceRunProgress, ReferenceRunResult,
    ScalarEllipticExecutionEnvironment, ScalarEllipticRunCancellation, ScalarEllipticRunDirective,
    ScalarEllipticRunObserver, ScalarEllipticRunOutcome, ScalarEllipticRunPlan,
    ScalarEllipticRunProgress, ScalarEllipticRunResult,
};
use eqiora::diagnostic::codes;
use eqiora::{Diagnostic, GraphPath};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::error::{
    cancellation_error, catch_native_panic, diagnostic_error, execution_error,
    internal_diagnostic_error, internal_error, panic_boundary,
};
use crate::realization::{PyRealization, PyScalarEllipticResult};
use crate::{PyModel, result_into_python};

const PROGRESS_PUBLICATION_INTERVAL: Duration = Duration::from_millis(100);
const RESULT_MATERIALIZATION_FAILURE: &str =
    "the completed native Result could not be materialized";

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
    const fn is_terminal(self) -> bool {
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
    fn from_reference(
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

    fn from_scalar_elliptic(
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

#[derive(Debug, Clone)]
enum RunFailure {
    Execution(Vec<Diagnostic>),
    Internal(Vec<Diagnostic>),
}

#[derive(Debug, Clone, Copy)]
enum NativeRunProgress {
    Reference(ReferenceRunProgress),
    ScalarElliptic(ScalarEllipticRunProgress),
}

impl NativeRunProgress {
    fn into_python(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Reference(progress) => {
                Py::new(py, PyRunProgress::from(progress)).map(Py::into_any)
            }
            Self::ScalarElliptic(progress) => {
                Py::new(py, PyScalarEllipticRunProgress::from(progress)).map(Py::into_any)
            }
        }
    }
}

#[derive(Debug)]
enum NativeRunOutput {
    Reference(ReferenceRunResult),
    ScalarElliptic(Box<ScalarEllipticRunResult>),
}

#[derive(Debug, Clone)]
enum NativeRunCancellation {
    Reference(ReferenceRunCancellation),
    ScalarElliptic(Box<ScalarEllipticRunCancellation>),
}

impl NativeRunCancellation {
    fn into_python(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Reference(cancellation) => {
                Py::new(py, PyRunCancellation::from(cancellation)).map(Py::into_any)
            }
            Self::ScalarElliptic(cancellation) => {
                Py::new(py, PyScalarEllipticRunCancellation::from(*cancellation)).map(Py::into_any)
            }
        }
    }

    fn diagnostic(&self) -> Diagnostic {
        let message = match self {
            Self::Reference(cancellation) => {
                let progress = cancellation.progress();
                format!(
                    "reference execution was cancelled at accepted model time {} after {} accepted steps",
                    progress.model_time(),
                    progress.accepted_steps()
                )
            }
            Self::ScalarElliptic(cancellation) => format!(
                "scalar-elliptic execution was cancelled at accepted application phase {:?}",
                cancellation.progress()
            ),
        };
        Diagnostic::error(codes::EXECUTION_CANCELLED, message).with_graph_path(GraphPath::new([
            "execution".to_owned(),
            "cancellation".to_owned(),
        ]))
    }
}

#[derive(Debug)]
enum RunTerminal {
    Completed(Option<NativeRunOutput>),
    Cancelled(NativeRunCancellation),
    Failed(RunFailure),
}

#[derive(Debug, Clone)]
enum RunTerminalKind {
    Completed,
    Cancelled(NativeRunCancellation),
    Failed(RunFailure),
}

#[derive(Debug)]
struct RunState {
    status: PyRunStatus,
    history: Vec<PyRunStatus>,
    progress: Option<NativeRunProgress>,
    terminal: Option<RunTerminal>,
    integrity_failed: bool,
}

impl RunState {
    fn accepted() -> Self {
        Self {
            status: PyRunStatus::Queued,
            history: vec![
                PyRunStatus::Created,
                PyRunStatus::Validating,
                PyRunStatus::Queued,
            ],
            progress: None,
            terminal: None,
            integrity_failed: false,
        }
    }

    fn transition(&mut self, status: PyRunStatus) {
        if self.status == status {
            return;
        }
        let allowed = matches!(
            (self.status, status),
            (
                PyRunStatus::Queued,
                PyRunStatus::Running | PyRunStatus::Cancelling | PyRunStatus::Failed
            ) | (
                PyRunStatus::Running,
                PyRunStatus::Cancelling | PyRunStatus::Completed | PyRunStatus::Failed
            ) | (
                PyRunStatus::Cancelling,
                PyRunStatus::Cancelled | PyRunStatus::Completed | PyRunStatus::Failed
            )
        );
        if allowed {
            self.status = status;
            self.history.push(status);
        } else {
            self.integrity_failed = true;
            self.terminal = Some(RunTerminal::Failed(RunFailure::Internal(vec![
                Diagnostic::error(
                    codes::INTERNAL_FAILURE,
                    format!(
                        "invalid native Run lifecycle transition from {:?} to {status:?}",
                        self.status
                    ),
                ),
            ])));
            self.status = PyRunStatus::Failed;
            self.history.push(PyRunStatus::Failed);
        }
    }
}

#[derive(Debug)]
struct RunShared {
    cancellation_requested: AtomicBool,
    state: Mutex<RunState>,
    changed: Condvar,
}

impl RunShared {
    fn new() -> Self {
        Self {
            cancellation_requested: AtomicBool::new(false),
            state: Mutex::new(RunState::accepted()),
            changed: Condvar::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, RunState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.integrity_failed = true;
                state.terminal = Some(RunTerminal::Failed(RunFailure::Internal(vec![
                    Diagnostic::error(
                        codes::INTERNAL_FAILURE,
                        "the native Run lifecycle lock was poisoned",
                    ),
                ])));
                state.transition(PyRunStatus::Failed);
                self.changed.notify_all();
                state
            }
        }
    }

    fn mark_running(&self) {
        let mut state = self.state();
        if !state.integrity_failed && state.status == PyRunStatus::Queued {
            state.transition(PyRunStatus::Running);
            self.changed.notify_all();
        }
    }

    fn request_cancellation(&self) -> bool {
        let mut state = self.state();
        if state.integrity_failed
            || state.status.is_terminal()
            || state.status == PyRunStatus::Cancelling
        {
            return false;
        }
        self.cancellation_requested.store(true, Ordering::Release);
        state.transition(PyRunStatus::Cancelling);
        self.changed.notify_all();
        true
    }

    fn cancellation_requested(&self) -> bool {
        self.cancellation_requested.load(Ordering::Acquire)
    }

    fn publish_progress(&self, progress: NativeRunProgress) {
        let mut state = self.state();
        if !state.integrity_failed {
            state.progress = Some(progress);
        }
        self.changed.notify_all();
    }

    fn finish(&self, terminal: RunTerminal) {
        let status = match terminal {
            RunTerminal::Completed(_) => PyRunStatus::Completed,
            RunTerminal::Cancelled(_) => PyRunStatus::Cancelled,
            RunTerminal::Failed(_) => PyRunStatus::Failed,
        };
        let mut state = self.state();
        if state.integrity_failed {
            return;
        }
        state.transition(status);
        if state.integrity_failed {
            return;
        }
        state.terminal = Some(terminal);
        self.changed.notify_all();
    }

    fn wait_terminal_kind(&self) -> RunTerminalKind {
        let mut state = self.state();
        loop {
            if let Some(terminal) = state.terminal.as_ref() {
                return match terminal {
                    RunTerminal::Completed(_) => RunTerminalKind::Completed,
                    RunTerminal::Cancelled(cancellation) => {
                        RunTerminalKind::Cancelled(cancellation.clone())
                    }
                    RunTerminal::Failed(failure) => RunTerminalKind::Failed(failure.clone()),
                };
            }
            state = match self.changed.wait(state) {
                Ok(state) => state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    state.integrity_failed = true;
                    let failure = RunFailure::Internal(vec![Diagnostic::error(
                        codes::INTERNAL_FAILURE,
                        "the native Run lifecycle wait was poisoned",
                    )]);
                    state.terminal = Some(RunTerminal::Failed(failure.clone()));
                    state.transition(PyRunStatus::Failed);
                    return RunTerminalKind::Failed(failure);
                }
            };
        }
    }

    fn take_completed(&self) -> Option<NativeRunOutput> {
        let mut state = self.state();
        match state.terminal.as_mut() {
            Some(RunTerminal::Completed(result)) => result.take(),
            _ => None,
        }
    }

    fn cancellation(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let cancellation = {
            let state = self.state();
            match state.terminal.as_ref() {
                Some(RunTerminal::Cancelled(cancellation)) => Some(cancellation.clone()),
                _ => None,
            }
        };
        cancellation
            .map(|cancellation| cancellation.into_python(py))
            .transpose()
    }
}

enum MaterializedResult {
    Empty,
    Materializing,
    Ready(Py<PyAny>),
    Failed(Vec<Diagnostic>),
}

struct ResultCache {
    state: Mutex<MaterializedResult>,
    changed: Condvar,
}

struct MaterializationClaim {
    cache: Arc<ResultCache>,
    committed: bool,
}

impl MaterializationClaim {
    fn new(cache: Arc<ResultCache>) -> Self {
        Self {
            cache,
            committed: false,
        }
    }

    fn commit(mut self, result: Py<PyAny>) {
        self.cache.store_ready(result);
        self.committed = true;
    }

    fn fail(mut self, diagnostics: Vec<Diagnostic>) {
        self.cache.store_failed(diagnostics);
        self.committed = true;
    }
}

impl Drop for MaterializationClaim {
    fn drop(&mut self) {
        if !self.committed {
            self.cache
                .store_failed(vec![result_materialization_diagnostic()]);
        }
    }
}

enum CacheClaim {
    Materialize,
    Wait,
    Ready,
    Failed(Vec<Diagnostic>),
}

fn result_materialization_diagnostic() -> Diagnostic {
    Diagnostic::error(codes::INTERNAL_FAILURE, RESULT_MATERIALIZATION_FAILURE)
}

impl ResultCache {
    fn new() -> Self {
        Self {
            state: Mutex::new(MaterializedResult::Empty),
            changed: Condvar::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, MaterializedResult> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                *state = MaterializedResult::Failed(vec![Diagnostic::error(
                    codes::INTERNAL_FAILURE,
                    "the completed Result cache lock was poisoned",
                )]);
                self.changed.notify_all();
                state
            }
        }
    }

    fn claim(&self) -> CacheClaim {
        let mut state = self.state();
        match &*state {
            MaterializedResult::Empty => {
                *state = MaterializedResult::Materializing;
                CacheClaim::Materialize
            }
            MaterializedResult::Materializing => CacheClaim::Wait,
            MaterializedResult::Ready(_) => CacheClaim::Ready,
            MaterializedResult::Failed(diagnostics) => CacheClaim::Failed(diagnostics.clone()),
        }
    }

    fn ready(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        let state = self.state();
        match &*state {
            MaterializedResult::Ready(result) => Some(result.clone_ref(py)),
            _ => None,
        }
    }

    fn store_ready(&self, result: Py<PyAny>) {
        let mut state = self.state();
        *state = MaterializedResult::Ready(result);
        self.changed.notify_all();
    }

    fn store_failed(&self, diagnostics: Vec<Diagnostic>) {
        let mut state = self.state();
        *state = MaterializedResult::Failed(diagnostics);
        self.changed.notify_all();
    }

    fn wait_for_materializer(&self) {
        let mut state = self.state();
        while matches!(*state, MaterializedResult::Materializing) {
            state = match self.changed.wait(state) {
                Ok(state) => state,
                Err(poisoned) => {
                    let mut state = poisoned.into_inner();
                    *state = MaterializedResult::Failed(vec![Diagnostic::error(
                        codes::INTERNAL_FAILURE,
                        "the completed Result cache wait was poisoned",
                    )]);
                    self.changed.notify_all();
                    return;
                }
            };
        }
    }
}

struct ReferenceSharedObserver {
    shared: Arc<RunShared>,
    last_publication: Option<Instant>,
}

impl ReferenceSharedObserver {
    fn new(shared: Arc<RunShared>) -> Self {
        Self {
            shared,
            last_publication: None,
        }
    }
}

impl ReferenceRunObserver for ReferenceSharedObserver {
    fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective {
        let cancellation_requested = self.shared.cancellation_requested();
        let now = Instant::now();
        let should_publish =
            progress_publication_due(self.last_publication, now, cancellation_requested);
        if should_publish {
            self.shared
                .publish_progress(NativeRunProgress::Reference(progress));
            self.last_publication = Some(now);
        }
        if cancellation_requested {
            ReferenceRunDirective::Cancel
        } else {
            ReferenceRunDirective::Continue
        }
    }
}

struct ScalarEllipticSharedObserver {
    shared: Arc<RunShared>,
}

impl ScalarEllipticSharedObserver {
    fn new(shared: Arc<RunShared>) -> Self {
        Self { shared }
    }
}

impl ScalarEllipticRunObserver for ScalarEllipticSharedObserver {
    fn observe(&mut self, progress: ScalarEllipticRunProgress) -> ScalarEllipticRunDirective {
        self.shared
            .publish_progress(NativeRunProgress::ScalarElliptic(progress));
        if self.shared.cancellation_requested() {
            ScalarEllipticRunDirective::Cancel
        } else {
            ScalarEllipticRunDirective::Continue
        }
    }
}

fn progress_publication_due(
    last_publication: Option<Instant>,
    now: Instant,
    cancellation_requested: bool,
) -> bool {
    cancellation_requested
        || last_publication
            .is_none_or(|last| now.duration_since(last) >= PROGRESS_PUBLICATION_INTERVAL)
}

#[derive(Debug)]
enum NativeRunJob {
    Reference {
        document: ModelDocument,
        plan: ReferenceRunPlan,
    },
    ScalarElliptic {
        document: ModelDocument,
        plan: Box<ScalarEllipticRunPlan>,
        environment: ScalarEllipticExecutionEnvironment,
    },
}

/// One process-local handle for an accepted native execution occurrence.
#[pyclass(name = "Run", module = "eqiora._eqiora", frozen, skip_from_py_object)]
pub(crate) struct PyRun {
    identity: RunIdentity,
    shared: Arc<RunShared>,
    result_cache: Arc<ResultCache>,
}

impl PyRun {
    fn submit_reference(
        py: Python<'_>,
        model: &PyModel,
        end_time: f64,
        max_step: f64,
    ) -> PyResult<Self> {
        let plan = ReferenceRunPlan::new(end_time, max_step)
            .map_err(|diagnostic| execution_error(py, &[diagnostic]))?;
        let document = model.document().clone();
        let identity = RunIdentity::from_reference(&document, &plan)
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?;
        Self::spawn(
            identity,
            NativeRunJob::Reference { document, plan },
            "eqiora-reference-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_scalar_elliptic(
        py: Python<'_>,
        model: &PyModel,
        realization: &PyRealization,
    ) -> PyResult<Self> {
        let document = model.document().clone();
        let plan = realization.plan().clone();
        let identity = RunIdentity::from_scalar_elliptic(&document, &plan)
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?;
        if identity.model_digest() != plan.model_digest() {
            return Err(diagnostic_error(
                py,
                &[Diagnostic::error(
                    codes::INVALID_REALIZATION,
                    "the accepted scalar-elliptic Realization belongs to a different Model artifact",
                )
                .with_graph_path(GraphPath::new([
                    "realization".to_owned(),
                    "model".to_owned(),
                ]))],
            ));
        }
        Self::spawn(
            identity,
            NativeRunJob::ScalarElliptic {
                document,
                plan: Box::new(plan),
                environment: ScalarEllipticExecutionEnvironment::host_serial(),
            },
            "eqiora-scalar-elliptic-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn spawn(
        identity: RunIdentity,
        job: NativeRunJob,
        thread_name: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        let shared = Arc::new(RunShared::new());
        let worker_shared = Arc::clone(&shared);
        let spawn = thread::Builder::new()
            .name(thread_name.to_owned())
            .spawn(move || run_worker(job, worker_shared));
        if let Err(error) = spawn {
            let diagnostics = vec![Diagnostic::error(
                codes::INTERNAL_FAILURE,
                format!("the native Run worker could not start: {error}"),
            )];
            shared.finish(RunTerminal::Failed(RunFailure::Internal(
                diagnostics.clone(),
            )));
            return Err(diagnostics);
        }
        Ok(Self {
            identity,
            shared,
            result_cache: Arc::new(ResultCache::new()),
        })
    }
}

#[pymethods]
impl PyRun {
    #[getter]
    fn status(&self) -> PyRunStatus {
        self.shared.state().status
    }

    #[getter]
    fn history(&self) -> Vec<PyRunStatus> {
        self.shared.state().history.clone()
    }

    #[getter]
    fn progress(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let progress = { self.shared.state().progress };
        progress
            .map(|progress| progress.into_python(py))
            .transpose()
    }

    /// Exact terminal cancellation evidence, once cancellation is accepted.
    #[getter]
    fn cancellation(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.shared.cancellation(py)
    }

    #[getter]
    fn done(&self) -> bool {
        self.shared.state().status.is_terminal()
    }

    #[getter]
    fn model_id(&self) -> &str {
        self.identity.model_id()
    }

    #[getter]
    fn model_digest(&self) -> &str {
        self.identity.model_digest()
    }

    #[getter]
    const fn model_revision(&self) -> u64 {
        self.identity.model_revision()
    }

    #[getter]
    fn plan_key(&self) -> &str {
        self.identity.plan_key()
    }

    #[getter]
    fn adapter(&self) -> &'static str {
        self.identity.adapter()
    }

    /// Request cancellation at the next accepted execution-family boundary.
    ///
    /// Returns whether this call recorded a new request. A run can still
    /// complete when its last accepted boundary won the race.
    fn cancel(&self) -> bool {
        self.shared.request_cancellation()
    }

    /// Block without the GIL until one terminal outcome is available.
    fn result(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        panic_boundary(py, || {
            let shared = Arc::clone(&self.shared);
            let terminal = py.detach(move || shared.wait_terminal_kind());
            match terminal {
                RunTerminalKind::Completed => {}
                RunTerminalKind::Cancelled(cancellation) => {
                    return Err(cancellation_error(py, &[cancellation.diagnostic()]));
                }
                RunTerminalKind::Failed(RunFailure::Execution(diagnostics)) => {
                    return Err(execution_error(py, &diagnostics));
                }
                RunTerminalKind::Failed(RunFailure::Internal(diagnostics)) => {
                    return Err(internal_diagnostic_error(py, &diagnostics));
                }
            }

            loop {
                match self.result_cache.claim() {
                    CacheClaim::Ready => {
                        return self.result_cache.ready(py).ok_or_else(|| {
                            internal_error(py, "the completed Result cache changed state")
                        });
                    }
                    CacheClaim::Failed(diagnostics) => {
                        return Err(internal_diagnostic_error(py, &diagnostics));
                    }
                    CacheClaim::Wait => {
                        let cache = Arc::clone(&self.result_cache);
                        py.detach(move || cache.wait_for_materializer());
                    }
                    CacheClaim::Materialize => break,
                }
            }
            let materialization = MaterializationClaim::new(Arc::clone(&self.result_cache));

            let Some(result) = self.shared.take_completed() else {
                return Err(internal_error(
                    py,
                    "the completed native Result payload was unavailable",
                ));
            };
            let projected = catch_native_panic(|| materialize_result(py, result, &self.identity));
            match projected {
                Ok(Ok(result)) => {
                    materialization.commit(result.clone_ref(py));
                    Ok(result)
                }
                Ok(Err(_)) | Err(_) => {
                    let diagnostics = vec![result_materialization_diagnostic()];
                    materialization.fail(diagnostics.clone());
                    Err(internal_diagnostic_error(py, &diagnostics))
                }
            }
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Run(status={:?}, model_digest={:?}, plan_key={:?})",
            self.status(),
            self.model_digest(),
            self.plan_key()
        )
    }
}

fn materialize_result(
    py: Python<'_>,
    result: NativeRunOutput,
    identity: &RunIdentity,
) -> PyResult<Py<PyAny>> {
    match result {
        NativeRunOutput::Reference(result) => result_into_python(py, result, identity.clone())
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
        NativeRunOutput::ScalarElliptic(result) => PyScalarEllipticResult::from_result(py, *result)
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
    }
}

enum NativeWorkerOutcome {
    Completed(NativeRunOutput),
    Cancelled(NativeRunCancellation),
}

fn execute_job(
    job: NativeRunJob,
    shared: &Arc<RunShared>,
) -> Result<NativeWorkerOutcome, Vec<Diagnostic>> {
    match job {
        NativeRunJob::Reference { document, plan } => {
            let mut observer = ReferenceSharedObserver::new(Arc::clone(shared));
            match document.run_reference_plan_controlled(plan, &mut observer)? {
                ReferenceRunOutcome::Completed(result) => Ok(NativeWorkerOutcome::Completed(
                    NativeRunOutput::Reference(result),
                )),
                ReferenceRunOutcome::Cancelled(cancellation) => Ok(NativeWorkerOutcome::Cancelled(
                    NativeRunCancellation::Reference(cancellation),
                )),
            }
        }
        NativeRunJob::ScalarElliptic {
            document,
            plan,
            environment,
        } => {
            let mut observer = ScalarEllipticSharedObserver::new(Arc::clone(shared));
            match document.run_scalar_elliptic_plan_controlled(*plan, environment, &mut observer)? {
                ScalarEllipticRunOutcome::Completed(result) => Ok(NativeWorkerOutcome::Completed(
                    NativeRunOutput::ScalarElliptic(result),
                )),
                ScalarEllipticRunOutcome::Cancelled(cancellation) => {
                    Ok(NativeWorkerOutcome::Cancelled(
                        NativeRunCancellation::ScalarElliptic(cancellation),
                    ))
                }
            }
        }
    }
}

fn run_worker(job: NativeRunJob, shared: Arc<RunShared>) {
    shared.mark_running();
    let outcome = catch_native_panic(|| execute_job(job, &shared));
    match outcome {
        Ok(Ok(NativeWorkerOutcome::Completed(result))) => {
            shared.finish(RunTerminal::Completed(Some(result)));
        }
        Ok(Ok(NativeWorkerOutcome::Cancelled(cancellation))) => {
            shared.finish(RunTerminal::Cancelled(cancellation));
        }
        Ok(Err(diagnostics)) => {
            shared.finish(RunTerminal::Failed(RunFailure::Execution(diagnostics)));
        }
        Err(diagnostic) => {
            shared.finish(RunTerminal::Failed(RunFailure::Internal(vec![diagnostic])));
        }
    }
}

#[pyfunction]
#[pyo3(signature = (model, *, end_time, max_step))]
pub(crate) fn submit(
    py: Python<'_>,
    model: &PyModel,
    end_time: f64,
    max_step: f64,
) -> PyResult<PyRun> {
    panic_boundary(py, || {
        PyRun::submit_reference(py, model, end_time, max_step)
    })
}

#[pyfunction]
pub(crate) fn submit_realization(
    py: Python<'_>,
    model: &PyModel,
    realization: &PyRealization,
) -> PyResult<PyRun> {
    panic_boundary(py, || PyRun::submit_scalar_elliptic(py, model, realization))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRunStatus>()?;
    module.add_class::<PyRunProgress>()?;
    module.add_class::<PyRunCancellation>()?;
    module.add_class::<PyScalarEllipticRunProgress>()?;
    module.add_class::<PyScalarEllipticRunCancellation>()?;
    module.add_class::<PyRun>()?;
    module.add_function(wrap_pyfunction!(submit, module)?)?;
    module.add_function(wrap_pyfunction!(submit_realization, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use eqiora::Diagnostic;
    use eqiora::diagnostic::codes;

    use super::{
        CacheClaim, MaterializationClaim, PyRunStatus, ResultCache, RunFailure, RunShared,
        RunState, RunTerminal, RunTerminalKind, progress_publication_due,
    };

    #[test]
    fn run_states_follow_the_explicit_branching_transition_table() {
        let mut completed = RunState::accepted();
        completed.transition(PyRunStatus::Running);
        completed.transition(PyRunStatus::Cancelling);
        completed.transition(PyRunStatus::Completed);
        assert_eq!(
            completed.history,
            [
                PyRunStatus::Created,
                PyRunStatus::Validating,
                PyRunStatus::Queued,
                PyRunStatus::Running,
                PyRunStatus::Cancelling,
                PyRunStatus::Completed,
            ]
        );
        assert!(!completed.integrity_failed);

        completed.transition(PyRunStatus::Cancelled);
        assert!(completed.integrity_failed);
        assert_eq!(completed.status, PyRunStatus::Failed);
        assert!(matches!(
            completed.terminal,
            Some(RunTerminal::Failed(RunFailure::Internal(_)))
        ));

        let mut cancelled = RunState::accepted();
        cancelled.transition(PyRunStatus::Cancelling);
        cancelled.transition(PyRunStatus::Cancelled);
        assert_eq!(cancelled.history.len(), 5);
        assert!(!cancelled.integrity_failed);
    }

    #[test]
    fn every_waiter_observes_one_failed_terminal() {
        let shared = Arc::new(RunShared::new());
        let waiters: Vec<_> = (0..2)
            .map(|_| {
                let shared = Arc::clone(&shared);
                thread::spawn(move || shared.wait_terminal_kind())
            })
            .collect();
        shared.finish(RunTerminal::Failed(RunFailure::Execution(vec![
            Diagnostic::error(codes::NONSQUARE_SYSTEM, "probe"),
        ])));
        for waiter in waiters {
            assert!(matches!(
                waiter.join().expect("waiter must not panic"),
                RunTerminalKind::Failed(RunFailure::Execution(_))
            ));
        }
    }

    #[test]
    fn abandoned_result_materialization_fails_and_wakes_future_callers() {
        let cache = Arc::new(ResultCache::new());
        assert!(matches!(cache.claim(), CacheClaim::Materialize));
        {
            let _claim = MaterializationClaim::new(Arc::clone(&cache));
        }
        let CacheClaim::Failed(diagnostics) = cache.claim() else {
            panic!("an abandoned materializer must leave one stable failure");
        };
        assert_eq!(diagnostics[0].code().to_string(), "EQ0002");
        assert_eq!(
            diagnostics[0].message(),
            "the completed native Result could not be materialized"
        );
    }

    #[test]
    fn progress_policy_coalesces_until_the_interval_or_cancellation() {
        let start = std::time::Instant::now();
        assert!(progress_publication_due(None, start, false));
        assert!(!progress_publication_due(
            Some(start),
            start + std::time::Duration::from_millis(99),
            false
        ));
        assert!(progress_publication_due(
            Some(start),
            start + std::time::Duration::from_millis(100),
            false
        ));
        assert!(progress_publication_due(Some(start), start, true));
    }
}
