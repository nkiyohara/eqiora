//! Common installed-Python ownership for accepted execution results.

use std::collections::BTreeMap;

use eqiora::api::ReferenceRunResult;
use eqiora::diagnostic::codes;
use eqiora::{Diagnostic, DimExponents};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyTuple};

use crate::array::PyArrayBuffer;
use crate::diagnostic_error;
use crate::elasticity::PyLinearElasticityEvidence;
use crate::execution::RunIdentity;
use crate::meshing::PyMesh;
use crate::model::PyModelFieldRef;
use crate::realization::PyRunManifest;
use crate::steady_stokes::PySteadyStokesEvidence;
use crate::trajectory::PyFieldSnapshot;

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

enum ResultPayload {
    Series {
        fields: Vec<Py<PySeries>>,
        lookup: BTreeMap<String, usize>,
    },
    Static(StaticResultPayload),
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
            ResultPayload::Static(_) => Vec::new(),
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
        };
        Ok(PyTuple::new(py, snapshots)?.unbind())
    }

    /// Select one exact static Field observation by Model-bound identity.
    #[pyo3(signature = (field, /))]
    fn field(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyFieldSnapshot>> {
        self.static_output(field)
            .map(|output| output.snapshot.clone_ref(py))
    }

    /// Select the exact accepted Mesh paired with one static Field.
    #[pyo3(signature = (field, /))]
    fn mesh(&self, py: Python<'_>, field: &PyModelFieldRef) -> PyResult<Py<PyMesh>> {
        self.static_output(field)
            .map(|output| output.mesh.clone_ref(py))
    }

    /// Return the exact durable Run manifest when this occurrence owns one.
    fn run_manifest(&self, py: Python<'_>) -> PyResult<Py<PyRunManifest>> {
        match &self.payload {
            ResultPayload::Static(payload) => Ok(payload.run_manifest.clone_ref(py)),
            ResultPayload::Series { .. } => Err(capability_error(
                py,
                "this Result occurrence has no durable Run manifest",
            )),
        }
    }

    fn __len__(&self) -> usize {
        match &self.payload {
            ResultPayload::Series { fields, .. } => fields.len(),
            ResultPayload::Static(_) => 0,
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
            },
            self.model_digest(),
            self.plan_key(),
        )
    }
}

impl PyRunResult {
    fn static_output(&self, field: &PyModelFieldRef) -> PyResult<&StaticFieldOutput> {
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
            ResultPayload::Static(_) | ResultPayload::Series { .. } => Err(capability_error(
                py,
                "this Result occurrence has no linear-elasticity evidence",
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
    module.add_class::<PyRunResult>()?;
    Ok(())
}
