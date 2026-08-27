//! Observation-only evidence projected from accepted common FSI states.

use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::{PyOverflowError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyTuple};

use crate::common_plan::PyPlan;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::realization::PyLinearSolveSummary;
use crate::result::PyRunResult;
use crate::trajectory::{PyState, PyTrajectory};

#[pyclass(
    name = "FsiStateEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFsiStateEvidence {
    state_digest: String,
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
impl PyFsiStateEvidence {
    #[getter]
    fn state_digest(&self) -> &str {
        &self.state_digest
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
}

#[pyclass(
    name = "FsiEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFsiEvidence {
    model_digest: String,
    request_identity: String,
    fluid_cells: ReadOnlyVector<u32>,
    solid_cells: ReadOnlyVector<u32>,
    interface_facets: ReadOnlyMatrix<u32>,
    state_owners: Vec<Py<PyState>>,
    states: Vec<Py<PyFsiStateEvidence>>,
}

#[pymethods]
impl PyFsiEvidence {
    #[getter]
    fn request_identity(&self) -> &str {
        &self.request_identity
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
    fn states(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.states.iter().map(|state| state.clone_ref(py)))?.unbind())
    }
    fn state(
        &self,
        py: Python<'_>,
        state: &Bound<'_, PyState>,
    ) -> PyResult<Py<PyFsiStateEvidence>> {
        if state.borrow().model_digest_value() != self.model_digest {
            return Err(PyValueError::new_err(
                "State belongs to a different exact Model artifact",
            ));
        }
        let position = self
            .state_owners
            .iter()
            .position(|owner| owner.bind(py).is(state))
            .ok_or_else(|| {
                PyValueError::new_err("State belongs to a different Result occurrence")
            })?;
        Ok(self.states[position].clone_ref(py))
    }
}

impl PyFsiEvidence {
    pub(crate) fn from_common(
        py: Python<'_>,
        plan: &PyPlan,
        trajectory: &PyTrajectory,
        request_identity: &str,
    ) -> PyResult<Self> {
        let native_plan = plan
            .fsi_native()
            .ok_or_else(|| PyValueError::new_err("FSI evidence requires an FSI Plan"))?;
        let owners = trajectory.state_handles(py);
        let mut states = Vec::with_capacity(owners.len());
        for owner in &owners {
            let state = owner.borrow(py);
            let native = state
                .common_native()
                .and_then(|state| state.fsi_accepted_solution())
                .ok_or_else(|| {
                    PyValueError::new_err("FSI evidence requires accepted common FSI output states")
                })?;
            let numerical = native.numerical_evidence();
            let energy = numerical.energy_balance();
            let actions = numerical.interface_actions();
            let mut interface_vertices = Vec::with_capacity(actions.len());
            let mut fluid_action = Vec::with_capacity(actions.len() * 2);
            let mut solid_action = Vec::with_capacity(actions.len() * 2);
            let mut imbalance = Vec::with_capacity(actions.len() * 2);
            for action in actions {
                interface_vertices.push(u32::try_from(action.vertex().index()).map_err(|_| {
                    PyOverflowError::new_err("FSI interface vertex exceeds uint32")
                })?);
                fluid_action.extend(action.fluid());
                solid_action.extend(action.solid());
                imbalance.extend(action.imbalance());
            }
            let assembly = numerical.assembly_report();
            states.push(Py::new(
                py,
                PyFsiStateEvidence {
                    state_digest: state.digest_value().to_owned(),
                    interface_vertices: ReadOnlyVector::new(interface_vertices),
                    fluid_action: ReadOnlyMatrix::new(actions.len(), 2, fluid_action),
                    solid_action: ReadOnlyMatrix::new(actions.len(), 2, solid_action),
                    action_imbalance: ReadOnlyMatrix::new(actions.len(), 2, imbalance),
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
                },
            )?);
        }
        let convert = |values: Vec<usize>| {
            values
                .into_iter()
                .map(|value| {
                    u32::try_from(value)
                        .map_err(|_| PyOverflowError::new_err("FSI cell index exceeds uint32"))
                })
                .collect::<PyResult<Vec<_>>>()
        };
        let facets = native_plan
            .interface_facet_vertices()
            .into_iter()
            .flatten()
            .map(|value| {
                u32::try_from(value)
                    .map_err(|_| PyOverflowError::new_err("FSI facet vertex exceeds uint32"))
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            model_digest: native_plan.model_digest().to_owned(),
            request_identity: request_identity.to_owned(),
            fluid_cells: ReadOnlyVector::new(convert(native_plan.fluid_cell_indices())?),
            solid_cells: ReadOnlyVector::new(convert(native_plan.solid_cell_indices())?),
            interface_facets: ReadOnlyMatrix::new(facets.len() / 2, 2, facets),
            state_owners: owners,
            states,
        })
    }
}

#[pyfunction]
fn evidence(py: Python<'_>, result: &PyRunResult) -> PyResult<Py<PyFsiEvidence>> {
    result.fsi_evidence(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFsiStateEvidence>()?;
    module.add_class::<PyFsiEvidence>()?;
    module.add_function(wrap_pyfunction!(evidence, module)?)?;
    Ok(())
}
