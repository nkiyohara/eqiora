//! Private PyO3 adapter for the public `eqiora` Python package.
//!
//! This crate is a language boundary, not a second implementation of Eqiora
//! semantics. It depends only on the public Rust facade.

mod array;
mod cad_authored;
mod common_plan;
mod differentiation;
mod elasticity;
mod error;
mod execution;
mod fsi_evidence;
mod geometry;
mod jax_ffi;
mod matrix;
mod meshing;
mod model;
mod model_io;
mod modeling;
mod package;
mod planar_operation;
mod realization;
mod result;
mod steady_stokes;
mod trajectory;
mod viewer;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use eqiora::api::ModelDocument;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyFloat, PyInt, PyModule, PyString};

pub(crate) use error::diagnostic_error;
#[doc(hidden)]
pub use error::panic_boundary;
use error::python_compile_admission_error;
use geometry::PyGeometry;
use model::PyModel;

const MAX_PYTHON_COMPILE_FILENAME_BYTES: usize = 4_096;
const MAX_PYTHON_COMPILE_SOURCE_BYTES: usize = 8 * 1_024 * 1_024;

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

/// Compile exactly one Eqiora source through the canonical Rust pipeline.
#[pyfunction]
#[pyo3(signature = (*, path=None, source=None, filename=None, geometry=None, parameters=None, component=None))]
#[allow(clippy::too_many_arguments)]
fn compile(
    py: Python<'_>,
    path: Option<&Bound<'_, PyAny>>,
    source: Option<&str>,
    filename: Option<&str>,
    geometry: Option<Py<PyGeometry>>,
    parameters: Option<&Bound<'_, PyDict>>,
    component: Option<&str>,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let (filename, source) = admitted_compile_source(py, path, source, filename)?;
        validate_python_compile_input(py, &filename, &source)?;
        let parameter_values = extract_parameter_values(parameters)?;
        match geometry {
            None => {
                if parameters.is_some() || component.is_some() {
                    return Err(python_compile_admission_error(
                        py,
                        "parameters= and component= require geometry= for definitions-only source compilation",
                    ));
                }
                py.detach(move || ModelDocument::compile(&filename, &source))
                    .map_err(|diagnostics| diagnostic_error(py, &diagnostics))
                    .and_then(|document| PyModel::from_document(py, document))
            }
            Some(geometry) => {
                let native_geometry = geometry.borrow(py).geometry().clone();
                let component = component.map(str::to_owned);
                let compiled = py.detach(move || {
                    let parameters = parameter_values
                        .iter()
                        .map(|(name, value)| (name.as_str(), *value))
                        .collect::<Vec<_>>();
                    ModelDocument::compile_with_geometry(
                        &filename,
                        &source,
                        &native_geometry,
                        component.as_deref(),
                        &parameters,
                    )
                });
                compiled
                    .map_err(|diagnostics| diagnostic_error(py, &diagnostics))
                    .and_then(|document| {
                        PyModel::from_document_with_geometry(py, document, geometry)
                    })
            }
        }
    })
}

fn admitted_compile_source(
    py: Python<'_>,
    path: Option<&Bound<'_, PyAny>>,
    source: Option<&str>,
    filename: Option<&str>,
) -> PyResult<(String, String)> {
    match (path, source) {
        (None, None) | (Some(_), Some(_)) => Err(python_compile_admission_error(
            py,
            "compile requires exactly one of path= or source=",
        )),
        (None, Some(source)) => Ok((filename.unwrap_or("<memory>").to_owned(), source.to_owned())),
        (Some(_), None) if filename.is_some() => Err(python_compile_admission_error(
            py,
            "filename= is available only with source=",
        )),
        (Some(path), None) => {
            let path = py.import("os")?.getattr("fspath")?.call1((path,))?;
            let path = path.cast::<PyString>().map_err(|_| {
                PyTypeError::new_err("path must resolve to a Unicode filesystem path")
            })?;
            let path = PathBuf::from(path.to_str()?);
            let logical = path.to_string_lossy().into_owned();
            let read_path = path.clone();
            let bytes = py
                .detach(move || {
                    let file = File::open(&read_path)?;
                    let mut bytes = Vec::new();
                    file.take((MAX_PYTHON_COMPILE_SOURCE_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)?;
                    Ok::<_, std::io::Error>(bytes)
                })
                .map_err(|error| {
                    python_compile_admission_error(
                        py,
                        &format!("could not read compile path {logical:?}: {error}"),
                    )
                })?;
            if bytes.len() > MAX_PYTHON_COMPILE_SOURCE_BYTES {
                return Err(python_compile_admission_error(
                    py,
                    "source exceeds the 8388608-byte compile/check v2 limit",
                ));
            }
            let source = String::from_utf8(bytes).map_err(|_| {
                python_compile_admission_error(py, "compile path is not valid UTF-8 source")
            })?;
            Ok((logical, source))
        }
    }
}

pub(crate) fn extract_parameter_values(
    parameters: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<(String, f64)>> {
    let Some(parameters) = parameters else {
        return Ok(Vec::new());
    };
    let mut values = Vec::with_capacity(parameters.len());
    for (name, value) in parameters.iter() {
        let name = name
            .cast::<PyString>()
            .map_err(|_| PyTypeError::new_err("parameter names must be strings"))?
            .to_str()?
            .to_owned();
        if value.cast::<PyBool>().is_ok() {
            return Err(PyTypeError::new_err(format!(
                "parameter {name:?} must be a real coherent-SI scalar, not bool"
            )));
        }
        let scalar = if let Ok(value) = value.cast::<PyFloat>() {
            value.value()
        } else if value.cast::<PyInt>().is_ok() {
            value.extract::<f64>()?
        } else {
            return Err(PyTypeError::new_err(format!(
                "parameter {name:?} must be a real coherent-SI scalar"
            )));
        };
        if !scalar.is_finite() {
            return Err(PyTypeError::new_err(format!(
                "parameter {name:?} must be finite"
            )));
        }
        values.push((name, scalar));
    }
    values.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(values)
}

fn validate_python_compile_input(py: Python<'_>, filename: &str, source: &str) -> PyResult<()> {
    if filename.is_empty()
        || filename.chars().count() > MAX_PYTHON_COMPILE_FILENAME_BYTES
        || filename.len() > MAX_PYTHON_COMPILE_FILENAME_BYTES
        || filename.chars().any(char::is_control)
    {
        return Err(python_compile_admission_error(
            py,
            "source filename must contain 1 to 4096 non-control UTF-8 bytes",
        ));
    }
    if source.chars().count() > MAX_PYTHON_COMPILE_SOURCE_BYTES
        || source.len() > MAX_PYTHON_COMPILE_SOURCE_BYTES
    {
        return Err(python_compile_admission_error(
            py,
            "source exceeds the 8388608-byte compile/check v2 limit",
        ));
    }
    Ok(())
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
    model::register(module)?;
    package::register(module)?;
    array::register(module)?;
    cad_authored::register(module)?;
    common_plan::register(module)?;
    differentiation::register(module)?;
    elasticity::register(module)?;
    jax_ffi::register_module(module)?;
    result::register(module)?;
    execution::register(module)?;
    fsi_evidence::register(module)?;
    geometry::register(module)?;
    meshing::register(module)?;
    modeling::register(module)?;
    planar_operation::register(module)?;
    realization::register(module)?;
    steady_stokes::register(module)?;
    trajectory::register(module)?;
    viewer::register(module)?;
    module.add_function(wrap_pyfunction!(compile, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use eqiora::api::ModelDocument;

    use super::python_distribution_version;

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
    fn ordinary_python_authoring_and_replay_use_the_current_contract() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let bytes = document.canonical_json().unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("eqiora.model-envelope/v8"));
        let replayed = ModelDocument::replay(&bytes).unwrap();
        assert_eq!(replayed.canonical_json().unwrap(), bytes);
        assert_eq!(replayed.digest().unwrap(), document.digest().unwrap());
        let artifact = eqiora::artifact::ModelEnvelope::from_json(
            &bytes,
            eqiora::artifact::ModelDecoderLimits::default(),
        )
        .unwrap();
        assert!(!artifact.requires_geometry_admission().unwrap());
    }
}
