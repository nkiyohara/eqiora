//! Thin immutable projection of the compiler's full-Model declaration labels.
use super::PyModel;
use eqiora::compiler::ResolvedNotation;
use eqiora::language::NotationProfile;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

/// An immutable declaration label with exact full-Model occurrence identity.
#[pyclass(
    name = "QuantityLabel",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyQuantityLabel {
    entry: ResolvedNotation,
    profile: NotationProfile,
}

#[pymethods]
impl PyQuantityLabel {
    fn __repr__(&self) -> String {
        format!(
            "QuantityLabel(selector={:?}, role={:?}, label={:?})",
            self.entry.selector(),
            self.entry.identity().role().name(),
            self.entry.render(self.profile),
        )
    }

    #[getter]
    fn identity(&self) -> String {
        self.entry.identity().to_string()
    }
    #[getter]
    fn scope(&self) -> String {
        self.entry.identity().scope().to_string()
    }
    #[getter]
    fn occurrence(&self) -> String {
        self.entry.identity().occurrence().to_string()
    }
    #[getter]
    fn role(&self) -> &'static str {
        self.entry.identity().role().name()
    }
    #[getter]
    fn family_member(&self) -> Option<String> {
        self.entry
            .identity()
            .member()
            .map(|value| value.to_string())
    }
    #[getter]
    fn selector(&self) -> &str {
        self.entry.selector()
    }
    #[getter]
    fn graph_id(&self) -> Option<String> {
        self.entry.graph_id().map(|id| id.ulid().to_string())
    }
    #[getter]
    fn definition_span(&self) -> Option<(String, u32, u32)> {
        self.entry.definition_span().map(span)
    }
    #[getter]
    fn instance_span(&self) -> Option<(String, u32, u32)> {
        self.entry.instance_span().map(span)
    }
    #[getter]
    fn label(&self) -> String {
        self.entry.render(self.profile)
    }
}

fn span(value: &eqiora::diagnostic::Span) -> (String, u32, u32) {
    (value.file.clone(), value.start, value.end)
}

pub(super) fn project(
    model: &PyModel,
    py: Python<'_>,
    profile: &str,
    identities: Option<Vec<String>>,
) -> PyResult<Py<PyTuple>> {
    let profile = match profile {
        "rich" => NotationProfile::Rich,
        "plain" => NotationProfile::Plain,
        "speech" => NotationProfile::Speech,
        _ => {
            return Err(PyValueError::new_err(
                "notation profile must be rich, plain, or speech",
            ));
        }
    };
    let document = model
        .document()
        .map_err(|error| crate::error::diagnostic_error(py, &[error]))?;
    let requested = identities.map(|values| {
        values
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let mut found = std::collections::BTreeSet::new();
    let mut entries = Vec::new();
    for entry in document.notation().iter() {
        let identity = entry.identity().to_string();
        if requested
            .as_ref()
            .is_none_or(|values| values.contains(&identity))
        {
            found.insert(identity);
            entries.push(Py::new(
                py,
                PyQuantityLabel {
                    entry: entry.clone(),
                    profile,
                },
            )?);
        }
    }
    if requested.as_ref().is_some_and(|values| *values != found) {
        return Err(PyValueError::new_err(
            "notation subview contains an identity outside this Model scope",
        ));
    }
    Ok(PyTuple::new(py, entries)?.unbind())
}
