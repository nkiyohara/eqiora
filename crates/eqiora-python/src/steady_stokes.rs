//! Python intent, Plan, and Result projection for steady Stokes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::Diagnostic;
use eqiora::api::{
    ResolvedSteadyStokesPlan2d, SteadyStokesIntent2d, UnstructuredP1ScalarFieldProjection2d,
};
use eqiora::artifact::{FieldSnapshotEnvelopeV1, RunManifestV2};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::diagnostic::codes;
use eqiora::numerics::SteadyStokesMiniSolution2d;
use eqiora::realization::SpaceFamily;
use eqiora::solver::{LinearSolver, PreconditionerPolicy, ReductionPolicy};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

use crate::error::diagnostic_error;
use crate::geometry::digest_to_hex;
use crate::meshing::PyMesh;
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::{PyLinearSolveSummary, PyRunManifest};
use crate::result::{PyRunResult, StaticResultParts, StaticScalarMetadata};
use crate::trajectory::PyFieldSnapshot;

/// Complete steady-Stokes request with no hidden numerical defaults.
#[pyclass(
    name = "SteadyStokes",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PySteadyStokes {
    native: SteadyStokesIntent2d,
}

#[pymethods]
impl PySteadyStokes {
    #[new]
    #[pyo3(signature = (*, length_scale_m, velocity_scale_m_per_s, pressure_scale_pa, relative_tolerance, absolute_tolerance, maximum_iterations))]
    fn new(
        py: Python<'_>,
        length_scale_m: f64,
        velocity_scale_m_per_s: f64,
        pressure_scale_pa: f64,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: i64,
    ) -> PyResult<Self> {
        let maximum_iterations = usize::try_from(maximum_iterations)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                let diagnostic = Diagnostic::error(
                    codes::INVALID_REALIZATION,
                    "steady-Stokes maximum_iterations must be strictly positive",
                );
                diagnostic_error(py, std::slice::from_ref(&diagnostic))
            })?;
        SteadyStokesIntent2d::new(
            length_scale_m,
            velocity_scale_m_per_s,
            pressure_scale_pa,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )
        .map(|native| Self { native })
        .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))
    }

    #[getter]
    fn length_scale_m(&self) -> f64 {
        self.native.scales().length().value()
    }

    #[getter]
    fn velocity_scale_m_per_s(&self) -> f64 {
        self.native.scales().velocity().value()
    }

    #[getter]
    fn pressure_scale_pa(&self) -> f64 {
        self.native.scales().pressure().value()
    }

    #[getter]
    fn relative_tolerance(&self) -> f64 {
        self.native.solver().relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.native.solver().absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.native.solver().maximum_iterations().get()
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
        self.length_scale_m().to_bits().hash(&mut hasher);
        self.velocity_scale_m_per_s().to_bits().hash(&mut hasher);
        self.pressure_scale_pa().to_bits().hash(&mut hasher);
        self.relative_tolerance().to_bits().hash(&mut hasher);
        self.absolute_tolerance().to_bits().hash(&mut hasher);
        self.maximum_iterations().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "SteadyStokes(length_scale_m={}, velocity_scale_m_per_s={}, pressure_scale_pa={}, relative_tolerance={:e}, absolute_tolerance={:e}, maximum_iterations={})",
            self.length_scale_m(),
            self.velocity_scale_m_per_s(),
            self.pressure_scale_pa(),
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
        )
    }
}

/// Immutable, inspectable steady-Stokes Plan resolved before submission.
#[pyclass(
    name = "SteadyStokesPlan",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PySteadyStokesPlan {
    native: ResolvedSteadyStokesPlan2d,
    mesh: Py<PyMesh>,
    model_digest: String,
    geometry_digest: String,
    correspondence_digest: String,
    mesh_digest: String,
    realization_digest: String,
    canonical_bytes: Vec<u8>,
    spatial_dimension: usize,
    velocity_space: &'static str,
    pressure_space: &'static str,
}

impl PySteadyStokesPlan {
    fn from_native(
        py: Python<'_>,
        native: ResolvedSteadyStokesPlan2d,
        mesh: Py<PyMesh>,
    ) -> PyResult<Self> {
        let accepted_mesh = mesh.borrow(py);
        let model_digest = native
            .model()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let correspondence_digest = accepted_mesh
            .accepted()
            .correspondence()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let mesh_digest = accepted_mesh
            .accepted()
            .mesh()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let realization_digest = native
            .realization()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let canonical_bytes = native
            .realization()
            .canonical_json()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?;
        let spatial_dimension = native
            .realization()
            .requirements()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .execution()
            .spatial_dimension()
            .get();
        let velocity_space = space_name(native.velocity_space())
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?;
        let pressure_space = space_name(native.pressure_space())
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?;
        let geometry_digest = digest_to_hex(&accepted_mesh.accepted().source().digest_bytes());
        drop(accepted_mesh);
        Ok(Self {
            geometry_digest,
            native,
            mesh,
            model_digest,
            correspondence_digest,
            mesh_digest,
            realization_digest,
            canonical_bytes,
            spatial_dimension,
            velocity_space,
            pressure_space,
        })
    }

    pub(crate) const fn native(&self) -> &ResolvedSteadyStokesPlan2d {
        &self.native
    }

    pub(crate) fn mesh(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }
}

#[pymethods]
impl PySteadyStokesPlan {
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
    fn realization_revision(&self) -> u64 {
        self.native.realization().realization_revision().get()
    }

    #[getter]
    const fn spatial_dimension(&self) -> usize {
        self.spatial_dimension
    }

    #[getter]
    const fn velocity_space(&self) -> &'static str {
        self.velocity_space
    }

    #[getter]
    const fn pressure_space(&self) -> &'static str {
        self.pressure_space
    }

    #[getter]
    fn length_scale_m(&self) -> f64 {
        self.native.intent().scales().length().value()
    }

    #[getter]
    fn velocity_scale_m_per_s(&self) -> f64 {
        self.native.intent().scales().velocity().value()
    }

    #[getter]
    fn pressure_scale_pa(&self) -> f64 {
        self.native.intent().scales().pressure().value()
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
        self.native.intent().solver().relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.native.intent().solver().absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.native.intent().solver().maximum_iterations().get()
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
            "SteadyStokesPlan(model_digest={:?}, realization_digest={:?}, solver_backend={:?})",
            self.model_digest,
            self.realization_digest,
            self.solver_backend(),
        )
    }
}

/// Resolve one steady-Stokes intent without executing it.
#[pyfunction]
#[pyo3(name = "resolve_steady_stokes")]
#[pyo3(signature = (model, intent, /, *, mesh))]
pub(crate) fn resolve(
    py: Python<'_>,
    model: &PyModel,
    intent: &PySteadyStokes,
    mesh: Py<PyMesh>,
) -> PyResult<PySteadyStokesPlan> {
    panic_boundary(py, || {
        let model = model.artifact().clone();
        let accepted = mesh.borrow(py).accepted().clone();
        let intent = intent.native;
        let native = py.detach(move || {
            ResolvedSteadyStokesPlan2d::resolve(&model, intent, &accepted, &FaerLinearSolver)
        });
        native
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))
            .and_then(|native| PySteadyStokesPlan::from_native(py, native, mesh))
    })
}

/// Native worker payload projected into the common Python Result.
#[derive(Debug)]
pub(crate) struct SteadyStokesRunMaterialization {
    run: RunManifestV2,
    snapshot: FieldSnapshotEnvelopeV1,
    projection: UnstructuredP1ScalarFieldProjection2d,
    solution: SteadyStokesMiniSolution2d,
    physical: SteadyStokesPhysicalEvidence,
}

impl SteadyStokesRunMaterialization {
    pub(crate) fn new(
        run: RunManifestV2,
        snapshot: FieldSnapshotEnvelopeV1,
        projection: UnstructuredP1ScalarFieldProjection2d,
        solution: SteadyStokesMiniSolution2d,
        physical: SteadyStokesPhysicalEvidence,
    ) -> Self {
        Self {
            run,
            snapshot,
            projection,
            solution,
            physical,
        }
    }
}

/// Physics observations that do not belong to Mesh, FieldSnapshot, or RunManifest.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SteadyStokesPhysicalEvidence {
    cylinder_force_on_fluid: [f64; 2],
    inlet_flux: f64,
    outlet_flux: f64,
    net_flux: f64,
    momentum_closure: [f64; 2],
}

impl SteadyStokesPhysicalEvidence {
    pub(crate) const fn new(
        cylinder_force_on_fluid: [f64; 2],
        inlet_flux: f64,
        outlet_flux: f64,
        net_flux: f64,
        momentum_closure: [f64; 2],
    ) -> Self {
        Self {
            cylinder_force_on_fluid,
            inlet_flux,
            outlet_flux,
            net_flux,
            momentum_closure,
        }
    }
}

/// Frozen scientific evidence selected from one accepted steady-Stokes Result.
#[pyclass(
    name = "SteadyStokesEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PySteadyStokesEvidence {
    run_digest: String,
    pressure_minimum: f64,
    pressure_maximum: f64,
    exact_bounds: ((f64, f64), (f64, f64)),
    cylinder_force_on_fluid: (f64, f64),
    inlet_flux: f64,
    outlet_flux: f64,
    net_flux: f64,
    constrained_reaction: (f64, f64),
    integrated_body_force: (f64, f64),
    integrated_boundary_traction: (f64, f64),
    momentum_closure: (f64, f64),
    solve: Py<PyLinearSolveSummary>,
    continuity_residual_norm: f64,
}

impl PySteadyStokesEvidence {
    fn from_materialization(
        py: Python<'_>,
        materialized: &SteadyStokesRunMaterialization,
    ) -> PyResult<Self> {
        let projection = &materialized.projection;
        let [[x_lower, x_upper], [y_lower, y_upper]] = *projection.bounds_m();
        let solution = &materialized.solution;
        let dimensionless = solution.dimensionless_solution();
        let solve = Py::new(
            py,
            PyLinearSolveSummary::from_report(dimensionless.solve_report()),
        )?;
        Ok(Self {
            run_digest: materialized
                .run
                .digest()
                .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
                .to_string(),
            pressure_minimum: projection.minimum(),
            pressure_maximum: projection.maximum(),
            exact_bounds: ((x_lower, x_upper), (y_lower, y_upper)),
            cylinder_force_on_fluid: tuple2(materialized.physical.cylinder_force_on_fluid),
            inlet_flux: materialized.physical.inlet_flux,
            outlet_flux: materialized.physical.outlet_flux,
            net_flux: materialized.physical.net_flux,
            constrained_reaction: tuple2(solution.boundary_reaction()),
            integrated_body_force: tuple2(solution.integrated_body_force()),
            integrated_boundary_traction: tuple2(solution.integrated_boundary_traction()),
            momentum_closure: tuple2(materialized.physical.momentum_closure),
            solve,
            continuity_residual_norm: dimensionless.continuity_residual_norm(),
        })
    }
}

#[pymethods]
impl PySteadyStokesEvidence {
    #[getter]
    fn run_digest(&self) -> &str {
        &self.run_digest
    }

    #[getter]
    const fn pressure_minimum(&self) -> f64 {
        self.pressure_minimum
    }

    #[getter]
    const fn pressure_maximum(&self) -> f64 {
        self.pressure_maximum
    }

    #[getter]
    const fn exact_bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.exact_bounds
    }

    #[getter]
    const fn cylinder_force_on_fluid(&self) -> (f64, f64) {
        self.cylinder_force_on_fluid
    }

    #[getter]
    const fn inlet_flux(&self) -> f64 {
        self.inlet_flux
    }

    #[getter]
    const fn outlet_flux(&self) -> f64 {
        self.outlet_flux
    }

    #[getter]
    const fn net_flux(&self) -> f64 {
        self.net_flux
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
    const fn integrated_boundary_traction(&self) -> (f64, f64) {
        self.integrated_boundary_traction
    }

    #[getter]
    const fn momentum_closure(&self) -> (f64, f64) {
        self.momentum_closure
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinearSolveSummary> {
        self.solve.clone_ref(py)
    }

    #[getter]
    const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }

    fn __repr__(&self) -> String {
        format!("SteadyStokesEvidence(run_digest={:?})", self.run_digest,)
    }
}

pub(crate) fn materialize_result(
    py: Python<'_>,
    materialized: SteadyStokesRunMaterialization,
    identity: crate::execution::RunIdentity,
    elapsed_seconds: f64,
    mesh: Py<PyMesh>,
) -> PyResult<PyRunResult> {
    let accepted_mesh_digest = mesh
        .borrow(py)
        .accepted()
        .mesh()
        .digest()
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
    if &accepted_mesh_digest != materialized.projection.mesh_artifact() {
        return Err(PyRuntimeError::new_err(
            "steady-Stokes Result projection references a different accepted Mesh",
        ));
    }

    let (field_id, snapshot) = PyFieldSnapshot::from_authored_scalar(
        py,
        &materialized.snapshot,
        &materialized.projection,
    )?;
    let bounds = materialized.projection.bounds_m();
    let scalar = StaticScalarMetadata::new(
        ((bounds[0][0], bounds[0][1]), (bounds[1][0], bounds[1][1])),
        materialized.projection.minimum(),
        materialized.projection.maximum(),
    );
    let run_manifest = Py::new(py, PyRunManifest::from_value(py, materialized.run.clone())?)?;
    let evidence = Py::new(
        py,
        PySteadyStokesEvidence::from_materialization(py, &materialized)?,
    )?;
    Ok(PyRunResult::from_static_steady_stokes(
        StaticResultParts {
            identity,
            elapsed_seconds,
            field_id,
            snapshot: Py::new(py, snapshot)?,
            mesh,
            run_manifest,
            scalar,
        },
        evidence,
    ))
}

#[pyfunction]
#[pyo3(signature = (result, /))]
fn steady_stokes_evidence(
    py: Python<'_>,
    result: &PyRunResult,
) -> PyResult<Py<PySteadyStokesEvidence>> {
    result.steady_stokes_evidence(py)
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

fn space_name(space: eqiora::realization::Space) -> Result<&'static str, Diagnostic> {
    match space.family() {
        SpaceFamily::SimplexP1Bubble => Ok("simplex-p1-bubble"),
        SpaceFamily::ContinuousLagrange { order } if order.get() == 1 => {
            Ok("continuous-lagrange-1")
        }
        _ => Err(Diagnostic::error(
            codes::INTERNAL_FAILURE,
            "resolved steady-Stokes Plan has no frozen public name for its discrete space",
        )),
    }
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
    module.add_class::<PySteadyStokes>()?;
    module.add_class::<PySteadyStokesPlan>()?;
    module.add_class::<PySteadyStokesEvidence>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    module.add_function(wrap_pyfunction!(steady_stokes_evidence, module)?)?;
    Ok(())
}
