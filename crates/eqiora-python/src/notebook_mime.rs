//! Shared filtering for Eqiora's rich Notebook MIME hooks.

use std::collections::HashSet;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

pub(crate) const TEXT_MIME: &str = "text/plain";
pub(crate) const WIDGET_MIME: &str = "application/vnd.jupyter.widget-view+json";

pub(crate) fn select_mime_types(
    py: Python<'_>,
    include: Option<&Bound<'_, PyAny>>,
    exclude: Option<&Bound<'_, PyAny>>,
) -> PyResult<HashSet<&'static str>> {
    let mut selected = HashSet::from([TEXT_MIME, WIDGET_MIME]);
    if let Some(include) = include {
        let include = mime_collection(py, include, "include")?;
        selected.retain(|mime| include.contains(*mime));
    }
    if let Some(exclude) = exclude {
        let exclude = mime_collection(py, exclude, "exclude")?;
        selected.retain(|mime| !exclude.contains(*mime));
    }
    Ok(selected)
}

fn mime_collection(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<HashSet<String>> {
    let collection = py.import("collections.abc")?.getattr("Collection")?;
    if !value.is_instance(&collection)? {
        return Err(PyTypeError::new_err(format!(
            "{name} must be None or a collection of MIME strings"
        )));
    }
    let mut members = HashSet::new();
    for member in value.try_iter()? {
        let member = member?.extract::<String>().map_err(|_| {
            PyTypeError::new_err(format!(
                "{name} must be None or a collection of MIME strings"
            ))
        })?;
        members.insert(member);
    }
    Ok(members)
}
