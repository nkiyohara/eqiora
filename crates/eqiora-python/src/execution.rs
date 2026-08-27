//! Bounded Python lifecycle over the shared semantic reference executor.

mod evidence;
mod worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use eqiora::api::{
    FixedReferenceFsiResult2d, ReferenceRunCancellation, ReferenceRunPlan, ReferenceRunProgress,
    ReferenceRunResult, ScalarEllipticExecutionEnvironment, ScalarEllipticRunCancellation,
    ScalarEllipticRunProgress, ScalarEllipticRunResult,
};
use eqiora::diagnostic::codes;
use eqiora::{Diagnostic, GraphPath};
use eqiora_numerics::{CommonState, CommonTransientRunRequest};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::PyModel;
use crate::common_plan::{CommonPlanKind, PyPlan};
use crate::elasticity::{
    PyLinearElasticityPlan, materialize_result as materialize_linear_elasticity,
};
use crate::error::{
    cancellation_error, catch_native_panic, diagnostic_error, execution_error,
    internal_diagnostic_error, internal_error, panic_boundary, validation_error,
};
use crate::fsi::{
    PyFixedMeshMonolithicPlan, materialize_result as materialize_fixed_mesh_monolithic,
};
use crate::meshing::PyMesh;
use crate::realization::{PyRealization, PyScalarEllipticResult};
use crate::result::result_into_python;
use crate::steady_stokes::{
    PySteadyStokesPlan, SteadyStokesRunMaterialization, materialize_result as materialize_stokes,
};
use crate::trajectory::PyState;

pub(crate) use evidence::RunIdentity;
use evidence::{
    PyCommonTransientRunCancellation, PyCommonTransientRunProgress, PyRunCancellation,
    PyRunProgress, PyRunStatus, PyScalarEllipticRunCancellation, PyScalarEllipticRunProgress,
};
use worker::{NativeRunJob, run_worker};

const RESULT_MATERIALIZATION_FAILURE: &str =
    "the completed native Result could not be materialized";

#[derive(Debug, Clone)]
enum RunFailure {
    Execution(Vec<Diagnostic>),
    Internal(Vec<Diagnostic>),
}

#[derive(Debug, Clone)]
enum NativeRunProgress {
    Reference(ReferenceRunProgress),
    ScalarElliptic(ScalarEllipticRunProgress),
    CommonTransient(PyCommonTransientRunProgress),
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
            Self::CommonTransient(progress) => Py::new(py, progress).map(Py::into_any),
        }
    }
}

#[derive(Debug)]
enum NativeRunOutput {
    Reference(ReferenceRunResult),
    ScalarElliptic(Box<ScalarEllipticRunResult>),
    SteadyStokes {
        result: Box<SteadyStokesRunMaterialization>,
        elapsed_seconds: f64,
    },
    LinearElasticity {
        result: Box<eqiora::api::MixedBoundaryElasticityResult2d>,
        elapsed_seconds: f64,
    },
    FixedMeshMonolithic {
        result: Box<FixedReferenceFsiResult2d>,
        elapsed_seconds: f64,
    },
    CommonScalar {
        result: Box<eqiora_numerics::scalar::ResolvedScalarEllipticCartesianSolution>,
        elapsed_seconds: f64,
    },
    CommonElasticity {
        result: Box<eqiora_numerics::solid::CartesianLinearElasticity2dSolution>,
        elapsed_seconds: f64,
    },
    CommonSteadyStokes {
        result: Box<eqiora_numerics::fluid::SteadyStokesMiniSolution2d>,
        elapsed_seconds: f64,
    },
    CommonTransient {
        states: Vec<(usize, CommonState)>,
        elapsed_seconds: f64,
    },
}

enum ResultMaterializationContext {
    None,
    SteadyStokes { mesh: Py<PyMesh> },
    FixedMeshMonolithic { model: Py<PyModel> },
    CommonPlan { plan: Py<PyPlan> },
}

#[derive(Debug, Clone)]
enum NativeRunCancellation {
    Reference(ReferenceRunCancellation),
    ScalarElliptic(Box<ScalarEllipticRunCancellation>),
    CommonTransient {
        accepted_steps: usize,
        maximum_steps: usize,
        model_time_s: f64,
        request_identity: String,
    },
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
            Self::CommonTransient {
                accepted_steps,
                maximum_steps,
                model_time_s,
                request_identity,
            } => Py::new(
                py,
                PyCommonTransientRunCancellation {
                    progress: PyCommonTransientRunProgress {
                        accepted_steps,
                        maximum_steps,
                        model_time_bits: model_time_s.to_bits(),
                    },
                    request_identity,
                },
            )
            .map(Py::into_any),
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
            Self::CommonTransient {
                accepted_steps,
                maximum_steps: _,
                model_time_s,
                request_identity,
            } => format!(
                "common transient execution {request_identity} was cancelled at accepted model time {model_time_s} after {accepted_steps} accepted steps"
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

/// One process-local handle for an accepted native execution occurrence.
#[pyclass(name = "Run", module = "eqiora._eqiora", frozen, skip_from_py_object)]
pub(crate) struct PyRun {
    identity: RunIdentity,
    materialization: ResultMaterializationContext,
    shared: Arc<RunShared>,
    result_cache: Arc<ResultCache>,
}

impl PyRun {
    fn submit_common(
        py: Python<'_>,
        plan: Py<PyPlan>,
        request: Option<CommonTransientRunRequest>,
    ) -> PyResult<Self> {
        let plan_ref = plan.borrow(py);
        let (identity, job, thread_name) = match (plan_ref.native(), request) {
            (CommonPlanKind::Scalar(native), None) => (
                RunIdentity::from_common_plan(native),
                NativeRunJob::CommonScalar(native.clone()),
                "eqiora-common-scalar-run",
            ),
            (CommonPlanKind::Elasticity(native), None) => (
                RunIdentity::from_common_elasticity(native),
                NativeRunJob::CommonElasticity(native.clone()),
                "eqiora-common-elasticity-run",
            ),
            (CommonPlanKind::SteadyStokes(native), None) => (
                RunIdentity::from_common_steady_stokes(native),
                NativeRunJob::CommonSteadyStokes(native.clone()),
                "eqiora-common-steady-stokes-run",
            ),
            (CommonPlanKind::TransientFlow(_), Some(request)) => (
                RunIdentity::from_common_transient(&request),
                NativeRunJob::CommonTransient(Box::new(request)),
                "eqiora-common-transient-run",
            ),
            (CommonPlanKind::TransientFlow(_), None) => {
                return Err(PyTypeError::new_err(
                    "transient submit requires State and one explicit horizon/output schedule family",
                ));
            }
            (
                CommonPlanKind::Scalar(_)
                | CommonPlanKind::Elasticity(_)
                | CommonPlanKind::SteadyStokes(_),
                Some(_),
            ) => {
                return Err(PyTypeError::new_err(
                    "steady submit accepts Plan alone and no transient Run controls",
                ));
            }
        };
        drop(plan_ref);
        Self::spawn(
            identity,
            job,
            ResultMaterializationContext::CommonPlan { plan },
            thread_name,
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_reference(
        py: Python<'_>,
        model: &PyModel,
        end_time: f64,
        max_step: f64,
    ) -> PyResult<Self> {
        let plan = ReferenceRunPlan::new(end_time, max_step)
            .map_err(|diagnostic| execution_error(py, &[diagnostic]))?;
        let document = model
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
        let identity = RunIdentity::from_reference(&document, &plan)
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?;
        Self::spawn(
            identity,
            NativeRunJob::Reference { document, plan },
            ResultMaterializationContext::None,
            "eqiora-reference-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_scalar_elliptic(
        py: Python<'_>,
        model: &PyModel,
        realization: &PyRealization,
    ) -> PyResult<Self> {
        let document = model
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
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
            ResultMaterializationContext::None,
            "eqiora-scalar-elliptic-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_steady_stokes(
        py: Python<'_>,
        model: &PyModel,
        plan: &PySteadyStokesPlan,
    ) -> PyResult<Self> {
        let identity = RunIdentity::from_steady_stokes(model.artifact(), plan.native())
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
        Self::spawn(
            identity,
            NativeRunJob::SteadyStokes(Box::new(plan.native().clone())),
            ResultMaterializationContext::SteadyStokes {
                mesh: plan.mesh(py),
            },
            "eqiora-steady-stokes-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_linear_elasticity(
        py: Python<'_>,
        model: &PyModel,
        plan: &PyLinearElasticityPlan,
    ) -> PyResult<Self> {
        let identity = RunIdentity::from_linear_elasticity(model.artifact(), plan.native())
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
        Self::spawn(
            identity,
            NativeRunJob::LinearElasticity(Box::new(plan.native().clone())),
            ResultMaterializationContext::None,
            "eqiora-linear-elasticity-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn submit_fixed_mesh_monolithic(
        py: Python<'_>,
        model: &PyModel,
        plan: &PyFixedMeshMonolithicPlan,
    ) -> PyResult<Self> {
        let identity = RunIdentity::from_fixed_mesh_monolithic(model.artifact(), plan.native())
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
        Self::spawn(
            identity,
            NativeRunJob::FixedMeshMonolithic(Box::new(plan.native().clone())),
            ResultMaterializationContext::FixedMeshMonolithic {
                model: plan.model(py),
            },
            "eqiora-fixed-mesh-monolithic-fsi-run",
        )
        .map_err(|diagnostics| internal_diagnostic_error(py, &diagnostics))
    }

    fn spawn(
        identity: RunIdentity,
        job: NativeRunJob,
        materialization: ResultMaterializationContext,
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
            materialization,
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
        let progress = { self.shared.state().progress.clone() };
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
            let projected = catch_native_panic(|| {
                materialize_result(py, result, &self.identity, &self.materialization)
            });
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
    context: &ResultMaterializationContext,
) -> PyResult<Py<PyAny>> {
    match result {
        NativeRunOutput::Reference(result) => result_into_python(py, result, identity.clone())
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
        NativeRunOutput::ScalarElliptic(result) => PyScalarEllipticResult::from_result(py, *result)
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
        NativeRunOutput::SteadyStokes {
            result,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::SteadyStokes { mesh } => materialize_stokes(
                py,
                *result,
                identity.clone(),
                elapsed_seconds,
                mesh.clone_ref(py),
            )
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
            ResultMaterializationContext::None
            | ResultMaterializationContext::FixedMeshMonolithic { .. }
            | ResultMaterializationContext::CommonPlan { .. } => Err(internal_error(
                py,
                "steady-Stokes Result lost its accepted Mesh context",
            )),
        },
        NativeRunOutput::LinearElasticity {
            result,
            elapsed_seconds,
        } => materialize_linear_elasticity(py, *result, identity.clone(), elapsed_seconds)
            .and_then(|result| Py::new(py, result))
            .map(Py::into_any),
        NativeRunOutput::FixedMeshMonolithic {
            result,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::FixedMeshMonolithic { model } => {
                materialize_fixed_mesh_monolithic(
                    py,
                    *result,
                    identity.clone(),
                    elapsed_seconds,
                    model.borrow(py),
                )
                .and_then(|result| Py::new(py, result))
                .map(Py::into_any)
            }
            ResultMaterializationContext::None
            | ResultMaterializationContext::SteadyStokes { .. }
            | ResultMaterializationContext::CommonPlan { .. } => Err(internal_error(
                py,
                "fixed-mesh monolithic FSI Result lost its accepted Model context",
            )),
        },
        NativeRunOutput::CommonScalar {
            result,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::CommonPlan { plan } => {
                crate::result::materialize_common_scalar(
                    py,
                    plan.borrow(py),
                    identity.clone(),
                    elapsed_seconds,
                    *result,
                )
                .and_then(|result| Py::new(py, result))
                .map(Py::into_any)
            }
            _ => Err(internal_error(
                py,
                "common scalar Result lost its exact Plan",
            )),
        },
        NativeRunOutput::CommonElasticity {
            result,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::CommonPlan { plan } => {
                crate::result::materialize_common_elasticity(
                    py,
                    plan.borrow(py),
                    identity.clone(),
                    elapsed_seconds,
                    *result,
                )
                .and_then(|result| Py::new(py, result))
                .map(Py::into_any)
            }
            _ => Err(internal_error(
                py,
                "common elasticity Result lost its exact Plan",
            )),
        },
        NativeRunOutput::CommonSteadyStokes {
            result,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::CommonPlan { plan } => {
                crate::result::materialize_common_steady_stokes(
                    py,
                    plan.borrow(py),
                    identity.clone(),
                    elapsed_seconds,
                    *result,
                )
                .and_then(|result| Py::new(py, result))
                .map(Py::into_any)
            }
            _ => Err(internal_error(
                py,
                "common steady-Stokes Result lost its exact Plan",
            )),
        },
        NativeRunOutput::CommonTransient {
            states,
            elapsed_seconds,
        } => match context {
            ResultMaterializationContext::CommonPlan { plan } => {
                crate::result::materialize_common_transient(
                    py,
                    plan.borrow(py),
                    identity.clone(),
                    elapsed_seconds,
                    states,
                )
                .and_then(|result| Py::new(py, result))
                .map(Py::into_any)
            }
            _ => Err(internal_error(
                py,
                "common transient Result lost its exact Plan",
            )),
        },
    }
}

#[pyfunction]
#[pyo3(signature = (plan, /, *, state=None, until_s=None, output_times_s=None, steps=None, output_steps=None))]
pub(crate) fn submit_plan(
    py: Python<'_>,
    plan: Py<PyPlan>,
    state: Option<&Bound<'_, PyState>>,
    until_s: Option<f64>,
    output_times_s: Option<Vec<f64>>,
    steps: Option<usize>,
    output_steps: Option<Vec<usize>>,
) -> PyResult<PyRun> {
    panic_boundary(py, || {
        let plan_ref = plan.borrow(py);
        let request = match plan_ref.transient_native() {
            None => {
                if state.is_some()
                    || until_s.is_some()
                    || output_times_s.is_some()
                    || steps.is_some()
                    || output_steps.is_some()
                {
                    return Err(PyTypeError::new_err(
                        "steady submit accepts Plan alone and no transient Run controls",
                    ));
                }
                None
            }
            Some(native_plan) => {
                let state = state.ok_or_else(|| {
                    PyTypeError::new_err("transient submit requires state=State(...)")
                })?;
                let state = state.borrow();
                let native_state = state.common_native().ok_or_else(|| {
                    PyValueError::new_err("State is not a common transient restart State")
                })?;
                if native_state.state_space_identity() != native_plan.state_space_identity() {
                    return Err(PyValueError::new_err(
                        "State belongs to a different exact common state space",
                    ));
                }
                match (until_s, output_times_s, steps, output_steps) {
                    (Some(until), Some(outputs), None, None) => Some(
                        CommonTransientRunRequest::from_times(
                            native_plan.clone(),
                            native_state.clone(),
                            until,
                            outputs,
                        )
                        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?,
                    ),
                    (None, None, Some(steps), Some(outputs)) => Some(
                        CommonTransientRunRequest::from_steps(
                            native_plan.clone(),
                            native_state.clone(),
                            steps,
                            outputs,
                        )
                        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?,
                    ),
                    _ => {
                        return Err(PyTypeError::new_err(
                            "transient submit requires exactly one complete until_s/output_times_s or steps/output_steps family",
                        ));
                    }
                }
            }
        };
        drop(plan_ref);
        PyRun::submit_common(py, plan, request)
    })
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

#[pyfunction]
pub(crate) fn submit_steady_stokes(
    py: Python<'_>,
    model: &PyModel,
    plan: &PySteadyStokesPlan,
) -> PyResult<PyRun> {
    panic_boundary(py, || PyRun::submit_steady_stokes(py, model, plan))
}

#[pyfunction]
pub(crate) fn submit_linear_elasticity(
    py: Python<'_>,
    model: &PyModel,
    plan: &PyLinearElasticityPlan,
) -> PyResult<PyRun> {
    panic_boundary(py, || PyRun::submit_linear_elasticity(py, model, plan))
}

#[pyfunction]
pub(crate) fn submit_fixed_mesh_monolithic(
    py: Python<'_>,
    model: &PyModel,
    plan: &PyFixedMeshMonolithicPlan,
) -> PyResult<PyRun> {
    panic_boundary(py, || PyRun::submit_fixed_mesh_monolithic(py, model, plan))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRunStatus>()?;
    module.add_class::<PyRunProgress>()?;
    module.add_class::<PyRunCancellation>()?;
    module.add_class::<PyScalarEllipticRunProgress>()?;
    module.add_class::<PyScalarEllipticRunCancellation>()?;
    module.add_class::<PyCommonTransientRunProgress>()?;
    module.add_class::<PyCommonTransientRunCancellation>()?;
    module.add_class::<PyRun>()?;
    module.add_function(wrap_pyfunction!(submit, module)?)?;
    module.add_function(wrap_pyfunction!(submit_realization, module)?)?;
    module.add_function(wrap_pyfunction!(submit_steady_stokes, module)?)?;
    module.add_function(wrap_pyfunction!(submit_linear_elasticity, module)?)?;
    module.add_function(wrap_pyfunction!(submit_fixed_mesh_monolithic, module)?)?;
    module.add_function(wrap_pyfunction!(submit_plan, module)?)?;
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
        RunState, RunTerminal, RunTerminalKind,
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
}
