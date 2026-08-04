//! Private PyO3 adapter for the public `eqiora` Python package.
//!
//! This crate is a language boundary, not a second implementation of Eqiora
//! semantics. It depends only on the public Rust facade.

mod array;
mod cad_authored;
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
mod package;
mod realization;
mod result;
mod steady_stokes;
mod trajectory;
use eqiora::api::ModelDocument;
use eqiora::artifact::{ModelDecoderLimits, ModelEnvelope};
use eqiora::control::{CompileOutcomeV2, CompileRequestV2, execute_compile_v2};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

pub(crate) use error::diagnostic_error;
#[doc(hidden)]
pub use error::panic_boundary;
use error::{compatibility_error, control_diagnostic_error};
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

/// Compile exactly one Eqiora model through the canonical Rust pipeline.
#[pyfunction]
#[pyo3(signature = (source, *, filename="<memory>"))]
fn compile(py: Python<'_>, source: &str, filename: &str) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let request =
            CompileRequestV2::new("python.compile", filename.to_owned(), source.to_owned())
                .map_err(|diagnostic| control_diagnostic_error(py, &[diagnostic]))?;
        execute_compile_request(py, request)
    })
}

/// Replay one canonical artifact through the current Model contract.
///
/// Self-contained Models receive immediate whole-program admission. A Model
/// whose typed definitions reference external Geometry retains exact artifact
/// identity and defers semantic admission until an operation supplies that
/// geometry closure.
#[pyfunction]
fn replay(py: Python<'_>, data: &[u8]) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let data = data.to_vec();
        let replayed = py.detach(move || {
            let artifact = ModelEnvelope::from_json(&data, ModelDecoderLimits::default())
                .map_err(|diagnostic| vec![diagnostic])?;
            let requires_geometry = artifact
                .requires_geometry_admission()
                .map_err(|diagnostic| vec![diagnostic])?;
            if requires_geometry {
                Ok((None, artifact))
            } else {
                ModelDocument::replay(&data).map(|document| (Some(document), artifact))
            }
        });
        replayed
            .map_err(|diagnostics| compatibility_error(py, &diagnostics))
            .and_then(|(document, artifact)| match document {
                Some(document) => PyModel::from_document(py, document),
                None => PyModel::from_artifact(py, artifact),
            })
    })
}

fn execute_compile_request(py: Python<'_>, request: CompileRequestV2) -> PyResult<PyModel> {
    let execution = py.detach(move || execute_compile_v2(&request));
    let (response, document) = execution.into_parts();
    match response.outcome() {
        CompileOutcomeV2::Accepted { .. } => document
            .map(|document| PyModel::from_document(py, document))
            .transpose()?
            .ok_or_else(|| {
                error::internal_error(
                    py,
                    "compile/check accepted a model without returning its immutable document",
                )
            }),
        CompileOutcomeV2::Rejected { diagnostics } => {
            Err(control_diagnostic_error(py, diagnostics))
        }
    }
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
    differentiation::register(module)?;
    elasticity::register(module)?;
    jax_ffi::register_module(module)?;
    result::register(module)?;
    execution::register(module)?;
    fsi::register(module)?;
    geometry::register(module)?;
    meshing::register(module)?;
    modeling::register(module)?;
    realization::register(module)?;
    steady_stokes::register(module)?;
    trajectory::register(module)?;
    module.add_function(wrap_pyfunction!(compile, module)?)?;
    module.add_function(wrap_pyfunction!(replay, module)?)?;
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
