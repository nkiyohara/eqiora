//! Opaque Python projections of accepted portable Realizations and Run evidence.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::api::{
    ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
    ScalarEllipticRunPlan, ScalarEllipticRunResult, ScalarFieldLocation,
};
use eqiora::artifact::{ExecutionTopologyV1, JsonDecoderLimits, RunManifestV2};
use eqiora::diagnostic::codes;
use eqiora::realization::RealizationRevision;
use eqiora::solver::{
    ConvergenceReason, LinearOperatorOrientation, LinearSolver, PreconditionerPolicy,
    ReductionPolicy, SolveReport,
};
use eqiora::{Diagnostic, GraphPath};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule, PyTuple};

use crate::array::PyArrayBuffer;
use crate::error::{
    compatibility_error, diagnostic_error, internal_diagnostic_error, panic_boundary,
    validation_error,
};
use crate::model::PyModel;

/// Numerical family selected by one bounded scalar-elliptic request.
#[pyclass(
    name = "ScalarEllipticMethod",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyScalarEllipticMethod {
    /// Continuous Q1 Galerkin finite elements.
    FiniteElement,
    /// Cell-centred finite volumes.
    FiniteVolume,
}

impl From<PyScalarEllipticMethod> for ScalarEllipticMethod {
    fn from(value: PyScalarEllipticMethod) -> Self {
        match value {
            PyScalarEllipticMethod::FiniteElement => Self::FiniteElement,
            PyScalarEllipticMethod::FiniteVolume => Self::FiniteVolume,
        }
    }
}

impl From<ScalarEllipticMethod> for PyScalarEllipticMethod {
    fn from(value: ScalarEllipticMethod) -> Self {
        match value {
            ScalarEllipticMethod::FiniteElement => Self::FiniteElement,
            ScalarEllipticMethod::FiniteVolume => Self::FiniteVolume,
        }
    }
}

/// One unbound typed application request, not accepted Realization identity.
#[pyclass(
    name = "ScalarElliptic",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyScalarElliptic {
    revision: u64,
    method: PyScalarEllipticMethod,
    cells_per_axis: NonZeroUsize,
}

#[pymethods]
impl PyScalarElliptic {
    #[new]
    #[pyo3(signature = (*, method, cells_per_axis, realization_revision=1))]
    fn new(
        py: Python<'_>,
        method: PyScalarEllipticMethod,
        cells_per_axis: usize,
        realization_revision: u64,
    ) -> PyResult<Self> {
        Ok(Self {
            revision: realization_revision,
            method,
            cells_per_axis: nonzero(py, "cells_per_axis", cells_per_axis)?,
        })
    }

    #[getter]
    const fn realization_revision(&self) -> u64 {
        self.revision
    }

    #[getter]
    const fn method(&self) -> PyScalarEllipticMethod {
        self.method
    }

    #[getter]
    const fn cells_per_axis(&self) -> usize {
        self.cells_per_axis.get()
    }

    fn __repr__(&self) -> String {
        format!(
            "ScalarElliptic(method={:?}, cells_per_axis={}, realization_revision={})",
            self.method, self.cells_per_axis, self.revision
        )
    }
}

impl PyScalarElliptic {
    fn intent(&self) -> ScalarEllipticIntent {
        ScalarEllipticIntent::new(
            RealizationRevision::new(self.revision),
            self.method.into(),
            self.cells_per_axis,
            NonZeroUsize::MIN,
        )
    }
}

/// One exact model-bound, capability-admitted portable Realization.
#[pyclass(
    name = "Realization",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyRealization {
    plan: ScalarEllipticRunPlan,
}

impl PyRealization {
    pub(crate) const fn plan(&self) -> &ScalarEllipticRunPlan {
        &self.plan
    }
}

#[pymethods]
impl PyRealization {
    /// Exact content-addressed Realization artifact digest.
    #[getter]
    fn digest(&self) -> &str {
        self.plan.key()
    }

    /// Exact Model artifact digest resolved by this Realization.
    #[getter]
    fn model_digest(&self) -> &str {
        self.plan.model_digest()
    }

    #[getter]
    const fn realization_revision(&self) -> u64 {
        self.plan.intent().realization_revision().get()
    }

    #[getter]
    fn method(&self) -> PyScalarEllipticMethod {
        self.plan.intent().method().into()
    }

    #[getter]
    const fn cells_per_axis(&self) -> usize {
        self.plan.intent().cells_per_axis().get()
    }

    #[getter]
    const fn workers(&self) -> usize {
        self.plan.intent().workers().get()
    }

    #[getter]
    const fn cell_count(&self) -> usize {
        self.plan.cell_count()
    }

    #[getter]
    const fn field_value_count(&self) -> usize {
        self.plan.field_value_count()
    }

    /// Runtime spatial dimension admitted by the exact Realization.
    #[getter]
    fn spatial_dimension(&self) -> usize {
        self.plan.requirements().spatial_dimension().get()
    }

    /// Planned primary Field extents in canonical Cartesian axis order.
    #[getter]
    fn field_logical_shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let extent = match self.plan.intent().method() {
            ScalarEllipticMethod::FiniteElement => self.plan.intent().cells_per_axis().get() + 1,
            ScalarEllipticMethod::FiniteVolume => self.plan.intent().cells_per_axis().get(),
        };
        PyTuple::new(py, std::iter::repeat_n(extent, self.spatial_dimension()))
    }

    /// Accepted coherent-SI coordinate bounds in canonical Cartesian axis order.
    ///
    /// Each entry is one closed `(lower, upper)` extent of the resolved volume
    /// Domain. Together with `field_logical_shape` and the Field location this
    /// is exactly the geometry needed to place every published value, so a
    /// client can compare an accepted Field against a known exact solution
    /// without restating the Domain that the Model already declares.
    #[getter]
    fn field_bounds<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            self.plan
                .field_projection()
                .bounds()
                .iter()
                .map(|extent| PyTuple::new(py, extent))
                .collect::<PyResult<Vec<_>>>()?,
        )
    }

    /// Canonical persisted Realization artifact bytes.
    fn to_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        panic_boundary(py, || {
            self.plan
                .artifact()
                .canonical_json()
                .map(|bytes| PyBytes::new(py, &bytes))
                .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.digest() == other.digest())
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        hash_value(&self.digest()) as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "Realization(method={:?}, cells_per_axis={}, workers={}, digest={:?})",
            self.method(),
            self.cells_per_axis(),
            self.workers(),
            self.digest()
        )
    }
}

/// Vertex or cell-centre meaning of one accepted scalar field summary.
#[pyclass(
    name = "ScalarFieldLocation",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyScalarFieldLocation {
    Vertex,
    CellCenter,
}

impl From<ScalarFieldLocation> for PyScalarFieldLocation {
    fn from(value: ScalarFieldLocation) -> Self {
        match value {
            ScalarFieldLocation::Vertex => Self::Vertex,
            ScalarFieldLocation::CellCenter => Self::CellCenter,
        }
    }
}

/// Accepted bounded field summary; complete arrays remain on the data plane.
#[pyclass(
    name = "ScalarFieldSummary",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyScalarFieldSummary {
    location: PyScalarFieldLocation,
    spatial_dimension: usize,
    logical_shape: [usize; 3],
    value_count: usize,
    minimum: f64,
    maximum: f64,
}

#[pymethods]
impl PyScalarFieldSummary {
    /// The extent of the accepted field, and where its values live.
    fn __repr__(&self) -> String {
        format!(
            "ScalarFieldSummary(minimum={:e}, maximum={:e}, value_count={}, location={:?})",
            self.minimum, self.maximum, self.value_count, self.location
        )
    }
    #[getter]
    const fn location(&self) -> PyScalarFieldLocation {
        self.location
    }

    #[getter]
    const fn spatial_dimension(&self) -> usize {
        self.spatial_dimension
    }

    #[getter]
    fn logical_shape<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, &self.logical_shape[..self.spatial_dimension])
    }

    #[getter]
    const fn value_count(&self) -> usize {
        self.value_count
    }

    #[getter]
    const fn minimum(&self) -> f64 {
        self.minimum
    }

    #[getter]
    const fn maximum(&self) -> f64 {
        self.maximum
    }
}

/// Accepted continuous balance evidence.
#[pyclass(
    name = "ScalarEllipticBalance",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyScalarEllipticBalance {
    boundary_total: f64,
    integrated_source: f64,
    relative_imbalance: f64,
}

#[pymethods]
impl PyScalarEllipticBalance {
    /// The conservation check, in full: it has only three numbers.
    fn __repr__(&self) -> String {
        format!(
            "ScalarEllipticBalance(integrated_source={:e}, boundary_total={:e}, relative_imbalance={:e})",
            self.integrated_source, self.boundary_total, self.relative_imbalance
        )
    }
    #[getter]
    const fn boundary_total(&self) -> f64 {
        self.boundary_total
    }

    #[getter]
    const fn integrated_source(&self) -> f64 {
        self.integrated_source
    }

    #[getter]
    const fn relative_imbalance(&self) -> f64 {
        self.relative_imbalance
    }
}

/// Accepted linear convergence reason.
#[pyclass(
    name = "ConvergenceReason",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyConvergenceReason {
    InitialResidualSatisfied,
    ResidualToleranceSatisfied,
}

impl From<ConvergenceReason> for PyConvergenceReason {
    fn from(value: ConvergenceReason) -> Self {
        match value {
            ConvergenceReason::InitialResidualSatisfied => Self::InitialResidualSatisfied,
            ConvergenceReason::ResidualToleranceSatisfied => Self::ResidualToleranceSatisfied,
        }
    }
}

/// Bounded projection of the independently accepted linear solve report.
#[pyclass(
    name = "LinearSolveSummary",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyLinearSolveSummary {
    backend: String,
    adapter: String,
    verification_adapter: String,
    orientation: String,
    algorithm: String,
    preconditioner: String,
    reduction: String,
    relative_tolerance: f64,
    absolute_tolerance: f64,
    maximum_iterations: usize,
    reason: PyConvergenceReason,
    completed_iterations: usize,
    initial_residual_norm: f64,
    reported_residual_norm: f64,
    true_residual_norm: f64,
    residual_target: f64,
}

impl PyLinearSolveSummary {
    pub(crate) fn from_report(report: &SolveReport) -> Self {
        let plan = report.solver_plan();
        Self {
            backend: report.backend().as_str().to_owned(),
            adapter: report.execution().adapter().as_str().to_owned(),
            verification_adapter: report.verification().adapter().as_str().to_owned(),
            orientation: match report.orientation() {
                LinearOperatorOrientation::Normal => "normal",
                LinearOperatorOrientation::Transposed => "transposed",
            }
            .to_owned(),
            algorithm: match report.algorithm() {
                LinearSolver::ConjugateGradient => "conjugate-gradient",
                LinearSolver::MinimumResidual => "minimum-residual",
                LinearSolver::BiConjugateGradientStabilized => "bicgstab",
            }
            .to_owned(),
            preconditioner: match report.preconditioner() {
                PreconditionerPolicy::Identity => "identity",
                PreconditionerPolicy::Jacobi => "jacobi",
            }
            .to_owned(),
            reduction: match report.reduction() {
                ReductionPolicy::Reproducible => "reproducible",
                ReductionPolicy::Fast => "fast",
            }
            .to_owned(),
            relative_tolerance: plan.relative_tolerance(),
            absolute_tolerance: plan.absolute_tolerance(),
            maximum_iterations: plan.maximum_iterations().get(),
            reason: report.reason().into(),
            completed_iterations: report.completed_iterations(),
            initial_residual_norm: report.initial_residual_norm(),
            reported_residual_norm: report.reported_residual_norm(),
            true_residual_norm: report.true_residual_norm(),
            residual_target: report.residual_target(),
        }
    }
}

#[pymethods]
impl PyLinearSolveSummary {
    /// What the solve did, and whether its true residual met the target.
    fn __repr__(&self) -> String {
        format!(
            "LinearSolveSummary(algorithm={:?}, preconditioner={:?}, completed_iterations={}, true_residual_norm={:e}, residual_target={:e})",
            self.algorithm,
            self.preconditioner,
            self.completed_iterations,
            self.true_residual_norm,
            self.residual_target
        )
    }
    #[getter]
    fn backend(&self) -> &str {
        &self.backend
    }

    #[getter]
    fn adapter(&self) -> &str {
        &self.adapter
    }

    #[getter]
    fn verification_adapter(&self) -> &str {
        &self.verification_adapter
    }

    #[getter]
    fn orientation(&self) -> &str {
        &self.orientation
    }

    #[getter]
    fn algorithm(&self) -> &str {
        &self.algorithm
    }

    #[getter]
    fn preconditioner(&self) -> &str {
        &self.preconditioner
    }

    #[getter]
    fn reduction(&self) -> &str {
        &self.reduction
    }

    #[getter]
    const fn relative_tolerance(&self) -> f64 {
        self.relative_tolerance
    }

    #[getter]
    const fn absolute_tolerance(&self) -> f64 {
        self.absolute_tolerance
    }

    #[getter]
    const fn maximum_iterations(&self) -> usize {
        self.maximum_iterations
    }

    #[getter]
    const fn reason(&self) -> PyConvergenceReason {
        self.reason
    }

    #[getter]
    const fn completed_iterations(&self) -> usize {
        self.completed_iterations
    }

    #[getter]
    const fn initial_residual_norm(&self) -> f64 {
        self.initial_residual_norm
    }

    #[getter]
    const fn reported_residual_norm(&self) -> f64 {
        self.reported_residual_norm
    }

    #[getter]
    const fn true_residual_norm(&self) -> f64 {
        self.true_residual_norm
    }

    #[getter]
    const fn residual_target(&self) -> f64 {
        self.residual_target
    }
}

/// Persisted exact Run v2 manifest linked to one accepted Realization.
#[pyclass(
    name = "RunManifest",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyRunManifest {
    value: RunManifestV2,
    digest: String,
}

impl PyRunManifest {
    fn from_value(py: Python<'_>, value: RunManifestV2) -> PyResult<Self> {
        let digest = value
            .digest()
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?
            .to_string();
        Ok(Self { value, digest })
    }
}

#[pymethods]
impl PyRunManifest {
    /// Decode and validate a persisted Run against its exact Realization.
    #[staticmethod]
    #[pyo3(signature = (data, *, realization))]
    fn from_json(py: Python<'_>, data: &[u8], realization: &PyRealization) -> PyResult<Self> {
        panic_boundary(py, || {
            let value = realization
                .plan
                .replay_run_manifest(data, JsonDecoderLimits::default())
                .map_err(|diagnostic| compatibility_error(py, &[diagnostic]))?;
            Self::from_value(py, value)
        })
    }

    #[getter]
    fn digest(&self) -> &str {
        &self.digest
    }

    #[getter]
    fn model_digest(&self) -> String {
        self.value.model().to_string()
    }

    #[getter]
    fn realization_digest(&self) -> String {
        self.value.realization().to_string()
    }

    #[getter]
    const fn semantic_revision(&self) -> u64 {
        self.value.semantic_revision()
    }

    #[getter]
    fn output_digests(&self) -> Vec<String> {
        self.value
            .outputs()
            .into_iter()
            .map(|digest| digest.to_string())
            .collect()
    }

    #[getter]
    fn adapter(&self) -> String {
        self.value.execution().adapter().to_owned()
    }

    #[getter]
    fn adapter_version(&self) -> String {
        self.value.execution().adapter_version().to_owned()
    }

    #[getter]
    fn solver_backend(&self) -> String {
        self.value.execution().solver_backend().to_owned()
    }

    #[getter]
    fn solver_backend_version(&self) -> String {
        self.value.execution().solver_backend_version().to_owned()
    }

    #[getter]
    fn workers(&self, py: Python<'_>) -> PyResult<usize> {
        match self
            .value
            .execution()
            .topology()
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?
        {
            ExecutionTopologyV1::Host { workers } => Ok(workers.get()),
            _ => Err(internal_diagnostic_error(
                py,
                &[Diagnostic::error(
                    codes::INTERNAL_FAILURE,
                    "the bounded Python scalar-elliptic Run is not host-local",
                )],
            )),
        }
    }

    #[getter]
    fn reduction(&self) -> &'static str {
        match self.value.execution().reduction() {
            ReductionPolicy::Reproducible => "reproducible",
            ReductionPolicy::Fast => "fast",
        }
    }

    fn to_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        panic_boundary(py, || {
            self.value
                .canonical_json()
                .map(|bytes| PyBytes::new(py, &bytes))
                .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.digest == other.digest)
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        hash_value(&self.digest) as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "RunManifest(realization_digest={:?}, digest={:?})",
            self.realization_digest(),
            self.digest
        )
    }
}

/// Accepted scalar-elliptic result and immutable producer/verifier evidence.
#[pyclass(
    name = "ScalarEllipticResult",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyScalarEllipticResult {
    realization: PyRealization,
    run_manifest: PyRunManifest,
    elapsed_seconds: f64,
    field: PyScalarFieldSummary,
    values: Py<PyArrayBuffer>,
    balance: PyScalarEllipticBalance,
    solve: PyLinearSolveSummary,
    output_fingerprint: String,
}

impl PyScalarEllipticResult {
    pub(crate) fn from_result(py: Python<'_>, result: ScalarEllipticRunResult) -> PyResult<Self> {
        let field = result.field();
        let balance = result.balance();
        let realization = PyRealization {
            plan: result.plan().clone(),
        };
        let elapsed_seconds = result.elapsed().as_secs_f64();
        let solve = PyLinearSolveSummary::from_report(result.solve());
        let output_fingerprint = hex(result.receipt().output().as_bytes());
        let run_manifest = PyRunManifest::from_value(py, result.run_manifest().clone())?;
        let values = PyArrayBuffer::from_owned_result(py, result.into_field_values())?;
        let mut logical_shape = [1_usize; 3];
        logical_shape[..field.spatial_dimension()].copy_from_slice(field.logical_shape());
        Ok(Self {
            realization,
            run_manifest,
            elapsed_seconds,
            field: PyScalarFieldSummary {
                location: field.location().into(),
                spatial_dimension: field.spatial_dimension(),
                logical_shape,
                value_count: field.value_count(),
                minimum: field.minimum(),
                maximum: field.maximum(),
            },
            values,
            balance: PyScalarEllipticBalance {
                boundary_total: balance.boundary_total(),
                integrated_source: balance.integrated_source(),
                relative_imbalance: balance.relative_imbalance(),
            },
            solve,
            output_fingerprint,
        })
    }
}

#[pymethods]
impl PyScalarEllipticResult {
    #[getter]
    fn realization(&self) -> PyRealization {
        self.realization.clone()
    }

    #[getter]
    fn run_manifest(&self) -> PyRunManifest {
        self.run_manifest.clone()
    }

    #[getter]
    const fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    #[getter]
    const fn field(&self) -> PyScalarFieldSummary {
        self.field
    }

    /// Complete primary Field values in canonical location order.
    #[getter]
    fn values(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.values.clone_ref(py)
    }

    #[getter]
    const fn balance(&self) -> PyScalarEllipticBalance {
        self.balance
    }

    #[getter]
    fn solve(&self) -> PyLinearSolveSummary {
        self.solve.clone()
    }

    /// Exact L2 accepted-output identity, not a durable Artifact digest.
    #[getter]
    fn output_fingerprint(&self) -> &str {
        &self.output_fingerprint
    }

    fn __repr__(&self) -> String {
        format!(
            "ScalarEllipticResult(realization_digest={:?}, values={}, output_fingerprint={:?})",
            self.realization.digest(),
            self.field.value_count,
            self.output_fingerprint
        )
    }
}

/// Resolve one request into an exact model-bound Realization before numerical allocation.
#[pyfunction]
pub(crate) fn preview_realization(
    py: Python<'_>,
    model: &PyModel,
    request: &PyScalarElliptic,
) -> PyResult<PyRealization> {
    panic_boundary(py, || {
        let document = model.document().clone();
        let intent = request.intent();
        let plan = py
            .detach(move || {
                document.preview_scalar_elliptic_run(
                    intent,
                    ScalarEllipticExecutionEnvironment::host_serial(),
                )
            })
            .map_err(|diagnostics| diagnostic_error(py, &diagnostics))?;
        Ok(PyRealization { plan })
    })
}

fn nonzero(py: Python<'_>, name: &str, value: usize) -> PyResult<NonZeroUsize> {
    NonZeroUsize::new(value).ok_or_else(|| {
        validation_error(
            py,
            &[Diagnostic::error(
                codes::INVALID_REALIZATION,
                format!("{name} must be non-zero"),
            )
            .with_graph_path(GraphPath::new(["realization".to_owned(), name.to_owned()]))],
        )
    })
}

fn hash_value(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyScalarEllipticMethod>()?;
    module.add_class::<PyScalarElliptic>()?;
    module.add_class::<PyRealization>()?;
    module.add_class::<PyScalarFieldLocation>()?;
    module.add_class::<PyScalarFieldSummary>()?;
    module.add_class::<PyScalarEllipticBalance>()?;
    module.add_class::<PyConvergenceReason>()?;
    module.add_class::<PyLinearSolveSummary>()?;
    module.add_class::<PyRunManifest>()?;
    module.add_class::<PyScalarEllipticResult>()?;
    module.add_function(wrap_pyfunction!(preview_realization, module)?)?;
    Ok(())
}
