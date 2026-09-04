//! Vendoring of exact standard-package source closures shipped in the wheel.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleRoleV1, NormalizedRelativePath,
    PackageReleaseV1, SourceFileV1, prepare_package_release_v1,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::error::{compatibility_error, panic_boundary};

type VendoredFacts = (String, String, String, String, String);

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

#[pyfunction(name = "_vendor_standard_package")]
#[pyo3(signature = (project_root, package, *, destination="packages"))]
fn vendor_standard_package(
    py: Python<'_>,
    project_root: &Bound<'_, PyAny>,
    package: &str,
    destination: &str,
) -> PyResult<Vec<VendoredFacts>> {
    panic_boundary(py, || {
        let project_root = crate::package::unicode_path(py, project_root)?;
        let package = package.to_owned();
        let destination = destination.to_owned();
        py.detach(move || vendor_native(&project_root, &package, &destination))
            .map_err(|message| {
                compatibility_error(
                    py,
                    &[Diagnostic::error(
                        codes::INVALID_ARTIFACT,
                        format!("standard package vendoring rejected: {message}"),
                    )],
                )
            })
    })
}

fn vendor_native(
    project_root: &Path,
    package: &str,
    destination: &str,
) -> Result<Vec<VendoredFacts>, String> {
    let destination = NormalizedRelativePath::parse(destination)
        .map_err(|error| format!("destination is invalid: {error}"))?;
    require_plain_directory(project_root, "project root")?;

    let releases = standard_closure(package)?;
    let destination_root = ensure_directory_tree(project_root, destination.as_str())?;

    for (sources, _) in &releases {
        let manifest = sources.manifest();
        let package_parent = ensure_directory_tree(&destination_root, manifest.name().as_str())?;
        let target = package_parent.join(manifest.version().as_str());
        if target.exists() {
            validate_existing_package(&target, sources)?;
        }
    }

    let mut facts = Vec::with_capacity(releases.len());
    for (sources, release) in releases {
        let identity = release
            .package_identity()
            .map_err(|error| format!("prepared package identity is invalid: {error}"))?;
        let source_digest = release
            .source_digest()
            .map_err(|error| format!("prepared package source identity is invalid: {error}"))?;
        let relative = format!(
            "{}/{}/{}",
            destination.as_str(),
            identity.name.as_str(),
            identity.version.as_str()
        );
        let target = project_root.join(&relative);
        if !target.exists() {
            publish_package(&target, &sources)?;
        }
        facts.push((
            identity.name.as_str().to_owned(),
            identity.version.as_str().to_owned(),
            identity.semantic_digest.to_hex(),
            source_digest.to_hex(),
            relative,
        ));
    }
    Ok(facts)
}

fn standard_closure(
    package: &str,
) -> Result<Vec<(AuthorPackageSourcesV1, PackageReleaseV1)>, String> {
    match package {
        "Eqiora.Fluid@0.2.0" => {
            let mechanics_sources = mechanics_sources()?;
            let mechanics = prepare_package_release_v1(mechanics_sources.clone(), &[])
                .map_err(|error| format!("bundled mechanics package is invalid: {error}"))?;
            let fluid_sources = fluid_sources()?;
            let fluid =
                prepare_package_release_v1(fluid_sources.clone(), std::slice::from_ref(&mechanics))
                    .map_err(|error| format!("bundled fluid package is invalid: {error}"))?;
            Ok(vec![(mechanics_sources, mechanics), (fluid_sources, fluid)])
        }
        "Eqiora.Solid@0.2.0" => {
            let solid_sources = solid_sources()?;
            let solid = prepare_package_release_v1(solid_sources.clone(), &[])
                .map_err(|error| format!("bundled solid package is invalid: {error}"))?;
            Ok(vec![(solid_sources, solid)])
        }
        _ => Err(format!(
            "unsupported exact package {package:?}; expected Eqiora.Fluid@0.2.0 or Eqiora.Solid@0.2.0"
        )),
    }
}

fn embedded_sources(
    manifest: &[u8],
    files: &[(&str, BundleRoleV1, &[u8])],
) -> Result<AuthorPackageSourcesV1, String> {
    let manifest = AuthorManifestV1::from_json(manifest)
        .map_err(|error| format!("bundled manifest is invalid: {error}"))?;
    let files = files
        .iter()
        .map(|(path, role, bytes)| {
            let path = NormalizedRelativePath::parse(*path)
                .map_err(|error| format!("bundled path is invalid: {error}"))?;
            Ok(SourceFileV1::new(path, *role, bytes.to_vec()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    AuthorPackageSourcesV1::new(manifest, files)
        .map_err(|error| format!("bundled package inventory is invalid: {error}"))
}

fn mechanics_sources() -> Result<AuthorPackageSourcesV1, String> {
    embedded_sources(
        include_bytes!("../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/package.json"),
        &[
            (
                "README.md",
                BundleRoleV1::Documentation,
                include_bytes!(
                    "../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/README.md"
                ),
            ),
            (
                "src/interfaces.eqi",
                BundleRoleV1::ModelSource,
                include_bytes!(
                    "../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/src/interfaces.eqi"
                ),
            ),
        ],
    )
}

fn fluid_sources() -> Result<AuthorPackageSourcesV1, String> {
    embedded_sources(
        include_bytes!("../../../packages/releases/Eqiora.Fluid/0.2.0/package.json"),
        &[
            (
                "README.md",
                BundleRoleV1::Documentation,
                include_bytes!("../../../packages/releases/Eqiora.Fluid/0.2.0/README.md"),
            ),
            (
                "src/fluid.eqi",
                BundleRoleV1::ModelSource,
                include_bytes!("../../../packages/releases/Eqiora.Fluid/0.2.0/src/fluid.eqi"),
            ),
        ],
    )
}

fn solid_sources() -> Result<AuthorPackageSourcesV1, String> {
    embedded_sources(
        include_bytes!("../../../packages/releases/Eqiora.Solid/0.2.0/package.json"),
        &[
            (
                "README.md",
                BundleRoleV1::Documentation,
                include_bytes!("../../../packages/releases/Eqiora.Solid/0.2.0/README.md"),
            ),
            (
                "src/solid.eqi",
                BundleRoleV1::ModelSource,
                include_bytes!("../../../packages/releases/Eqiora.Solid/0.2.0/src/solid.eqi"),
            ),
        ],
    )
}

fn require_plain_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{label} {} must be a directory, not a symbolic link",
            path.display()
        ));
    }
    Ok(())
}

fn ensure_directory_tree(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = NormalizedRelativePath::parse(relative)
        .map_err(|error| format!("directory path is invalid: {error}"))?;
    let mut current = root.to_path_buf();
    for segment in relative.as_str().split('/') {
        current.push(segment);
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("cannot create {}: {error}", current.display())),
        }
        require_plain_directory(&current, "vendor directory")?;
    }
    Ok(current)
}

fn expected_files(sources: &AuthorPackageSourcesV1) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut expected = BTreeMap::new();
    expected.insert(
        "package.json".to_owned(),
        sources
            .manifest()
            .canonical_json()
            .map_err(|error| format!("cannot encode bundled manifest: {error}"))?,
    );
    for file in sources.files() {
        expected.insert(file.path().as_str().to_owned(), file.bytes().to_vec());
    }
    Ok(expected)
}

fn validate_existing_package(root: &Path, sources: &AuthorPackageSourcesV1) -> Result<(), String> {
    require_plain_directory(root, "existing package")?;
    let expected = expected_files(sources)?;
    let actual = collect_files(root)?;
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected_paths {
        return Err(format!(
            "existing package {} has a different file inventory",
            root.display()
        ));
    }
    for (relative, bytes) in expected {
        let path = root.join(&relative);
        let actual = fs::read(&path).map_err(|error| {
            format!(
                "cannot read existing package file {}: {error}",
                path.display()
            )
        })?;
        if actual != bytes {
            return Err(format!(
                "existing package file {} has different bytes",
                path.display()
            ));
        }
    }
    Ok(())
}

fn collect_files(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut files = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot inspect {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| format!("cannot inspect vendor entry: {error}"))?;
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
            if kind.is_symlink() {
                return Err(format!(
                    "existing package contains symbolic link {}",
                    entry.path().display()
                ));
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| "vendor entry escaped its package root".to_owned())?
                    .to_str()
                    .ok_or_else(|| "vendor entry path is not UTF-8".to_owned())?
                    .replace(std::path::MAIN_SEPARATOR, "/");
                files.insert(relative);
            } else {
                return Err(format!(
                    "existing package contains unsupported entry {}",
                    entry.path().display()
                ));
            }
        }
    }
    Ok(files)
}

fn publish_package(target: &Path, sources: &AuthorPackageSourcesV1) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "vendor target has no parent directory".to_owned())?;
    let sequence = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
    let stage = parent.join(format!(".eqiora-vendor-{}-{sequence}", std::process::id()));
    fs::create_dir(&stage)
        .map_err(|error| format!("cannot create vendor staging directory: {error}"))?;
    let result = (|| {
        for (relative, bytes) in expected_files(sources)? {
            let path = stage.join(&relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create vendor package directory: {error}"))?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| format!("cannot create vendor package file: {error}"))?;
            file.write_all(&bytes)
                .map_err(|error| format!("cannot write vendor package file: {error}"))?;
            file.sync_all()
                .map_err(|error| format!("cannot synchronize vendor package file: {error}"))?;
        }
        fs::rename(&stage, target).map_err(|error| {
            format!(
                "cannot publish vendor package {}: {error}",
                target.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(vendor_standard_package, module)?)?;
    Ok(())
}
