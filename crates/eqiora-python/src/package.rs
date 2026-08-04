//! Exact offline Model Package compilation through the existing Python Model.

use std::path::PathBuf;

use eqiora::Diagnostic;
use eqiora::api::package::{PackageCompilationError, PackagedModelDocument};
use eqiora::diagnostic::codes;
use eqiora::package::{DirectoryPackageStore, ResolutionRecordV1};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule, PyString};

use crate::error::{compatibility_error, diagnostic_error, panic_boundary};
use crate::model::PyModel;

enum CompilePackageFailure {
    Compatibility(Diagnostic),
    Diagnostics(Vec<Diagnostic>),
}

/// Compile exactly one root-local Model from a caller-selected locked package store.
#[pyfunction]
#[pyo3(signature = (store_root, resolution, *, entry_model))]
fn compile_package(
    py: Python<'_>,
    store_root: &Bound<'_, PyAny>,
    resolution: &Bound<'_, PyAny>,
    entry_model: &str,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let store_root = unicode_store_root(py, store_root)?;
        let resolution = resolution.cast::<PyBytes>()?.as_bytes().to_vec();
        let entry_model = entry_model.to_owned();
        let compiled =
            py.detach(move || compile_package_native(store_root, resolution, entry_model));
        match compiled {
            Ok(packaged) => PyModel::from_packaged(py, packaged),
            Err(CompilePackageFailure::Compatibility(diagnostic)) => {
                Err(compatibility_error(py, &[diagnostic]))
            }
            Err(CompilePackageFailure::Diagnostics(diagnostics)) => {
                Err(diagnostic_error(py, &diagnostics))
            }
        }
    })
}

fn unicode_store_root(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let path = py.import("os")?.getattr("fspath")?.call1((value,))?;
    let path = path.cast::<PyString>()?.to_str()?;
    Ok(PathBuf::from(path))
}

fn compile_package_native(
    store_root: PathBuf,
    resolution_bytes: Vec<u8>,
    entry_model: String,
) -> Result<PackagedModelDocument, CompilePackageFailure> {
    let store = DirectoryPackageStore::open_ambient(store_root)
        .map_err(|error| compatibility_failure(format!("package store rejected: {error}")))?;
    let resolution = ResolutionRecordV1::from_json(&resolution_bytes)
        .map_err(|error| compatibility_failure(format!("resolution record rejected: {error}")))?;
    let canonical = resolution
        .canonical_json()
        .map_err(|error| compatibility_failure(format!("resolution record rejected: {error}")))?;
    if canonical != resolution_bytes {
        return Err(compatibility_failure(
            "resolution bytes are not the exact canonical ResolutionRecordV1 wire",
        ));
    }
    PackagedModelDocument::compile_locked(&store, &resolution, &entry_model)
        .map_err(map_package_compilation_error)
}

fn map_package_compilation_error(error: PackageCompilationError) -> CompilePackageFailure {
    match error {
        PackageCompilationError::Diagnostics(diagnostics) => {
            CompilePackageFailure::Diagnostics(diagnostics)
        }
        other => compatibility_failure(format!("locked package compilation rejected: {other}")),
    }
}

fn compatibility_failure(message: impl Into<String>) -> CompilePackageFailure {
    CompilePackageFailure::Compatibility(Diagnostic::error(codes::INVALID_ARTIFACT, message))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_package, module)?)?;
    Ok(())
}
