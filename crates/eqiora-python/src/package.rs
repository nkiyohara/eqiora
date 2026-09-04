//! Exact offline Model Package compilation through the existing Python Model.

use std::path::PathBuf;

use eqiora::Diagnostic;
use eqiora::api::ModelDocument;
use eqiora::api::package::{PackageCompilationError, PackagedModelDocument};
use eqiora::artifact::ModelArtifactReference;
use eqiora::diagnostic::codes;
use eqiora::package::{DirectoryPackageStore, PackageCompilationRecordV2, ResolutionRecordV1};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyModule, PyString, PyTuple};

use crate::error::{compatibility_error, diagnostic_error, panic_boundary};
use crate::extract_parameter_values;
use crate::geometry::PyGeometry;
use crate::model::PyModel;
use crate::python_distribution_version;

const STRUCTURAL_CONFORMANCE_PROFILE: &str = "eqiora.package.structural-conformance-v1";

type PackageFacts = (String, String, String, String);
type ConformanceFacts = (
    String,
    String,
    String,
    String,
    u32,
    u32,
    u32,
    PackageFacts,
    Vec<PackageFacts>,
    String,
    String,
    String,
    String,
    u64,
    String,
    bool,
);

enum CompilePackageFailure {
    Compatibility(Diagnostic),
    Diagnostics(Vec<Diagnostic>),
}

/// Resolve and lock one local package project into one store.
#[pyfunction]
fn resolve_local_project(
    py: Python<'_>,
    project_root: &Bound<'_, PyAny>,
    store_root: &Bound<'_, PyAny>,
) -> PyResult<Py<PyBytes>> {
    update_local_project(py, project_root, store_root, LocalDependencyEdit::Lock)
}

/// Add or replace one exact local dependency and update the project lock.
#[pyfunction]
#[pyo3(signature = (project_root, store_root, name, *, version, path))]
fn add_local_dependency(
    py: Python<'_>,
    project_root: &Bound<'_, PyAny>,
    store_root: &Bound<'_, PyAny>,
    name: String,
    version: String,
    path: String,
) -> PyResult<Py<PyBytes>> {
    update_local_project(
        py,
        project_root,
        store_root,
        LocalDependencyEdit::Add {
            name,
            version,
            path,
        },
    )
}

/// Remove one direct dependency and update the project lock.
#[pyfunction]
fn remove_local_dependency(
    py: Python<'_>,
    project_root: &Bound<'_, PyAny>,
    store_root: &Bound<'_, PyAny>,
    name: String,
) -> PyResult<Py<PyBytes>> {
    update_local_project(
        py,
        project_root,
        store_root,
        LocalDependencyEdit::Remove(name),
    )
}

enum LocalDependencyEdit {
    Lock,
    Add {
        name: String,
        version: String,
        path: String,
    },
    Remove(String),
}

fn update_local_project(
    py: Python<'_>,
    project_root: &Bound<'_, PyAny>,
    store_root: &Bound<'_, PyAny>,
    edit: LocalDependencyEdit,
) -> PyResult<Py<PyBytes>> {
    panic_boundary(py, || {
        let project_root = unicode_path(py, project_root)?;
        let store_root = unicode_path(py, store_root)?;
        let resolution = py
            .detach(move || match edit {
                LocalDependencyEdit::Lock => {
                    PackagedModelDocument::resolve_local_package_project_v1(
                        project_root,
                        store_root,
                    )
                }
                LocalDependencyEdit::Add {
                    name,
                    version,
                    path,
                } => PackagedModelDocument::add_local_package_dependency_v1(
                    project_root,
                    store_root,
                    &name,
                    &version,
                    &path,
                ),
                LocalDependencyEdit::Remove(name) => {
                    PackagedModelDocument::remove_local_package_dependency_v1(
                        project_root,
                        store_root,
                        &name,
                    )
                }
            })
            .map_err(|error| {
                compatibility_error(
                    py,
                    &[Diagnostic::error(
                        codes::INVALID_ARTIFACT,
                        format!("local package resolution rejected: {error}"),
                    )],
                )
            })?;
        let bytes = resolution.canonical_json().map_err(|error| {
            compatibility_error(
                py,
                &[Diagnostic::error(
                    codes::INVALID_ARTIFACT,
                    format!("local package resolution rejected: {error}"),
                )],
            )
        })?;
        Ok(PyBytes::new(py, &bytes).unbind())
    })
}

/// Compile one root-local or imported Model, or one Geometry-bound Component.
#[pyfunction]
#[pyo3(signature = (store_root, resolution, *, entry_model=None, geometry=None, component=None, parameters=None))]
fn compile_package(
    py: Python<'_>,
    store_root: &Bound<'_, PyAny>,
    resolution: &Bound<'_, PyAny>,
    entry_model: Option<&str>,
    geometry: Option<Py<PyGeometry>>,
    component: Option<&str>,
    parameters: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let store_root = unicode_path(py, store_root)?;
        let resolution = resolution.cast::<PyBytes>()?.as_bytes().to_vec();
        let compiled = match (entry_model, geometry.as_ref(), component) {
            (Some(entry_model), None, None) if parameters.is_none() => {
                let entry_model = entry_model.to_owned();
                py.detach(move || compile_package_model_native(store_root, resolution, entry_model))
            }
            (None, Some(geometry), Some(component)) => {
                let native_geometry = geometry.borrow(py).geometry().clone();
                let component = component.to_owned();
                let parameter_values = extract_parameter_values(parameters)?;
                py.detach(move || {
                    compile_package_component_native(
                        store_root,
                        resolution,
                        component,
                        native_geometry,
                        parameter_values,
                    )
                })
            }
            _ => {
                return Err(PyTypeError::new_err(
                    "compile_package requires exactly entry_model=, or both geometry= and component=; parameters= is valid only with Geometry-bound Component compilation",
                ));
            }
        };
        match compiled {
            Ok(packaged) => match geometry {
                Some(geometry) => PyModel::from_packaged_with_geometry(py, packaged, geometry),
                None => PyModel::from_packaged(py, packaged),
            },
            Err(CompilePackageFailure::Compatibility(diagnostic)) => {
                Err(compatibility_error(py, &[diagnostic]))
            }
            Err(CompilePackageFailure::Diagnostics(diagnostics)) => {
                Err(diagnostic_error(py, &diagnostics))
            }
        }
    })
}

pub(crate) fn unicode_path(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let path = py.import("os")?.getattr("fspath")?.call1((value,))?;
    let path = path.cast::<PyString>()?.to_str()?;
    Ok(PathBuf::from(path))
}

fn compile_package_model_native(
    store_root: PathBuf,
    resolution_bytes: Vec<u8>,
    entry_model: String,
) -> Result<PackagedModelDocument, CompilePackageFailure> {
    let (store, resolution) = open_locked_package(store_root, resolution_bytes)?;
    PackagedModelDocument::compile_locked(&store, &resolution, &entry_model)
        .map_err(map_package_compilation_error)
}

fn compile_package_component_native(
    store_root: PathBuf,
    resolution_bytes: Vec<u8>,
    component: String,
    geometry: eqiora::geometry::CanonicalGeometryV1,
    parameters: Vec<(String, f64)>,
) -> Result<PackagedModelDocument, CompilePackageFailure> {
    let (store, resolution) = open_locked_package(store_root, resolution_bytes)?;
    let parameters = parameters
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect::<Vec<_>>();
    PackagedModelDocument::compile_locked_with_geometry(
        &store,
        &resolution,
        &component,
        &geometry,
        &parameters,
    )
    .map_err(map_package_compilation_error)
}

fn open_locked_package(
    store_root: PathBuf,
    resolution_bytes: Vec<u8>,
) -> Result<(DirectoryPackageStore, ResolutionRecordV1), CompilePackageFailure> {
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
    Ok((store, resolution))
}

/// Check one exact locked package closure through deterministic replay.
#[pyfunction(name = "_check_package_conformance")]
#[pyo3(signature = (store_root, resolution, *, entry_model, profile))]
fn check_package_conformance(
    py: Python<'_>,
    store_root: &Bound<'_, PyAny>,
    resolution: &Bound<'_, PyAny>,
    entry_model: &str,
    profile: &str,
) -> PyResult<Py<PyTuple>> {
    panic_boundary(py, || {
        if profile != STRUCTURAL_CONFORMANCE_PROFILE {
            return Err(compatibility_error(
                py,
                &[Diagnostic::error(
                    codes::INVALID_ARTIFACT,
                    format!("unsupported package conformance profile `{profile}`"),
                )],
            ));
        }

        let resolution_bytes = resolution.cast::<PyBytes>()?.as_bytes().to_vec();
        let resolution = py.detach(move || decode_conformance_resolution(resolution_bytes));
        let resolution = resolution.map_err(|failure| python_failure(py, failure))?;
        let store_root = unicode_path(py, store_root)?;
        let entry_model = entry_model.to_owned();
        let profile = profile.to_owned();
        let checked = py.detach(move || {
            check_package_conformance_native(store_root, resolution, entry_model, profile)
        });
        let facts = checked.map_err(|failure| python_failure(py, failure))?;
        conformance_facts_tuple(py, facts)
    })
}

fn decode_conformance_resolution(
    resolution_bytes: Vec<u8>,
) -> Result<ResolutionRecordV1, CompilePackageFailure> {
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
    Ok(resolution)
}

fn check_package_conformance_native(
    store_root: PathBuf,
    resolution: ResolutionRecordV1,
    entry_model: String,
    profile: String,
) -> Result<ConformanceFacts, CompilePackageFailure> {
    let store = DirectoryPackageStore::open_ambient(store_root)
        .map_err(|error| compatibility_failure(format!("package store rejected: {error}")))?;
    let first = PackagedModelDocument::compile_locked(&store, &resolution, &entry_model)
        .map_err(map_package_compilation_error)?;
    let second = PackagedModelDocument::compile_locked(&store, &resolution, &entry_model)
        .map_err(map_package_compilation_error)?;

    let first_compilation = replay_compilation(first.compilation(), &resolution)?;
    let second_compilation = replay_compilation(second.compilation(), &resolution)?;
    let first_model = canonical_model_facts(first.model())?;
    let second_model = canonical_model_facts(second.model())?;
    let replayed_first = replay_model_facts(&first_model.0)?;
    let replayed_second = replay_model_facts(&second_model.0)?;
    if first_compilation != second_compilation
        || first_model != second_model
        || replayed_first != first_model
        || replayed_second != second_model
    {
        return Err(compatibility_failure(
            "locked package compilation did not reproduce exact replay identities",
        ));
    }

    let record = first.compilation();
    let packages = record
        .packages()
        .iter()
        .map(package_facts)
        .collect::<Vec<_>>();
    let root_package = record
        .packages()
        .iter()
        .find(|package| package.package() == record.root())
        .map(package_facts)
        .ok_or_else(|| compatibility_failure("package inventory does not contain its root"))?;
    let toolchain = record.toolchain();
    let reference = &first_model.1;
    let distribution_version = python_distribution_version(eqiora::VERSION).ok_or_else(|| {
        compatibility_failure("Eqiora release identity has no admitted Python version mapping")
    })?;

    Ok((
        profile,
        distribution_version,
        toolchain.compiler().as_str().to_owned(),
        toolchain.compiler_version().as_str().to_owned(),
        toolchain.semantic_canonicalization_version(),
        toolchain.source_bundle_version(),
        toolchain.resolution_version(),
        root_package,
        packages,
        entry_model,
        record.resolution_digest().to_hex(),
        first_compilation.2.clone(),
        reference.model().ulid().to_string(),
        reference.semantic_revision().get(),
        first_model.2,
        true,
    ))
}

fn replay_compilation(
    record: &PackageCompilationRecordV2,
    resolution: &ResolutionRecordV1,
) -> Result<(Vec<u8>, PackageCompilationRecordV2, String), CompilePackageFailure> {
    let bytes = record
        .canonical_json()
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    let replayed = PackageCompilationRecordV2::from_json(&bytes)
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    replayed
        .validate_against(resolution)
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    let replayed_bytes = replayed
        .canonical_json()
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    let identity = record
        .digest()
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    let replayed_identity = replayed
        .digest()
        .map_err(|error| compatibility_failure(format!("compilation replay rejected: {error}")))?;
    if replayed_bytes != bytes || &replayed != record || replayed_identity != identity {
        return Err(compatibility_failure(
            "package compilation record changed during exact replay",
        ));
    }
    Ok((bytes, replayed, identity.to_hex()))
}

fn canonical_model_facts(
    document: &ModelDocument,
) -> Result<(Vec<u8>, ModelArtifactReference, String), CompilePackageFailure> {
    let bytes = document.canonical_json().map_err(|error| {
        compatibility_failure(format!("canonical Model replay rejected: {error}"))
    })?;
    let reference = document.artifact_reference().map_err(|error| {
        compatibility_failure(format!("canonical Model replay rejected: {error}"))
    })?;
    let identity = document.digest().map_err(|error| {
        compatibility_failure(format!("canonical Model replay rejected: {error}"))
    })?;
    Ok((bytes, reference, identity))
}

fn replay_model_facts(
    bytes: &[u8],
) -> Result<(Vec<u8>, ModelArtifactReference, String), CompilePackageFailure> {
    let replayed = ModelDocument::replay(bytes)
        .map_err(|_| compatibility_failure("canonical Model replay rejected"))?;
    canonical_model_facts(&replayed)
}

fn package_facts(package: &eqiora::package::CompilationPackageV1) -> PackageFacts {
    let identity = package.package();
    (
        identity.name.as_str().to_owned(),
        identity.version.as_str().to_owned(),
        identity.semantic_digest.to_hex(),
        package.source_digest().to_hex(),
    )
}

fn conformance_facts_tuple(py: Python<'_>, facts: ConformanceFacts) -> PyResult<Py<PyTuple>> {
    let (
        profile,
        distribution_version,
        compiler,
        compiler_version,
        semantic_version,
        source_version,
        resolution_version,
        root_package,
        packages,
        entry_model,
        resolution_identity,
        compilation_identity,
        object_id,
        revision,
        canonical_identity,
        replay_agreement,
    ) = facts;
    let packages = PyTuple::new(py, packages)?.into_any().unbind();
    let items = [
        profile.into_py_any(py)?,
        distribution_version.into_py_any(py)?,
        compiler.into_py_any(py)?,
        compiler_version.into_py_any(py)?,
        semantic_version.into_py_any(py)?,
        source_version.into_py_any(py)?,
        resolution_version.into_py_any(py)?,
        root_package.into_py_any(py)?,
        packages,
        entry_model.into_py_any(py)?,
        resolution_identity.into_py_any(py)?,
        compilation_identity.into_py_any(py)?,
        object_id.into_py_any(py)?,
        revision.into_py_any(py)?,
        canonical_identity.into_py_any(py)?,
        replay_agreement.into_py_any(py)?,
    ];
    Ok(PyTuple::new(py, items)?.unbind())
}

fn python_failure(py: Python<'_>, failure: CompilePackageFailure) -> PyErr {
    match failure {
        CompilePackageFailure::Compatibility(diagnostic) => compatibility_error(py, &[diagnostic]),
        CompilePackageFailure::Diagnostics(diagnostics) => diagnostic_error(py, &diagnostics),
    }
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
    module.add_function(wrap_pyfunction!(resolve_local_project, module)?)?;
    module.add_function(wrap_pyfunction!(add_local_dependency, module)?)?;
    module.add_function(wrap_pyfunction!(remove_local_dependency, module)?)?;
    module.add_function(wrap_pyfunction!(compile_package, module)?)?;
    module.add_function(wrap_pyfunction!(check_package_conformance, module)?)?;
    Ok(())
}
