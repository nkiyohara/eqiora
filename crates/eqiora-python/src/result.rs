//! Common installed-Python ownership for accepted execution results.

use std::collections::BTreeMap;
use std::time::Duration;

use eqiora::diagnostic::codes;
use eqiora::numerics::{
    CartesianLinearElasticity2dSolution, ResolvedScalarEllipticCartesianSolution,
    SteadyStokesMiniSolution2d,
};
use eqiora::{Diagnostic, DimExponents};
use eqiora_numerics::{CommonElasticityObservation, CommonSteadyStokesObservation};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::array::PyArrayBuffer;
use crate::common_plan::{CommonPlanKind, PyPlan};
use crate::diagnostic_error;
use crate::elasticity::PyLinearElasticityEvidence;
use crate::execution::RunIdentity;
use crate::fsi_evidence::PyFsiEvidence;
use crate::geometry::PyGeometrySelection;
use crate::meshing::PyMesh;
use crate::model::PyModelFieldRef;
use crate::realization::PyLinearSolveSummary;
use crate::steady_stokes::PySteadyStokesEvidence;
use crate::trajectory::{PyBoundaryFlux, PyBoundaryForce, PyState, PyTrajectory};

mod field_output;

use field_output::FieldOutputBlock;
pub(crate) use field_output::PyFieldOutput;

/// One read-only, field-local sampled series in SI units.
#[pyclass(name = "Series", module = "eqiora._eqiora", frozen)]
pub(crate) struct PySeries {
    field: Option<Py<PyModelFieldRef>>,
    id: String,
    name: Option<String>,
    dimension: DimExponents,
    time: Py<PyArrayBuffer>,
    values: Py<PyArrayBuffer>,
}

#[pymethods]
impl PySeries {
    #[getter]
    fn field(&self, py: Python<'_>) -> Option<Py<PyModelFieldRef>> {
        self.field.as_ref().map(|field| field.clone_ref(py))
    }
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    #[getter]
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[getter]
    fn dimension(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        (
            self.dimension.mass,
            self.dimension.length,
            self.dimension.time,
            self.dimension.current,
            self.dimension.temperature,
            self.dimension.amount,
            self.dimension.luminous_intensity,
        )
    }

    #[getter]
    fn time(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.time.clone_ref(py)
    }

    #[getter]
    fn values(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.values.clone_ref(py)
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        Ok(self.values.borrow(py).len())
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let time = self.time.borrow(py).snapshot(py)?;
        let values = self.values.borrow(py).snapshot(py)?;
        if time.len() != values.len() {
            return Err(PyRuntimeError::new_err(
                "Series time and value buffers report different lengths",
            ));
        }
        let samples = PyList::new(py, time.into_iter().zip(values))?;
        Ok(samples.as_any().try_iter()?.into_any().unbind())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let label = self.name.as_deref().unwrap_or(&self.id);
        Ok(format!("Series({label:?}, samples={})", self.__len__(py)?))
    }
}

enum StaticScientificEvidence {
    SteadyStokes(Py<PySteadyStokesEvidence>),
    LinearElasticity(Py<PyLinearElasticityEvidence>),
}

struct CommonFieldResultPayload {
    outputs: Vec<Py<PyFieldOutput>>,
    lookup: BTreeMap<String, usize>,
    solve: Py<PyLinearSolveSummary>,
    evidence: Option<StaticScientificEvidence>,
    steady_stokes_observation: Option<CommonSteadyStokesObservation>,
}

struct CommonTrajectoryResultPayload {
    trajectory: Py<PyTrajectory>,
    fsi_evidence: Option<Py<PyFsiEvidence>>,
}

struct CommonOdeResultPayload {
    fields: Vec<Py<PySeries>>,
    lookup: BTreeMap<String, usize>,
    states: Vec<eqiora_numerics::CommonOdeState>,
}

enum ResultPayload {
    Fields(Box<CommonFieldResultPayload>),
    Trajectory(CommonTrajectoryResultPayload),
    Ode(CommonOdeResultPayload),
}

/// One accepted execution occurrence with typed output relationships.
#[pyclass(
    name = "Result",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyRunResult {
    identity: RunIdentity,
    elapsed_seconds: f64,
    payload: ResultPayload,
}

impl PyRunResult {
    pub(crate) fn common_state_at(
        &self,
        py: Python<'_>,
        state_space_identity: &str,
        time_s: f64,
    ) -> Option<Py<PyState>> {
        let ResultPayload::Trajectory(payload) = &self.payload else {
            return None;
        };
        payload
            .trajectory
            .borrow(py)
            .state_handles(py)
            .into_iter()
            .find(|state| {
                let state = state.borrow(py);
                state
                    .common_native()
                    .is_some_and(|native| native.state_space_identity() == state_space_identity)
                    && state.time_s_value().to_bits() == time_s.to_bits()
            })
    }
}

#[pymethods]
impl PyRunResult {
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

    #[getter]
    fn adapter_version(&self) -> &'static str {
        self.identity.adapter_version()
    }

    #[getter]
    const fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    /// Independently sampled common-ODE series in canonical Field order.
    #[getter]
    fn fields(&self, py: Python<'_>) -> Vec<Py<PySeries>> {
        match &self.payload {
            ResultPayload::Ode(payload) => payload
                .fields
                .iter()
                .map(|field| field.clone_ref(py))
                .collect(),
            ResultPayload::Fields(_) | ResultPayload::Trajectory(_) => Vec::new(),
        }
    }

    /// Select one no-Mesh scalar series by exact Model-bound Field identity.
    #[pyo3(signature = (field, /))]
    fn series(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PySeries>> {
        if field.exact_model_digest() != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let ResultPayload::Ode(payload) = &self.payload else {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        };
        let index = payload
            .lookup
            .get(field.exact_id())
            .copied()
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(payload.fields[index].clone_ref(py))
    }

    /// Exact durable spatial trajectory when this Result owns one.
    #[getter]
    fn trajectory(&self, py: Python<'_>) -> PyResult<Py<PyTrajectory>> {
        match &self.payload {
            ResultPayload::Trajectory(payload) => Ok(payload.trajectory.clone_ref(py)),
            ResultPayload::Fields(_) | ResultPayload::Ode(_) => Err(capability_error(
                py,
                "this Result occurrence has no spatial Trajectory",
            )),
        }
    }

    /// Select the exact accepted Mesh paired with one static Field.
    #[pyo3(signature = (field, /))]
    fn mesh(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyMesh>> {
        if let ResultPayload::Fields(_) = &self.payload {
            return self
                .common_output(py, field)
                .map(|output| output.borrow(py).mesh_handle(py));
        }
        Err(PyKeyError::new_err(field.exact_id().to_owned()))
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> PyResult<Py<PyLinearSolveSummary>> {
        match &self.payload {
            ResultPayload::Fields(payload) => Ok(payload.solve.clone_ref(py)),
            _ => Err(capability_error(
                py,
                "this Result occurrence has no linear solve summary",
            )),
        }
    }

    /// Select one common multi-field output by exact FieldRef identity.
    #[pyo3(signature = (field, /))]
    fn output(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyFieldOutput>> {
        self.common_output(py, field)
    }

    /// Signed force pair on one exact Geometry boundary of this Result.
    #[pyo3(signature = (selection, /))]
    fn boundary_force(
        &self,
        py: Python<'_>,
        selection: Py<PyGeometrySelection>,
    ) -> PyResult<Py<PyBoundaryForce>> {
        let payload = self.steady_stokes_payload(py, "boundary-force")?;
        let selected = selection.borrow(py);
        self.validate_observable_selection(py, payload, &selected)?;
        let name = selected.canonical_name().to_owned();
        if name != "cylinder" {
            return Err(PyKeyError::new_err(format!(
                "this Result has no boundary-force observable for {name:?}"
            )));
        }
        let geometry_digest = selected.bound_source_digest().to_owned();
        drop(selected);
        let mesh = payload.outputs[0].borrow(py).mesh_handle(py);
        let mesh_digest = mesh.borrow(py).exact_mesh_digest().to_owned();
        let force = payload
            .steady_stokes_observation
            .as_ref()
            .expect("validated steady-Stokes observation")
            .cylinder_force_on_fluid();
        Py::new(
            py,
            PyBoundaryForce::new(
                selection,
                name,
                geometry_digest,
                self.identity.plan_key(),
                "result",
                &mesh_digest,
                force,
            ),
        )
    }

    /// Signed volume flux on one exact Geometry boundary of this Result.
    #[pyo3(signature = (selection, /))]
    fn boundary_flux(
        &self,
        py: Python<'_>,
        selection: Py<PyGeometrySelection>,
    ) -> PyResult<Py<PyBoundaryFlux>> {
        let payload = self.steady_stokes_payload(py, "boundary-flux")?;
        let selected = selection.borrow(py);
        self.validate_observable_selection(py, payload, &selected)?;
        let name = selected.canonical_name().to_owned();
        let observation = payload
            .steady_stokes_observation
            .as_ref()
            .expect("validated steady-Stokes observation");
        let value = match name.as_str() {
            "inlet" => observation.inlet_flux(),
            "outlet" => observation.outlet_flux(),
            _ => {
                return Err(PyKeyError::new_err(format!(
                    "this Result has no boundary-flux observable for {name:?}"
                )));
            }
        };
        let geometry_digest = selected.bound_source_digest().to_owned();
        drop(selected);
        let mesh = payload.outputs[0].borrow(py).mesh_handle(py);
        let mesh_digest = mesh.borrow(py).exact_mesh_digest().to_owned();
        Py::new(
            py,
            PyBoundaryFlux::new(
                selection,
                name,
                geometry_digest,
                self.identity.plan_key(),
                &mesh_digest,
                value,
            ),
        )
    }

    fn __repr__(&self) -> String {
        let fields = match &self.payload {
            ResultPayload::Fields(payload) => payload.outputs.len(),
            ResultPayload::Trajectory(_) => 0,
            ResultPayload::Ode(payload) => payload.fields.len(),
        };
        format!(
            "Result(fields={}, model_digest={:?}, plan_key={:?})",
            fields,
            self.model_digest(),
            self.plan_key(),
        )
    }
}

impl PyRunResult {
    pub(crate) fn plan_key_value(&self) -> &str {
        self.identity.plan_key()
    }
    pub(crate) fn common_ode_state_at(
        &self,
        state_space_identity: &str,
        time_s: f64,
    ) -> Option<eqiora_numerics::CommonOdeState> {
        let ResultPayload::Ode(payload) = &self.payload else {
            return None;
        };
        payload
            .states
            .iter()
            .find(|state| {
                state.state_space_identity() == state_space_identity
                    && state.time_s().to_bits() == time_s.to_bits()
            })
            .cloned()
    }

    fn common_output(
        &self,
        py: Python<'_>,
        field: &PyModelFieldRef,
    ) -> PyResult<Py<PyFieldOutput>> {
        if field.exact_model_digest() != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let ResultPayload::Fields(payload) = &self.payload else {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        };
        let index = payload
            .lookup
            .get(field.exact_id())
            .copied()
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(payload.outputs[index].clone_ref(py))
    }

    fn steady_stokes_payload(
        &self,
        py: Python<'_>,
        observable: &str,
    ) -> PyResult<&CommonFieldResultPayload> {
        let ResultPayload::Fields(payload) = &self.payload else {
            return Err(capability_error(
                py,
                &format!("this Result occurrence has no {observable} observable"),
            ));
        };
        if payload.steady_stokes_observation.is_none() {
            return Err(capability_error(
                py,
                &format!("this Result occurrence has no {observable} observable"),
            ));
        }
        Ok(payload)
    }

    fn validate_observable_selection(
        &self,
        py: Python<'_>,
        payload: &CommonFieldResultPayload,
        selection: &PyGeometrySelection,
    ) -> PyResult<()> {
        let mesh = payload.outputs[0].borrow(py).mesh_handle(py);
        let source_digest = mesh.borrow(py).source_digest_value().to_owned();
        if selection.bound_source_digest() != source_digest {
            return Err(PyValueError::new_err(
                "GeometrySelection belongs to a foreign or stale Geometry revision",
            ));
        }
        if selection.canonical_dimension() != 1 {
            return Err(PyValueError::new_err(
                "Result boundary observables require a codimension-one GeometrySelection",
            ));
        }
        Ok(())
    }

    pub(crate) fn from_common_scalar(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        field: (String, DimExponents),
        cells: [usize; 2],
        elapsed: Duration,
        run: ResolvedScalarEllipticCartesianSolution,
    ) -> PyResult<Self> {
        let (field_id, field_dimension) = field;
        let (values, solve, association, logical_shape, space) = match run {
            ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => (
                solution.field().vertex_values().to_vec(),
                solution.solve_report().clone(),
                "vertex",
                [cells[0] + 1, cells[1] + 1],
                "continuous-lagrange-p1",
            ),
            ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => (
                solution.cell_values().to_vec(),
                solution.solve_report().clone(),
                "cell",
                cells,
                "cell-constant",
            ),
        };
        let values = PyArrayBuffer::from_owned_result(py, values)?;
        let solve = Py::new(py, PyLinearSolveSummary::from_report(&solve))?;
        let field = Py::new(
            py,
            PyModelFieldRef::from_exact(identity.model_digest().to_owned(), field_id.clone()),
        )?;
        let coefficient_count = logical_shape[0] * logical_shape[1];
        let output = Py::new(
            py,
            PyFieldOutput::new(
                field,
                mesh,
                field_dimension,
                Vec::new(),
                space,
                vec![FieldOutputBlock::new(
                    association,
                    values,
                    coefficient_count,
                    logical_shape.to_vec(),
                )],
            ),
        )?;
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::Fields(Box::new(CommonFieldResultPayload {
                outputs: vec![output],
                lookup: BTreeMap::from([(field_id, 0)]),
                solve,
                evidence: None,
                steady_stokes_observation: None,
            })),
        })
    }

    pub(crate) fn from_common_steady_stokes(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        field_ids: (String, String),
        elapsed: Duration,
        run: SteadyStokesMiniSolution2d,
        observation: CommonSteadyStokesObservation,
    ) -> PyResult<Self> {
        let (velocity_field_id, pressure_field_id) = field_ids;
        let evidence = Py::new(
            py,
            PySteadyStokesEvidence::from_common(py, identity.plan_key(), &observation)?,
        )?;
        let velocity_vertex_count = run.velocity().vertex_values().len();
        let velocity_cell_count = run.velocity().cell_bubble_values().len();
        let pressure_vertex_count = run.pressure().vertex_values().len();
        let velocity_vertices: Vec<f64> = run
            .velocity()
            .vertex_values()
            .iter()
            .flat_map(|value| value.iter().copied())
            .collect();
        let velocity_cells: Vec<f64> = run
            .velocity()
            .cell_bubble_values()
            .iter()
            .flat_map(|value| value.iter().copied())
            .collect();
        let pressure_vertices = run.pressure().vertex_values().to_vec();
        if velocity_vertices.len() != velocity_vertex_count * 2
            || velocity_cells.len() != velocity_cell_count * 2
            || pressure_vertices.len() != pressure_vertex_count
        {
            return Err(PyRuntimeError::new_err(
                "steady-Stokes coefficient blocks disagree with typed FieldOutput metadata",
            ));
        }
        let solve = Py::new(
            py,
            PyLinearSolveSummary::from_report(run.dimensionless_solution().solve_report()),
        )?;
        let velocity_field = Py::new(
            py,
            PyModelFieldRef::from_exact(
                identity.model_digest().to_owned(),
                velocity_field_id.clone(),
            ),
        )?;
        let pressure_field = Py::new(
            py,
            PyModelFieldRef::from_exact(
                identity.model_digest().to_owned(),
                pressure_field_id.clone(),
            ),
        )?;
        let velocity = Py::new(
            py,
            PyFieldOutput::new(
                velocity_field,
                mesh.clone_ref(py),
                DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
                vec![2],
                "simplex-p1-bubble",
                vec![
                    FieldOutputBlock::new(
                        "vertex",
                        PyArrayBuffer::from_owned_result(py, velocity_vertices)?,
                        velocity_vertex_count,
                        vec![velocity_vertex_count, 2],
                    ),
                    FieldOutputBlock::new(
                        "cell-bubble",
                        PyArrayBuffer::from_owned_result(py, velocity_cells)?,
                        velocity_cell_count,
                        vec![velocity_cell_count, 2],
                    ),
                ],
            ),
        )?;
        let pressure = Py::new(
            py,
            PyFieldOutput::new(
                pressure_field,
                mesh,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
                Vec::new(),
                "continuous-lagrange-p1",
                vec![FieldOutputBlock::new(
                    "vertex",
                    PyArrayBuffer::from_owned_result(py, pressure_vertices)?,
                    pressure_vertex_count,
                    vec![pressure_vertex_count],
                )],
            ),
        )?;
        let lookup = BTreeMap::from([(velocity_field_id, 0), (pressure_field_id, 1)]);
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::Fields(Box::new(CommonFieldResultPayload {
                outputs: vec![velocity, pressure],
                lookup,
                solve,
                evidence: Some(StaticScientificEvidence::SteadyStokes(evidence)),
                steady_stokes_observation: Some(observation),
            })),
        })
    }

    pub(crate) fn from_common_elasticity(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        displacement_field_id: String,
        elapsed: Duration,
        run: CartesianLinearElasticity2dSolution,
        observation: CommonElasticityObservation,
    ) -> PyResult<Self> {
        let evidence = Py::new(
            py,
            PyLinearElasticityEvidence::from_common(py, identity.plan_key(), &observation)?,
        )?;
        let values = run.displacement().values().to_vec();
        if !values.len().is_multiple_of(2) {
            return Err(PyRuntimeError::new_err(
                "elasticity displacement coefficients disagree with two-component FieldOutput metadata",
            ));
        }
        let vertex_count = values.len() / 2;
        let solve = Py::new(py, PyLinearSolveSummary::from_report(run.solve_report()))?;
        let field = Py::new(
            py,
            PyModelFieldRef::from_exact(
                identity.model_digest().to_owned(),
                displacement_field_id.clone(),
            ),
        )?;
        let displacement = Py::new(
            py,
            PyFieldOutput::new(
                field,
                mesh,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
                vec![2],
                "continuous-lagrange-p1",
                vec![FieldOutputBlock::new(
                    "vertex",
                    PyArrayBuffer::from_owned_result(py, values)?,
                    vertex_count,
                    vec![vertex_count, 2],
                )],
            ),
        )?;
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::Fields(Box::new(CommonFieldResultPayload {
                outputs: vec![displacement],
                lookup: BTreeMap::from([(displacement_field_id, 0)]),
                solve,
                evidence: Some(StaticScientificEvidence::LinearElasticity(evidence)),
                steady_stokes_observation: None,
            })),
        })
    }

    pub(crate) fn steady_stokes_evidence(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<PySteadyStokesEvidence>> {
        match &self.payload {
            ResultPayload::Fields(payload) => match &payload.evidence {
                Some(StaticScientificEvidence::SteadyStokes(evidence)) => {
                    Ok(evidence.clone_ref(py))
                }
                _ => Err(capability_error(
                    py,
                    "this Result occurrence has no steady-Stokes evidence",
                )),
            },
            ResultPayload::Trajectory(_) | ResultPayload::Ode(_) => Err(capability_error(
                py,
                "this transient Result has no steady-Stokes evidence",
            )),
        }
    }

    pub(crate) fn linear_elasticity_evidence(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<PyLinearElasticityEvidence>> {
        match &self.payload {
            ResultPayload::Fields(payload) => match &payload.evidence {
                Some(StaticScientificEvidence::LinearElasticity(evidence)) => {
                    Ok(evidence.clone_ref(py))
                }
                _ => Err(capability_error(
                    py,
                    "this Result occurrence has no linear-elasticity evidence",
                )),
            },
            ResultPayload::Trajectory(_) | ResultPayload::Ode(_) => Err(capability_error(
                py,
                "this Result occurrence has no linear-elasticity evidence",
            )),
        }
    }

    pub(crate) fn fsi_evidence(&self, py: Python<'_>) -> PyResult<Py<PyFsiEvidence>> {
        match &self.payload {
            ResultPayload::Trajectory(payload) => payload
                .fsi_evidence
                .as_ref()
                .map(|evidence| evidence.clone_ref(py))
                .ok_or_else(|| capability_error(py, "this transient Result has no FSI evidence")),
            ResultPayload::Fields(_) | ResultPayload::Ode(_) => Err(capability_error(
                py,
                "this Result occurrence has no FSI evidence",
            )),
        }
    }
}

pub(crate) fn materialize_common_scalar(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    result: ResolvedScalarEllipticCartesianSolution,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::Scalar(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common scalar output crossed a different Plan",
        ));
    };
    PyRunResult::from_common_scalar(
        py,
        identity,
        plan.mesh_handle(py),
        (native.field_id().to_owned(), native.field_dimension()),
        native.cells(),
        Duration::from_secs_f64(elapsed_seconds),
        result,
    )
}

pub(crate) fn materialize_common_steady_stokes(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    result: eqiora_numerics::CommonSteadyStokesRunOutput,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::SteadyStokes(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common steady-Stokes output crossed a different Plan",
        ));
    };
    if result.plan_identity() != native.identity() {
        return Err(PyRuntimeError::new_err(
            "common steady-Stokes output crossed a different exact Plan",
        ));
    }
    let (result, observation) = result.into_parts();
    PyRunResult::from_common_steady_stokes(
        py,
        identity,
        plan.mesh_handle(py),
        (
            native.velocity_field_id().to_owned(),
            native.pressure_field_id().to_owned(),
        ),
        Duration::from_secs_f64(elapsed_seconds),
        result,
        observation,
    )
}

pub(crate) fn materialize_common_elasticity(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    result: eqiora_numerics::CommonElasticityRunOutput,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::Elasticity(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common elasticity output crossed a different Plan",
        ));
    };
    if result.plan_identity() != native.identity() {
        return Err(PyRuntimeError::new_err(
            "common elasticity output crossed a different exact Plan",
        ));
    }
    let (result, observation) = result.into_parts();
    PyRunResult::from_common_elasticity(
        py,
        identity,
        plan.mesh_handle(py),
        native.displacement_field_id().to_owned(),
        Duration::from_secs_f64(elapsed_seconds),
        result,
        observation,
    )
}

pub(crate) fn materialize_common_transient(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    states: Vec<(usize, eqiora_numerics::CommonState)>,
) -> PyResult<PyRunResult> {
    if !matches!(
        plan.native(),
        CommonPlanKind::TransientFlow(_) | CommonPlanKind::Fsi(_)
    ) {
        return Err(PyRuntimeError::new_err(
            "common transient output crossed a different Plan",
        ));
    }
    let trajectory = Py::new(
        py,
        PyTrajectory::from_common(py, &plan, identity.plan_key(), states)?,
    )?;
    let fsi_evidence = if matches!(plan.native(), CommonPlanKind::Fsi(_)) {
        Some(Py::new(
            py,
            PyFsiEvidence::from_common(py, &plan, &trajectory.borrow(py), identity.plan_key())?,
        )?)
    } else {
        None
    };
    Ok(PyRunResult {
        identity,
        elapsed_seconds,
        payload: ResultPayload::Trajectory(CommonTrajectoryResultPayload {
            trajectory,
            fsi_evidence,
        }),
    })
}

pub(crate) fn materialize_common_ode(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    result: eqiora_numerics::CommonOdeRunResult,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::Ode(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common ODE output crossed a different Plan",
        ));
    };
    let states = result.states().to_vec();
    let times = states
        .iter()
        .map(eqiora_numerics::CommonOdeState::time_s)
        .collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut lookup = BTreeMap::new();
    for (column, (field, dimension)) in native
        .field_ids()
        .zip(native.field_dimensions())
        .enumerate()
    {
        let id = field.to_string();
        let values = states.iter().map(|state| state.values()[column]).collect();
        let series = Py::new(
            py,
            PySeries {
                field: Some(Py::new(
                    py,
                    PyModelFieldRef::from_exact(native.model_digest().to_owned(), id.clone()),
                )?),
                id: id.clone(),
                name: None,
                dimension: *dimension,
                time: PyArrayBuffer::from_owned_result(py, times.clone())?,
                values: PyArrayBuffer::from_owned_result(py, values)?,
            },
        )?;
        lookup.insert(id, fields.len());
        fields.push(series);
    }
    Ok(PyRunResult {
        identity,
        elapsed_seconds,
        payload: ResultPayload::Ode(CommonOdeResultPayload {
            fields,
            lookup,
            states,
        }),
    })
}

fn capability_error(py: Python<'_>, message: &str) -> PyErr {
    diagnostic_error(py, &[Diagnostic::error(codes::NOT_IMPLEMENTED, message)])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySeries>()?;
    module.add_class::<PyFieldOutput>()?;
    module.add_class::<PyRunResult>()?;
    Ok(())
}
