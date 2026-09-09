//! Exact Observable selection happens once at the immutable Model boundary.
use super::*;
use eqiora::{Id, kinds};
use pyo3::exceptions::{PyKeyError, PyValueError};

/// Exact derived output selected from one immutable Model.
#[pyclass(
    name = "ObservableRef",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyObservableRef {
    pub(crate) model_digest: String,
    pub(crate) id: Id<kinds::Observable>,
}

#[pymethods]
impl PyObservableRef {
    fn __repr__(&self) -> String {
        format!(
            "ObservableRef(model_digest={:?}, id={:?})",
            self.model_digest,
            self.id.ulid().to_string()
        )
    }

    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }
    #[getter]
    fn id(&self) -> String {
        self.id.ulid().to_string()
    }
}

pub(super) fn select(
    model: &PyModel,
    py: Python<'_>,
    selection: &str,
) -> PyResult<PyObservableRef> {
    let raw = model
        .document()
        .ok()
        .and_then(|document| document.aliases().get(selection).copied());
    let id = match raw {
        Some(raw) => raw
            .downcast::<kinds::Observable>()
            .ok_or_else(|| PyValueError::new_err("selection does not identify an Observable"))?,
        None => {
            let ulid = selection
                .parse::<ulid::Ulid>()
                .map_err(|_| PyKeyError::new_err(selection.to_owned()))?;
            Id::from_ulid(ulid)
        }
    };
    if !model
        .artifact_ids(EntityKind::Observable)
        .map_err(|errors| validation_error(py, &errors))?
        .iter()
        .any(|candidate| candidate == &id.ulid().to_string())
    {
        return Err(PyKeyError::new_err(
            "Observable is outside this exact Model",
        ));
    }
    let reference = model
        .artifact()
        .artifact_reference()
        .map_err(|error| validation_error(py, &[error]))?;
    Ok(PyObservableRef {
        model_digest: reference.artifact().to_string(),
        id,
    })
}
