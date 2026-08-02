//! Python intent, Plan, and Result projection for steady Stokes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::Diagnostic;
use eqiora::api::{
    CircularHoleSteadyStokesResult2d, ResolvedSteadyStokesPlan2d, SteadyStokesIntent2d,
};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::diagnostic::codes;
use eqiora::realization::SpaceFamily;
use eqiora::solver::{LinearSolver, PreconditionerPolicy, ReductionPolicy};
use numpy::PyArray2;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

use crate::array::PyArrayBuffer;
use crate::error::diagnostic_error;
use crate::geometry::digest_to_hex;
use crate::matrix::ReadOnlyMatrix;
use crate::meshing::PyMesh;
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::PyLinearSolveSummary;

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
        mesh: &PyMesh,
    ) -> PyResult<Self> {
        let model_digest = native
            .model()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let correspondence_digest = mesh
            .accepted()
            .correspondence()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))?
            .to_string();
        let mesh_digest = mesh
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
        Ok(Self {
            geometry_digest: digest_to_hex(&mesh.accepted().source().digest_bytes()),
            native,
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
    const fn workers(&self) -> usize {
        1
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
    mesh: &PyMesh,
) -> PyResult<PySteadyStokesPlan> {
    panic_boundary(py, || {
        let model = model.artifact().clone();
        let accepted = mesh.accepted().clone();
        let intent = intent.native;
        let native = py.detach(move || {
            ResolvedSteadyStokesPlan2d::resolve(&model, intent, &accepted, &FaerLinearSolver)
        });
        native
            .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))
            .and_then(|native| PySteadyStokesPlan::from_native(py, native, mesh))
    })
}

/// Frozen result of the one accepted exact-cylinder steady-Stokes operation.
#[pyclass(
    name = "CircularHoleSteadyStokesResult",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyCircularHoleSteadyStokesResult {
    model_digest: String,
    semantic_revision: u64,
    chordal_realization_digest: String,
    chordal_realization_json: Vec<u8>,
    exact_source_digest: String,
    realized_geometry_digest: String,
    correspondence_digest: String,
    realization_digest: String,
    realization_revision: u64,
    run_digest: String,
    run_manifest_json: Vec<u8>,
    snapshot_digest: String,
    mesh_digest: String,
    pressure_field_id: String,
    support_domain_id: String,
    pressure_dimension: (i8, i8, i8, i8, i8, i8, i8),
    bounds: ((f64, f64), (f64, f64)),
    coordinates: ReadOnlyMatrix<f64>,
    triangles: ReadOnlyMatrix<u32>,
    pressure: Py<PyArrayBuffer>,
    pressure_minimum: f64,
    pressure_maximum: f64,
    requested_max_boundary_error: f64,
    boundary_evaluation_allowance: f64,
    boundary_error_bound: f64,
    circle_segments: usize,
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

impl PartialEq for PyCircularHoleSteadyStokesResult {
    fn eq(&self, other: &Self) -> bool {
        self.run_digest == other.run_digest
    }
}

impl Eq for PyCircularHoleSteadyStokesResult {}

impl Hash for PyCircularHoleSteadyStokesResult {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.run_digest.hash(state);
    }
}

impl PyCircularHoleSteadyStokesResult {
    pub(crate) fn from_native(
        py: Python<'_>,
        result: CircularHoleSteadyStokesResult2d,
    ) -> PyResult<Self> {
        let model_digest = result
            .model()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let chordal_realization_digest = result
            .chordal_realization()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let chordal_realization_json = result
            .chordal_realization()
            .canonical_json()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let realized_geometry_digest = result
            .realized_geometry()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let correspondence_digest = result
            .correspondence()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let realization_digest = result
            .realization()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let run_digest = result
            .run()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let run_manifest_json = result
            .run()
            .canonical_json()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let snapshot_digest = result
            .snapshot()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let mesh_digest = result
            .mesh()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;

        let projection = result.pressure_projection();
        let dimension = projection.value_dimension();
        let [[x_lower, x_upper], [y_lower, y_upper]] = *projection.bounds_m();
        let pressure_minimum = projection.minimum();
        let pressure_maximum = projection.maximum();
        let pressure_field_id = projection.field().to_string();
        let support_domain_id = projection.support_domain().to_string();
        let coordinate_rows = projection.vertices_m().len();
        let triangle_rows = projection.triangles().len();

        let solution = result.solution();
        let dimensionless = solution.dimensionless_solution();
        let solve = Py::new(
            py,
            PyLinearSolveSummary::from_report(dimensionless.solve_report()),
        )?;
        let continuity_residual_norm = dimensionless.continuity_residual_norm();
        let constrained_reaction = tuple2(solution.boundary_reaction());
        let integrated_body_force = tuple2(solution.integrated_body_force());
        let integrated_boundary_traction = tuple2(solution.integrated_boundary_traction());

        let semantic_revision = projection.semantic_revision();
        let realization_revision = result.realization().realization_revision().get();
        let exact_source_digest = digest_to_hex(&result.source().digest_bytes());
        let chordal = result.chordal_realization();
        let requested_max_boundary_error = chordal.requested_max_boundary_error_m();
        let boundary_evaluation_allowance = chordal.boundary_evaluation_allowance_m();
        let boundary_error_bound = chordal.boundary_error_bound_m();
        let circle_segments = usize::try_from(chordal.circle_segments())
            .expect("accepted chordal segment count fits local usize");
        let cylinder_force_on_fluid = tuple2(result.cylinder_force_on_fluid());
        let inlet_flux = result.inlet_flux();
        let outlet_flux = result.outlet_flux();
        let net_flux = result.net_flux();
        let momentum_closure = tuple2(result.momentum_closure());

        let projection = result.into_pressure_projection();
        let (coordinates, triangles, pressure_values) = projection.into_arrays();
        let mut flat_coordinates = Vec::with_capacity(coordinate_rows * 2);
        flat_coordinates.extend(
            coordinates
                .into_iter()
                .flat_map(|coordinate| coordinate.into_iter()),
        );
        let mut flat_triangles = Vec::with_capacity(triangle_rows * 3);
        flat_triangles.extend(
            triangles
                .into_iter()
                .flat_map(|triangle| triangle.into_iter()),
        );
        let pressure = PyArrayBuffer::from_owned_result(py, pressure_values)?;

        Ok(Self {
            model_digest: model_digest.to_string(),
            semantic_revision,
            chordal_realization_digest: chordal_realization_digest.to_string(),
            chordal_realization_json,
            exact_source_digest,
            realized_geometry_digest: realized_geometry_digest.to_string(),
            correspondence_digest: correspondence_digest.to_string(),
            realization_digest: realization_digest.to_string(),
            realization_revision,
            run_digest: run_digest.to_string(),
            run_manifest_json,
            snapshot_digest: snapshot_digest.to_string(),
            mesh_digest: mesh_digest.to_string(),
            pressure_field_id,
            support_domain_id,
            pressure_dimension: (
                dimension.mass,
                dimension.length,
                dimension.time,
                dimension.current,
                dimension.temperature,
                dimension.amount,
                dimension.luminous_intensity,
            ),
            bounds: ((x_lower, x_upper), (y_lower, y_upper)),
            coordinates: ReadOnlyMatrix::new(coordinate_rows, 2, flat_coordinates),
            triangles: ReadOnlyMatrix::new(triangle_rows, 3, flat_triangles),
            pressure,
            pressure_minimum,
            pressure_maximum,
            requested_max_boundary_error,
            boundary_evaluation_allowance,
            boundary_error_bound,
            circle_segments,
            cylinder_force_on_fluid,
            inlet_flux,
            outlet_flux,
            net_flux,
            constrained_reaction,
            integrated_body_force,
            integrated_boundary_traction,
            momentum_closure,
            solve,
            continuity_residual_norm,
        })
    }
}

#[pymethods]
impl PyCircularHoleSteadyStokesResult {
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }

    #[getter]
    const fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }

    #[getter]
    fn chordal_realization_digest(&self) -> &str {
        &self.chordal_realization_digest
    }

    #[getter]
    fn chordal_realization_json(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, &self.chordal_realization_json).unbind()
    }

    #[getter]
    fn exact_source_digest(&self) -> &str {
        &self.exact_source_digest
    }

    #[getter]
    fn realized_geometry_digest(&self) -> &str {
        &self.realized_geometry_digest
    }

    #[getter]
    fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }

    #[getter]
    fn realization_digest(&self) -> &str {
        &self.realization_digest
    }

    #[getter]
    const fn realization_revision(&self) -> u64 {
        self.realization_revision
    }

    #[getter]
    fn run_digest(&self) -> &str {
        &self.run_digest
    }

    #[getter]
    fn run_manifest_json(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, &self.run_manifest_json).unbind()
    }

    #[getter]
    fn snapshot_digest(&self) -> &str {
        &self.snapshot_digest
    }

    #[getter]
    fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    #[getter]
    fn pressure_field_id(&self) -> &str {
        &self.pressure_field_id
    }

    #[getter]
    fn support_domain_id(&self) -> &str {
        &self.support_domain_id
    }

    #[getter]
    const fn pressure_dimension(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        self.pressure_dimension
    }

    #[getter]
    const fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.bounds
    }

    #[getter]
    fn coordinates(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.coordinates.numpy(py)
    }

    #[getter]
    fn triangles(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.triangles.numpy(py)
    }

    #[getter]
    fn pressure(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.pressure.clone_ref(py)
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
    const fn requested_max_boundary_error(&self) -> f64 {
        self.requested_max_boundary_error
    }

    #[getter]
    const fn boundary_evaluation_allowance(&self) -> f64 {
        self.boundary_evaluation_allowance
    }

    #[getter]
    const fn boundary_error_bound(&self) -> f64 {
        self.boundary_error_bound
    }

    #[getter]
    const fn circle_segments(&self) -> usize {
        self.circle_segments
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
        format!(
            "CircularHoleSteadyStokesResult(run_digest='{}')",
            self.run_digest
        )
    }
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
    module.add_class::<PyCircularHoleSteadyStokesResult>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    Ok(())
}
