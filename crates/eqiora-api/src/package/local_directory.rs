//! Exact local-directory package resolution and store preparation.

mod transaction;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};

use cap_fs_ext::{
    DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt, OpenOptionsSyncExt,
};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};
use eqiora_package::{
    BundleEntryV1, BundleRoleV1, DirectoryPackageInstaller, ExactResolver, ExactVersion,
    ModelPackageIdentityV1, NormalizedRelativePath, PackageDependencyV1, PackageDirectory,
    PackageManifestV1, PackageReleaseV1, PackageSourcesV1, QualifiedName, ResolutionRecordV1,
    SourceFileV1,
};
use serde::{Deserialize, Serialize};

use super::{PackagePreparationError, PackagedModelDocument, prepare_package_release_v1};

const MAX_LOCAL_PACKAGE_DIRECTORIES_V1: usize = 65_536;
const MAX_LOCAL_DEPENDENCY_DEPTH: usize = 64;
const MAX_PROJECT_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_PACKAGE_README_BYTES: usize = 1024 * 1024;
const MAX_PROJECT_LOCK_BYTES: usize = 1024 * 1024 * 1024;
const PROJECT_MANIFEST: &str = "eqiora.toml";
const PROJECT_LOCK: &str = "eqiora.lock";

type PackageKey = (QualifiedName, ExactVersion);

#[derive(Clone)]
struct LocalPackageSource {
    path: PathBuf,
    relative_path: PathBuf,
    source_root: NormalizedRelativePath,
    name: QualifiedName,
    version: ExactVersion,
    entry: String,
    files: Vec<SourceFileV1>,
    dependencies: BTreeMap<QualifiedName, PackageKey>,
}

#[derive(Clone)]
struct PreparedLocalPackage {
    release: PackageReleaseV1,
    dependencies: BTreeMap<ModelPackageIdentityV1, PackageReleaseV1>,
}

struct PreparedLocalProject {
    project: Dir,
    root: PreparedLocalPackage,
    packages: BTreeMap<PackageKey, LocalPackageSource>,
    prepared: BTreeMap<PackageKey, PreparedLocalPackage>,
}

#[derive(Default)]
struct LocalProjectOverrides {
    manifest: Option<LocalProjectManifest>,
    sources: BTreeMap<PathBuf, String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalProjectManifest {
    package: LocalPackageManifest,
    #[serde(default)]
    dependencies: BTreeMap<String, LocalProjectDependency>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalPackageManifest {
    name: String,
    version: String,
    #[serde(default = "default_source_root")]
    source: String,
    entry: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalProjectDependency {
    version: String,
    path: String,
}

fn default_source_root() -> String {
    "src".to_owned()
}

impl PackagedModelDocument {
    /// Maximum root-plus-dependency count admitted by local exact resolution.
    pub const MAX_LOCAL_PROJECT_PACKAGES_V1: usize = MAX_LOCAL_PACKAGE_DIRECTORIES_V1;

    /// Add or replace an exact local dependency and publish the validated manifest/lock pair.
    ///
    /// # Errors
    /// Returns an error if the request, dependency closure, installation, or publication fails.
    pub fn add_local_package_dependency_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        name: &str,
        version: &str,
        path: &str,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        update_local_package_project(project_root.into(), store_root.into(), |manifest| {
            manifest.dependencies.insert(
                name.to_owned(),
                LocalProjectDependency {
                    version: version.to_owned(),
                    path: path.to_owned(),
                },
            );
            Ok(true)
        })
    }

    /// Remove a direct dependency and publish the validated manifest/lock pair.
    ///
    /// # Errors
    /// Returns an error for an absent dependency or an invalid remaining project, or if
    /// installation or publication fails. Remaining source imports must still resolve.
    pub fn remove_local_package_dependency_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        name: &str,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        update_local_package_project(project_root.into(), store_root.into(), |manifest| {
            if manifest.dependencies.remove(name).is_none() {
                return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                    "direct dependency `{name}` does not exist"
                )));
            }
            Ok(true)
        })
    }

    /// Resolve one local package project, write its exact lock, and populate an offline store.
    ///
    /// `eqiora.toml` owns the root package name, exact version, source root, entry module, and
    /// direct local dependencies. Dependency manifests use the same format. Resolution discovers
    /// bounded `.eqi` inventories, generates closed package manifests, validates the exact graph,
    /// and atomically replaces `eqiora.lock` with canonical [`ResolutionRecordV1`] bytes.
    ///
    /// # Errors
    ///
    /// Returns a project, directory, graph, compiler-preparation, exact-identity, installation,
    /// or lock-publication failure. The previous lock remains usable on failure.
    pub fn resolve_local_package_project_v1<R, P>(
        project_root: R,
        store_root: P,
    ) -> Result<ResolutionRecordV1, PackagePreparationError>
    where
        R: Into<PathBuf>,
        P: Into<PathBuf>,
    {
        resolve_local_package_project_v1(project_root, store_root)
    }

    /// Read and validate the exact lock of one local package project without resolving it.
    ///
    /// # Errors
    ///
    /// Returns a bounded no-follow filesystem or canonical lock-contract failure.
    pub fn load_local_package_project_lock_v1(
        project_root: impl Into<PathBuf>,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let project_path = project_root.into();
        let project = open_project_root(&project_path)?;
        read_project_lock(&project)
    }
}

fn resolve_local_package_project_v1<R, P>(
    project_root: R,
    store_root: P,
) -> Result<ResolutionRecordV1, PackagePreparationError>
where
    R: Into<PathBuf>,
    P: Into<PathBuf>,
{
    update_local_package_project(project_root.into(), store_root.into(), |_| Ok(false))
}

fn update_local_package_project(
    project_path: PathBuf,
    store_root: PathBuf,
    edit: impl FnOnce(&mut LocalProjectManifest) -> Result<bool, PackagePreparationError>,
) -> Result<ResolutionRecordV1, PackagePreparationError> {
    let project = open_project_root(&project_path)?;
    let transaction_error = |error| {
        PackagePreparationError::LocalDirectoryGraph(format!("project transaction failed: {error}"))
    };
    let _guard = transaction::write_guard(&project).map_err(transaction_error)?;
    transaction::recover(&project).map_err(transaction_error)?;
    let mut manifest = transaction::read(&project, PROJECT_MANIFEST, MAX_PROJECT_MANIFEST_BYTES)
        .map_err(transaction_error)?;
    let mut candidate = read_project_manifest(&project)?;
    let changed = edit(&mut candidate)?;
    if changed {
        manifest = toml::to_string_pretty(&candidate)
            .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?
            .into_bytes();
    }
    let prepared = prepare_local_package_project(
        project,
        &project_path,
        LocalProjectOverrides {
            manifest: Some(candidate),
            sources: BTreeMap::new(),
        },
    )?;
    let dependencies = prepared
        .root
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&prepared.root.release, &dependencies)?;
    let installer = DirectoryPackageInstaller::open_ambient(&store_root).map_err(|source| {
        PackagePreparationError::Installation {
            store_root: store_root.clone(),
            source,
        }
    })?;
    for release in dependencies
        .iter()
        .chain(std::iter::once(&prepared.root.release))
    {
        let _receipt =
            installer
                .install(release)
                .map_err(|source| PackagePreparationError::Installation {
                    store_root: store_root.clone(),
                    source,
                })?;
    }
    transaction::commit(&prepared.project, &manifest, &resolution.canonical_json()?)
        .map_err(transaction_error)?;
    Ok(resolution)
}

pub(crate) fn analyze_local_package_editor_project_v1(
    version: u64,
    project_root: impl Into<PathBuf>,
    overrides: &BTreeMap<PathBuf, String>,
) -> Result<
    (
        crate::editor::EditorWorkspaceSnapshot,
        BTreeMap<String, PathBuf>,
    ),
    PackagePreparationError,
> {
    let project_path = project_root.into();
    let project = open_project_root(&project_path)?;
    let _guard = transaction::read_guard(&project)
        .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
    transaction::require_complete(&project)
        .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
    let prepared = prepare_local_package_project(
        project,
        &project_path,
        LocalProjectOverrides {
            manifest: None,
            sources: overrides.clone(),
        },
    )?;
    let dependencies = prepared
        .root
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&prepared.root.release, &dependencies)?;
    let resolved = ExactResolver
        .resolve_releases(&resolution, &prepared.root.release, &dependencies)
        .map_err(PackagePreparationError::Resolution)?;
    let namespaces =
        super::compilation_namespaces(&resolved).map_err(super::map_compilation_preparation)?;
    let input = super::compiler_input(&resolved, &namespaces)
        .map_err(super::map_compilation_preparation)?;
    let mut relative_paths = BTreeMap::new();
    for (key, package) in &prepared.packages {
        let identity = prepared
            .prepared
            .get(key)
            .expect("every local package was prepared")
            .release
            .package_identity()?;
        let namespace = namespaces
            .get(&identity)
            .expect("every resolved local package has a compilation namespace");
        for file in package
            .files
            .iter()
            .filter(|file| file.role() == BundleRoleV1::ModelSource)
        {
            let source = std::str::from_utf8(file.bytes()).map_err(|error| {
                PackagePreparationError::LocalDirectoryGraph(format!(
                    "model source `{}` is not UTF-8: {error}",
                    file.path()
                ))
            })?;
            let unit = eqiora_compiler::ResolvedSourceUnit::new(
                namespace.clone(),
                file.path().as_str(),
                source,
            )?;
            relative_paths.insert(
                unit.diagnostic_file(),
                package
                    .relative_path
                    .join(package.source_root.as_str())
                    .join(
                        file.path()
                            .as_str()
                            .strip_prefix("src/")
                            .expect("generated model source path"),
                    ),
            );
        }
    }
    Ok((
        crate::editor::EditorWorkspaceSnapshot::analyze_modules(version, input),
        relative_paths,
    ))
}

fn prepare_local_package_project(
    project: Dir,
    project_path: &Path,
    mut overrides: LocalProjectOverrides,
) -> Result<PreparedLocalProject, PackagePreparationError> {
    let mut packages = BTreeMap::<PackageKey, LocalPackageSource>::new();
    let mut indexed_paths = BTreeMap::new();
    let root_key = load_local_package(
        &project,
        project_path,
        PathBuf::new(),
        0,
        &mut overrides,
        &mut packages,
        &mut indexed_paths,
    )?;
    let mut visiting = BTreeSet::new();
    let mut prepared = BTreeMap::new();
    let root_package = prepare_local_package(&root_key, &packages, &mut visiting, &mut prepared)?;
    if let Some(path) = overrides.sources.keys().next() {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "editor override `{}` is not a declared model source",
            path.display()
        )));
    }
    Ok(PreparedLocalProject {
        project,
        root: root_package,
        packages,
        prepared,
    })
}

fn load_local_package(
    project: &Dir,
    project_path: &Path,
    relative_path: PathBuf,
    depth: usize,
    overrides: &mut LocalProjectOverrides,
    packages: &mut BTreeMap<PackageKey, LocalPackageSource>,
    indexed_paths: &mut BTreeMap<PathBuf, PackageKey>,
) -> Result<PackageKey, PackagePreparationError> {
    if let Some(key) = indexed_paths.get(&relative_path) {
        return Ok(key.clone());
    }
    if depth > MAX_LOCAL_DEPENDENCY_DEPTH {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local dependencies exceed the {MAX_LOCAL_DEPENDENCY_DEPTH} depth limit"
        )));
    }
    if indexed_paths.len() >= MAX_LOCAL_PACKAGE_DIRECTORIES_V1 {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local project exceeds the {MAX_LOCAL_PACKAGE_DIRECTORIES_V1} package limit"
        )));
    }
    let directory = if relative_path.as_os_str().is_empty() {
        project.try_clone().map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot retain local project root: {error}"
            ))
        })?
    } else {
        let path = relative_path.to_str().ok_or_else(|| {
            PackagePreparationError::LocalDirectoryGraph(
                "local package path is not UTF-8".to_owned(),
            )
        })?;
        open_dependency_directory(project, path).map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot open local package `{path}`: {error}"
            ))
        })?
    };
    let _dependency_guard = if relative_path.as_os_str().is_empty() {
        None
    } else {
        let guard = transaction::read_guard(&directory)
            .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
        transaction::require_complete(&directory)
            .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
        guard
    };
    let manifest = if relative_path.as_os_str().is_empty() {
        match overrides.manifest.take() {
            Some(manifest) => manifest,
            None => read_project_manifest(&directory)?,
        }
    } else {
        read_project_manifest(&directory)?
    };
    let name = QualifiedName::parse(&manifest.package.name)?;
    let version = ExactVersion::parse(&manifest.package.version)?;
    let key = (name.clone(), version.clone());
    if let Some(previous) = packages.get(&key) {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local package `{name}@{version}` is supplied by both {} and {}",
            previous.path.display(),
            project_path.join(&relative_path).display()
        )));
    }
    indexed_paths.insert(relative_path.clone(), key.clone());

    let source_root = NormalizedRelativePath::parse(&manifest.package.source)?;
    let source_directory = open_relative_directory(&directory, &source_root).map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "cannot open source root `{source_root}` for `{name}`: {error}"
        ))
    })?;
    let discovered = PackageDirectory::try_from_dir(source_directory)
        .and_then(|source| source.discover_project_sources())
        .map_err(|source| PackagePreparationError::Directory {
            path: project_path.join(&relative_path).join(source_root.as_str()),
            source,
        })?;
    let entry_path = format!("src/{}.eqi", manifest.package.entry.replace('.', "/"));
    let mut files = Vec::with_capacity(discovered.len());
    for (path, source) in discovered {
        let package_path = NormalizedRelativePath::parse(format!("src/{path}"))?;
        let editor_path = relative_path.join(source_root.as_str()).join(path.as_str());
        let bytes = overrides
            .sources
            .remove(&editor_path)
            .map_or_else(|| source.into_bytes(), String::into_bytes);
        files.push(SourceFileV1::new(
            package_path,
            BundleRoleV1::ModelSource,
            bytes,
        ));
    }
    if !files.iter().any(|file| file.path().as_str() == entry_path) {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "package `{name}` entry module `{}` does not identify a discovered source",
            manifest.package.entry
        )));
    }
    match transaction::read(&directory, "README.md", MAX_PACKAGE_README_BYTES) {
        Ok(bytes) => files.push(SourceFileV1::new(
            NormalizedRelativePath::parse("README.md")?,
            BundleRoleV1::Documentation,
            bytes,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot read package `{name}` README.md: {error}"
            )));
        }
    }

    // Reserve the identity before descending so an ancestor cannot be supplied
    // again from a different directory while its dependencies are being read.
    packages.insert(
        key.clone(),
        LocalPackageSource {
            path: project_path.join(&relative_path),
            relative_path: relative_path.clone(),
            source_root,
            name,
            version,
            entry: manifest.package.entry,
            files,
            dependencies: BTreeMap::new(),
        },
    );
    let mut dependencies = BTreeMap::new();
    for (declared_name, dependency) in manifest.dependencies {
        let dependency_name = QualifiedName::parse(&declared_name)?;
        let dependency_version = ExactVersion::parse(&dependency.version)?;
        let dependency_path = resolve_dependency_path(&relative_path, &dependency.path)?;
        let actual = load_local_package(
            project,
            project_path,
            dependency_path.clone(),
            depth + 1,
            overrides,
            packages,
            indexed_paths,
        )?;
        let expected = (dependency_name.clone(), dependency_version);
        if actual != expected {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "dependency `{declared_name}` at `{}` declares `{}@{}` instead of `{}@{}`",
                dependency_path.display(),
                actual.0,
                actual.1,
                expected.0,
                expected.1
            )));
        }
        dependencies.insert(dependency_name, expected);
    }
    packages
        .get_mut(&key)
        .expect("reserved package identity")
        .dependencies = dependencies;
    Ok(key)
}

fn open_relative_directory(root: &Dir, path: &NormalizedRelativePath) -> std::io::Result<Dir> {
    let mut directory = root.try_clone()?;
    for segment in path.as_str().split('/') {
        directory = directory.open_dir_nofollow(segment)?;
    }
    Ok(directory)
}

fn open_dependency_directory(root: &Dir, path: &str) -> std::io::Result<Dir> {
    let mut directory = root.try_clone()?;
    for segment in path.split('/') {
        directory = if segment == ".." {
            // Only explicitly declared dependency paths grant parent traversal.
            // Sources and artifacts remain confined to the resulting directory.
            directory.open_parent_dir(ambient_authority())?
        } else {
            directory.open_dir_nofollow(segment)?
        };
    }
    Ok(directory)
}

fn resolve_dependency_path(
    declaring: &Path,
    value: &str,
) -> Result<PathBuf, PackagePreparationError> {
    if value.is_empty() || value.len() > 4096 || value.contains('\\') {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "invalid local dependency path `{value}`"
        )));
    }
    let mut resolved = declaring.to_path_buf();
    let mut descended = false;
    for segment in value.split('/') {
        match segment {
            ".." => {
                if descended {
                    return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                        "local dependency path `{value}` has a parent segment after a named directory"
                    )));
                }
                if resolved.as_os_str().is_empty() || resolved.ends_with("..") {
                    resolved.push("..");
                } else {
                    resolved.pop();
                }
            }
            "" | "." => {
                return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                    "invalid local dependency path `{value}`"
                )));
            }
            segment => {
                NormalizedRelativePath::parse(segment)?;
                descended = true;
                resolved.push(segment);
            }
        }
    }
    if resolved.components().count() > 64 || resolved.as_os_str().len() > 4096 {
        return Err(PackagePreparationError::LocalDirectoryGraph(
            "local dependency path exceeds its depth or length limit".to_owned(),
        ));
    }
    Ok(resolved)
}

fn open_project_root(path: &Path) -> Result<Dir, PackagePreparationError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .maybe_dir(true)
        .follow(FollowSymlinks::No)
        .nonblock(true);
    let root = File::open_ambient_with(path, &options, ambient_authority()).map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "cannot open local package project {}: {error}",
            path.display()
        ))
    })?;
    if !root
        .metadata()
        .map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot inspect local package project {}: {error}",
                path.display()
            ))
        })?
        .is_dir()
    {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local package project {} is not a directory",
            path.display()
        )));
    }
    Ok(Dir::from_std_file(root.into_std()))
}

fn read_project_manifest(project: &Dir) -> Result<LocalProjectManifest, PackagePreparationError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = project
        .open_with(PROJECT_MANIFEST, &options)
        .map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot open {PROJECT_MANIFEST}: {error}"
            ))
        })?;
    let metadata = file.metadata().map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "cannot inspect {PROJECT_MANIFEST}: {error}"
        ))
    })?;
    if !metadata.is_file() {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "{PROJECT_MANIFEST} is not a regular file"
        )));
    }
    if metadata.len() > MAX_PROJECT_MANIFEST_BYTES as u64 {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "{PROJECT_MANIFEST} exceeds the {MAX_PROJECT_MANIFEST_BYTES} byte limit"
        )));
    }
    let mut bytes = Vec::new();
    file.take((MAX_PROJECT_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot read {PROJECT_MANIFEST}: {error}"
            ))
        })?;
    if bytes.len() > MAX_PROJECT_MANIFEST_BYTES {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "{PROJECT_MANIFEST} exceeds the {MAX_PROJECT_MANIFEST_BYTES} byte limit"
        )));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "{PROJECT_MANIFEST} is not UTF-8: {error}"
        ))
    })?;
    toml::from_str(text).map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "cannot decode {PROJECT_MANIFEST}: {error}"
        ))
    })
}

fn read_project_lock(project: &Dir) -> Result<ResolutionRecordV1, PackagePreparationError> {
    let _guard = transaction::read_guard(project)
        .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
    let bytes = transaction::accepted_lock(project)
        .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
    let resolution = ResolutionRecordV1::from_json(&bytes)?;
    if resolution.canonical_json()? != bytes {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "{PROJECT_LOCK} is not canonical"
        )));
    }
    Ok(resolution)
}

fn prepare_local_package(
    key: &PackageKey,
    packages: &BTreeMap<PackageKey, LocalPackageSource>,
    visiting: &mut BTreeSet<PackageKey>,
    prepared: &mut BTreeMap<PackageKey, PreparedLocalPackage>,
) -> Result<PreparedLocalPackage, PackagePreparationError> {
    if let Some(package) = prepared.get(key) {
        return Ok(package.clone());
    }
    let package = packages
        .get(key)
        .expect("only indexed root and dependency keys are prepared");
    if !visiting.insert(key.clone()) {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local package dependency cycle reaches `{}`",
            key.0
        )));
    }

    let mut dependencies = BTreeMap::new();
    let mut requirements = Vec::with_capacity(package.dependencies.len());
    for target_key in package.dependencies.values() {
        if !packages.contains_key(target_key) {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "local package `{}` is missing dependency `{}@{}`",
                key.0, target_key.0, target_key.1
            )));
        }
        let child = prepare_local_package(target_key, packages, visiting, prepared)?;
        let actual = child.release.package_identity()?;
        dependencies.extend(child.dependencies.clone());
        dependencies.insert(actual.clone(), child.release);
        requirements.push(PackageDependencyV1::new(actual));
    }

    let bundle = package
        .files
        .iter()
        .map(|file| BundleEntryV1::new(file.path().clone(), file.role()))
        .collect();
    let manifest = PackageManifestV1::new(
        &package.entry,
        package.name.clone(),
        package.version.clone(),
        requirements,
        bundle,
    )?;
    let sources = PackageSourcesV1::new(manifest, package.files.clone())?;
    let release =
        prepare_package_release_v1(sources, &dependencies.values().cloned().collect::<Vec<_>>())
            .map_err(|source| PackagePreparationError::DirectoryPreparation {
                path: package.path.clone(),
                source: Box::new(source),
            })?;
    visiting.remove(key);
    let package = PreparedLocalPackage {
        release,
        dependencies,
    };
    prepared.insert(key.clone(), package.clone());
    Ok(package)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use eqiora_package::{
        BundleEntryV1, BundleRoleV1, DirectoryPackageStore, ExactVersion, NormalizedRelativePath,
        PackageDependencyV1, PackageManifestV1, SourceFileV1,
    };

    use super::*;
    use crate::package::PackagedModelDocument;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    const SOURCE_PATH: &str = "src/main.eqi";

    pub(super) struct TestDirectory(pub(super) PathBuf);

    impl TestDirectory {
        pub(super) fn create(name: &str) -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "eqiora-local-package-{name}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn child(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).expect("create child directory");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    fn author_sources(
        name: &str,
        source: &str,
        dependencies: Vec<PackageDependencyV1>,
    ) -> PackageSourcesV1 {
        let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
        let manifest = PackageManifestV1::new(
            "main",
            QualifiedName::parse(name).expect("package name"),
            ExactVersion::parse("1.0.0").expect("version"),
            dependencies,
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("package manifest");
        PackageSourcesV1::new(
            manifest,
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                source.as_bytes().to_vec(),
            )],
        )
        .expect("author sources")
    }

    fn exact_dependency(release: &PackageReleaseV1) -> PackageDependencyV1 {
        PackageDependencyV1::new(release.package_identity().expect("package identity"))
    }

    fn write_package(
        package_root: &Path,
        source_root: &str,
        sources: &PackageSourcesV1,
        dependencies: &[(&PackageReleaseV1, &str)],
    ) {
        let source_directory = package_root.join(source_root);
        fs::create_dir_all(&source_directory).expect("create source directory");
        fs::write(
            source_directory.join("main.eqi"),
            sources.files()[0].bytes(),
        )
        .expect("write source");
        let package = sources.manifest();
        let mut manifest = format!(
            "[package]\nname = \"{}\"\nversion = \"{}\"\nsource = \"{source_root}\"\nentry = \"main\"\n",
            package.name(),
            package.version()
        );
        for (dependency, dependency_path) in dependencies {
            let identity = dependency.package_identity().expect("dependency identity");
            manifest.push_str(&format!(
                "\n[dependencies.\"{}\"]\nversion = \"{}\"\npath = \"{dependency_path}\"\n",
                identity.name, identity.version
            ));
        }
        fs::write(package_root.join(PROJECT_MANIFEST), manifest).expect("write package manifest");
    }

    #[test]
    fn local_project_locks_deterministically_and_reopens_offline() {
        let fixture = TestDirectory::create("complete");
        let library_path = fixture.child("library");
        let auxiliary_path = fixture.child("auxiliary");
        let first_store = fixture.child("store-first");
        let second_store = fixture.child("store-second");

        let library_sources = author_sources(
            "org.example.Library",
            "public model Shared { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
            vec![],
        );
        let library_release =
            prepare_package_release_v1(library_sources.clone(), &[]).expect("library release");
        write_package(&library_path, "src", &library_sources, &[]);

        let auxiliary_sources = author_sources(
            "org.example.Auxiliary",
            "public model Other {}",
            vec![exact_dependency(&library_release)],
        );
        let auxiliary_release = prepare_package_release_v1(
            auxiliary_sources.clone(),
            std::slice::from_ref(&library_release),
        )
        .expect("auxiliary release");
        write_package(
            &auxiliary_path,
            "src",
            &auxiliary_sources,
            &[(&library_release, "../library")],
        );

        let root_sources = author_sources(
            "org.example.Root",
            "import org.example.Library.main as library; model Local {}",
            vec![
                exact_dependency(&library_release),
                exact_dependency(&auxiliary_release),
            ],
        );
        write_package(
            &fixture.0,
            "root/src",
            &root_sources,
            &[
                (&auxiliary_release, "auxiliary"),
                (&library_release, "library"),
            ],
        );

        let first =
            resolve_local_package_project_v1(&fixture.0, &first_store).expect("first resolution");
        let lock_bytes = fs::read(fixture.0.join(PROJECT_LOCK)).expect("read exact lock");
        assert_eq!(lock_bytes, first.canonical_json().expect("canonical lock"));
        let reopened = ResolutionRecordV1::from_json(&lock_bytes).expect("reopen exact lock");
        let second = resolve_local_package_project_v1(&fixture.0, &second_store)
            .expect("repeated resolution");
        assert_eq!(
            first.canonical_json().expect("first lock"),
            second.canonical_json().expect("second lock")
        );

        let store = DirectoryPackageStore::open_ambient(&first_store).expect("offline store");
        let model = PackagedModelDocument::compile_locked(&store, &reopened, "library.Shared")
            .expect("compile imported public Model");
        model
            .compilation()
            .validate_against(&first)
            .expect("exact local resolution lineage");
        assert!(model.model().aliases().contains_key("law"));
    }

    #[test]
    fn local_project_editor_analysis_is_read_only_and_accepts_source_overrides() {
        let fixture = TestDirectory::create("editor");
        let library_path = fixture.child("library");

        let library_sources = author_sources(
            "org.example.EditorLibrary",
            "public component Resistor {}",
            vec![],
        );
        let library_release =
            prepare_package_release_v1(library_sources.clone(), &[]).expect("library release");
        write_package(&library_path, "src", &library_sources, &[]);
        let root_source = "import org.example.EditorLibrary.main as library; model Main { instance load: library.Resistor(); }";
        let root_sources = author_sources(
            "org.example.EditorRoot",
            root_source,
            vec![exact_dependency(&library_release)],
        );
        write_package(
            &fixture.0,
            "root/src",
            &root_sources,
            &[(&library_release, "library")],
        );

        let (workspace, paths) = analyze_local_package_editor_project_v1(
            41,
            &fixture.0,
            &BTreeMap::from([(
                PathBuf::from("root").join(SOURCE_PATH),
                format!("// unsaved\n{root_source}"),
            )]),
        )
        .expect("read-only editor analysis");
        let root_file = paths
            .iter()
            .find_map(|(file, path)| {
                (path == &PathBuf::from("root").join(SOURCE_PATH)).then_some(file)
            })
            .expect("root source location");
        let library_file = paths
            .iter()
            .find_map(|(file, path)| {
                (path == &PathBuf::from("library").join(SOURCE_PATH)).then_some(file)
            })
            .expect("library source location");
        assert_eq!(workspace.version(), 41);
        assert!(workspace.document(root_file).is_some());
        assert!(workspace.document(library_file).is_some());
        assert_eq!(workspace.references().len(), 1);
        assert_eq!(workspace.references()[0].definition().file(), library_file);
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }

    #[test]
    fn changed_local_content_generates_a_new_exact_identity() {
        let fixture = TestDirectory::create("mismatch");
        let dependency_path = fixture.child("dependency");
        let store = fixture.child("store");

        let admitted_sources = author_sources(
            "org.example.Dependency",
            "public model Shared { parameter gain: 1 = 1; }",
            vec![],
        );
        let admitted_release =
            prepare_package_release_v1(admitted_sources, &[]).expect("expected dependency release");
        let root_sources = author_sources(
            "org.example.Root",
            "model Local {}",
            vec![exact_dependency(&admitted_release)],
        );
        write_package(
            &fixture.0,
            "root/src",
            &root_sources,
            &[(&admitted_release, "dependency")],
        );
        let admitted_sources = author_sources(
            "org.example.Dependency",
            "public model Shared { parameter gain: 1 = 1; }",
            vec![],
        );
        write_package(&dependency_path, "src", &admitted_sources, &[]);
        let accepted =
            resolve_local_package_project_v1(&fixture.0, &store).expect("initial exact lock");
        let previous_lock = accepted.canonical_json().expect("previous lock");

        let changed_sources = author_sources(
            "org.example.Dependency",
            "public model Shared { parameter gain: 1 = 2; }",
            vec![],
        );
        fs::write(
            dependency_path.join(SOURCE_PATH),
            changed_sources.files()[0].bytes(),
        )
        .expect("change dependency source");

        let changed = resolve_local_package_project_v1(&fixture.0, &store)
            .expect("changed author source generates a new exact release");
        let changed_lock = changed.canonical_json().expect("changed lock");
        assert_ne!(changed_lock, previous_lock);
        assert_eq!(
            fs::read(fixture.0.join(PROJECT_LOCK)).expect("updated lock"),
            changed_lock
        );
    }

    #[test]
    fn project_manifest_rejects_path_escape_and_unknown_fields_before_lock() {
        let fixture = TestDirectory::create("invalid-project");
        let store = fixture.child("store");
        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Invalid\"\nversion = \"1.0.0\"\nsource = \"../root\"\nentry = \"main\"\nextra = true\n",
        )
        .expect("write invalid manifest");

        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert!(!fixture.0.join(PROJECT_LOCK).exists());

        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Invalid\"\nversion = \"1.0.0\"\nsource = \"../root\"\nentry = \"main\"\n",
        )
        .expect("write escaping manifest");
        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }

    #[test]
    fn dependency_paths_are_explicit_bounded_and_portable() {
        assert_eq!(
            resolve_dependency_path(Path::new(""), "../library").unwrap(),
            Path::new("../library")
        );
        assert_eq!(
            resolve_dependency_path(Path::new("../library"), "../other").unwrap(),
            Path::new("../other")
        );
        for invalid in [
            "/absolute",
            "C:/absolute",
            "library/../other",
            "./library",
            "library//child",
            "library\\child",
            "",
        ] {
            assert!(
                resolve_dependency_path(Path::new(""), invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(resolve_dependency_path(Path::new(""), &"../".repeat(65)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn external_dependency_rejects_intermediate_symlinks() {
        let fixture = TestDirectory::create("external-symlink");
        let project = fixture.child("project");
        let library = fixture.child("library");
        let directory = open_project_root(&project).unwrap();
        std::os::unix::fs::symlink(&library, fixture.0.join("alias")).unwrap();
        assert!(open_dependency_directory(&directory, "../library").is_ok());
        assert!(open_dependency_directory(&directory, "../alias").is_err());
        assert!(open_dependency_directory(&directory, "../alias/child").is_err());
    }

    #[test]
    fn failed_reresolution_preserves_the_accepted_lock() {
        let fixture = TestDirectory::create("preserve-lock");
        let store = fixture.child("store");
        let sources = author_sources("org.example.Root", "model Main {}", vec![]);
        write_package(&fixture.0, "src", &sources, &[]);
        resolve_local_package_project_v1(&fixture.0, &store).expect("initial resolution");
        let accepted = fs::read(fixture.0.join(PROJECT_LOCK)).expect("accepted lock");

        fs::write(fixture.0.join("src/Main.eqi"), "model Other {}")
            .expect("write portable case collision");
        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert_eq!(
            fs::read(fixture.0.join(PROJECT_LOCK)).expect("preserved lock"),
            accepted
        );
    }

    #[test]
    fn local_dependency_cycle_is_rejected_before_lock() {
        let fixture = TestDirectory::create("cycle");
        let child = fixture.child("child");
        let store = fixture.child("store");
        let root = author_sources("org.example.Root", "model Main {}", vec![]);
        let dependency = author_sources("org.example.Child", "public model Shared {}", vec![]);
        write_package(&fixture.0, "src", &root, &[]);
        write_package(&child, "src", &dependency, &[]);
        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Root\"\nversion = \"1.0.0\"\nentry = \"main\"\n\n[dependencies.\"org.example.Child\"]\nversion = \"1.0.0\"\npath = \"child\"\n",
        )
        .expect("write root manifest");
        fs::write(
            child.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Child\"\nversion = \"1.0.0\"\nentry = \"main\"\n\n[dependencies.\"org.example.Root\"]\nversion = \"1.0.0\"\npath = \"..\"\n",
        )
        .expect("write child manifest");

        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }

    #[test]
    fn partial_lock_write_preserves_the_accepted_project_pair() {
        let fixture = TestDirectory::create("partial-lock-write");
        let store_path = fixture.child("store");
        let sources = author_sources(
            "org.example.Root",
            "model Main { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
            vec![],
        );
        write_package(&fixture.0, "src", &sources, &[]);
        let accepted = resolve_local_package_project_v1(&fixture.0, &store_path)
            .expect("initial project lock");
        let manifest = fs::read(fixture.0.join(PROJECT_MANIFEST)).expect("accepted manifest");
        let lock = fs::read(fixture.0.join(PROJECT_LOCK)).expect("accepted lock");
        fs::write(
            fixture.0.join("src/main.eqi"),
            "model Main { parameter gain: 1 = 3; relation law continuous { gain - 3 = 0; } }",
        )
        .expect("changed source");
        let candidate = prepare_local_package_project(
            open_project_root(&fixture.0).unwrap(),
            &fixture.0,
            LocalProjectOverrides::default(),
        )
        .expect("prepare changed project");
        let changed = ResolutionRecordV1::from_exact_releases(&candidate.root.release, &[])
            .expect("changed lock");
        assert_ne!(accepted, changed);
        let guard = transaction::write_guard(&candidate.project).unwrap();
        let failure = transaction::commit_using(
            &candidate.project,
            &manifest,
            &changed.canonical_json().unwrap(),
            || Ok(()),
            |file, bytes| {
                file.write_all(&bytes[..bytes.len() / 2])?;
                Err(std::io::Error::other(
                    "injected failure after partial staging write",
                ))
            },
        )
        .expect_err("partial write must fail");
        drop(guard);
        assert!(failure.to_string().contains("injected failure"));
        assert_eq!(
            fs::read(fixture.0.join(PROJECT_MANIFEST)).unwrap(),
            manifest
        );
        assert_eq!(fs::read(fixture.0.join(PROJECT_LOCK)).unwrap(), lock);
        assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".eqiora-project-file-")
        }));
        let store = DirectoryPackageStore::open_ambient(&store_path).expect("accepted store");
        PackagedModelDocument::compile_locked(&store, &accepted, "Main")
            .expect("previous accepted project remains usable");
    }

    #[test]
    fn proposed_dependency_changes_are_validated_without_publishing() {
        let fixture = TestDirectory::create("proposed-dependency");
        let store_path = fixture.child("store");
        let library_path = fixture.child("library");
        let root = author_sources(
            "org.example.Root",
            "model Main { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
            vec![],
        );
        let library = author_sources("org.example.Library", "public component Shared {}", vec![]);
        write_package(&fixture.0, "src", &root, &[]);
        write_package(&library_path, "src", &library, &[]);
        let accepted =
            resolve_local_package_project_v1(&fixture.0, &store_path).expect("accepted project");
        let manifest_bytes = fs::read(fixture.0.join(PROJECT_MANIFEST)).unwrap();
        let lock_bytes = fs::read(fixture.0.join(PROJECT_LOCK)).unwrap();
        let directory = open_project_root(&fixture.0).unwrap();

        for version in ["1.0.0", "2.0.0"] {
            let mut manifest = read_project_manifest(&directory).unwrap();
            manifest.dependencies.insert(
                "org.example.Library".to_owned(),
                LocalProjectDependency {
                    version: version.to_owned(),
                    path: "library".to_owned(),
                },
            );
            let candidate = prepare_local_package_project(
                directory.try_clone().unwrap(),
                &fixture.0,
                LocalProjectOverrides {
                    manifest: Some(manifest),
                    sources: BTreeMap::new(),
                },
            );
            if version == "1.0.0" {
                let candidate = candidate.expect("validate proposed dependency");
                assert_eq!(candidate.root.release.manifest().dependencies().len(), 1);
                assert_eq!(candidate.root.dependencies.len(), 1);
            } else {
                let error = candidate.err().expect("reject foreign dependency version");
                assert!(error.to_string().contains("instead of"));
            }
            assert_eq!(
                fs::read(fixture.0.join(PROJECT_MANIFEST)).unwrap(),
                manifest_bytes
            );
            assert_eq!(fs::read(fixture.0.join(PROJECT_LOCK)).unwrap(), lock_bytes);
        }
        let store = DirectoryPackageStore::open_ambient(&store_path).unwrap();
        PackagedModelDocument::compile_locked(&store, &accepted, "Main")
            .expect("accepted closure remains usable after candidate validation");

        assert!(
            PackagedModelDocument::add_local_package_dependency_v1(
                &fixture.0,
                &store_path,
                "org.example.Library",
                "2.0.0",
                "library",
            )
            .is_err()
        );
        assert_eq!(
            fs::read(fixture.0.join(PROJECT_MANIFEST)).unwrap(),
            manifest_bytes
        );
        assert_eq!(fs::read(fixture.0.join(PROJECT_LOCK)).unwrap(), lock_bytes);

        let added = PackagedModelDocument::add_local_package_dependency_v1(
            &fixture.0,
            &store_path,
            "org.example.Library",
            "1.0.0",
            "library",
        )
        .expect("publish dependency addition");
        assert_eq!(
            read_project_manifest(&directory)
                .unwrap()
                .dependencies
                .len(),
            1
        );
        assert_eq!(read_project_lock(&directory).unwrap(), added);
        PackagedModelDocument::compile_locked(&store, &added, "Main").unwrap();

        let removed = PackagedModelDocument::remove_local_package_dependency_v1(
            &fixture.0,
            &store_path,
            "org.example.Library",
        )
        .expect("publish dependency removal");
        assert!(
            read_project_manifest(&directory)
                .unwrap()
                .dependencies
                .is_empty()
        );
        assert_eq!(read_project_lock(&directory).unwrap(), removed);
        assert_eq!(removed, accepted);
        PackagedModelDocument::compile_locked(&store, &removed, "Main").unwrap();
    }

    #[test]
    fn dependency_depth_is_bounded_before_reading_another_manifest() {
        let fixture = TestDirectory::create("depth");
        let directory = open_project_root(&fixture.0).expect("project root");
        let result = load_local_package(
            &directory,
            &fixture.0,
            PathBuf::new(),
            MAX_LOCAL_DEPENDENCY_DEPTH + 1,
            &mut LocalProjectOverrides::default(),
            &mut BTreeMap::new(),
            &mut BTreeMap::new(),
        );
        assert!(
            result
                .expect_err("bounded depth")
                .to_string()
                .contains("depth limit")
        );
    }

    #[test]
    fn dependency_cannot_replace_an_ancestor_from_another_directory() {
        let fixture = TestDirectory::create("duplicate-ancestor");
        let child = fixture.child("child");
        let duplicate = fixture.child("duplicate");
        let store = fixture.child("store");
        let root_sources = author_sources("org.example.Root", "model Main {}", vec![]);
        let child_sources = author_sources("org.example.Child", "model Main {}", vec![]);
        let root_release = prepare_package_release_v1(root_sources.clone(), &[]).unwrap();
        let child_release = prepare_package_release_v1(child_sources.clone(), &[]).unwrap();
        write_package(
            &fixture.0,
            "src",
            &root_sources,
            &[(&child_release, "child")],
        );
        write_package(
            &child,
            "src",
            &child_sources,
            &[(&root_release, "../duplicate")],
        );
        write_package(&duplicate, "src", &root_sources, &[]);
        let error = resolve_local_package_project_v1(&fixture.0, &store)
            .expect_err("duplicate ancestor must fail before preparation");
        assert!(error.to_string().contains("supplied by both"));
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }

    #[cfg(unix)]
    #[test]
    fn project_preparation_and_lock_publication_retain_the_opened_directory() {
        let fixture = TestDirectory::create("retained-project");
        let original = fixture.child("project");
        let moved = fixture.0.join("moved");
        let root = author_sources("org.example.Original", "model Main {}", vec![]);
        write_package(&original, "src", &root, &[]);
        let directory = open_project_root(&original).unwrap();
        fs::rename(&original, &moved).unwrap();
        fs::create_dir(&original).unwrap();
        fs::write(original.join(PROJECT_MANIFEST), "replacement manifest").unwrap();
        let candidate =
            prepare_local_package_project(directory, &original, LocalProjectOverrides::default())
                .expect("prepare through retained root");
        let resolution =
            ResolutionRecordV1::from_exact_releases(&candidate.root.release, &[]).unwrap();
        assert_eq!(resolution.root().name.as_str(), "org.example.Original");
        let _guard = transaction::write_guard(&candidate.project).unwrap();
        let manifest = transaction::read(
            &candidate.project,
            PROJECT_MANIFEST,
            MAX_PROJECT_MANIFEST_BYTES,
        )
        .unwrap();
        transaction::commit(
            &candidate.project,
            &manifest,
            &resolution.canonical_json().unwrap(),
        )
        .unwrap();
        assert_eq!(
            fs::read(moved.join(PROJECT_LOCK)).unwrap(),
            resolution.canonical_json().unwrap()
        );
        assert!(!original.join(PROJECT_LOCK).exists());
        assert_eq!(
            fs::read(original.join(PROJECT_MANIFEST)).unwrap(),
            b"replacement manifest"
        );
    }

    #[test]
    fn manifest_entry_selects_a_nested_module_during_offline_replay() {
        let fixture = TestDirectory::create("nested-entry");
        let store_path = fixture.child("store");
        fs::create_dir_all(fixture.0.join("sources/models")).expect("source tree");
        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Entry\"\nversion = \"1.0.0\"\nsource = \"sources\"\nentry = \"models.selected\"\n",
        ).expect("manifest");
        fs::write(
            fixture.0.join("sources/models/selected.eqi"),
            "model Main { parameter selected: 1 = 2; relation law continuous { selected - 2 = 0; } }",
        )
        .expect("selected module");
        fs::write(
            fixture.0.join("sources/main.eqi"),
            "model Main { parameter decoy: 1 = 3; relation law continuous { decoy - 3 = 0; } }",
        )
        .expect("decoy module");
        let resolution = resolve_local_package_project_v1(&fixture.0, &store_path)
            .expect("resolve explicit entry");
        fs::remove_dir_all(fixture.0.join("sources")).expect("remove authored sources");
        let store = DirectoryPackageStore::open_ambient(&store_path).expect("offline store");
        let document = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
            .expect("compile the locked entry");
        assert!(document.model().aliases().contains_key("selected"));
        assert!(!document.model().aliases().contains_key("decoy"));
    }

    #[cfg(unix)]
    #[test]
    fn source_root_rejects_an_intermediate_symlink() {
        let fixture = TestDirectory::create("source-link");
        let store = fixture.child("store");
        let sources = author_sources("org.example.Root", "model Main {}", vec![]);
        write_package(&fixture.0, "actual/src", &sources, &[]);
        std::os::unix::fs::symlink("actual", fixture.0.join("linked"))
            .expect("create intermediate symlink");
        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "[package]\nname = \"org.example.Root\"\nversion = \"1.0.0\"\nsource = \"linked/src\"\nentry = \"main\"\n",
        )
        .expect("write manifest");
        let error = resolve_local_package_project_v1(&fixture.0, &store)
            .expect_err("intermediate symlink must fail");
        assert!(error.to_string().contains("cannot open source root"));
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }
}
