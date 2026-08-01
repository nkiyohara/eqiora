//! Python projection of the shared exact-cylinder steady-Stokes application.

use std::hash::{Hash, Hasher};

use eqiora::api::CircularHoleSteadyStokesResult2d;
use eqiora::artifact::{ModelDecoderLimits, ModelEnvelope};
use eqiora::backends::faer::FaerLinearSolver;
use eqiora::geometry::CanonicalGeometryV1;
use numpy::PyArray2;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};

use crate::array::PyArrayBuffer;
use crate::error::diagnostic_error;
use crate::geometry::{PyRectangleWithCircularHole, digest_to_hex};
use crate::matrix::ReadOnlyMatrix;
use crate::meshing::PyCircularHoleChordalMesh;
use crate::panic_boundary;
use crate::realization::PyLinearSolveSummary;

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
    fn from_native(py: Python<'_>, result: CircularHoleSteadyStokesResult2d) -> PyResult<Self> {
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

/// Execute the one accepted exact-cylinder steady-Stokes application path.
#[pyfunction]
#[pyo3(signature = (*, model, geometry, mesh))]
pub(crate) fn solve_exact_cylinder_stokes(
    py: Python<'_>,
    model: &[u8],
    geometry: &PyRectangleWithCircularHole,
    mesh: &PyCircularHoleChordalMesh,
) -> PyResult<PyCircularHoleSteadyStokesResult> {
    panic_boundary(py, || {
        let model = model.to_vec();
        let source: CanonicalGeometryV1 = geometry.geometry().clone();
        let mesh_source: CanonicalGeometryV1 = mesh.source().clone();
        let accepted = mesh.accepted().clone();
        let native = py.detach(move || {
            if source != mesh_source {
                return Err(eqiora::Diagnostic::error(
                    eqiora::diagnostic::codes::INVALID_REALIZATION,
                    "exact-cylinder geometry and chordal mesh name different source revisions",
                ));
            }
            let model = ModelEnvelope::from_json(&model, ModelDecoderLimits::default())?;
            CircularHoleSteadyStokesResult2d::solve_reference(&model, &accepted, &FaerLinearSolver)
        });
        native
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))
            .and_then(|result| PyCircularHoleSteadyStokesResult::from_native(py, result))
    })
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCircularHoleSteadyStokesResult>()?;
    module.add_function(wrap_pyfunction!(solve_exact_cylinder_stokes, module)?)?;
    Ok(())
}
