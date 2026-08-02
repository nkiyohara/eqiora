//! Python projection of the shared mixed-boundary elasticity application.

use std::hash::{Hash, Hasher};

use eqiora::api::MixedBoundaryElasticityResult2d;
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use numpy::PyArray2;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};

use crate::error::diagnostic_error;
use crate::matrix::ReadOnlyMatrix;
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::PyLinearSolveSummary;

/// Frozen result of the accepted mixed-boundary elasticity operation.
#[pyclass(
    name = "MixedBoundaryElasticityResult",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyMixedBoundaryElasticityResult {
    model_digest: String,
    semantic_revision: u64,
    realization_digest: String,
    realization_revision: u64,
    run_digest: String,
    run_manifest_json: Vec<u8>,
    cells_per_axis: usize,
    bounds: ((f64, f64), (f64, f64)),
    displacement_dimension: (i8, i8, i8, i8, i8, i8, i8),
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
    displacement: ReadOnlyMatrix<f64>,
    constrained_reaction: (f64, f64),
    integrated_body_force: (f64, f64),
    assembly_packets: usize,
    assembly_targets: usize,
    solve: Py<PyLinearSolveSummary>,
    case_id: &'static str,
}

impl PartialEq for PyMixedBoundaryElasticityResult {
    fn eq(&self, other: &Self) -> bool {
        self.run_digest == other.run_digest
    }
}

impl Eq for PyMixedBoundaryElasticityResult {}

impl Hash for PyMixedBoundaryElasticityResult {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.run_digest.hash(state);
    }
}

impl PyMixedBoundaryElasticityResult {
    fn from_native(py: Python<'_>, result: MixedBoundaryElasticityResult2d) -> PyResult<Self> {
        let model_digest = result
            .model()
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
        let solution = result.solution();
        let solve = Py::new(
            py,
            PyLinearSolveSummary::from_report(solution.solve_report()),
        )?;
        let assembly = solution.assembly_report();

        let rows = result.vertices_m().len();
        let mut coordinates = Vec::with_capacity(rows * 2);
        coordinates.extend(
            result
                .vertices_m()
                .iter()
                .flat_map(|coordinate| coordinate.iter().copied()),
        );
        let mut displacement = Vec::with_capacity(rows * 2);
        displacement.extend(
            result
                .displacements_m()
                .iter()
                .flat_map(|value| value.iter().copied()),
        );
        let cell_rows = result.cells().len();
        let mut cells = Vec::with_capacity(cell_rows * 4);
        cells.extend(result.cells().iter().flat_map(|cell| cell.iter().copied()));
        let [[x_minimum, x_maximum], [y_minimum, y_maximum]] = *result.bounds_m();
        let dimension = result.displacement_dimension();
        let constrained_reaction = tuple2(solution.boundary_reaction());
        let integrated_body_force = tuple2(solution.integrated_body_force());

        Ok(Self {
            model_digest: model_digest.to_string(),
            semantic_revision: result.semantic_revision(),
            realization_digest: realization_digest.to_string(),
            realization_revision: result.realization_revision(),
            run_digest: run_digest.to_string(),
            run_manifest_json,
            cells_per_axis: result.cells_per_axis(),
            bounds: ((x_minimum, x_maximum), (y_minimum, y_maximum)),
            displacement_dimension: (
                dimension.mass,
                dimension.length,
                dimension.time,
                dimension.current,
                dimension.temperature,
                dimension.amount,
                dimension.luminous_intensity,
            ),
            coordinates: ReadOnlyMatrix::new(rows, 2, coordinates),
            cells: ReadOnlyMatrix::new(cell_rows, 4, cells),
            displacement: ReadOnlyMatrix::new(rows, 2, displacement),
            constrained_reaction,
            integrated_body_force,
            assembly_packets: assembly.packet_count(),
            assembly_targets: assembly.target_count(),
            solve,
            case_id: result.scientific_case_id(),
        })
    }
}

#[pymethods]
impl PyMixedBoundaryElasticityResult {
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }

    #[getter]
    const fn semantic_revision(&self) -> u64 {
        self.semantic_revision
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
    const fn cells_per_axis(&self) -> usize {
        self.cells_per_axis
    }

    #[getter]
    const fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.bounds
    }

    #[getter]
    const fn displacement_dimension(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        self.displacement_dimension
    }

    #[getter]
    fn coordinates(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.coordinates.numpy(py)
    }

    #[getter]
    fn cells(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.cells.numpy(py)
    }

    #[getter]
    fn displacement(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.displacement.numpy(py)
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
    const fn case_id(&self) -> &'static str {
        self.case_id
    }

    fn __repr__(&self) -> String {
        format!(
            "MixedBoundaryElasticityResult(run_digest='{}')",
            self.run_digest
        )
    }
}

/// Execute the accepted mixed-boundary elasticity application path.
#[pyfunction]
#[pyo3(signature = (model, /))]
pub(crate) fn solve_mixed_boundary_elasticity(
    py: Python<'_>,
    model: &PyModel,
) -> PyResult<PyMixedBoundaryElasticityResult> {
    panic_boundary(py, || {
        let document = model
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
        let result = py.detach(move || {
            MixedBoundaryElasticityResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
        });
        result
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))
            .and_then(|result| PyMixedBoundaryElasticityResult::from_native(py, result))
    })
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyMixedBoundaryElasticityResult>()?;
    module.add_function(wrap_pyfunction!(solve_mixed_boundary_elasticity, module)?)?;
    Ok(())
}
