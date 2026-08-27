//! Python intent, Plan, Result trajectory, and typed evidence for bounded FSI.

use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::api::{
    FixedMeshMonolithicFsiIntent2d, FixedReferenceFsiResult2d, ResolvedFixedMeshMonolithicFsiPlan2d,
};
use eqiora::meshing::MeshEntity;
use eqiora::solver::REFERENCE_LINEAR_SOLVER;
use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::{PyOverflowError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule, PyTuple};

use crate::error::diagnostic_error;
use crate::execution::RunIdentity;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::{PyLinearSolveSummary, PyRunManifest};
use crate::result::PyRunResult;
use crate::trajectory::{PyState, PyTrajectory};

/// Complete fixed-mesh monolithic FSI request with no hidden numerical state.
#[pyclass(
    name = "FixedMeshMonolithic",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyFixedMeshMonolithic {
    native: FixedMeshMonolithicFsiIntent2d,
}

#[pymethods]
impl PyFixedMeshMonolithic {
    #[new]
    #[pyo3(signature = (*, time_step_s, steps, initial_velocity_m_per_s, initial_free_interface_displacement_m, length_scale_m, velocity_scale_m_per_s, pressure_scale_pa, relative_tolerance, absolute_tolerance, maximum_iterations))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        time_step_s: f64,
        steps: i64,
        initial_velocity_m_per_s: [f64; 2],
        initial_free_interface_displacement_m: [f64; 2],
        length_scale_m: f64,
        velocity_scale_m_per_s: f64,
        pressure_scale_pa: f64,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: i64,
    ) -> PyResult<Self> {
        let steps = positive_usize(py, "steps", steps)?;
        let maximum_iterations = positive_usize(py, "maximum_iterations", maximum_iterations)?;
        FixedMeshMonolithicFsiIntent2d::new(
            time_step_s,
            steps,
            initial_velocity_m_per_s,
            initial_free_interface_displacement_m,
            length_scale_m,
            velocity_scale_m_per_s,
            pressure_scale_pa,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )
        .map(|native| Self { native })
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
    }

    #[getter]
    fn time_step_s(&self) -> f64 {
        self.native.time_step_s()
    }

    #[getter]
    fn steps(&self) -> usize {
        self.native.steps().get()
    }

    #[getter]
    const fn initial_velocity_m_per_s(&self) -> (f64, f64) {
        tuple2(self.native.initial_velocity_m_per_s())
    }

    #[getter]
    const fn initial_free_interface_displacement_m(&self) -> (f64, f64) {
        tuple2(self.native.initial_free_interface_displacement_m())
    }

    #[getter]
    fn length_scale_m(&self) -> f64 {
        self.native.length_scale_m()
    }

    #[getter]
    fn velocity_scale_m_per_s(&self) -> f64 {
        self.native.velocity_scale_m_per_s()
    }

    #[getter]
    fn pressure_scale_pa(&self) -> f64 {
        self.native.pressure_scale_pa()
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

    fn __hash__(&self) -> u64 {
        hash_intent(self.native)
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedMeshMonolithic(time_step_s={}, steps={}, relative_tolerance={:e}, absolute_tolerance={:e}, maximum_iterations={})",
            self.time_step_s(),
            self.steps(),
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
        )
    }
}

/// Immutable, fully resolved fixed-mesh monolithic FSI Plan.
#[pyclass(
    name = "FixedMeshMonolithicPlan",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFixedMeshMonolithicPlan {
    native: ResolvedFixedMeshMonolithicFsiPlan2d,
    model: Py<PyModel>,
    model_digest: String,
    geometry_digest: String,
    correspondence_digest: String,
    mesh_digest: String,
    realization_digest: String,
    canonical_bytes: Vec<u8>,
}

impl PyFixedMeshMonolithicPlan {
    fn from_native(
        py: Python<'_>,
        native: ResolvedFixedMeshMonolithicFsiPlan2d,
        model: Py<PyModel>,
    ) -> PyResult<Self> {
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
            model,
            model_digest,
            geometry_digest,
            correspondence_digest,
            mesh_digest,
            realization_digest,
            canonical_bytes,
        })
    }

    pub(crate) const fn native(&self) -> &ResolvedFixedMeshMonolithicFsiPlan2d {
        &self.native
    }

    pub(crate) fn model(&self, py: Python<'_>) -> Py<PyModel> {
        self.model.clone_ref(py)
    }
}

#[pymethods]
impl PyFixedMeshMonolithicPlan {
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
        2
    }

    #[getter]
    const fn coupling_method(&self) -> &'static str {
        "monolithic"
    }

    #[getter]
    const fn geometry_motion(&self) -> &'static str {
        "none"
    }

    #[getter]
    const fn mesh_kind(&self) -> &'static str {
        "imported-affine-simplicial"
    }

    #[getter]
    const fn fluid_velocity_space(&self) -> &'static str {
        "simplex-p1-bubble"
    }

    #[getter]
    const fn fluid_pressure_space(&self) -> &'static str {
        "continuous-lagrange-1"
    }

    #[getter]
    const fn solid_velocity_space(&self) -> &'static str {
        "continuous-lagrange-1"
    }

    #[getter]
    const fn solid_displacement_space(&self) -> &'static str {
        "backward-euler-eliminated-continuous-lagrange-1"
    }

    #[getter]
    const fn time_integrator(&self) -> &'static str {
        "backward-euler"
    }

    #[getter]
    fn time_step_s(&self) -> f64 {
        self.native.intent().time_step_s()
    }

    #[getter]
    fn steps(&self) -> usize {
        self.native.intent().steps().get()
    }

    #[getter]
    const fn initial_velocity_m_per_s(&self) -> (f64, f64) {
        tuple2(self.native.intent().initial_velocity_m_per_s())
    }

    #[getter]
    const fn initial_free_interface_displacement_m(&self) -> (f64, f64) {
        tuple2(self.native.intent().initial_free_interface_displacement_m())
    }

    #[getter]
    fn length_scale_m(&self) -> f64 {
        self.native.intent().length_scale_m()
    }

    #[getter]
    fn velocity_scale_m_per_s(&self) -> f64 {
        self.native.intent().velocity_scale_m_per_s()
    }

    #[getter]
    fn pressure_scale_pa(&self) -> f64 {
        self.native.intent().pressure_scale_pa()
    }

    #[getter]
    const fn solver_algorithm(&self) -> &'static str {
        "minimum-residual"
    }

    #[getter]
    const fn preconditioner(&self) -> &'static str {
        "identity"
    }

    #[getter]
    const fn reduction(&self) -> &'static str {
        "reproducible"
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

    fn __hash__(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.model_digest.hash(&mut hasher);
        hash_intent_into(&self.native.intent(), &mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedMeshMonolithicPlan(model_digest={:?}, realization_digest={:?})",
            self.model_digest, self.realization_digest,
        )
    }
}

/// Scientific and solver observations bound to one exact accepted state.
#[pyclass(
    name = "FixedMeshMonolithicStateEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFixedMeshMonolithicStateEvidence {
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
impl PyFixedMeshMonolithicStateEvidence {
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

    fn __repr__(&self) -> String {
        format!(
            "FixedMeshMonolithicStateEvidence(state_digest={:?})",
            self.state_digest,
        )
    }
}

/// Typed evidence for one exact fixed-mesh monolithic FSI Result.
#[pyclass(
    name = "FixedMeshMonolithicEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFixedMeshMonolithicEvidence {
    model_digest: String,
    trajectory_digest: String,
    run_digest: String,
    fluid_cells: ReadOnlyVector<u32>,
    solid_cells: ReadOnlyVector<u32>,
    interface_facets: ReadOnlyMatrix<u32>,
    state_owners: [Py<PyState>; 2],
    states: [Py<PyFixedMeshMonolithicStateEvidence>; 2],
    case_ids: [&'static str; 2],
}

#[pymethods]
impl PyFixedMeshMonolithicEvidence {
    #[getter]
    fn trajectory_digest(&self) -> &str {
        &self.trajectory_digest
    }

    #[getter]
    fn run_digest(&self) -> &str {
        &self.run_digest
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

    /// Select evidence only through the exact state object owned by this Result.
    #[pyo3(signature = (state, /))]
    fn state(
        &self,
        py: Python<'_>,
        state: &Bound<'_, PyState>,
    ) -> PyResult<Py<PyFixedMeshMonolithicStateEvidence>> {
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
        if state.borrow().digest_value() != self.states[position].borrow(py).state_digest {
            return Err(PyValueError::new_err(
                "State identity differs from its accepted FSI evidence",
            ));
        }
        Ok(self.states[position].clone_ref(py))
    }

    #[getter]
    const fn case_ids(&self) -> (&'static str, &'static str) {
        (self.case_ids[0], self.case_ids[1])
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedMeshMonolithicEvidence(run_digest={:?})",
            self.run_digest,
        )
    }
}

impl PyFixedMeshMonolithicEvidence {
    fn from_native(
        py: Python<'_>,
        result: &FixedReferenceFsiResult2d,
        trajectory: &PyTrajectory,
    ) -> PyResult<Self> {
        let fluid_cells = result
            .partition()
            .fluid_cells()
            .iter()
            .map(|cell| u32::try_from(cell.index()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PyOverflowError::new_err("FSI cell index exceeds uint32"))?;
        let solid_cells = result
            .partition()
            .solid_cells()
            .iter()
            .map(|cell| u32::try_from(cell.index()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PyOverflowError::new_err("FSI cell index exceeds uint32"))?;
        let mut interface_facets =
            Vec::with_capacity(result.partition().interface_facets().len() * 2);
        for facet in result.partition().interface_facets() {
            let vertices = result
                .mesh()
                .entity_vertices(MeshEntity::new(1, facet.index()))
                .expect("accepted FSI interface facet owns connectivity");
            for vertex in vertices {
                interface_facets.push(u32::try_from(vertex.index()).map_err(|_| {
                    PyOverflowError::new_err("FSI interface vertex index exceeds uint32")
                })?);
            }
        }
        let state_owners: [Py<PyState>; 2] = trajectory
            .state_handles(py)
            .try_into()
            .map_err(|_| PyValueError::new_err("accepted FSI trajectory must have two states"))?;
        let states = [
            Py::new(py, project_state_evidence(py, result, 0)?)?,
            Py::new(py, project_state_evidence(py, result, 1)?)?,
        ];
        for (owner, evidence) in state_owners.iter().zip(&states) {
            if owner.borrow(py).digest_value() != evidence.borrow(py).state_digest {
                return Err(PyValueError::new_err(
                    "accepted FSI state evidence differs from the Trajectory state",
                ));
            }
        }
        let run_digest = result
            .run()
            .digest()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .to_string();
        Ok(Self {
            model_digest: trajectory.model_digest_value().to_owned(),
            trajectory_digest: trajectory.digest_value().to_owned(),
            run_digest,
            fluid_cells: ReadOnlyVector::new(fluid_cells),
            solid_cells: ReadOnlyVector::new(solid_cells),
            interface_facets: ReadOnlyMatrix::new(
                result.partition().interface_facets().len(),
                2,
                interface_facets,
            ),
            state_owners,
            states,
            case_ids: result.scientific_case_ids(),
        })
    }
}

fn project_state_evidence(
    py: Python<'_>,
    result: &FixedReferenceFsiResult2d,
    position: usize,
) -> PyResult<PyFixedMeshMonolithicStateEvidence> {
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
        interface_vertices.push(
            u32::try_from(action.vertex().index()).map_err(|_| {
                PyOverflowError::new_err("FSI interface vertex index exceeds uint32")
            })?,
        );
        fluid_action.extend(action.fluid());
        solid_action.extend(action.solid());
        action_imbalance.extend(action.imbalance());
    }
    let assembly = numerical.assembly_report();
    let state_digest = state
        .digest()
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
        .to_string();
    Ok(PyFixedMeshMonolithicStateEvidence {
        state_digest,
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

/// Resolve one complete FSI intent without executing it.
#[pyfunction]
#[pyo3(name = "resolve_fixed_mesh_monolithic")]
#[pyo3(signature = (model, intent, /))]
pub(crate) fn resolve(
    py: Python<'_>,
    model: &Bound<'_, PyModel>,
    intent: &PyFixedMeshMonolithic,
) -> PyResult<PyFixedMeshMonolithicPlan> {
    panic_boundary(py, || {
        let document = model
            .borrow()
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
        let native_intent = intent.native;
        let native = py.detach(move || {
            ResolvedFixedMeshMonolithicFsiPlan2d::resolve(
                &document,
                native_intent,
                &REFERENCE_LINEAR_SOLVER,
            )
        });
        native
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
            .and_then(|native| {
                PyFixedMeshMonolithicPlan::from_native(py, native, model.clone().unbind())
            })
    })
}

pub(crate) fn materialize_result(
    py: Python<'_>,
    result: FixedReferenceFsiResult2d,
    identity: RunIdentity,
    elapsed_seconds: f64,
    model: PyRef<'_, PyModel>,
) -> PyResult<PyRunResult> {
    let replay = result
        .trajectory_replay()
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
    let trajectory = Py::new(
        py,
        PyTrajectory::from_replay(py, &model, result.mesh_artifact(), &replay, result.run())?,
    )?;
    let evidence = Py::new(
        py,
        PyFixedMeshMonolithicEvidence::from_native(py, &result, &trajectory.borrow(py))?,
    )?;
    let run_manifest = Py::new(py, PyRunManifest::from_value(py, result.run().clone())?)?;
    Ok(PyRunResult::from_fixed_mesh_monolithic_fsi(
        identity,
        elapsed_seconds,
        trajectory,
        run_manifest,
        evidence,
    ))
}

#[pyfunction]
#[pyo3(signature = (result, /))]
fn fixed_mesh_monolithic_evidence(
    py: Python<'_>,
    result: &PyRunResult,
) -> PyResult<Py<PyFixedMeshMonolithicEvidence>> {
    result.fixed_mesh_monolithic_evidence(py)
}

fn positive_usize(_py: Python<'_>, name: &str, value: i64) -> PyResult<NonZeroUsize> {
    usize::try_from(value)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "fixed-mesh monolithic FSI {name} must be strictly positive"
            ))
        })
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

fn hash_float<H: Hasher>(value: f64, hasher: &mut H) {
    let normalized = if value == 0.0 { 0.0 } else { value };
    normalized.to_bits().hash(hasher);
}

fn hash_intent_into<H: Hasher>(intent: &FixedMeshMonolithicFsiIntent2d, hasher: &mut H) {
    hash_float(intent.time_step_s(), hasher);
    intent.steps().hash(hasher);
    for value in intent
        .initial_velocity_m_per_s()
        .into_iter()
        .chain(intent.initial_free_interface_displacement_m())
    {
        hash_float(value, hasher);
    }
    for value in [
        intent.length_scale_m(),
        intent.velocity_scale_m_per_s(),
        intent.pressure_scale_pa(),
        intent.relative_tolerance(),
        intent.absolute_tolerance(),
    ] {
        hash_float(value, hasher);
    }
    intent.maximum_iterations().hash(hasher);
}

fn hash_intent(intent: FixedMeshMonolithicFsiIntent2d) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_intent_into(&intent, &mut hasher);
    hasher.finish()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFixedMeshMonolithic>()?;
    module.add_class::<PyFixedMeshMonolithicPlan>()?;
    module.add_class::<PyFixedMeshMonolithicStateEvidence>()?;
    module.add_class::<PyFixedMeshMonolithicEvidence>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    module.add_function(wrap_pyfunction!(fixed_mesh_monolithic_evidence, module)?)?;
    Ok(())
}
