//! Immutable Python projection of one accepted spatial trajectory.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eqiora::DimExponents;
use eqiora::api::{FixedMeshFieldTrajectoryReplay2dV1, UnstructuredP1ScalarFieldProjection2d};
use eqiora::artifact::{
    ArtifactDigest, CanonicalModelArtifact, CartesianQ1FieldSnapshotEnvelopeV1,
    FieldSnapshotEnvelopeV1, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora::kernel::ValueFrame;
use eqiora::meshing::{DiscreteFieldAssociation, DiscreteFieldShape};
use eqiora_numerics::CommonState;
use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::{PyIndexError, PyKeyError, PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule, PyTuple};
use sha2::{Digest, Sha256};

use crate::diagnostic_error;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::meshing::PyMesh;
use crate::model::{PyModel, PyModelFieldRef};
mod presentation;

use presentation::TrajectoryPresentation;

enum ProjectedValues {
    Scalar(ReadOnlyVector<f64>),
    Vector(ReadOnlyMatrix<f64>),
}

impl ProjectedValues {
    fn numpy(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Scalar(values) => Ok(values.numpy(py)?.into_any()),
            Self::Vector(values) => Ok(values.numpy(py)?.into_any()),
        }
    }
}

struct ProjectedBlock {
    association: &'static str,
    digest: String,
    values: ProjectedValues,
    support_indices: Arc<ReadOnlyVector<u32>>,
}

/// One exact semantic Field observation in an accepted trajectory state.
#[pyclass(
    name = "FieldSnapshot",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyFieldSnapshot {
    digest: String,
    mesh_digest: String,
    field: Py<PyModelFieldRef>,
    field_id: String,
    support_domain_id: String,
    dimension: DimExponents,
    value_shape: Vec<u32>,
    frame: &'static str,
    blocks: Vec<ProjectedBlock>,
}

impl PartialEq for PyFieldSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl Eq for PyFieldSnapshot {}

impl Hash for PyFieldSnapshot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.hash(state);
    }
}

#[pymethods]
impl PyFieldSnapshot {
    /// Exact accepted Field snapshot identity.
    #[getter]
    fn digest(&self) -> &str {
        &self.digest
    }

    /// Exact Mesh artifact on which this observation is defined.
    #[getter]
    fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    /// Exact Model-bound semantic Field identity.
    #[getter]
    fn field(&self, py: Python<'_>) -> Py<PyModelFieldRef> {
        self.field.clone_ref(py)
    }

    /// Exact volume Domain supporting this Field.
    #[getter]
    fn support_domain_id(&self) -> &str {
        &self.support_domain_id
    }

    /// Coherent-SI base exponents in M,L,T,I,Theta,N,J order.
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

    /// Exact mathematical component shape; an empty tuple is scalar.
    #[getter]
    fn value_shape(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.value_shape.iter().copied())?.unbind())
    }

    /// Coordinate-frame meaning of the mathematical components.
    #[getter]
    fn frame(&self) -> &'static str {
        self.frame
    }

    /// Coefficient associations in exact snapshot-edge order.
    #[getter]
    fn associations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.blocks.iter().map(|block| block.association))?.unbind())
    }

    /// Exact block identities paired with their coefficient associations.
    #[getter]
    fn block_digests(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let entries = self
            .blocks
            .iter()
            .map(|block| (block.association, block.digest.as_str()));
        Ok(PyTuple::new(py, entries)?.unbind())
    }

    /// Read-only NumPy coefficients for one exact association.
    #[pyo3(signature = (association, /))]
    fn values(&self, py: Python<'_>, association: &str) -> PyResult<Py<PyAny>> {
        self.blocks
            .iter()
            .find(|block| block.association == association)
            .ok_or_else(|| PyKeyError::new_err(association.to_owned()))?
            .values
            .numpy(py)
    }

    /// Read-only exact global mesh-entity indices in the Field support.
    #[pyo3(signature = (association, /))]
    fn support_indices(&self, py: Python<'_>, association: &str) -> PyResult<Py<PyArray1<u32>>> {
        self.blocks
            .iter()
            .find(|block| block.association == association)
            .ok_or_else(|| PyKeyError::new_err(association.to_owned()))?
            .support_indices
            .numpy(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "FieldSnapshot(field_id={:?}, digest={:?})",
            self.field_id, self.digest,
        )
    }
}

/// One accepted physical state in exact trajectory order.
#[pyclass(
    name = "State",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyState {
    digest: String,
    model_digest: String,
    step: u64,
    time_s: f64,
    fields: Vec<Py<PyFieldSnapshot>>,
    field_lookup: BTreeMap<String, usize>,
    model: Option<Py<PyModel>>,
    mesh: Option<Py<PyMesh>>,
    native: Option<CommonState>,
    plan_identity: Option<String>,
    source_request_identity: Option<String>,
    source_trajectory_identity: Option<String>,
    source_kind: Option<&'static str>,
}

impl PartialEq for PyState {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl Eq for PyState {}

impl Hash for PyState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.hash(state);
    }
}

impl PyState {
    pub(crate) fn digest_value(&self) -> &str {
        &self.digest
    }

    pub(crate) fn model_digest_value(&self) -> &str {
        &self.model_digest
    }

    pub(crate) fn from_common(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        native: CommonState,
        step: u64,
        source_request_identity: Option<&str>,
        source_trajectory_identity: Option<&str>,
    ) -> PyResult<Self> {
        let native_plan = plan
            .transient_native()
            .expect("common State requires a transient Plan");
        let mesh = plan.mesh_handle(py);
        let mesh_digest = mesh.borrow(py).exact_mesh_digest().to_owned();
        let velocity =
            PyFieldSnapshot::from_common_velocity(py, native_plan, &native, &mesh_digest)?;
        let pressure =
            PyFieldSnapshot::from_common_pressure(py, native_plan, &native, &mesh_digest)?;
        let mut field_lookup = BTreeMap::new();
        field_lookup.insert(native_plan.velocity_field_id().to_owned(), 0);
        field_lookup.insert(native_plan.pressure_field_id().to_owned(), 1);
        Ok(Self {
            digest: native.identity().to_owned(),
            model_digest: native_plan.model_digest().to_owned(),
            step,
            time_s: native.time_s(),
            fields: vec![Py::new(py, velocity)?, Py::new(py, pressure)?],
            field_lookup,
            model: Some(plan.model_handle(py)),
            mesh: Some(mesh),
            native: Some(native),
            plan_identity: Some(native_plan.identity().to_owned()),
            source_request_identity: source_request_identity.map(str::to_owned),
            source_trajectory_identity: source_trajectory_identity.map(str::to_owned),
            source_kind: Some(if source_request_identity.is_some() {
                "result"
            } else {
                "zero"
            }),
        })
    }

    pub(crate) fn common_native(&self) -> Option<&CommonState> {
        self.native.as_ref()
    }

    pub(crate) const fn time_s_value(&self) -> f64 {
        self.time_s
    }
}

#[pymethods]
impl PyState {
    #[staticmethod]
    #[pyo3(signature = (plan, /, *, time_s=0.0))]
    fn zero(py: Python<'_>, plan: &crate::common_plan::PyPlan, time_s: f64) -> PyResult<Self> {
        let native_plan = plan.transient_native().ok_or_else(|| {
            PyValueError::new_err("State.zero requires an admitted transient Plan")
        })?;
        let state = native_plan
            .zero_state(time_s)
            .map_err(|diagnostic| crate::error::validation_error(py, &[diagnostic]))?;
        Self::from_common(py, plan, state, 0, None, None)
    }

    #[staticmethod]
    #[pyo3(signature = (plan, result, /, *, time_s))]
    fn from_result(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        result: &crate::result::PyRunResult,
        time_s: f64,
    ) -> PyResult<Py<PyState>> {
        if !time_s.is_finite() || time_s < 0.0 {
            return Err(PyValueError::new_err(
                "State.from_result time_s must be finite and non-negative",
            ));
        }
        let native = plan
            .transient_native()
            .ok_or_else(|| PyValueError::new_err("State.from_result requires a transient Plan"))?;
        let state_space_identity = native.state_space_identity();
        result
            .common_state_at(py, &state_space_identity, time_s)
            .ok_or_else(|| {
                PyValueError::new_err(
                    "Result contains no State at time_s compatible with the exact Plan state space",
                )
            })
    }

    #[getter]
    fn digest(&self) -> &str {
        &self.digest
    }

    #[getter]
    const fn step(&self) -> u64 {
        self.step
    }

    #[getter]
    const fn time_s(&self) -> f64 {
        self.time_s
    }

    #[getter]
    fn state_space_identity(&self) -> &str {
        self.native
            .as_ref()
            .map_or(self.digest.as_str(), CommonState::state_space_identity)
    }

    #[getter]
    fn mesh(&self, py: Python<'_>) -> Option<Py<PyMesh>> {
        self.mesh.as_ref().map(|mesh| mesh.clone_ref(py))
    }

    #[getter]
    fn model(&self, py: Python<'_>) -> Option<Py<PyModel>> {
        self.model.as_ref().map(|model| model.clone_ref(py))
    }

    #[getter]
    fn source_plan_identity(&self) -> Option<&str> {
        self.plan_identity.as_deref()
    }

    #[getter]
    fn source_request_identity(&self) -> Option<&str> {
        self.source_request_identity.as_deref()
    }

    #[getter]
    fn source_trajectory_identity(&self) -> Option<&str> {
        self.source_trajectory_identity.as_deref()
    }

    #[getter]
    const fn source_kind(&self) -> Option<&'static str> {
        self.source_kind
    }

    #[getter]
    fn fields(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.fields.iter().map(|field| field.clone_ref(py)))?.unbind())
    }

    /// Select one complete Field observation by exact Model-bound identity.
    #[pyo3(signature = (field, /))]
    fn field(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyFieldSnapshot>> {
        if field.exact_model_digest() != self.model_digest {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let index = self
            .field_lookup
            .get(field.exact_id())
            .copied()
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(self.fields[index].clone_ref(py))
    }

    fn __repr__(&self) -> String {
        format!(
            "State(step={}, time_s={}, digest={:?})",
            self.step, self.time_s, self.digest,
        )
    }
}

/// Immutable installed-Python projection of one accepted trajectory.
///
/// Common transient execution and accepted fixed-mesh replay both retain their
/// exact owning lineage without fabricating a Realization artifact.
#[pyclass(
    name = "Trajectory",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyTrajectory {
    model_digest: String,
    geometry_digest: String,
    correspondence_digest: String,
    mesh_digest: String,
    realization_digest: Option<String>,
    plan_identity: Option<String>,
    run_digest: Option<String>,
    request_identity: Option<String>,
    trajectory_digest: String,
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
    states: Vec<Py<PyState>>,
    state_lookup: BTreeMap<u64, usize>,
    common_mesh: Option<Py<PyMesh>>,
    presentation: TrajectoryPresentation,
}

impl PartialEq for PyTrajectory {
    fn eq(&self, other: &Self) -> bool {
        self.trajectory_digest == other.trajectory_digest
    }
}

impl Eq for PyTrajectory {}

impl Hash for PyTrajectory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.trajectory_digest.hash(state);
    }
}

#[pymethods]
impl PyTrajectory {
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
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
    fn realization_digest(&self) -> Option<&str> {
        self.realization_digest.as_deref()
    }

    #[getter]
    fn plan_identity(&self) -> Option<&str> {
        self.plan_identity.as_deref()
    }

    #[getter]
    fn run_digest(&self) -> Option<&str> {
        self.run_digest.as_deref()
    }

    #[getter]
    fn request_identity(&self) -> Option<&str> {
        self.request_identity.as_deref()
    }

    #[getter]
    fn digest(&self) -> &str {
        &self.trajectory_digest
    }

    #[getter]
    const fn dimension(&self) -> usize {
        2
    }

    #[getter]
    fn coordinates(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        match &self.common_mesh {
            Some(mesh) => mesh.borrow(py).coordinate_array(py),
            None => self.coordinates.numpy(py),
        }
    }

    #[getter]
    fn cells(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        match &self.common_mesh {
            Some(mesh) => mesh.borrow(py).cell_array(py),
            None => self.cells.numpy(py),
        }
    }

    #[getter]
    fn states(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.states.iter().map(|state| state.clone_ref(py)))?.unbind())
    }

    /// Select one accepted state by its exact step ordinal.
    #[pyo3(signature = (step, /))]
    fn state(&self, py: Python<'_>, step: u64) -> PyResult<Py<PyState>> {
        let index = self.state_lookup.get(&step).copied().ok_or_else(|| {
            PyIndexError::new_err(format!("trajectory has no accepted step {step}"))
        })?;
        Ok(self.states[index].clone_ref(py))
    }

    fn __repr__(&self) -> String {
        format!(
            "Trajectory(digest={:?}, states={})",
            self.digest(),
            self.states.len()
        )
    }

    #[pyo3(signature = (include=None, exclude=None))]
    fn _repr_mimebundle_(
        slf: Py<Self>,
        py: Python<'_>,
        include: Option<&Bound<'_, PyAny>>,
        exclude: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        presentation::mimebundle(slf, py, include, exclude)
    }
}

impl PyTrajectory {
    pub(crate) fn digest_value(&self) -> &str {
        &self.trajectory_digest
    }

    pub(crate) fn model_digest_value(&self) -> &str {
        &self.model_digest
    }

    pub(crate) fn state_handles(&self, py: Python<'_>) -> Vec<Py<PyState>> {
        self.states
            .iter()
            .map(|state| state.clone_ref(py))
            .collect()
    }

    pub(crate) fn from_common(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        run_identity: &str,
        states: Vec<(usize, CommonState)>,
    ) -> PyResult<Self> {
        let native = plan.transient_native().ok_or_else(|| {
            PyRuntimeError::new_err("common Trajectory requires a transient Plan")
        })?;
        let mesh = plan.mesh_handle(py);
        let mesh_ref = mesh.borrow(py);
        let trajectory_digest = {
            let mut hasher = Sha256::new();
            hasher.update(b"eqiora.common-trajectory/v1\0");
            hasher.update(run_identity.as_bytes());
            for (_, state) in &states {
                hasher.update(state.identity().as_bytes());
            }
            hex_sha256(hasher.finalize().as_slice())
        };
        let mut state_lookup = BTreeMap::new();
        let mut projected = Vec::with_capacity(states.len());
        for (step, state) in states {
            let step = u64::try_from(step)
                .map_err(|_| PyOverflowError::new_err("accepted step exceeds Python u64"))?;
            if state_lookup.insert(step, projected.len()).is_some() {
                return Err(PyRuntimeError::new_err(
                    "common Trajectory contains a duplicate output step",
                ));
            }
            projected.push(Py::new(
                py,
                PyState::from_common(
                    py,
                    plan,
                    state,
                    step,
                    Some(run_identity),
                    Some(&trajectory_digest),
                )?,
            )?);
        }
        let geometry_digest = mesh_ref.source_digest_value().to_owned();
        let correspondence_digest = mesh_ref.correspondence_digest_value().to_owned();
        let mesh_digest = mesh_ref.exact_mesh_digest().to_owned();
        drop(mesh_ref);
        Ok(Self {
            model_digest: native.model_digest().to_owned(),
            geometry_digest,
            correspondence_digest,
            mesh_digest,
            realization_digest: None,
            plan_identity: Some(native.identity().to_owned()),
            run_digest: None,
            request_identity: Some(run_identity.to_owned()),
            trajectory_digest,
            coordinates: ReadOnlyMatrix::new(0, 2, Vec::new()),
            cells: ReadOnlyMatrix::new(0, 0, Vec::new()),
            states: projected,
            state_lookup,
            common_mesh: Some(mesh),
            presentation: TrajectoryPresentation::default(),
        })
    }

    pub(crate) fn from_replay(
        py: Python<'_>,
        model: &PyModel,
        mesh: &SimplicialMeshEnvelopeV1,
        replay: &FixedMeshFieldTrajectoryReplay2dV1<'_>,
        run: &RunManifestV2,
    ) -> PyResult<Self> {
        let mut accepted_states = replay.states();
        let first = accepted_states.next().ok_or_else(|| {
            PyRuntimeError::new_err("accepted fixed-mesh trajectory unexpectedly has no states")
        })?;
        let model_digest = first.model_artifact().to_string();
        let current_model_digest = model
            .artifact()
            .artifact_reference()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?
            .artifact()
            .to_string();
        if model_digest != current_model_digest {
            return Err(PyRuntimeError::new_err(
                "accepted trajectory Model differs from the supplied Python Model",
            ));
        }
        let mesh_digest = first.mesh_artifact().to_string();
        if mesh_digest != artifact_digest(py, mesh.digest())? {
            return Err(PyRuntimeError::new_err(
                "accepted trajectory mesh differs from the supplied mesh artifact",
            ));
        }
        let trajectory_artifact = replay
            .trajectory()
            .digest()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
        if run.model() != first.model_artifact()
            || run.realization() != first.realization_artifact()
            || run.outputs() != vec![trajectory_artifact.clone()]
        {
            return Err(PyRuntimeError::new_err(
                "accepted trajectory Run differs from the supplied Run lineage",
            ));
        }
        let trajectory_digest = trajectory_artifact.to_string();

        let mesh_value = mesh.mesh();
        let vertex_count = mesh_value.vertices().len();
        let mut coordinates = Vec::with_capacity(vertex_count * 2);
        for coordinate in mesh_value.vertices() {
            if coordinate.len() != 2 {
                return Err(PyRuntimeError::new_err(
                    "accepted 2D trajectory contains a non-2D coordinate",
                ));
            }
            coordinates.extend(coordinate);
        }
        let cell_count = mesh_value.cells().len();
        let mut cells = Vec::with_capacity(cell_count * 3);
        for cell in mesh_value.cells() {
            if cell.len() != 3 {
                return Err(PyRuntimeError::new_err(
                    "accepted affine-triangle trajectory contains a non-triangle cell",
                ));
            }
            for &vertex in cell {
                cells.push(u32::try_from(vertex).map_err(|_| {
                    PyOverflowError::new_err("trajectory vertex index exceeds Python uint32")
                })?);
            }
        }

        let all_states = std::iter::once(first).chain(accepted_states);
        let mut field_refs = BTreeMap::<String, Py<PyModelFieldRef>>::new();
        let mut support_arrays =
            BTreeMap::<(String, &'static str), Arc<ReadOnlyVector<u32>>>::new();
        let mut states = Vec::with_capacity(all_states.size_hint().0);
        let mut state_lookup = BTreeMap::new();
        for (state_index, state) in all_states.enumerate() {
            let snapshots = replay.fields(state_index).ok_or_else(|| {
                PyRuntimeError::new_err("accepted replay omitted one state Field inventory")
            })?;
            let mut fields = Vec::with_capacity(snapshots.len());
            let mut field_lookup = BTreeMap::new();
            for (field_index, snapshot) in snapshots.enumerate() {
                let field_id = snapshot.field().ulid().to_string();
                let field_ref = match field_refs.get(&field_id) {
                    Some(field) => field.clone_ref(py),
                    None => {
                        let field = Py::new(py, model.field_ref_from_id(py, snapshot.field())?)?;
                        field_refs.insert(field_id.clone(), field.clone_ref(py));
                        field
                    }
                };
                let exact_blocks = replay.blocks(state_index, field_index).ok_or_else(|| {
                    PyRuntimeError::new_err("accepted replay omitted one Field block inventory")
                })?;
                let mut blocks = Vec::with_capacity(exact_blocks.len());
                for block in exact_blocks {
                    let association = block.association();
                    let association_name = association_name(association);
                    let support_key = (
                        snapshot.support_domain().ulid().to_string(),
                        association_name,
                    );
                    let support_indices = match support_arrays.get(&support_key) {
                        Some(indices) => Arc::clone(indices),
                        None => {
                            let native = replay
                                .support_indices(state_index, field_index, association)
                                .ok_or_else(|| {
                                    PyRuntimeError::new_err(
                                        "accepted replay omitted one Field support membership",
                                    )
                                })?
                                .iter()
                                .map(|&index| {
                                    u32::try_from(index).map_err(|_| {
                                        PyOverflowError::new_err(
                                            "Field support index exceeds Python uint32",
                                        )
                                    })
                                })
                                .collect::<PyResult<Vec<_>>>()?;
                            let indices = Arc::new(ReadOnlyVector::new(native));
                            support_arrays.insert(support_key, Arc::clone(&indices));
                            indices
                        }
                    };
                    let entity_count = block
                        .entity_count()
                        .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?;
                    let shape = block.component_shape();
                    let values = match shape {
                        DiscreteFieldShape::Scalar => {
                            ProjectedValues::Scalar(ReadOnlyVector::new(block.values().to_vec()))
                        }
                        DiscreteFieldShape::Vector { components } => {
                            let columns = usize::try_from(components.get()).map_err(|_| {
                                PyOverflowError::new_err(
                                    "trajectory Field component count exceeds local usize",
                                )
                            })?;
                            ProjectedValues::Vector(ReadOnlyMatrix::new(
                                entity_count,
                                columns,
                                block.values().to_vec(),
                            ))
                        }
                    };
                    blocks.push(ProjectedBlock {
                        association: association_name,
                        digest: artifact_digest(py, block.digest())?,
                        values,
                        support_indices,
                    });
                }
                let value_shape = snapshot
                    .value_shape()
                    .extents()
                    .iter()
                    .map(|extent| extent.get())
                    .collect();
                let projected = Py::new(
                    py,
                    PyFieldSnapshot {
                        digest: artifact_digest(py, snapshot.digest())?,
                        mesh_digest: mesh_digest.clone(),
                        field: field_ref,
                        field_id: field_id.clone(),
                        support_domain_id: snapshot.support_domain().ulid().to_string(),
                        dimension: snapshot.dimension(),
                        value_shape,
                        frame: frame_name(snapshot.frame()),
                        blocks,
                    },
                )?;
                if field_lookup.insert(field_id, fields.len()).is_some() {
                    return Err(PyRuntimeError::new_err(
                        "accepted trajectory state contains a duplicate Field identity",
                    ));
                }
                fields.push(projected);
            }
            let digest = artifact_digest(py, state.digest())?;
            if state_lookup.insert(state.step(), states.len()).is_some() {
                return Err(PyRuntimeError::new_err(
                    "accepted trajectory contains a duplicate step ordinal",
                ));
            }
            states.push(Py::new(
                py,
                PyState {
                    digest,
                    model_digest: model_digest.clone(),
                    step: state.step(),
                    time_s: state.time_s(),
                    fields,
                    field_lookup,
                    model: None,
                    mesh: None,
                    native: None,
                    plan_identity: None,
                    source_request_identity: None,
                    source_trajectory_identity: None,
                    source_kind: None,
                },
            )?);
        }

        Ok(Self {
            model_digest,
            geometry_digest: first.geometry_artifact().to_string(),
            correspondence_digest: first.correspondence_artifact().to_string(),
            mesh_digest,
            realization_digest: Some(first.realization_artifact().to_string()),
            plan_identity: None,
            run_digest: Some(artifact_digest(py, run.digest())?),
            request_identity: None,
            trajectory_digest,
            coordinates: ReadOnlyMatrix::new(vertex_count, 2, coordinates),
            cells: ReadOnlyMatrix::new(cell_count, 3, cells),
            states,
            state_lookup,
            common_mesh: None,
            presentation: TrajectoryPresentation::default(),
        })
    }
}

impl PyFieldSnapshot {
    fn from_common_velocity(
        py: Python<'_>,
        plan: &eqiora_numerics::CommonTransientFlowPlan,
        state: &CommonState,
        mesh_digest: &str,
    ) -> PyResult<Self> {
        const VELOCITY: DimExponents = DimExponents {
            length: 1,
            time: -1,
            ..DimExponents::DIMENSIONLESS
        };
        let mut blocks = Vec::new();
        if let Some(values) = state.velocity_vertex_values() {
            blocks.push(common_vector_block("vertex", values)?);
        }
        blocks.push(common_vector_block("cell", state.velocity_cell_values())?);
        Self::from_common_parts(
            py,
            plan,
            mesh_digest,
            plan.velocity_field_id(),
            VELOCITY,
            vec![2],
            "spatial-cartesian",
            blocks,
        )
    }

    fn from_common_pressure(
        py: Python<'_>,
        plan: &eqiora_numerics::CommonTransientFlowPlan,
        state: &CommonState,
        mesh_digest: &str,
    ) -> PyResult<Self> {
        const PRESSURE: DimExponents = DimExponents {
            mass: 1,
            length: -1,
            time: -2,
            ..DimExponents::DIMENSIONLESS
        };
        let block = match (state.pressure_vertex_values(), state.pressure_cell_values()) {
            (Some(values), None) => common_scalar_block("vertex", values)?,
            (None, Some(values)) => common_scalar_block("cell", values)?,
            _ => {
                return Err(PyRuntimeError::new_err(
                    "common pressure State lost its exact coefficient association",
                ));
            }
        };
        Self::from_common_parts(
            py,
            plan,
            mesh_digest,
            plan.pressure_field_id(),
            PRESSURE,
            Vec::new(),
            "invariant",
            vec![block],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_common_parts(
        py: Python<'_>,
        plan: &eqiora_numerics::CommonTransientFlowPlan,
        mesh_digest: &str,
        field_id: &str,
        dimension: DimExponents,
        value_shape: Vec<u32>,
        frame: &'static str,
        blocks: Vec<ProjectedBlock>,
    ) -> PyResult<Self> {
        let mut hasher = Sha256::new();
        hasher.update(b"eqiora.common-field-snapshot/v1\0");
        hasher.update(plan.model_digest().as_bytes());
        hasher.update(mesh_digest.as_bytes());
        hasher.update(field_id.as_bytes());
        for block in &blocks {
            hasher.update(block.digest.as_bytes());
        }
        let digest = hex_sha256(hasher.finalize().as_slice());
        let field = Py::new(
            py,
            PyModelFieldRef::from_exact(plan.model_digest().to_owned(), field_id.to_owned()),
        )?;
        Ok(Self {
            digest,
            mesh_digest: mesh_digest.to_owned(),
            field,
            field_id: field_id.to_owned(),
            support_domain_id: plan.domain_id(),
            dimension,
            value_shape,
            frame,
            blocks,
        })
    }

    /// Project one already validated authored static scalar observation.
    pub(crate) fn from_authored_scalar(
        py: Python<'_>,
        snapshot: &FieldSnapshotEnvelopeV1,
        projection: &UnstructuredP1ScalarFieldProjection2d,
    ) -> PyResult<(String, Self)> {
        let field_id = projection.field().ulid().to_string();
        if snapshot.field() != projection.field()
            || snapshot.support_domain() != projection.support_domain()
            || snapshot.dimension() != projection.value_dimension()
            || !snapshot.value_shape().is_scalar()
            || snapshot.frame() != ValueFrame::Invariant
        {
            return Err(PyRuntimeError::new_err(
                "accepted static scalar projection differs from its Field snapshot meaning",
            ));
        }
        let blocks = snapshot.block_artifacts();
        let [(association, block_digest)] = blocks.as_slice() else {
            return Err(PyRuntimeError::new_err(
                "accepted static scalar snapshot must contain one coefficient block",
            ));
        };
        if *association != DiscreteFieldAssociation::Vertex {
            return Err(PyRuntimeError::new_err(
                "accepted static scalar snapshot must use vertex coefficients",
            ));
        }

        let support_indices = (0..projection.vertices_m().len())
            .map(|index| {
                u32::try_from(index).map_err(|_| {
                    PyOverflowError::new_err("Field support index exceeds Python uint32")
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let field = Py::new(
            py,
            PyModelFieldRef::from_exact(projection.model_artifact().to_string(), field_id.clone()),
        )?;
        Ok((
            field_id.clone(),
            Self {
                digest: artifact_digest(py, snapshot.digest())?,
                mesh_digest: projection.mesh_artifact().to_string(),
                field,
                field_id,
                support_domain_id: projection.support_domain().ulid().to_string(),
                dimension: projection.value_dimension(),
                value_shape: Vec::new(),
                frame: "invariant",
                blocks: vec![ProjectedBlock {
                    association: "vertex",
                    digest: block_digest.to_string(),
                    values: ProjectedValues::Scalar(ReadOnlyVector::new(
                        projection.values().to_vec(),
                    )),
                    support_indices: Arc::new(ReadOnlyVector::new(support_indices)),
                }],
            },
        ))
    }

    /// Project one validated generated-Cartesian continuous-Q1 vector observation.
    pub(crate) fn from_cartesian_q1_vector(
        py: Python<'_>,
        snapshot: &CartesianQ1FieldSnapshotEnvelopeV1,
        dimension: DimExponents,
        components: usize,
        vertex_count: usize,
    ) -> PyResult<(String, Self)> {
        let field_id = snapshot.field().ulid().to_string();
        let model_digest = snapshot.model_artifact().to_string();
        let mesh_digest = snapshot.mesh_artifact().to_string();
        let snapshot_digest = artifact_digest(py, snapshot.digest())?;
        let expected_values = vertex_count.checked_mul(components).ok_or_else(|| {
            PyOverflowError::new_err("Cartesian Q1 Field coefficient shape exceeds local usize")
        })?;
        if components == 0 || snapshot.coefficients().len() != expected_values {
            return Err(PyRuntimeError::new_err(
                "accepted Cartesian Q1 vector coefficients differ from their Mesh shape",
            ));
        }
        let support_indices = (0..vertex_count)
            .map(|index| {
                u32::try_from(index).map_err(|_| {
                    PyOverflowError::new_err("Field support index exceeds Python uint32")
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let field = Py::new(
            py,
            PyModelFieldRef::from_exact(model_digest, field_id.clone()),
        )?;
        let component_extent = u32::try_from(components)
            .map_err(|_| PyOverflowError::new_err("Field component count exceeds Python u32"))?;
        Ok((
            field_id.clone(),
            Self {
                digest: snapshot_digest.clone(),
                mesh_digest,
                field,
                field_id,
                support_domain_id: snapshot.support_domain().ulid().to_string(),
                dimension,
                value_shape: vec![component_extent],
                frame: "spatial-cartesian",
                blocks: vec![ProjectedBlock {
                    association: "vertex",
                    // This artifact owns its Q1 coefficient block directly rather than
                    // naming a separate DiscreteField artifact.
                    digest: snapshot_digest,
                    values: ProjectedValues::Vector(ReadOnlyMatrix::new(
                        vertex_count,
                        components,
                        snapshot.coefficients().to_vec(),
                    )),
                    support_indices: Arc::new(ReadOnlyVector::new(support_indices)),
                }],
            },
        ))
    }
}

fn common_vector_block(association: &'static str, values: &[[f64; 2]]) -> PyResult<ProjectedBlock> {
    let coefficients = values.iter().flatten().copied().collect::<Vec<_>>();
    common_block(
        association,
        &coefficients.clone(),
        ProjectedValues::Vector(ReadOnlyMatrix::new(values.len(), 2, coefficients)),
        values.len(),
    )
}

fn common_scalar_block(association: &'static str, values: &[f64]) -> PyResult<ProjectedBlock> {
    common_block(
        association,
        values,
        ProjectedValues::Scalar(ReadOnlyVector::new(values.to_vec())),
        values.len(),
    )
}

fn common_block(
    association: &'static str,
    coefficients: &[f64],
    values: ProjectedValues,
    count: usize,
) -> PyResult<ProjectedBlock> {
    let support_indices = (0..count)
        .map(|index| {
            u32::try_from(index)
                .map_err(|_| PyOverflowError::new_err("Field support index exceeds Python uint32"))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let mut hasher = Sha256::new();
    hasher.update(b"eqiora.common-field-block/v1\0");
    hasher.update(association.as_bytes());
    for value in coefficients {
        hasher.update(value.to_bits().to_be_bytes());
    }
    Ok(ProjectedBlock {
        association,
        digest: hex_sha256(hasher.finalize().as_slice()),
        values,
        support_indices: Arc::new(ReadOnlyVector::new(support_indices)),
    })
}

fn hex_sha256(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn artifact_digest(
    py: Python<'_>,
    value: Result<ArtifactDigest, eqiora::Diagnostic>,
) -> PyResult<String> {
    value
        .map(|digest| digest.to_string())
        .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))
}

const fn association_name(value: DiscreteFieldAssociation) -> &'static str {
    match value {
        DiscreteFieldAssociation::Vertex => "vertex",
        DiscreteFieldAssociation::Cell => "cell",
    }
}

const fn frame_name(value: ValueFrame) -> &'static str {
    match value {
        ValueFrame::Invariant => "invariant",
        ValueFrame::SpatialCartesian => "spatial-cartesian",
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFieldSnapshot>()?;
    module.add_class::<PyState>()?;
    module.add_class::<PyTrajectory>()?;
    Ok(())
}
