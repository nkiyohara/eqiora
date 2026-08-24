//! Common installed-Python projection of accepted Geometry-to-Mesh paths.

mod gmsh;
mod mesh;
mod plan;

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::error::validation_error;

pub(crate) use mesh::PyMesh;
use mesh::{generate, import_gmsh};
use plan::{PyMeshPlan, PyMeshRequest, resolve};

fn request_error(py: Python<'_>, message: impl Into<String>) -> PyErr {
    let diagnostic = Diagnostic::error(codes::INVALID_ARTIFACT, message);
    validation_error(py, std::slice::from_ref(&diagnostic))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyMeshRequest>()?;
    module.add_class::<PyMeshPlan>()?;
    module.add_class::<PyMesh>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    module.add_function(wrap_pyfunction!(generate, module)?)?;
    module.add_function(wrap_pyfunction!(import_gmsh, module)?)?;
    Ok(())
}
