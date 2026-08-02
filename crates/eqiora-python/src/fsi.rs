//! Python projection of the shared fixed-reference FSI application.

use std::hash::{Hash, Hasher};

use eqiora::api::FixedReferenceFsiResult2d;
use eqiora::meshing::MeshEntity;
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyTuple};

use crate::error::diagnostic_error;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::PyLinearSolveSummary;
use crate::trajectory::PyTrajectory;

/// Frozen projection of one accepted step in the two-state trajectory.
#[pyclass(
    name = "FixedReferenceFsiStep",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFixedReferenceFsiStep {
    ordinal: u64,
    time_s: f64,
    velocity: ReadOnlyMatrix<f64>,
    bubble_velocity: ReadOnlyMatrix<f64>,
    pressure_vertices: ReadOnlyVector<u32>,
    pressure: ReadOnlyVector<f64>,
    displacement: ReadOnlyMatrix<f64>,
    interface_vertices: ReadOnlyVector<u32>,
    fluid_action: ReadOnlyMatrix<f64>,
    solid_action: ReadOnlyMatrix<f64>,
    action_imbalance: ReadOnlyMatrix<f64>,
    previous_kinetic_energy_j_per_m: f64,
    next_kinetic_energy_j_per_m: f64,
    previous_elastic_energy_j_per_m: f64,
    next_elastic_energy_j_per_m: f64,
    kinetic_increment_j_per_m: f64,
    elastic_increment_j_per_m: f64,
    viscous_dissipation_j_per_m: f64,
    energy_defect_j_per_m: f64,
    numerical_residual_norm: f64,
    continuity_residual_norm: f64,
    kinematic_residual_norm: f64,
    interface_velocity_jump_norm: f64,
    interface_action_imbalance_n_per_m: f64,
    solve: Py<PyLinearSolveSummary>,
    assembly_packets: usize,
    assembly_targets: usize,
}

#[pymethods]
impl PyFixedReferenceFsiStep {
    #[getter]
    const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    #[getter]
    const fn time_s(&self) -> f64 {
        self.time_s
    }

    #[getter]
    fn velocity(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.velocity.numpy(py)
    }

    #[getter]
    fn bubble_velocity(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.bubble_velocity.numpy(py)
    }

    #[getter]
    fn pressure_vertices(&self, py: Python<'_>) -> PyResult<Py<PyArray1<u32>>> {
        self.pressure_vertices.numpy(py)
    }

    #[getter]
    fn pressure(&self, py: Python<'_>) -> PyResult<Py<PyArray1<f64>>> {
        self.pressure.numpy(py)
    }

    #[getter]
    fn displacement(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.displacement.numpy(py)
    }

    #[getter]
    fn interface_vertices(&self, py: Python<'_>) -> PyResult<Py<PyArray1<u32>>> {
        self.interface_vertices.numpy(py)
    }

    #[getter]
    fn fluid_action(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.fluid_action.numpy(py)
    }

    #[getter]
    fn solid_action(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.solid_action.numpy(py)
    }

    #[getter]
    fn action_imbalance(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.action_imbalance.numpy(py)
    }

    #[getter]
    const fn previous_kinetic_energy_j_per_m(&self) -> f64 {
        self.previous_kinetic_energy_j_per_m
    }

    #[getter]
    const fn next_kinetic_energy_j_per_m(&self) -> f64 {
        self.next_kinetic_energy_j_per_m
    }

    #[getter]
    const fn previous_elastic_energy_j_per_m(&self) -> f64 {
        self.previous_elastic_energy_j_per_m
    }

    #[getter]
    const fn next_elastic_energy_j_per_m(&self) -> f64 {
        self.next_elastic_energy_j_per_m
    }

    #[getter]
    const fn kinetic_increment_j_per_m(&self) -> f64 {
        self.kinetic_increment_j_per_m
    }

    #[getter]
    const fn elastic_increment_j_per_m(&self) -> f64 {
        self.elastic_increment_j_per_m
    }

    #[getter]
    const fn viscous_dissipation_j_per_m(&self) -> f64 {
        self.viscous_dissipation_j_per_m
    }

    #[getter]
    const fn energy_defect_j_per_m(&self) -> f64 {
        self.energy_defect_j_per_m
    }

    #[getter]
    const fn numerical_residual_norm(&self) -> f64 {
        self.numerical_residual_norm
    }

    #[getter]
    const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }

    #[getter]
    const fn kinematic_residual_norm(&self) -> f64 {
        self.kinematic_residual_norm
    }

    #[getter]
    const fn interface_velocity_jump_norm(&self) -> f64 {
        self.interface_velocity_jump_norm
    }

    #[getter]
    const fn interface_action_imbalance_n_per_m(&self) -> f64 {
        self.interface_action_imbalance_n_per_m
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinearSolveSummary> {
        self.solve.clone_ref(py)
    }

    #[getter]
    const fn assembly_packets(&self) -> usize {
        self.assembly_packets
    }

    #[getter]
    const fn assembly_targets(&self) -> usize {
        self.assembly_targets
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedReferenceFsiStep(ordinal={}, time_s={})",
            self.ordinal, self.time_s
        )
    }
}

/// Frozen two-step result of the accepted fixed-reference FSI application.
#[pyclass(
    name = "FixedReferenceFsiResult",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyFixedReferenceFsiResult {
    semantic_revision: u64,
    realization_revision: u64,
    run_manifest_json: Vec<u8>,
    trajectory: Py<PyTrajectory>,
    fluid_cells: ReadOnlyVector<u32>,
    solid_cells: ReadOnlyVector<u32>,
    interface_facets: ReadOnlyMatrix<u32>,
    steps: [Py<PyFixedReferenceFsiStep>; 2],
    case_ids: [&'static str; 2],
}

impl PartialEq for PyFixedReferenceFsiResult {
    fn eq(&self, other: &Self) -> bool {
        self.run_manifest_json == other.run_manifest_json
    }
}

impl Eq for PyFixedReferenceFsiResult {}

impl Hash for PyFixedReferenceFsiResult {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.run_manifest_json.hash(state);
    }
}

impl PyFixedReferenceFsiResult {
    fn from_native(
        py: Python<'_>,
        model: &PyModel,
        result: FixedReferenceFsiResult2d,
    ) -> PyResult<Self> {
        let run_manifest_json = result
            .run()
            .canonical_json()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let replay = result
            .trajectory_replay()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        let trajectory = Py::new(
            py,
            PyTrajectory::from_replay(py, model, result.mesh_artifact(), &replay, result.run())?,
        )?;

        let fluid_cells = result
            .partition()
            .fluid_cells()
            .iter()
            .map(|cell| u32::try_from(cell.index()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                pyo3::exceptions::PyOverflowError::new_err("FSI cell index exceeds u32")
            })?;
        let solid_cells = result
            .partition()
            .solid_cells()
            .iter()
            .map(|cell| u32::try_from(cell.index()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                pyo3::exceptions::PyOverflowError::new_err("FSI cell index exceeds u32")
            })?;
        let mut interface_facets =
            Vec::with_capacity(result.partition().interface_facets().len() * 2);
        for facet in result.partition().interface_facets() {
            let vertices = result
                .mesh()
                .entity_vertices(MeshEntity::new(1, facet.index()))
                .expect("accepted FSI interface facet owns connectivity");
            for vertex in vertices {
                interface_facets.push(u32::try_from(vertex.index()).map_err(|_| {
                    pyo3::exceptions::PyOverflowError::new_err(
                        "FSI interface vertex index exceeds u32",
                    )
                })?);
            }
        }
        let steps = [
            Py::new(py, project_step(py, &result, 0)?)?,
            Py::new(py, project_step(py, &result, 1)?)?,
        ];

        Ok(Self {
            semantic_revision: result.semantic_revision(),
            realization_revision: result.realization_revision(),
            run_manifest_json,
            trajectory,
            fluid_cells: ReadOnlyVector::new(fluid_cells),
            solid_cells: ReadOnlyVector::new(solid_cells),
            interface_facets: ReadOnlyMatrix::new(
                result.partition().interface_facets().len(),
                2,
                interface_facets,
            ),
            steps,
            case_ids: result.scientific_case_ids(),
        })
    }
}

#[pymethods]
impl PyFixedReferenceFsiResult {
    #[getter]
    const fn semantic_revision(&self) -> u64 {
        self.semantic_revision
    }

    #[getter]
    const fn realization_revision(&self) -> u64 {
        self.realization_revision
    }

    #[getter]
    fn run_manifest_json(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, &self.run_manifest_json).unbind()
    }

    /// Accepted general trajectory projection over the exact durable replay.
    #[getter]
    fn trajectory(&self, py: Python<'_>) -> Py<PyTrajectory> {
        self.trajectory.clone_ref(py)
    }

    #[getter]
    fn fluid_cells(&self, py: Python<'_>) -> PyResult<Py<PyArray1<u32>>> {
        self.fluid_cells.numpy(py)
    }

    #[getter]
    fn solid_cells(&self, py: Python<'_>) -> PyResult<Py<PyArray1<u32>>> {
        self.solid_cells.numpy(py)
    }

    #[getter]
    fn interface_facets(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.interface_facets.numpy(py)
    }

    #[getter]
    fn steps(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.steps.iter().map(|step| step.clone_ref(py)))?.unbind())
    }

    #[pyo3(signature = (ordinal, /))]
    fn step(&self, py: Python<'_>, ordinal: isize) -> PyResult<Py<PyFixedReferenceFsiStep>> {
        let index = match ordinal {
            1 => 0,
            2 => 1,
            _ => return Err(PyIndexError::new_err("FSI step ordinal must be 1 or 2")),
        };
        Ok(self.steps[index].clone_ref(py))
    }

    #[getter]
    const fn case_ids(&self) -> (&'static str, &'static str) {
        (self.case_ids[0], self.case_ids[1])
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "FixedReferenceFsiResult(run_digest='{}')",
            self.trajectory.borrow(py).run_digest_value()
        )
    }
}

fn project_step(
    py: Python<'_>,
    result: &FixedReferenceFsiResult2d,
    position: usize,
) -> PyResult<PyFixedReferenceFsiStep> {
    let solution = &result.solutions()[position];
    let state = &result.states()[position];
    let numerical = solution.numerical_evidence();
    let energy = numerical.energy_balance();
    let actions = numerical.interface_actions();
    let mut interface_vertices = Vec::with_capacity(actions.len());
    let mut fluid_action = Vec::with_capacity(actions.len() * 2);
    let mut solid_action = Vec::with_capacity(actions.len() * 2);
    let mut action_imbalance = Vec::with_capacity(actions.len() * 2);
    for action in actions {
        interface_vertices.push(u32::try_from(action.vertex().index()).map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err("FSI interface vertex index exceeds u32")
        })?);
        fluid_action.extend(action.fluid());
        solid_action.extend(action.solid());
        action_imbalance.extend(action.imbalance());
    }
    let assembly = numerical.assembly_report();
    Ok(PyFixedReferenceFsiStep {
        ordinal: state.step(),
        time_s: state.time_s(),
        velocity: vector_matrix(solution.vertex_velocity_coefficients()),
        bubble_velocity: vector_matrix(solution.fluid_velocity_bubble_coefficients()),
        pressure_vertices: ReadOnlyVector::new(
            solution
                .fluid_pressure_vertices()
                .iter()
                .map(|vertex| u32::try_from(vertex.index()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    pyo3::exceptions::PyOverflowError::new_err(
                        "FSI pressure vertex index exceeds u32",
                    )
                })?,
        ),
        pressure: ReadOnlyVector::new(solution.fluid_pressure_coefficients().to_vec()),
        displacement: vector_matrix(solution.solid_displacement_coefficients()),
        interface_vertices: ReadOnlyVector::new(interface_vertices),
        fluid_action: ReadOnlyMatrix::new(actions.len(), 2, fluid_action),
        solid_action: ReadOnlyMatrix::new(actions.len(), 2, solid_action),
        action_imbalance: ReadOnlyMatrix::new(actions.len(), 2, action_imbalance),
        previous_kinetic_energy_j_per_m: energy.previous_kinetic(),
        next_kinetic_energy_j_per_m: energy.next_kinetic(),
        previous_elastic_energy_j_per_m: energy.previous_elastic(),
        next_elastic_energy_j_per_m: energy.next_elastic(),
        kinetic_increment_j_per_m: energy.kinetic_increment(),
        elastic_increment_j_per_m: energy.elastic_increment(),
        viscous_dissipation_j_per_m: energy.viscous_dissipation(),
        energy_defect_j_per_m: energy.defect(),
        numerical_residual_norm: numerical.residual_norm(),
        continuity_residual_norm: numerical.continuity_residual_norm(),
        kinematic_residual_norm: numerical.kinematic_residual_norm(),
        interface_velocity_jump_norm: numerical.interface_velocity_jump_norm(),
        interface_action_imbalance_n_per_m: numerical.interface_action_imbalance_norm(),
        solve: Py::new(
            py,
            PyLinearSolveSummary::from_report(numerical.solve_report()),
        )?,
        assembly_packets: assembly.packet_count(),
        assembly_targets: assembly.target_count(),
    })
}

fn vector_matrix(values: &[[f64; 2]]) -> ReadOnlyMatrix<f64> {
    ReadOnlyMatrix::new(
        values.len(),
        2,
        values
            .iter()
            .flat_map(|value| value.iter().copied())
            .collect(),
    )
}

/// Execute the accepted fixed-reference two-step FSI application path.
#[pyfunction]
#[pyo3(signature = (model, /))]
pub(crate) fn solve_fixed_reference_fsi(
    py: Python<'_>,
    model: &PyModel,
) -> PyResult<PyFixedReferenceFsiResult> {
    panic_boundary(py, || {
        let document = model.document().clone();
        let result = py.detach(move || {
            FixedReferenceFsiResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
        });
        result
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))
            .and_then(|result| PyFixedReferenceFsiResult::from_native(py, model, result))
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFixedReferenceFsiStep>()?;
    module.add_class::<PyFixedReferenceFsiResult>()?;
    module.add_function(wrap_pyfunction!(solve_fixed_reference_fsi, module)?)?;
    Ok(())
}
