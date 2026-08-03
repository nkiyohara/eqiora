//! Python intent, Plan, evidence, and common-Result projection for linear elasticity.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::Diagnostic;
use eqiora::api::{
    LinearElasticityIntent2d, MixedBoundaryElasticityResult2d, ResolvedLinearElasticityPlan2d,
};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::diagnostic::codes;
use eqiora::realization::{
    DiscretizationMethod, MeshKind, MeshPolicy, QuadraturePolicy, ResolutionSource, SpaceFamily,
    VectorLayoutKind,
};
use eqiora::solver::{LinearSolver, PreconditionerPolicy, ReductionPolicy, ScalarType};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

use crate::error::diagnostic_error;
use crate::execution::RunIdentity;
use crate::meshing::PyMesh;
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::{PyLinearSolveSummary, PyRunManifest};
use crate::result::{PyRunResult, StaticResultParts};
use crate::trajectory::PyFieldSnapshot;

/// Complete linear-elasticity request with no hidden numerical defaults.
#[pyclass(
    name = "LinearElasticity",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyLinearElasticity {
    native: LinearElasticityIntent2d,
}

#[pymethods]
impl PyLinearElasticity {
    #[new]
    #[pyo3(signature = (*, cells_per_axis, relative_tolerance, absolute_tolerance, maximum_iterations))]
    fn new(
        py: Python<'_>,
        cells_per_axis: i64,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: i64,
    ) -> PyResult<Self> {
        let cells_per_axis = positive_usize(py, "cells_per_axis", cells_per_axis)?;
        let maximum_iterations = positive_usize(py, "maximum_iterations", maximum_iterations)?;
        LinearElasticityIntent2d::new(
            cells_per_axis,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )
        .map(|native| Self { native })
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
    }

    #[getter]
    fn cells_per_axis(&self) -> usize {
        self.native.cells_per_axis().get()
    }

    #[getter]
    fn relative_tolerance(&self) -> f64 {
        self.native.relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.native.absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.native.maximum_iterations().get()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.native == other.native)
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.cells_per_axis().hash(&mut hasher);
        self.relative_tolerance().to_bits().hash(&mut hasher);
        self.absolute_tolerance().to_bits().hash(&mut hasher);
        self.maximum_iterations().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "LinearElasticity(cells_per_axis={}, relative_tolerance={:e}, absolute_tolerance={:e}, maximum_iterations={})",
            self.cells_per_axis(),
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
        )
    }
}

/// Immutable, inspectable linear-elasticity Plan resolved before submission.
#[pyclass(
    name = "LinearElasticityPlan",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyLinearElasticityPlan {
    native: ResolvedLinearElasticityPlan2d,
    model_digest: String,
    geometry_digest: String,
    correspondence_digest: String,
    mesh_digest: String,
    realization_digest: String,
    canonical_bytes: Vec<u8>,
}

impl PyLinearElasticityPlan {
    fn from_native(py: Python<'_>, native: ResolvedLinearElasticityPlan2d) -> PyResult<Self> {
        let model_digest = native
            .model()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        let geometry_digest = native
            .geometry()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        let correspondence_digest = native
            .correspondence()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        let mesh_digest = native
            .mesh_artifact()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        let realization_digest = native
            .realization()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        let canonical_bytes = native
            .realization()
            .canonical_json()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
        Ok(Self {
            native,
            model_digest,
            geometry_digest,
            correspondence_digest,
            mesh_digest,
            realization_digest,
            canonical_bytes,
        })
    }

    pub(crate) const fn native(&self) -> &ResolvedLinearElasticityPlan2d {
        &self.native
    }
}

#[pymethods]
impl PyLinearElasticityPlan {
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }

    #[getter]
    fn semantic_revision(&self) -> u64 {
        self.native.realization().semantic_revision().get()
    }

    #[getter]
    fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }

    #[getter]
    fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }

    #[getter]
    fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    #[getter]
    fn realization_digest(&self) -> &str {
        &self.realization_digest
    }

    #[getter]
    fn realization_revision(&self) -> PyResult<u64> {
        match self.native.resolved().source() {
            ResolutionSource::Explicit(revision) => Ok(revision.get()),
            _ => Err(frozen_fact_error("Realization revision")),
        }
    }

    #[getter]
    fn spatial_dimension(&self) -> usize {
        self.native
            .resolved()
            .requirements()
            .spatial_dimension()
            .get()
    }

    #[getter]
    fn cells_per_axis(&self) -> usize {
        self.native.intent().cells_per_axis().get()
    }

    #[getter]
    fn discretization_method(&self) -> PyResult<&'static str> {
        match self.native.resolved().plan().discretization().method() {
            DiscretizationMethod::ContinuousGalerkin => Ok("continuous-galerkin"),
            _ => Err(frozen_fact_error("discretization method")),
        }
    }

    #[getter]
    fn mesh_kind(&self) -> PyResult<&'static str> {
        match self.native.resolved().plan().discretization().mesh().kind() {
            MeshKind::GeneratedCartesian => Ok("generated-cartesian"),
            _ => Err(frozen_fact_error("Mesh kind")),
        }
    }

    #[getter]
    fn mesh_policy(&self) -> PyResult<&'static str> {
        match self.native.resolved().plan().discretization().mesh() {
            MeshPolicy::GeneratedUniform { .. } => Ok("generated-uniform"),
            _ => Err(frozen_fact_error("Mesh policy")),
        }
    }

    #[getter]
    fn field_space(&self) -> PyResult<&'static str> {
        match self.native.resolved().plan().space().family() {
            SpaceFamily::ContinuousLagrange { order } if order.get() == 1 => {
                Ok("continuous-lagrange-1")
            }
            _ => Err(frozen_fact_error("Field space")),
        }
    }

    #[getter]
    fn quadrature(&self) -> PyResult<&'static str> {
        match self.native.resolved().plan().discretization().quadrature() {
            QuadraturePolicy::GaussLegendre { .. } => Ok("gauss-legendre"),
            _ => Err(frozen_fact_error("quadrature")),
        }
    }

    #[getter]
    fn quadrature_points_per_axis(&self) -> PyResult<usize> {
        match self.native.resolved().plan().discretization().quadrature() {
            QuadraturePolicy::GaussLegendre { points_per_axis } => Ok(points_per_axis.get()),
            _ => Err(frozen_fact_error("quadrature")),
        }
    }

    #[getter]
    fn scalar_type(&self) -> PyResult<&'static str> {
        match self.native.resolved().requirements().scalar_type() {
            ScalarType::F64 => Ok("f64"),
            _ => Err(frozen_fact_error("scalar type")),
        }
    }

    #[getter]
    fn vector_layout(&self) -> PyResult<&'static str> {
        match self.native.resolved().requirements().vector_layout() {
            VectorLayoutKind::Replicated => Ok("replicated"),
            _ => Err(frozen_fact_error("vector layout")),
        }
    }

    #[getter]
    const fn coefficient_association(&self) -> &'static str {
        "vertex"
    }

    #[getter]
    fn solver_algorithm(&self) -> &'static str {
        linear_solver_name(self.native.intent().solver().algorithm())
    }

    #[getter]
    fn preconditioner(&self) -> &'static str {
        preconditioner_name(self.native.intent().solver().preconditioner())
    }

    #[getter]
    fn reduction(&self) -> &'static str {
        reduction_name(self.native.intent().solver().reduction())
    }

    #[getter]
    fn relative_tolerance(&self) -> f64 {
        self.native.intent().relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.native.intent().absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.native.intent().maximum_iterations().get()
    }

    #[getter]
    fn solver_backend(&self) -> &'static str {
        self.native.solver_provider().id().as_str()
    }

    #[getter]
    fn execution_adapter(&self) -> &'static str {
        self.native.execution_provider().id().as_str()
    }

    #[getter]
    fn workers(&self) -> usize {
        self.native.workers().get()
    }

    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, &self.canonical_bytes).unbind()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.native == other.native)
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.realization_digest.hash(&mut hasher);
        self.solver_backend().hash(&mut hasher);
        self.native
            .solver_provider()
            .implementation_version()
            .hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "LinearElasticityPlan(model_digest={:?}, realization_digest={:?}, solver_backend={:?})",
            self.model_digest,
            self.realization_digest,
            self.solver_backend(),
        )
    }
}

/// Frozen scientific evidence selected from one accepted structural Result.
#[pyclass(
    name = "LinearElasticityEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyLinearElasticityEvidence {
    run_digest: String,
    constrained_reaction: (f64, f64),
    integrated_body_force: (f64, f64),
    assembly_packets: usize,
    assembly_targets: usize,
    solve: Py<PyLinearSolveSummary>,
    exact_bounds: ((f64, f64), (f64, f64)),
}

impl PyLinearElasticityEvidence {
    fn from_native(py: Python<'_>, result: &MixedBoundaryElasticityResult2d) -> PyResult<Self> {
        let solution = result.solution();
        let assembly = solution.assembly_report();
        let [[x_lower, x_upper], [y_lower, y_upper]] = *result.bounds_m();
        Ok(Self {
            run_digest: result
                .run()
                .digest()
                .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
                .to_string(),
            constrained_reaction: tuple2(solution.boundary_reaction()),
            integrated_body_force: tuple2(solution.integrated_body_force()),
            assembly_packets: assembly.packet_count(),
            assembly_targets: assembly.target_count(),
            solve: Py::new(
                py,
                PyLinearSolveSummary::from_report(solution.solve_report()),
            )?,
            exact_bounds: ((x_lower, x_upper), (y_lower, y_upper)),
        })
    }
}

#[pymethods]
impl PyLinearElasticityEvidence {
    #[getter]
    fn run_digest(&self) -> &str {
        &self.run_digest
    }

    #[getter]
    const fn constrained_reaction(&self) -> (f64, f64) {
        self.constrained_reaction
    }

    #[getter]
    const fn integrated_body_force(&self) -> (f64, f64) {
        self.integrated_body_force
    }

    #[getter]
    const fn assembly_packets(&self) -> usize {
        self.assembly_packets
    }

    #[getter]
    const fn assembly_targets(&self) -> usize {
        self.assembly_targets
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinearSolveSummary> {
        self.solve.clone_ref(py)
    }

    #[getter]
    const fn exact_bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.exact_bounds
    }

    fn __repr__(&self) -> String {
        format!("LinearElasticityEvidence(run_digest={:?})", self.run_digest)
    }
}

/// Resolve one structural intent without executing it.
#[pyfunction]
#[pyo3(name = "resolve_linear_elasticity")]
#[pyo3(signature = (model, intent, /))]
pub(crate) fn resolve(
    py: Python<'_>,
    model: &PyModel,
    intent: &PyLinearElasticity,
) -> PyResult<PyLinearElasticityPlan> {
    panic_boundary(py, || {
        let document = model
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
        let intent = intent.native;
        let native = py.detach(move || {
            ResolvedLinearElasticityPlan2d::resolve(&document, intent, &FaerLinearSolver)
        });
        native
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
            .and_then(|native| PyLinearElasticityPlan::from_native(py, native))
    })
}

pub(crate) fn materialize_result(
    py: Python<'_>,
    result: MixedBoundaryElasticityResult2d,
    identity: RunIdentity,
    elapsed_seconds: f64,
) -> PyResult<PyRunResult> {
    let mesh = Py::new(
        py,
        PyMesh::from_cartesian(
            py,
            result.geometry().clone(),
            result.mesh_artifact().clone(),
            result.correspondence().clone(),
            result.realization().clone(),
        )?,
    )?;
    let (field_id, snapshot) = PyFieldSnapshot::from_cartesian_q1_vector(
        py,
        result.displacement_snapshot(),
        result.displacement_dimension(),
        2,
        result.vertices_m().len(),
    )?;
    let run_manifest = Py::new(py, PyRunManifest::from_value(py, result.run().clone())?)?;
    let evidence = Py::new(py, PyLinearElasticityEvidence::from_native(py, &result)?)?;
    Ok(PyRunResult::from_static_linear_elasticity(
        StaticResultParts {
            identity,
            elapsed_seconds,
            field_id,
            snapshot: Py::new(py, snapshot)?,
            mesh,
            run_manifest,
        },
        evidence,
    ))
}

#[pyfunction]
#[pyo3(signature = (result, /))]
fn linear_elasticity_evidence(
    py: Python<'_>,
    result: &PyRunResult,
) -> PyResult<Py<PyLinearElasticityEvidence>> {
    result.linear_elasticity_evidence(py)
}

fn positive_usize(py: Python<'_>, name: &str, value: i64) -> PyResult<NonZeroUsize> {
    usize::try_from(value)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| {
            diagnostic_error(
                py,
                &[Diagnostic::error(
                    codes::INVALID_REALIZATION,
                    format!("linear-elasticity {name} must be strictly positive"),
                )],
            )
        })
}

fn frozen_fact_error(name: &str) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(format!(
        "resolved linear-elasticity Plan has no frozen public name for its {name}"
    ))
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

const fn linear_solver_name(value: LinearSolver) -> &'static str {
    match value {
        LinearSolver::ConjugateGradient => "conjugate-gradient",
        LinearSolver::MinimumResidual => "minimum-residual",
        LinearSolver::BiConjugateGradientStabilized => "bicgstab",
        LinearSolver::SparseLu => "sparse-lu",
    }
}

const fn preconditioner_name(value: PreconditionerPolicy) -> &'static str {
    match value {
        PreconditionerPolicy::Identity => "identity",
        PreconditionerPolicy::Jacobi => "jacobi",
    }
}

const fn reduction_name(value: ReductionPolicy) -> &'static str {
    match value {
        ReductionPolicy::Reproducible => "reproducible",
        ReductionPolicy::Fast => "fast",
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLinearElasticity>()?;
    module.add_class::<PyLinearElasticityPlan>()?;
    module.add_class::<PyLinearElasticityEvidence>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    module.add_function(wrap_pyfunction!(linear_elasticity_evidence, module)?)?;
    Ok(())
}
