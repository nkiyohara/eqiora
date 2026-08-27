//! Immutable Python projection of one accepted spatial trajectory.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eqiora::DimExponents;
use eqiora::artifact::ArtifactDigest;
use eqiora_numerics::{CommonInitialField, CommonInitialValues, CommonState};
use numpy::{PyArray1, PyArray2};
use pyo3::exceptions::{PyIndexError, PyKeyError, PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyModule, PySequence, PyTuple};
use sha2::{Digest, Sha256};

use crate::matrix::{ReadOnlyMatrix, ReadOnlyVector};
use crate::meshing::PyMesh;
use crate::model::{PyModel, PyModelFieldRef};
mod presentation;

use presentation::TrajectoryPresentation;

/// Immutable exact-Field-bound coherent-SI initial coefficients.
#[pyclass(
    name = "InitialField",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyInitialField {
    pub(crate) native: CommonInitialField,
    field: Py<PyModelFieldRef>,
}

#[pymethods]
impl PyInitialField {
    #[new]
    #[pyo3(signature = (field, /, *, vertex_values=None, cell_values=None))]
    fn new(
        py: Python<'_>,
        field: Py<PyModelFieldRef>,
        vertex_values: Option<&Bound<'_, PyAny>>,
        cell_values: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let field_ref = field.borrow(py);
        let model = ArtifactDigest::from_hex(field_ref.exact_model_digest().to_owned())
            .map_err(|diagnostic| crate::error::validation_error(py, &[diagnostic]))?;
        let id = ulid::Ulid::from_string(field_ref.exact_id())
            .map(eqiora::Id::<eqiora::kinds::Field>::from_ulid)
            .map_err(|_| PyValueError::new_err("FieldRef contains an invalid exact Field ULID"))?;
        let vertex = vertex_values.map(extract_initial_values).transpose()?;
        let cell = cell_values.map(extract_initial_values).transpose()?;
        let native = CommonInitialField::new(model, id, vertex, cell)
            .map_err(|diagnostic| crate::error::validation_error(py, &[diagnostic]))?;
        drop(field_ref);
        Ok(Self { native, field })
    }

    #[getter]
    fn field(&self, py: Python<'_>) -> Py<PyModelFieldRef> {
        self.field.clone_ref(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "InitialField(field={:?}, vertex_values={}, cell_values={})",
            self.native.field().to_string(),
            self.native.vertex().is_some(),
            self.native.cell().is_some(),
        )
    }
}

fn extract_initial_values(value: &Bound<'_, PyAny>) -> PyResult<CommonInitialValues> {
    let normalized = value
        .cast::<PySequence>()
        .is_err()
        .then(|| value.call_method0("tolist"))
        .transpose()
        .map_err(|_| {
            PyValueError::new_err(
                "InitialField values must be a finite scalar or 2-vector sequence",
            )
        })?;
    let sequence = normalized
        .as_ref()
        .unwrap_or(value)
        .cast::<PySequence>()
        .map_err(|_| {
            PyValueError::new_err(
                "InitialField values must be a finite scalar or 2-vector sequence",
            )
        })?;
    let length = sequence.len()?;
    if length == 0 {
        return Err(PyValueError::new_err(
            "InitialField value sequences must be nonempty",
        ));
    }
    let first = sequence.get_item(0)?;
    if first.cast::<PySequence>().is_ok() && !first.is_instance_of::<pyo3::types::PyString>() {
        let mut values = Vec::with_capacity(length);
        for index in 0..length {
            let row = sequence
                .get_item(index)?
                .cast_into::<PySequence>()
                .map_err(|_| PyValueError::new_err("InitialField vector rows must be sequences"))?;
            if row.len()? != 2 {
                return Err(PyValueError::new_err(
                    "InitialField vectors must have exactly two components",
                ));
            }
            let mut vector = [0.0; 2];
            for (component, value) in vector.iter_mut().enumerate() {
                let item = row.get_item(component)?;
                if item.is_instance_of::<PyBool>() {
                    return Err(PyValueError::new_err("InitialField values reject booleans"));
                }
                *value = item.extract::<f64>().map_err(|_| {
                    PyValueError::new_err("InitialField values must be finite real numbers")
                })?;
            }
            values.push(vector);
        }
        Ok(CommonInitialValues::Vector2(values.into_boxed_slice()))
    } else {
        let mut values = Vec::with_capacity(length);
        for index in 0..length {
            let item = sequence.get_item(index)?;
            if item.is_instance_of::<PyBool>() {
                return Err(PyValueError::new_err("InitialField values reject booleans"));
            }
            values.push(item.extract::<f64>().map_err(|_| {
                PyValueError::new_err("InitialField values must be finite real numbers")
            })?);
        }
        Ok(CommonInitialValues::Scalar(values.into_boxed_slice()))
    }
}

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

impl PyFieldSnapshot {
    fn from_common_fsi(
        py: Python<'_>,
        plan: &eqiora_numerics::CommonFsiPlan,
        state: &CommonState,
        mesh_digest: &str,
    ) -> PyResult<Vec<Self>> {
        const VELOCITY: DimExponents = DimExponents {
            length: 1,
            time: -1,
            ..DimExponents::DIMENSIONLESS
        };
        const PRESSURE: DimExponents = DimExponents {
            mass: 1,
            length: -1,
            time: -2,
            ..DimExponents::DIMENSIONLESS
        };
        const DISPLACEMENT: DimExponents = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        let velocity = state.velocity_vertex_values().ok_or_else(|| {
            PyRuntimeError::new_err("FSI State omitted shared vertex velocity coefficients")
        })?;
        let pressure = state.pressure_vertex_values().ok_or_else(|| {
            PyRuntimeError::new_err("FSI State omitted fluid pressure coefficients")
        })?;
        let displacement = state.fsi_solid_displacement_values().ok_or_else(|| {
            PyRuntimeError::new_err("FSI State omitted solid displacement coefficients")
        })?;
        let fluid_vertices = plan.fluid_vertex_indices();
        let fluid_cells = plan.fluid_cell_indices();
        let solid_vertices = plan.solid_vertex_indices();
        let fluid_velocity = select_vectors(velocity, &fluid_vertices)?;
        let solid_velocity = select_vectors(velocity, &solid_vertices)?;
        let solid_displacement = select_vectors(displacement, &solid_vertices)?;
        let fluid_velocity_blocks = vec![
            common_vector_block_at("vertex", &fluid_velocity, &fluid_vertices)?,
            common_vector_block_at("cell", state.velocity_cell_values(), &fluid_cells)?,
        ];
        Ok(vec![
            Self::from_common_exact_parts(
                py,
                plan.model_digest(),
                mesh_digest,
                &plan.field_ids()[0],
                &plan.domain_ids()[0],
                VELOCITY,
                vec![2],
                "spatial-cartesian",
                fluid_velocity_blocks,
            )?,
            Self::from_common_exact_parts(
                py,
                plan.model_digest(),
                mesh_digest,
                &plan.field_ids()[1],
                &plan.domain_ids()[0],
                PRESSURE,
                Vec::new(),
                "invariant",
                vec![common_scalar_block_at("vertex", pressure, &fluid_vertices)?],
            )?,
            Self::from_common_exact_parts(
                py,
                plan.model_digest(),
                mesh_digest,
                &plan.field_ids()[2],
                &plan.domain_ids()[1],
                VELOCITY,
                vec![2],
                "spatial-cartesian",
                vec![common_vector_block_at(
                    "vertex",
                    &solid_velocity,
                    &solid_vertices,
                )?],
            )?,
            Self::from_common_exact_parts(
                py,
                plan.model_digest(),
                mesh_digest,
                &plan.field_ids()[3],
                &plan.domain_ids()[1],
                DISPLACEMENT,
                vec![2],
                "spatial-cartesian",
                vec![common_vector_block_at(
                    "vertex",
                    &solid_displacement,
                    &solid_vertices,
                )?],
            )?,
        ])
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
    ode_native: Option<eqiora_numerics::CommonOdeState>,
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
            ode_native: None,
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

    pub(crate) fn common_ode_native(&self) -> Option<&eqiora_numerics::CommonOdeState> {
        self.ode_native.as_ref()
    }

    fn from_common_ode(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        native: eqiora_numerics::CommonOdeState,
        source_request_identity: Option<&str>,
    ) -> Self {
        Self {
            digest: native.identity().to_owned(),
            model_digest: native.model_digest().to_owned(),
            step: 0,
            time_s: native.time_s(),
            fields: Vec::new(),
            field_lookup: native
                .field_ids()
                .iter()
                .enumerate()
                .map(|(index, field)| (field.to_string(), index))
                .collect(),
            model: Some(plan.model_handle(py)),
            mesh: None,
            native: None,
            ode_native: Some(native.clone()),
            plan_identity: plan.ode_native().map(|plan| plan.identity().to_owned()),
            source_request_identity: source_request_identity.map(str::to_owned),
            source_trajectory_identity: None,
            source_kind: Some(native.source_kind()),
        }
    }

    fn from_common_fsi(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        native: CommonState,
        step: u64,
        source_request_identity: Option<&str>,
    ) -> PyResult<Self> {
        let native_plan = plan.fsi_native().expect("FSI State requires FSI Plan");
        let mesh = plan.mesh_handle(py);
        let mesh_digest = mesh.borrow(py).exact_mesh_digest().to_owned();
        let fields = PyFieldSnapshot::from_common_fsi(py, native_plan, &native, &mesh_digest)?
            .into_iter()
            .map(|field| Py::new(py, field))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            digest: native.identity().to_owned(),
            model_digest: native_plan.model_digest().to_owned(),
            step,
            time_s: native.time_s(),
            fields,
            field_lookup: native_plan
                .field_ids()
                .iter()
                .enumerate()
                .map(|(index, field)| (field.clone(), index))
                .collect(),
            model: Some(plan.model_handle(py)),
            mesh: Some(mesh),
            native: Some(native),
            ode_native: None,
            plan_identity: Some(native_plan.identity().to_owned()),
            source_request_identity: source_request_identity.map(str::to_owned),
            source_trajectory_identity: None,
            source_kind: Some(if source_request_identity.is_some() {
                "result"
            } else {
                "initial"
            }),
        })
    }

    pub(crate) const fn time_s_value(&self) -> f64 {
        self.time_s
    }
}

#[pymethods]
impl PyState {
    #[staticmethod]
    #[pyo3(signature = (plan, /, *, fields=None, time_s=None))]
    fn initial(
        py: Python<'_>,
        plan: &crate::common_plan::PyPlan,
        fields: Option<&Bound<'_, PyTuple>>,
        time_s: Option<f64>,
    ) -> PyResult<Self> {
        if let Some(native_plan) = plan.ode_native() {
            if fields.is_some() || time_s.is_some() {
                return Err(PyValueError::new_err(
                    "ODE State.initial(plan) accepts no FSI field/time arguments",
                ));
            }
            let state = native_plan
                .initial_state()
                .map_err(|diagnostic| crate::error::validation_error(py, &[diagnostic]))?;
            return Ok(Self::from_common_ode(py, plan, state, None));
        }
        let native_plan = plan.fsi_native().ok_or_else(|| {
            PyValueError::new_err("State.initial requires an ODE or fixed-reference FSI Plan")
        })?;
        let fields = fields.ok_or_else(|| {
            PyValueError::new_err("FSI State.initial requires fields=(InitialField(...), ...)")
        })?;
        let time_s = time_s
            .ok_or_else(|| PyValueError::new_err("FSI State.initial requires explicit time_s="))?;
        let fields = fields
            .iter()
            .map(|value| {
                value
                    .extract::<PyRef<'_, PyInitialField>>()
                    .map(|field| field.native.clone())
                    .map_err(|_| {
                        PyValueError::new_err("fields must contain only InitialField values")
                    })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let state = native_plan
            .initial_state(time_s, fields)
            .map_err(|diagnostic| crate::error::validation_error(py, &[diagnostic]))?;
        Self::from_common_fsi(py, plan, state, 0, None)
    }

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
        if let Some(native) = plan.ode_native() {
            let state = result.common_ode_state_at(native.state_space_identity(), time_s)
                .ok_or_else(|| PyValueError::new_err(
                    "Result contains no State at time_s compatible with the exact ODE Plan state space"
                ))?;
            return Py::new(
                py,
                Self::from_common_ode(py, plan, state, Some(result.plan_key_value())),
            );
        }
        let native = plan
            .transient_native()
            .ok_or_else(|| PyValueError::new_err("State.from_result requires a transient Plan"))?;
        result
            .common_state_at(py, &native.state_space_identity(), time_s)
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
        self.ode_native.as_ref().map_or_else(
            || {
                self.native
                    .as_ref()
                    .map_or(self.digest.as_str(), CommonState::state_space_identity)
            },
            eqiora_numerics::CommonOdeState::state_space_identity,
        )
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

    #[getter]
    fn field_refs(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let Some(native) = &self.ode_native else {
            return Ok(PyTuple::empty(py).unbind());
        };
        PyTuple::new(
            py,
            native.field_ids().iter().map(|field| {
                PyModelFieldRef::from_exact(self.model_digest.clone(), field.to_string())
            }),
        )
        .map(|value| value.unbind())
    }

    #[pyo3(signature = (field, /))]
    fn value(&self, field: &PyModelFieldRef) -> PyResult<f64> {
        if field.exact_model_digest() != self.model_digest {
            return Err(PyValueError::new_err(
                "FieldRef belongs to a different exact Model artifact",
            ));
        }
        let native = self.ode_native.as_ref().ok_or_else(|| {
            PyValueError::new_err("State.value is available only for no-Mesh scalar ODE States")
        })?;
        let index = native
            .field_ids()
            .iter()
            .position(|id| id.to_string() == field.exact_id())
            .ok_or_else(|| PyKeyError::new_err(field.exact_id().to_owned()))?;
        Ok(native.values()[index])
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
        let (model_digest, plan_identity, realization_digest, is_fsi) =
            if let Some(native) = plan.transient_native() {
                (
                    native.model_digest().to_owned(),
                    native.identity().to_owned(),
                    None,
                    false,
                )
            } else if let Some(native) = plan.fsi_native() {
                (
                    native.model_digest().to_owned(),
                    native.identity().to_owned(),
                    Some(native.realization_digest().to_owned()),
                    true,
                )
            } else {
                return Err(PyRuntimeError::new_err(
                    "common Trajectory requires a transient or FSI Plan",
                ));
            };
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
            let projected_state = if is_fsi {
                let mut value =
                    PyState::from_common_fsi(py, plan, state, step, Some(run_identity))?;
                value.source_trajectory_identity = Some(trajectory_digest.clone());
                value
            } else {
                PyState::from_common(
                    py,
                    plan,
                    state,
                    step,
                    Some(run_identity),
                    Some(&trajectory_digest),
                )?
            };
            projected.push(Py::new(py, projected_state)?);
        }
        let geometry_digest = mesh_ref.source_digest_value().to_owned();
        let correspondence_digest = mesh_ref.correspondence_digest_value().to_owned();
        let mesh_digest = mesh_ref.exact_mesh_digest().to_owned();
        drop(mesh_ref);
        Ok(Self {
            model_digest,
            geometry_digest,
            correspondence_digest,
            mesh_digest,
            realization_digest,
            plan_identity: Some(plan_identity),
            run_digest: Some(run_identity.to_owned()),
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

    #[allow(clippy::too_many_arguments)]
    fn from_common_exact_parts(
        py: Python<'_>,
        model_digest: &str,
        mesh_digest: &str,
        field_id: &str,
        support_domain_id: &str,
        dimension: DimExponents,
        value_shape: Vec<u32>,
        frame: &'static str,
        blocks: Vec<ProjectedBlock>,
    ) -> PyResult<Self> {
        let mut hasher = Sha256::new();
        hasher.update(b"eqiora.common-field-snapshot/v1\0");
        hasher.update(model_digest.as_bytes());
        hasher.update(mesh_digest.as_bytes());
        hasher.update(field_id.as_bytes());
        for block in &blocks {
            hasher.update(block.digest.as_bytes());
        }
        Ok(Self {
            digest: hex_sha256(hasher.finalize().as_slice()),
            mesh_digest: mesh_digest.to_owned(),
            field: Py::new(
                py,
                PyModelFieldRef::from_exact(model_digest.to_owned(), field_id.to_owned()),
            )?,
            field_id: field_id.to_owned(),
            support_domain_id: support_domain_id.to_owned(),
            dimension,
            value_shape,
            frame,
            blocks,
        })
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

fn select_vectors(values: &[[f64; 2]], indices: &[usize]) -> PyResult<Vec<[f64; 2]>> {
    indices
        .iter()
        .map(|&index| {
            values.get(index).copied().ok_or_else(|| {
                PyRuntimeError::new_err(
                    "FSI Field support index exceeds shared vertex coefficients",
                )
            })
        })
        .collect()
}

fn common_vector_block_at(
    association: &'static str,
    values: &[[f64; 2]],
    indices: &[usize],
) -> PyResult<ProjectedBlock> {
    if values.len() != indices.len() {
        return Err(PyRuntimeError::new_err(
            "FSI vector block cardinality differs from exact support",
        ));
    }
    let coefficients = values.iter().flatten().copied().collect::<Vec<_>>();
    common_block_at(
        association,
        &coefficients,
        ProjectedValues::Vector(ReadOnlyMatrix::new(values.len(), 2, coefficients.clone())),
        indices,
    )
}

fn common_scalar_block_at(
    association: &'static str,
    values: &[f64],
    indices: &[usize],
) -> PyResult<ProjectedBlock> {
    if values.len() != indices.len() {
        return Err(PyRuntimeError::new_err(
            "FSI scalar block cardinality differs from exact support",
        ));
    }
    common_block_at(
        association,
        values,
        ProjectedValues::Scalar(ReadOnlyVector::new(values.to_vec())),
        indices,
    )
}

fn common_block_at(
    association: &'static str,
    coefficients: &[f64],
    values: ProjectedValues,
    indices: &[usize],
) -> PyResult<ProjectedBlock> {
    let support_indices = indices
        .iter()
        .map(|&index| {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFieldSnapshot>()?;
    module.add_class::<PyInitialField>()?;
    module.add_class::<PyState>()?;
    module.add_class::<PyTrajectory>()?;
    Ok(())
}
