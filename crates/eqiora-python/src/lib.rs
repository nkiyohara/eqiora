//! Private PyO3 adapter for the public `eqiora` Python package.
//!
//! This crate is a language boundary, not a second implementation of Eqiora
//! semantics. It depends only on the public Rust facade.

mod array;
mod differentiation;
mod elasticity;
mod error;
mod execution;
mod fsi;
mod geometry;
mod jax_ffi;
mod matrix;
mod meshing;
mod model;
mod modeling;
mod realization;
mod steady_stokes;

use std::collections::BTreeMap;

use eqiora::DimExponents;
use eqiora::compatibility::ExactModelCodec;
use eqiora::control::{CompileOutcomeV1, CompileRequestV1, execute_compile_v1};
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule, PyTuple};

use array::PyArrayBuffer;
pub(crate) use error::diagnostic_error;
#[doc(hidden)]
pub use error::panic_boundary;
use error::{compatibility_error, control_diagnostic_error};
use execution::RunIdentity;
use model::PyModel;

fn python_distribution_version(cargo_version: &str) -> Option<String> {
    if cargo_version.contains('+') {
        return None;
    }
    let (release, prerelease) = match cargo_version.split_once('-') {
        Some((release, prerelease)) => (release, Some(prerelease)),
        None => (cargo_version, None),
    };
    let release_components = release.split('.').collect::<Vec<_>>();
    if release_components.len() != 3
        || release_components.iter().any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    let Some(prerelease) = prerelease else {
        return Some(release.to_owned());
    };
    let mut components = prerelease.split('.');
    let label = components.next()?;
    let serial = components.next()?;
    if components.next().is_some()
        || serial.is_empty()
        || !serial.bytes().all(|byte| byte.is_ascii_digit())
        || serial.parse::<u64>().ok()?.to_string() != serial
    {
        return None;
    }
    let marker = match label {
        "alpha" => "a",
        "beta" => "b",
        "rc" => "rc",
        _ => return None,
    };
    Some(format!("{release}{marker}{serial}"))
}

/// Canonical model and transaction wire selected before an operation begins.
#[pyclass(
    name = "ExactModelCodec",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PyExactModelCodec {
    /// Original scalar model vocabulary.
    V1,
    /// Scalar physical Domains, Ports, and across/through symbols.
    V2,
    /// Shaped Fields and field-valued boundary interfaces.
    V3,
    /// Canonical tensor operators.
    V4,
    /// Content-addressed canonical pure operators.
    V5,
    /// Spatial-periodic boundary Connections.
    V6,
    /// Domains that name an authored geometry by digest and entity set.
    V7,
}

impl From<PyExactModelCodec> for ExactModelCodec {
    fn from(value: PyExactModelCodec) -> Self {
        match value {
            PyExactModelCodec::V1 => Self::V1,
            PyExactModelCodec::V2 => Self::V2,
            PyExactModelCodec::V3 => Self::V3,
            PyExactModelCodec::V4 => Self::V4,
            PyExactModelCodec::V5 => Self::V5,
            PyExactModelCodec::V6 => Self::V6,
            PyExactModelCodec::V7 => Self::V7,
        }
    }
}

impl TryFrom<ExactModelCodec> for PyExactModelCodec {
    type Error = eqiora::Diagnostic;

    fn try_from(value: ExactModelCodec) -> Result<Self, Self::Error> {
        match value {
            ExactModelCodec::V1 => Ok(Self::V1),
            ExactModelCodec::V2 => Ok(Self::V2),
            ExactModelCodec::V3 => Ok(Self::V3),
            ExactModelCodec::V4 => Ok(Self::V4),
            ExactModelCodec::V5 => Ok(Self::V5),
            ExactModelCodec::V6 => Ok(Self::V6),
            ExactModelCodec::V7 => Ok(Self::V7),
            _ => Err(eqiora::Diagnostic::error(
                eqiora::diagnostic::codes::NOT_IMPLEMENTED,
                "model uses a codec unsupported by this Python SDK",
            )),
        }
    }
}

/// One read-only, field-local sampled series in SI units.
#[pyclass(name = "Series", module = "eqiora._eqiora", frozen)]
struct PySeries {
    id: String,
    name: Option<String>,
    dimension: DimExponents,
    time: Py<PyArrayBuffer>,
    values: Py<PyArrayBuffer>,
}

#[pymethods]
impl PySeries {
    /// Stable Field ULID.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    /// Source alias, absent for a model reconstructed without symbol data.
    #[getter]
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// SI base-dimension exponents in M,L,T,I,Theta,N,J order.
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

    /// Field-local model time in seconds.
    #[getter]
    fn time(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.time.clone_ref(py)
    }

    /// Field-local values expressed in coherent SI units.
    #[getter]
    fn values(&self, py: Python<'_>) -> Py<PyArrayBuffer> {
        self.values.clone_ref(py)
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        Ok(self.values.borrow(py).len())
    }

    /// Iterate `(time, value)` samples, which is what a series is.
    ///
    /// Both buffers are snapshotted once and the pairs are yielded from that
    /// snapshot. There is deliberately no `__getitem__`: a single element
    /// cannot be read without materializing the whole buffer, so indexing would
    /// make `for sample in series` quadratic while looking ordinary.
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

/// Immutable collection of independently sampled field series.
#[pyclass(name = "Result", module = "eqiora._eqiora", frozen)]
struct PyRunResult {
    fields: Vec<Py<PySeries>>,
    lookup: BTreeMap<String, usize>,
    identity: RunIdentity,
    elapsed_seconds: f64,
}

#[pymethods]
impl PyRunResult {
    /// Exact canonical Model identity executed by this reference result.
    #[getter]
    fn model_id(&self) -> &str {
        self.identity.model_id()
    }

    /// Domain-separated digest of the exact immutable Model artifact.
    #[getter]
    fn model_digest(&self) -> &str {
        self.identity.model_digest()
    }

    /// Exact semantic revision recorded by the Model artifact.
    #[getter]
    const fn model_revision(&self) -> u64 {
        self.identity.model_revision()
    }

    /// Exact replay key of the admitted semantic-reference plan.
    #[getter]
    fn plan_key(&self) -> &str {
        self.identity.plan_key()
    }

    /// Concrete native adapter that produced this accepted result.
    #[getter]
    fn adapter(&self) -> &'static str {
        self.identity.adapter()
    }

    /// Adapter implementation version used by this execution occurrence.
    #[getter]
    fn adapter_version(&self) -> &'static str {
        self.identity.adapter_version()
    }

    /// Observed native wall time through accepted result projection.
    #[getter]
    const fn elapsed_seconds(&self) -> f64 {
        self.elapsed_seconds
    }

    /// Series in stable Field-ID order.
    #[getter]
    fn fields(&self, py: Python<'_>) -> Vec<Py<PySeries>> {
        self.fields
            .iter()
            .map(|field| field.clone_ref(py))
            .collect()
    }

    fn __len__(&self) -> usize {
        self.fields.len()
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PySeries>> {
        let index = self
            .lookup
            .get(key)
            .copied()
            .ok_or_else(|| PyKeyError::new_err(key.to_owned()))?;
        Ok(self.fields[index].clone_ref(py))
    }

    fn __repr__(&self) -> String {
        format!(
            "Result(fields={}, model_digest={:?}, plan_key={:?})",
            self.fields.len(),
            self.model_digest(),
            self.plan_key()
        )
    }
}

/// Compile exactly one Eqiora model through the canonical Rust pipeline.
#[pyfunction]
#[pyo3(signature = (source, *, filename="<memory>"))]
fn compile(py: Python<'_>, source: &str, filename: &str) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let request =
            CompileRequestV1::new_current("python.compile", filename.to_owned(), source.to_owned())
                .map_err(|diagnostic| control_diagnostic_error(py, &[diagnostic]))?;
        execute_compile_request(py, request)
    })
}

/// Compile through one exact compatibility codec.
#[pyfunction]
#[pyo3(signature = (source, *, filename="<memory>", codec))]
fn compile_exact(
    py: Python<'_>,
    source: &str,
    filename: &str,
    codec: PyExactModelCodec,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let request = CompileRequestV1::new_exact(
            "python.compile-exact",
            codec.into(),
            filename.to_owned(),
            source.to_owned(),
        )
        .map_err(|diagnostic| control_diagnostic_error(py, &[diagnostic]))?;
        execute_compile_request(py, request)
    })
}

/// Define native declarations through one exact compatibility codec.
#[pyfunction]
#[pyo3(signature = (name, *declarations, codec))]
fn define_exact(
    py: Python<'_>,
    name: String,
    declarations: &Bound<'_, PyTuple>,
    codec: PyExactModelCodec,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        modeling::define_model_exact(py, name, declarations, codec.into())
            .and_then(|document| PyModel::from_document(py, document))
    })
}

/// Replay one canonical envelope through one exact compatibility codec.
#[pyfunction]
#[pyo3(signature = (data, *, codec))]
fn replay_exact(py: Python<'_>, data: &[u8], codec: PyExactModelCodec) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let data = data.to_vec();
        py.detach(move || ExactModelCodec::from(codec).replay(&data))
            .map_err(|diagnostics| compatibility_error(py, &diagnostics))
            .and_then(|document| PyModel::from_document(py, document))
    })
}

fn execute_compile_request(py: Python<'_>, request: CompileRequestV1) -> PyResult<PyModel> {
    let execution = py.detach(move || execute_compile_v1(&request));
    let (response, document) = execution.into_parts();
    match response.outcome() {
        CompileOutcomeV1::Accepted { .. } => document
            .map(|document| PyModel::from_document(py, document))
            .transpose()?
            .ok_or_else(|| {
                error::internal_error(
                    py,
                    "compile/check accepted a model without returning its immutable document",
                )
            }),
        CompileOutcomeV1::Rejected { diagnostics } => {
            Err(control_diagnostic_error(py, diagnostics))
        }
    }
}

fn result_into_python(
    py: Python<'_>,
    result: eqiora::api::ReferenceRunResult,
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
        fields,
        lookup,
        identity,
        elapsed_seconds,
    })
}

#[pymodule]
pub fn _eqiora(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let version = python_distribution_version(eqiora::VERSION).ok_or_else(|| {
        PyRuntimeError::new_err(format!(
            "Eqiora Cargo release identity {:?} has no admitted Python mapping",
            eqiora::VERSION
        ))
    })?;
    module.add("__version__", version)?;
    error::register(module)?;
    module.add_class::<PyExactModelCodec>()?;
    model::register(module)?;
    array::register(module)?;
    differentiation::register(module)?;
    elasticity::register(module)?;
    jax_ffi::register_module(module)?;
    module.add_class::<PySeries>()?;
    module.add_class::<PyRunResult>()?;
    execution::register(module)?;
    fsi::register(module)?;
    geometry::register(module)?;
    meshing::register(module)?;
    modeling::register(module)?;
    realization::register(module)?;
    steady_stokes::register(module)?;
    module.add_function(wrap_pyfunction!(compile, module)?)?;
    module.add_function(wrap_pyfunction!(compile_exact, module)?)?;
    module.add_function(wrap_pyfunction!(define_exact, module)?)?;
    module.add_function(wrap_pyfunction!(replay_exact, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use eqiora::api::ModelDocument;

    use super::{ExactModelCodec, PyExactModelCodec, python_distribution_version};

    const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

    #[test]
    fn python_distribution_version_is_derived_fail_closed_from_cargo_semver() {
        assert_eq!(
            python_distribution_version("0.1.0-alpha.1").as_deref(),
            Some("0.1.0a1")
        );
        assert_eq!(
            python_distribution_version("1.2.3-beta.4").as_deref(),
            Some("1.2.3b4")
        );
        assert_eq!(
            python_distribution_version("1.2.3-rc.5").as_deref(),
            Some("1.2.3rc5")
        );
        assert_eq!(
            python_distribution_version("1.2.3").as_deref(),
            Some("1.2.3")
        );
        for rejected in [
            "0.1.0-dev.1",
            "0.1.0-alpha",
            "0.1.0-alpha.01",
            "0.1.0-alpha.1.extra",
            "0.1.0+local",
        ] {
            assert!(
                python_distribution_version(rejected).is_none(),
                "unexpected Python mapping for {rejected:?}"
            );
        }
    }

    #[test]
    fn python_codec_conversion_is_exact_for_every_supported_generation() {
        for (python, rust) in [
            (PyExactModelCodec::V1, ExactModelCodec::V1),
            (PyExactModelCodec::V2, ExactModelCodec::V2),
            (PyExactModelCodec::V3, ExactModelCodec::V3),
            (PyExactModelCodec::V4, ExactModelCodec::V4),
            (PyExactModelCodec::V5, ExactModelCodec::V5),
            (PyExactModelCodec::V6, ExactModelCodec::V6),
            (PyExactModelCodec::V7, ExactModelCodec::V7),
        ] {
            assert_eq!(ExactModelCodec::from(python), rust);
            assert_eq!(PyExactModelCodec::try_from(rust).unwrap(), python);
        }
    }

    #[test]
    fn ordinary_python_authoring_observes_the_current_codec() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        assert_eq!(document.exact_codec(), ExactModelCodec::CURRENT);
        assert_eq!(
            PyExactModelCodec::try_from(document.exact_codec()).unwrap(),
            PyExactModelCodec::V7
        );
        assert!(
            String::from_utf8(document.canonical_json().unwrap())
                .unwrap()
                .contains("eqiora.model-envelope/v7")
        );
    }

    #[test]
    fn explicit_python_compatibility_keeps_v1_through_v7_separate() {
        for (python, schema) in [
            (PyExactModelCodec::V1, "eqiora.model-envelope/v1"),
            (PyExactModelCodec::V2, "eqiora.model-envelope/v2"),
            (PyExactModelCodec::V3, "eqiora.model-envelope/v3"),
            (PyExactModelCodec::V4, "eqiora.model-envelope/v4"),
            (PyExactModelCodec::V5, "eqiora.model-envelope/v5"),
            (PyExactModelCodec::V6, "eqiora.model-envelope/v6"),
            (PyExactModelCodec::V7, "eqiora.model-envelope/v7"),
        ] {
            let codec = ExactModelCodec::from(python);
            let document = codec.compile("decay.eqi", SOURCE).unwrap();
            let bytes = document.canonical_json().unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains(schema));
            assert_eq!(codec.replay(&bytes).unwrap().exact_codec(), codec);
        }
    }
}
