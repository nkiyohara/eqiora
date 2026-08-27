//! Common installed-Python ownership for accepted execution results.

use std::collections::BTreeMap;
use std::time::Duration;

use eqiora::api::ReferenceRunResult;
use eqiora::diagnostic::codes;
use eqiora::numerics::{
    CartesianLinearElasticity2dSolution, ResolvedScalarEllipticCartesianSolution,
    SteadyStokesMiniSolution2d,
};
use eqiora::{Diagnostic, DimExponents};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyTuple};

use crate::array::PyArrayBuffer;
use crate::common_plan::{CommonPlanKind, PyPlan};
use crate::diagnostic_error;
use crate::elasticity::PyLinearElasticityEvidence;
use crate::execution::RunIdentity;
use crate::fsi::PyFixedMeshMonolithicEvidence;
use crate::meshing::PyMesh;
use crate::model::PyModelFieldRef;
use crate::realization::{PyLinearSolveSummary, PyRunManifest};
use crate::steady_stokes::PySteadyStokesEvidence;
use crate::trajectory::{PyFieldSnapshot, PyState, PyTrajectory};

/// Immutable coefficients for one exact Model Field on one exact Mesh.
#[pyclass(
    name = "FieldOutput",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFieldOutput {
    field: Py<PyModelFieldRef>,
    mesh: Py<PyMesh>,
    dimension: DimExponents,
    components: usize,
    vertex_values: Py<PyArrayBuffer>,
    vertex_count: usize,
    cell_bubble_values: Option<Py<PyArrayBuffer>>,
    cell_bubble_count: usize,
}

#[pymethods]
impl PyFieldOutput {
    #[getter]
    fn field(&self, py: Python<'_>) -> Py<PyModelFieldRef> {
        self.field.clone_ref(py)
    }
    #[getter]
    fn mesh(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }
    #[getter]
    fn dimension(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        let value = self.dimension;
        (
            value.mass,
            value.length,
            value.time,
            value.current,
            value.temperature,
            value.amount,
            value.luminous_intensity,
        )
    }
    #[getter]
    const fn components(&self) -> usize {
        self.components
    }
    #[getter]
    const fn vertex_count(&self) -> usize {
        self.vertex_count
    }
    #[getter]
    fn vertex_values(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.vertex_values.clone_ref(py)
    }
    #[getter]
    const fn cell_bubble_count(&self) -> usize {
        self.cell_bubble_count
    }
    #[getter]
    fn cell_bubble_values(&self, py: Python<'_>) -> Option<Py<PyArrayBuffer>> {
        self.cell_bubble_values
            .as_ref()
            .map(|values| values.clone_ref(py))
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "FieldOutput(field={:?}, components={}, vertex_count={}, cell_bubble_count={})",
            self.field.borrow(py).exact_id(),
            self.components,
            self.vertex_count,
            self.cell_bubble_count,
        )
    }
}

/// One read-only, field-local sampled series in SI units.
#[pyclass(name = "Series", module = "eqiora._eqiora", frozen)]
pub(crate) struct PySeries {
    id: String,
    name: Option<String>,
    dimension: DimExponents,
    time: Py<PyArrayBuffer>,
    values: Py<PyArrayBuffer>,
}

#[pymethods]
impl PySeries {
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

pub(crate) struct StaticResultParts {
    pub(crate) identity: RunIdentity,
    pub(crate) elapsed_seconds: f64,
    pub(crate) field_id: String,
    pub(crate) snapshot: Py<PyFieldSnapshot>,
    pub(crate) mesh: Py<PyMesh>,
    pub(crate) run_manifest: Py<PyRunManifest>,
}

struct StaticFieldOutput {
    snapshot: Py<PyFieldSnapshot>,
    mesh: Py<PyMesh>,
}

enum StaticScientificEvidence {
    SteadyStokes(Py<PySteadyStokesEvidence>),
    LinearElasticity(Py<PyLinearElasticityEvidence>),
}

struct StaticResultPayload {
    outputs: Vec<StaticFieldOutput>,
    lookup: BTreeMap<String, usize>,
    run_manifest: Py<PyRunManifest>,
    evidence: StaticScientificEvidence,
}

struct TrajectoryResultPayload {
    trajectory: Py<PyTrajectory>,
    run_manifest: Py<PyRunManifest>,
    evidence: Py<PyFixedMeshMonolithicEvidence>,
}

struct ScalarResultPayload {
    field_id: String,
    mesh: Py<PyMesh>,
    values: Py<PyArrayBuffer>,
    field_location: &'static str,
    logical_shape: [usize; 2],
    solve: Py<PyLinearSolveSummary>,
}

struct CommonFieldResultPayload {
    outputs: Vec<Py<PyFieldOutput>>,
    lookup: BTreeMap<String, usize>,
    solve: Py<PyLinearSolveSummary>,
}

struct CommonTrajectoryResultPayload {
    trajectory: Py<PyTrajectory>,
}

enum ResultPayload {
    Series {
        fields: Vec<Py<PySeries>>,
        lookup: BTreeMap<String, usize>,
    },
    Static(StaticResultPayload),
    Trajectory(TrajectoryResultPayload),
    Scalar(ScalarResultPayload),
    CommonFields(CommonFieldResultPayload),
    CommonTrajectory(CommonTrajectoryResultPayload),
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

#[pymethods]
impl PyRunResult {
    pub(crate) fn common_state_at(
        &self,
        py: Python<'_>,
        state_space_identity: &str,
        time_s: f64,
    ) -> Option<Py<PyState>> {
        let ResultPayload::CommonTrajectory(payload) = &self.payload else {
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

    /// Independently sampled semantic-reference series in stable Field order.
    #[getter]
    fn fields(&self, py: Python<'_>) -> Vec<Py<PySeries>> {
        match &self.payload {
            ResultPayload::Series { fields, .. } => {
                fields.iter().map(|field| field.clone_ref(py)).collect()
            }
            ResultPayload::Static(_)
            | ResultPayload::Trajectory(_)
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_) => Vec::new(),
            ResultPayload::CommonTrajectory(_) => Vec::new(),
        }
    }

    /// Static spatial observations in exact accepted output order.
    #[getter]
    fn snapshots(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let snapshots = match &self.payload {
            ResultPayload::Series { .. } => Vec::new(),
            ResultPayload::Static(payload) => payload
                .outputs
                .iter()
                .map(|output| output.snapshot.clone_ref(py))
                .collect(),
            ResultPayload::Trajectory(_)
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_) => Vec::new(),
            ResultPayload::CommonTrajectory(_) => Vec::new(),
        };
        Ok(PyTuple::new(py, snapshots)?.unbind())
    }

    /// Exact durable spatial trajectory when this Result owns one.
    #[getter]
    fn trajectory(&self, py: Python<'_>) -> PyResult<Py<PyTrajectory>> {
        match &self.payload {
            ResultPayload::Trajectory(payload) => Ok(payload.trajectory.clone_ref(py)),
            ResultPayload::CommonTrajectory(payload) => Ok(payload.trajectory.clone_ref(py)),
            ResultPayload::Series { .. }
            | ResultPayload::Static(_)
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_) => Err(capability_error(
                py,
                "this Result occurrence has no spatial Trajectory",
            )),
        }
    }

    /// Select one exact static Field observation by Model-bound identity.
    #[pyo3(signature = (field, /))]
    fn field(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyFieldSnapshot>> {
        self.static_output(py, field)
            .map(|output| output.snapshot.clone_ref(py))
    }

    /// Select the exact accepted Mesh paired with one static Field.
    #[pyo3(signature = (field, /))]
    fn mesh(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyMesh>> {
        if let ResultPayload::Scalar(payload) = &self.payload {
            self.validate_scalar_field(field)?;
            return Ok(payload.mesh.clone_ref(py));
        }
        if let ResultPayload::CommonFields(_) = &self.payload {
            return self
                .common_output(py, field)
                .map(|output| output.borrow(py).mesh.clone_ref(py));
        }
        self.static_output(py, field)
            .map(|output| output.mesh.clone_ref(py))
    }

    /// Exact primary scalar coefficients for a common scalar Plan Result.
    #[getter]
    fn values(&self, py: Python<'_>) -> PyResult<Py<PyArrayBuffer>> {
        match &self.payload {
            ResultPayload::Scalar(payload) => Ok(payload.values.clone_ref(py)),
            _ => Err(capability_error(
                py,
                "this Result occurrence has no primary scalar coefficient buffer",
            )),
        }
    }

    #[getter]
    fn field_location(&self, py: Python<'_>) -> PyResult<&'static str> {
        match &self.payload {
            ResultPayload::Scalar(payload) => Ok(payload.field_location),
            _ => Err(capability_error(
                py,
                "this Result occurrence has no scalar Field location",
            )),
        }
    }

    #[getter]
    fn logical_shape(&self, py: Python<'_>) -> PyResult<(usize, usize)> {
        match &self.payload {
            ResultPayload::Scalar(payload) => {
                Ok((payload.logical_shape[0], payload.logical_shape[1]))
            }
            _ => Err(capability_error(
                py,
                "this Result occurrence has no scalar logical shape",
            )),
        }
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> PyResult<Py<PyLinearSolveSummary>> {
        match &self.payload {
            ResultPayload::Scalar(payload) => Ok(payload.solve.clone_ref(py)),
            ResultPayload::CommonFields(payload) => Ok(payload.solve.clone_ref(py)),
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

    /// Return the exact durable Run manifest when this occurrence owns one.
    fn run_manifest(&self, py: Python<'_>) -> PyResult<Py<PyRunManifest>> {
        match &self.payload {
            ResultPayload::Static(payload) => Ok(payload.run_manifest.clone_ref(py)),
            ResultPayload::Trajectory(payload) => Ok(payload.run_manifest.clone_ref(py)),
            ResultPayload::Series { .. }
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_)
            | ResultPayload::CommonTrajectory(_) => Err(capability_error(
                py,
                "this Result occurrence has no durable Run manifest",
            )),
        }
    }

    fn __len__(&self) -> usize {
        match &self.payload {
            ResultPayload::Series { fields, .. } => fields.len(),
            ResultPayload::Static(_)
            | ResultPayload::Trajectory(_)
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_)
            | ResultPayload::CommonTrajectory(_) => 0,
        }
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PySeries>> {
        let ResultPayload::Series { fields, lookup } = &self.payload else {
            return Err(PyKeyError::new_err(key.to_owned()));
        };
        let index = lookup
            .get(key)
            .copied()
            .ok_or_else(|| PyKeyError::new_err(key.to_owned()))?;
        Ok(fields[index].clone_ref(py))
    }

    fn __repr__(&self) -> String {
        format!(
            "Result(fields={}, snapshots={}, model_digest={:?}, plan_key={:?})",
            self.__len__(),
            match &self.payload {
                ResultPayload::Series { .. } => 0,
                ResultPayload::Static(payload) => payload.outputs.len(),
                ResultPayload::Trajectory(_)
                | ResultPayload::Scalar(_)
                | ResultPayload::CommonFields(_)
                | ResultPayload::CommonTrajectory(_) => 0,
            },
            self.model_digest(),
            self.plan_key(),
        )
    }
}

impl PyRunResult {
    fn validate_scalar_field(&self, field: &PyModelFieldRef) -> PyResult<()> {
        if field.exact_model_digest() != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let ResultPayload::Scalar(payload) = &self.payload else {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        };
        if field.exact_id() != payload.field_id {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        }
        Ok(())
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
        let ResultPayload::CommonFields(payload) = &self.payload else {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        };
        let index = payload
            .lookup
            .get(field.exact_id())
            .copied()
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(payload.outputs[index].clone_ref(py))
    }

    pub(crate) fn from_common_scalar(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        field_id: String,
        cells: [usize; 2],
        elapsed: Duration,
        run: ResolvedScalarEllipticCartesianSolution,
    ) -> PyResult<Self> {
        let (values, solve, field_location, logical_shape) = match run {
            ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => (
                solution.field().vertex_values().to_vec(),
                solution.solve_report().clone(),
                "vertex",
                [cells[0] + 1, cells[1] + 1],
            ),
            ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => (
                solution.cell_values().to_vec(),
                solution.solve_report().clone(),
                "cell-center",
                cells,
            ),
        };
        let values = PyArrayBuffer::from_owned_result(py, values)?;
        let solve = Py::new(py, PyLinearSolveSummary::from_report(&solve))?;
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::Scalar(ScalarResultPayload {
                field_id,
                mesh,
                values,
                field_location,
                logical_shape,
                solve,
            }),
        })
    }

    pub(crate) fn from_common_steady_stokes(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        velocity_field_id: String,
        pressure_field_id: String,
        elapsed: Duration,
        run: SteadyStokesMiniSolution2d,
    ) -> PyResult<Self> {
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
            PyFieldOutput {
                field: velocity_field,
                mesh: mesh.clone_ref(py),
                dimension: DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
                components: 2,
                vertex_values: PyArrayBuffer::from_owned_result(py, velocity_vertices)?,
                vertex_count: velocity_vertex_count,
                cell_bubble_values: Some(PyArrayBuffer::from_owned_result(py, velocity_cells)?),
                cell_bubble_count: velocity_cell_count,
            },
        )?;
        let pressure = Py::new(
            py,
            PyFieldOutput {
                field: pressure_field,
                mesh,
                dimension: DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
                components: 1,
                vertex_values: PyArrayBuffer::from_owned_result(py, pressure_vertices)?,
                vertex_count: pressure_vertex_count,
                cell_bubble_values: None,
                cell_bubble_count: 0,
            },
        )?;
        let lookup = BTreeMap::from([(velocity_field_id, 0), (pressure_field_id, 1)]);
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::CommonFields(CommonFieldResultPayload {
                outputs: vec![velocity, pressure],
                lookup,
                solve,
            }),
        })
    }

    pub(crate) fn from_common_elasticity(
        py: Python<'_>,
        identity: RunIdentity,
        mesh: Py<PyMesh>,
        displacement_field_id: String,
        elapsed: Duration,
        run: CartesianLinearElasticity2dSolution,
    ) -> PyResult<Self> {
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
            PyFieldOutput {
                field,
                mesh,
                dimension: DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
                components: 2,
                vertex_values: PyArrayBuffer::from_owned_result(py, values)?,
                vertex_count,
                cell_bubble_values: None,
                cell_bubble_count: 0,
            },
        )?;
        Ok(Self {
            identity,
            elapsed_seconds: elapsed.as_secs_f64(),
            payload: ResultPayload::CommonFields(CommonFieldResultPayload {
                outputs: vec![displacement],
                lookup: BTreeMap::from([(displacement_field_id, 0)]),
                solve,
            }),
        })
    }

    fn static_output(
        &self,
        py: Python<'_>,
        field: &PyModelFieldRef,
    ) -> PyResult<&StaticFieldOutput> {
        if matches!(
            &self.payload,
            ResultPayload::Trajectory(_)
                | ResultPayload::CommonFields(_)
                | ResultPayload::CommonTrajectory(_)
        ) {
            return Err(capability_error(
                py,
                "this Result occurrence owns a Trajectory, not a static Field output",
            ));
        }
        if field.exact_model_digest() != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let ResultPayload::Static(payload) = &self.payload else {
            return Err(PyKeyError::new_err(field.exact_id().to_owned()));
        };
        let index = payload
            .lookup
            .get(field.exact_id())
            .copied()
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(&payload.outputs[index])
    }

    pub(crate) fn steady_stokes_evidence(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<PySteadyStokesEvidence>> {
        match &self.payload {
            ResultPayload::Static(StaticResultPayload {
                evidence: StaticScientificEvidence::SteadyStokes(evidence),
                ..
            }) => Ok(evidence.clone_ref(py)),
            ResultPayload::Series { .. } => Err(capability_error(
                py,
                "this Result occurrence has no steady-Stokes evidence",
            )),
            ResultPayload::Static(_) => Err(capability_error(
                py,
                "this Result occurrence has no steady-Stokes evidence",
            )),
            ResultPayload::Trajectory(_) => Err(capability_error(
                py,
                "this Result occurrence has no steady-Stokes evidence",
            )),
            ResultPayload::Scalar(_) => Err(capability_error(
                py,
                "this Result occurrence has no steady-Stokes evidence",
            )),
            ResultPayload::CommonFields(_) => Err(capability_error(
                py,
                "this common Result does not fabricate a durable steady-Stokes evidence artifact",
            )),
            ResultPayload::CommonTrajectory(_) => Err(capability_error(
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
            ResultPayload::Static(StaticResultPayload {
                evidence: StaticScientificEvidence::LinearElasticity(evidence),
                ..
            }) => Ok(evidence.clone_ref(py)),
            ResultPayload::Static(_)
            | ResultPayload::Series { .. }
            | ResultPayload::Trajectory(_)
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_)
            | ResultPayload::CommonTrajectory(_) => Err(capability_error(
                py,
                "this Result occurrence has no linear-elasticity evidence",
            )),
        }
    }

    pub(crate) fn fixed_mesh_monolithic_evidence(
        &self,
        py: Python<'_>,
    ) -> PyResult<Py<PyFixedMeshMonolithicEvidence>> {
        match &self.payload {
            ResultPayload::Trajectory(payload) => Ok(payload.evidence.clone_ref(py)),
            ResultPayload::Static(_)
            | ResultPayload::Series { .. }
            | ResultPayload::Scalar(_)
            | ResultPayload::CommonFields(_)
            | ResultPayload::CommonTrajectory(_) => Err(capability_error(
                py,
                "this Result occurrence has no fixed-mesh monolithic FSI evidence",
            )),
        }
    }

    pub(crate) fn from_static_steady_stokes(
        parts: StaticResultParts,
        evidence: Py<PySteadyStokesEvidence>,
    ) -> Self {
        let mut lookup = BTreeMap::new();
        lookup.insert(parts.field_id, 0);
        Self {
            identity: parts.identity,
            elapsed_seconds: parts.elapsed_seconds,
            payload: ResultPayload::Static(StaticResultPayload {
                outputs: vec![StaticFieldOutput {
                    snapshot: parts.snapshot,
                    mesh: parts.mesh,
                }],
                lookup,
                run_manifest: parts.run_manifest,
                evidence: StaticScientificEvidence::SteadyStokes(evidence),
            }),
        }
    }

    pub(crate) fn from_static_linear_elasticity(
        parts: StaticResultParts,
        evidence: Py<PyLinearElasticityEvidence>,
    ) -> Self {
        let mut lookup = BTreeMap::new();
        lookup.insert(parts.field_id, 0);
        Self {
            identity: parts.identity,
            elapsed_seconds: parts.elapsed_seconds,
            payload: ResultPayload::Static(StaticResultPayload {
                outputs: vec![StaticFieldOutput {
                    snapshot: parts.snapshot,
                    mesh: parts.mesh,
                }],
                lookup,
                run_manifest: parts.run_manifest,
                evidence: StaticScientificEvidence::LinearElasticity(evidence),
            }),
        }
    }

    pub(crate) fn from_fixed_mesh_monolithic_fsi(
        identity: RunIdentity,
        elapsed_seconds: f64,
        trajectory: Py<PyTrajectory>,
        run_manifest: Py<PyRunManifest>,
        evidence: Py<PyFixedMeshMonolithicEvidence>,
    ) -> Self {
        Self {
            identity,
            elapsed_seconds,
            payload: ResultPayload::Trajectory(TrajectoryResultPayload {
                trajectory,
                run_manifest,
                evidence,
            }),
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
        native.field_id().to_owned(),
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
    result: SteadyStokesMiniSolution2d,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::SteadyStokes(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common steady-Stokes output crossed a different Plan",
        ));
    };
    PyRunResult::from_common_steady_stokes(
        py,
        identity,
        plan.mesh_handle(py),
        native.velocity_field_id().to_owned(),
        native.pressure_field_id().to_owned(),
        Duration::from_secs_f64(elapsed_seconds),
        result,
    )
}

pub(crate) fn materialize_common_elasticity(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    result: CartesianLinearElasticity2dSolution,
) -> PyResult<PyRunResult> {
    let CommonPlanKind::Elasticity(native) = plan.native() else {
        return Err(PyRuntimeError::new_err(
            "common elasticity output crossed a different Plan",
        ));
    };
    PyRunResult::from_common_elasticity(
        py,
        identity,
        plan.mesh_handle(py),
        native.displacement_field_id().to_owned(),
        Duration::from_secs_f64(elapsed_seconds),
        result,
    )
}

pub(crate) fn materialize_common_transient(
    py: Python<'_>,
    plan: PyRef<'_, PyPlan>,
    identity: RunIdentity,
    elapsed_seconds: f64,
    states: Vec<(usize, eqiora_numerics::CommonState)>,
) -> PyResult<PyRunResult> {
    if !matches!(plan.native(), CommonPlanKind::TransientFlow(_)) {
        return Err(PyRuntimeError::new_err(
            "common transient output crossed a different Plan",
        ));
    }
    let trajectory = Py::new(
        py,
        PyTrajectory::from_common(py, &plan, identity.plan_key(), states)?,
    )?;
    Ok(PyRunResult {
        identity,
        elapsed_seconds,
        payload: ResultPayload::CommonTrajectory(CommonTrajectoryResultPayload { trajectory }),
    })
}

pub(crate) fn result_into_python(
    py: Python<'_>,
    result: ReferenceRunResult,
    identity: RunIdentity,
) -> PyResult<PyRunResult> {
    let elapsed_seconds = result.evidence().elapsed().as_secs_f64();
    let series = result.into_series();
    let mut fields = Vec::with_capacity(series.len());
    let mut lookup = BTreeMap::new();
    for owned in series {
        let id = owned.field().ulid().to_string();
        let name = owned.name().map(str::to_owned);
        let dimension = owned.dimension();
        let (time_values, field_values) = owned.into_buffers();
        let time = PyArrayBuffer::from_owned_result(py, time_values)?;
        let values = PyArrayBuffer::from_owned_result(py, field_values)?;
        let index = fields.len();
        let field = Py::new(
            py,
            PySeries {
                id: id.clone(),
                name: name.clone(),
                dimension,
                time,
                values,
            },
        )?;
        lookup.insert(id, index);
        if let Some(name) = name {
            lookup.insert(name, index);
        }
        fields.push(field);
    }
    Ok(PyRunResult {
        identity,
        elapsed_seconds,
        payload: ResultPayload::Series { fields, lookup },
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
