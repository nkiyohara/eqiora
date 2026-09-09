//! Immutable projections of the native mathematical rendering owner.
use super::{PyModel, parse_profile};
use eqiora::api::{MathReference, MathRendering};
use eqiora::kernel::KernelNode;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

/// Exact semantic targets shared by every presentation profile.
#[pyclass(
    name = "MathReference",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyMathReference {
    value: MathReference,
}

#[pymethods]
impl PyMathReference {
    fn __repr__(&self) -> String {
        format!(
            "MathReference(graph_id={:?}, role={:?}, declarations={}, operator={:?})",
            self.graph_id(),
            self.role(),
            self.value.declarations().len(),
            self.operator()
        )
    }
    #[getter]
    fn graph_id(&self) -> Option<String> {
        self.value.graph_id().map(|id| id.ulid().to_string())
    }
    #[getter]
    fn role(&self) -> Option<&'static str> {
        self.value.role().map(|role| role.name())
    }
    #[getter]
    fn declarations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(
            py,
            self.value.declarations().iter().map(ToString::to_string),
        )?
        .unbind())
    }
    #[getter]
    fn operator(&self) -> Option<String> {
        self.value.operator().map(|id| id.to_string())
    }
}

/// Selected rendering with plain/speech fallbacks and exact identity references.
#[pyclass(
    name = "MathRendering",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyMathRendering {
    pub(crate) value: MathRendering,
}

#[pymethods]
impl PyMathRendering {
    fn __repr__(&self) -> String {
        format!(
            "MathRendering(profile={:?}, references={}, used_fallback={})",
            self.profile(),
            self.value.references().len(),
            self.value.used_fallback()
        )
    }
    #[getter]
    fn text(&self) -> &str {
        self.value.text()
    }
    #[getter]
    fn plain(&self) -> &str {
        self.value.plain()
    }
    #[getter]
    fn speech(&self) -> &str {
        self.value.speech()
    }
    #[getter]
    fn used_fallback(&self) -> bool {
        self.value.used_fallback()
    }
    #[getter]
    fn profile(&self) -> &'static str {
        use eqiora::language::NotationProfile;
        match self.value.profile() {
            NotationProfile::Latex => "latex",
            NotationProfile::MathMl => "mathml",
            NotationProfile::Unicode => "unicode",
            NotationProfile::Plain => "plain",
            NotationProfile::Speech => "speech",
        }
    }
    #[getter]
    fn references(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let refs = self
            .value
            .references()
            .iter()
            .cloned()
            .map(|value| Py::new(py, PyMathReference { value }))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyTuple::new(py, refs)?.unbind())
    }
}

pub(super) fn equations(
    model: &PyModel,
    py: Python<'_>,
    relation: &str,
    profile: &str,
) -> PyResult<Py<PyTuple>> {
    let profile = parse_profile(profile)?;
    let document = model
        .document()
        .map_err(|error| crate::error::diagnostic_error(py, &[error]))?;
    let alias = document.aliases().get(relation).copied();
    let id = document
        .program()
        .nodes()
        .find_map(|node| match node {
            KernelNode::Relation(value)
                if alias == Some(value.id().erase())
                    || value.id().ulid().to_string() == relation =>
            {
                Some(value.id().erase())
            }
            _ => None,
        })
        .ok_or_else(|| PyValueError::new_err("selection is not an exact Relation in this Model"))?;
    project(py, document.render_equations(id, profile))
}

pub(super) fn formulations(
    model: &PyModel,
    py: Python<'_>,
    profile: &str,
) -> PyResult<Py<PyTuple>> {
    let profile = parse_profile(profile)?;
    let document = model
        .document()
        .map_err(|error| crate::error::diagnostic_error(py, &[error]))?;
    project(py, document.render_formulations(profile))
}

fn project(
    py: Python<'_>,
    values: Result<Vec<MathRendering>, eqiora::Diagnostic>,
) -> PyResult<Py<PyTuple>> {
    let values = values.map_err(|error| crate::error::diagnostic_error(py, &[error]))?;
    let values = values
        .into_iter()
        .map(|value| Py::new(py, PyMathRendering { value }))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, values)?.unbind())
}
