//! Immutable Python projection of one accepted spatial trajectory.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use eqiora::DimExponents;
use eqiora::api::FixedMeshFieldTrajectoryReplay2dV1;
use eqiora::artifact::{ArtifactDigest, RunManifestV2, SimplicialMeshEnvelopeV1};
use eqiora::kernel::ValueFrame;
use eqiora::meshing::{DiscreteFieldAssociation, DiscreteFieldShape};
use numpy::PyArray2;
use pyo3::exceptions::{PyIndexError, PyKeyError, PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule, PyTuple};

use crate::diagnostic_error;
use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::model::{PyModel, PyModelFieldRef};

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
    fn value_shape(&self) -> Vec<u32> {
        self.value_shape.clone()
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
}

impl PyTrajectory {
    pub(crate) fn coordinates_numpy(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.coordinates.numpy(py)
    }

    pub(crate) fn cells_numpy(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.cells.numpy(py)
    }

    #[allow(clippy::too_many_arguments)]
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
            .document()
            .artifact_reference()
            .map_err(|error| diagnostic_error(py, std::slice::from_ref(&error)))?
            .artifact()
            .to_string();
        if model_digest != current_model_digest {
            return Err(PyRuntimeError::new_err(
                "accepted trajectory Model differs from the supplied Python Model",
            ));
        }

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
                        association: association_name(block.association()),
                        digest: artifact_digest(py, block.digest())?,
                        values,
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
                        field: field_ref,
                        field_id: field_id.clone(),
                        support_domain_id: snapshot.support_domain().ulid().to_string(),
                        dimension: snapshot.dimension(),
                        value_shape,
                        frame: frame_name(snapshot.frame()),
                        blocks,
                    },
                )?;
                field_lookup.insert(field_id, fields.len());
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
            mesh_digest: first.mesh_artifact().to_string(),
            realization_digest: first.realization_artifact().to_string(),
            run_digest: artifact_digest(py, run.digest())?,
            trajectory_digest: artifact_digest(py, replay.trajectory().digest())?,
            coordinates: ReadOnlyMatrix::new(vertex_count, 2, coordinates),
            cells: ReadOnlyMatrix::new(cell_count, 3, cells),
            states,
            state_lookup,
        })
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
