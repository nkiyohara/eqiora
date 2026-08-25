//! Immutable Python projection of one accepted spatial trajectory.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use eqiora::DimExponents;
use eqiora::api::{FixedMeshFieldTrajectoryReplay2dV1, UnstructuredP1ScalarFieldProjection2d};
use eqiora::artifact::{
    ArtifactDigest, CanonicalModelArtifact, CartesianQ1FieldSnapshotEnvelopeV1,
    FieldSnapshotEnvelopeV1, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora::kernel::ValueFrame;
use eqiora::meshing::{DiscreteFieldAssociation, DiscreteFieldShape};
use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::{PyIndexError, PyKeyError, PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyModule, PyTuple};

use crate::diagnostic_error;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::model::{PyModel, PyModelFieldRef};
use crate::notebook_mime::{TEXT_MIME, WIDGET_MIME, select_mime_types};

const TRAJECTORY_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: this N3 viewer supports only an accepted fixed-mesh 2D Trajectory with one consistent invariant scalar vertex Field.";
const CORRUPT_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: the installed Eqiora Notebook presentation runtime or assets are incomplete. Reinstall eqiora[notebook].";

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
    name = "TrajectoryState",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
pub(crate) struct PyTrajectoryState {
    digest: String,
    model_digest: String,
    step: u64,
    time_s: f64,
    fields: Vec<Py<PyFieldSnapshot>>,
    field_lookup: BTreeMap<String, usize>,
}

impl PartialEq for PyTrajectoryState {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl Eq for PyTrajectoryState {}

impl Hash for PyTrajectoryState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.digest.hash(state);
    }
}

impl PyTrajectoryState {
    pub(crate) fn digest_value(&self) -> &str {
        &self.digest
    }

    pub(crate) fn model_digest_value(&self) -> &str {
        &self.model_digest
    }
}

#[pymethods]
impl PyTrajectoryState {
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
            "TrajectoryState(step={}, time_s={}, digest={:?})",
            self.step, self.time_s, self.digest,
        )
    }
}

/// Immutable installed-Python projection of one accepted trajectory.
///
/// The public name is general while the current constructor is deliberately
/// narrow: only the accepted fixed-mesh affine-triangle 2D V1 replay can
/// produce this value.
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
    realization_digest: String,
    run_digest: String,
    trajectory_digest: String,
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
    states: Vec<Py<PyTrajectoryState>>,
    state_lookup: BTreeMap<u64, usize>,
    presentation: Mutex<PresentationState>,
}

enum PresentationState {
    Empty,
    Creating,
    Ready(Py<PyAny>),
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
    fn realization_digest(&self) -> &str {
        &self.realization_digest
    }

    #[getter]
    fn run_digest(&self) -> &str {
        &self.run_digest
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
        self.coordinates.numpy(py)
    }

    #[getter]
    fn cells(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.cells.numpy(py)
    }

    #[getter]
    fn states(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.states.iter().map(|state| state.clone_ref(py)))?.unbind())
    }

    /// Select one accepted state by its exact step ordinal.
    #[pyo3(signature = (step, /))]
    fn state(&self, py: Python<'_>, step: u64) -> PyResult<Py<PyTrajectoryState>> {
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
        let selected = select_mime_types(py, include, exclude)?;
        let output = PyDict::new(py);
        if selected.is_empty() {
            return Ok(output.unbind());
        }

        let trajectory = slf.get();
        let representation = trajectory.__repr__();
        if !selected.contains(WIDGET_MIME) {
            if selected.contains(TEXT_MIME) {
                output.set_item(TEXT_MIME, representation)?;
            }
            return Ok(output.unbind());
        }

        let coordinates = trajectory.coordinates.numpy(py)?;
        let cells = trajectory.cells.numpy(py)?;
        let states = PyTuple::new(
            py,
            trajectory.states.iter().map(|state| state.clone_ref(py)),
        )?;
        let token = PyDict::new(py);
        token.set_item("model_digest", &trajectory.model_digest)?;
        token.set_item("geometry_digest", &trajectory.geometry_digest)?;
        token.set_item("correspondence_digest", &trajectory.correspondence_digest)?;
        token.set_item("mesh_digest", &trajectory.mesh_digest)?;
        token.set_item("realization_digest", &trajectory.realization_digest)?;
        token.set_item("run_digest", &trajectory.run_digest)?;
        token.set_item("trajectory_digest", &trajectory.trajectory_digest)?;
        token.set_item("coordinates", coordinates.bind(py))?;
        token.set_item("cells", cells.bind(py))?;
        token.set_item("states", &states)?;

        let current = {
            let mut state = trajectory
                .presentation
                .lock()
                .map_err(|_| PyRuntimeError::new_err("Trajectory presentation lock is poisoned"))?;
            match std::mem::replace(&mut *state, PresentationState::Creating) {
                PresentationState::Empty => None,
                PresentationState::Ready(delegate) => Some(delegate),
                PresentationState::Creating => {
                    if selected.contains(TEXT_MIME) {
                        output.set_item(
                            TEXT_MIME,
                            format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                        )?;
                    }
                    return Ok(output.unbind());
                }
            }
        };

        match call_presentation_adapter(py, slf.bind(py), &token, current.as_ref()) {
            Ok(AdapterOutcome::Absent) => {
                trajectory.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(TEXT_MIME, representation)?;
                }
            }
            Ok(AdapterOutcome::Unsupported) => {
                if let Some(delegate) = current {
                    close_delegate(py, &delegate);
                }
                trajectory.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(
                        TEXT_MIME,
                        format!("{representation}\n{TRAJECTORY_NOTEBOOK_MESSAGE}"),
                    )?;
                }
            }
            Ok(AdapterOutcome::Rich {
                delegate,
                widget_view,
            }) => {
                trajectory.set_presentation_state(PresentationState::Ready(delegate))?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(TEXT_MIME, representation)?;
                }
                output.set_item(WIDGET_MIME, widget_view)?;
            }
            Err(delegate) => {
                if let Some(delegate) = delegate.or(current) {
                    close_delegate(py, &delegate);
                }
                trajectory.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(
                        TEXT_MIME,
                        format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                    )?;
                }
            }
        }
        Ok(output.unbind())
    }
}

impl PyTrajectory {
    pub(crate) fn digest_value(&self) -> &str {
        &self.trajectory_digest
    }

    pub(crate) fn model_digest_value(&self) -> &str {
        &self.model_digest
    }

    pub(crate) fn state_handles(&self, py: Python<'_>) -> Vec<Py<PyTrajectoryState>> {
        self.states
            .iter()
            .map(|state| state.clone_ref(py))
            .collect()
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
                PyTrajectoryState {
                    digest,
                    model_digest: model_digest.clone(),
                    step: state.step(),
                    time_s: state.time_s(),
                    fields,
                    field_lookup,
                },
            )?);
        }

        Ok(Self {
            model_digest,
            geometry_digest: first.geometry_artifact().to_string(),
            correspondence_digest: first.correspondence_artifact().to_string(),
            mesh_digest,
            realization_digest: first.realization_artifact().to_string(),
            run_digest: artifact_digest(py, run.digest())?,
            trajectory_digest,
            coordinates: ReadOnlyMatrix::new(vertex_count, 2, coordinates),
            cells: ReadOnlyMatrix::new(cell_count, 3, cells),
            states,
            state_lookup,
            presentation: Mutex::new(PresentationState::Empty),
        })
    }

    fn set_presentation_state(&self, next: PresentationState) -> PyResult<()> {
        let mut state = self
            .presentation
            .lock()
            .map_err(|_| PyRuntimeError::new_err("Trajectory presentation lock is poisoned"))?;
        *state = next;
        Ok(())
    }
}

enum AdapterOutcome {
    Absent,
    Unsupported,
    Rich {
        delegate: Py<PyAny>,
        widget_view: Py<PyAny>,
    },
}

fn call_presentation_adapter(
    py: Python<'_>,
    trajectory: &Bound<'_, PyTrajectory>,
    token: &Bound<'_, PyDict>,
    current: Option<&Py<PyAny>>,
) -> Result<AdapterOutcome, Option<Py<PyAny>>> {
    let module = py.import("eqiora._presentation").map_err(|_| None)?;
    let adapter = module.getattr("trajectory_mimebundle").map_err(|_| None)?;
    let current = current.map_or_else(|| py.None(), |value| value.clone_ref(py));
    let result = adapter
        .call1((trajectory, token, current))
        .map_err(|_| None)?;
    let tuple = result.cast::<PyTuple>().map_err(|_| None)?;
    if tuple.len() != 3 {
        return Err(tuple.get_item(1).ok().map(Bound::unbind));
    }
    let status = tuple
        .get_item(0)
        .and_then(|value| value.extract::<String>())
        .map_err(|_| tuple.get_item(1).ok().map(Bound::unbind))?;
    if status == "absent"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Absent);
    }
    if status == "unsupported"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Unsupported);
    }
    if status != "rich" {
        return Err(tuple
            .get_item(1)
            .ok()
            .and_then(|value| (!value.is_none()).then(|| value.unbind())));
    }
    let delegate = tuple.get_item(1).map_err(|_| None)?;
    if delegate.is_none() {
        return Err(None);
    }
    let delegate = delegate.unbind();
    let hook_result = tuple
        .get_item(2)
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let hook_tuple = hook_result
        .cast::<PyTuple>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if hook_tuple.len() != 2
        || !hook_tuple
            .get_item(1)
            .is_ok_and(|value| value.is_instance_of::<PyDict>())
    {
        return Err(Some(delegate));
    }
    let data = hook_tuple
        .get_item(0)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .cast_into::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let widget_view = data
        .get_item(WIDGET_MIME)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .ok_or_else(|| Some(delegate.clone_ref(py)))?;
    let widget = widget_view
        .cast::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if widget.len() != 3
        || widget
            .get_item("version_major")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(2)
        || widget
            .get_item("version_minor")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(0)
        || widget
            .get_item("model_id")
            .ok()
            .flatten()
            .and_then(|value| value.extract::<String>().ok())
            .is_none_or(|model_id| model_id.is_empty())
    {
        return Err(Some(delegate));
    }
    Ok(AdapterOutcome::Rich {
        delegate,
        widget_view: widget_view.unbind(),
    })
}

fn close_delegate(py: Python<'_>, delegate: &Py<PyAny>) {
    let _ = delegate.bind(py).call_method0("close");
}

fn exact_u8(value: Bound<'_, PyAny>) -> Option<u8> {
    if value.is_instance_of::<PyBool>() {
        None
    } else {
        value.extract::<u8>().ok()
    }
}

impl PyFieldSnapshot {
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
    module.add_class::<PyTrajectoryState>()?;
    module.add_class::<PyTrajectory>()?;
    Ok(())
}
