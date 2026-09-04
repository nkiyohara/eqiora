//! Exact local-directory package resolution and store preparation.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_fs_ext::{
    DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt, OpenOptionsSyncExt,
};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};
use eqiora_package::{
    AuthorPackageDirectory, AuthorPackageSourcesV1, BundleRoleV1, DirectoryPackageInstaller,
    ExactResolver, ExactVersion, ModelPackageIdentityV1, NormalizedRelativePath, PackageReleaseV1,
    QualifiedName, ResolutionRecordV1, SourceFileV1,
};
use serde::Deserialize;

use super::{PackagePreparationError, PackagedModelDocument, prepare_package_release_v1};

const MAX_LOCAL_PACKAGE_DIRECTORIES_V1: usize = 65_536;
const MAX_PROJECT_MANIFEST_BYTES: usize = 1024 * 1024;
const PROJECT_MANIFEST: &str = "eqiora.toml";
const PROJECT_LOCK: &str = "eqiora.lock";
const PROJECT_SCHEMA: &str = "eqiora.project.v1";
static NEXT_LOCK_STAGE: AtomicU64 = AtomicU64::new(0);

type PackageKey = (QualifiedName, ExactVersion);

#[derive(Clone)]
struct LocalPackageSource {
    path: PathBuf,
    relative_path: NormalizedRelativePath,
    sources: AuthorPackageSourcesV1,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalProjectManifest {
    schema: String,
    root: String,
    sources: BTreeMap<String, LocalProjectSource>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalProjectSource {
    path: String,
}

impl PackagedModelDocument {
    /// Maximum root-plus-dependency count admitted by local exact resolution.
    pub const MAX_LOCAL_PROJECT_SOURCES_V1: usize = MAX_LOCAL_PACKAGE_DIRECTORIES_V1;

    /// Resolve one local package project, write its exact lock, and populate an offline store.
    ///
    /// `eqiora.toml` names one root source, its complete local source closure, and the root
    /// package's dependency aliases. Every path is project-relative and opened without following
    /// symbolic links. Resolution validates the compiler-derived exact graph before atomically
    /// replacing `eqiora.lock` with canonical [`ResolutionRecordV1`] bytes.
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
}

fn resolve_local_package_project_v1<R, P>(
    project_root: R,
    store_root: P,
) -> Result<ResolutionRecordV1, PackagePreparationError>
where
    R: Into<PathBuf>,
    P: Into<PathBuf>,
{
    let project_path = project_root.into();
    let prepared = prepare_local_package_project(&project_path, &BTreeMap::new())?;
    let dependencies = prepared
        .root
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&prepared.root.release, &dependencies)?;
    let store_root = store_root.into();
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
    publish_project_lock(&prepared.project, &resolution)?;
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
    let prepared = prepare_local_package_project(&project_path, overrides)?;
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
            .sources
            .files()
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
                PathBuf::from(package.relative_path.as_str()).join(file.path().as_str()),
            );
        }
    }
    Ok((
        crate::editor::EditorWorkspaceSnapshot::analyze_modules(version, input),
        relative_paths,
    ))
}

fn prepare_local_package_project(
    project_path: &Path,
    overrides: &BTreeMap<PathBuf, String>,
) -> Result<PreparedLocalProject, PackagePreparationError> {
    let project = open_project_root(project_path)?;
    let manifest = read_project_manifest(&project)?;
    let (root_source, paths) = normalize_project_manifest(&manifest)?;
    let mut unused_overrides = overrides.clone();

    let mut packages = BTreeMap::<PackageKey, LocalPackageSource>::new();
    let mut source_keys = BTreeMap::new();
    for (source_name, relative_path) in paths {
        let directory = project
            .open_dir_nofollow(relative_path.as_str())
            .map_err(|source| {
                PackagePreparationError::LocalDirectoryGraph(format!(
                    "cannot open local project source `{source_name}` at {relative_path}: {source}"
                ))
            })?;
        let directory = AuthorPackageDirectory::try_from_dir(directory).map_err(|source| {
            PackagePreparationError::Directory {
                path: project_path.join(relative_path.as_str()),
                source,
            }
        })?;
        let sources =
            directory
                .read_sources()
                .map_err(|source| PackagePreparationError::Directory {
                    path: project_path.join(relative_path.as_str()),
                    source,
                })?;
        let sources = apply_editor_overrides(sources, &relative_path, &mut unused_overrides)?;
        let key = (
            sources.manifest().name().clone(),
            sources.manifest().version().clone(),
        );
        source_keys.insert(source_name, key.clone());
        if let Some(previous) = packages.get(&key) {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "local package `{}@{}` is supplied by both {} and {}",
                key.0,
                key.1,
                previous.path.display(),
                project_path.join(relative_path.as_str()).display()
            )));
        }
        packages.insert(
            key,
            LocalPackageSource {
                path: project_path.join(relative_path.as_str()),
                relative_path,
                sources,
            },
        );
    }

    let root_key = source_keys
        .get(&root_source)
        .cloned()
        .expect("normalized root source exists");
    validate_project_dependencies(&manifest, &root_key, &source_keys, &packages)?;
    let mut visiting = BTreeSet::new();
    let mut prepared = BTreeMap::new();
    let root_package = prepare_local_package(&root_key, &packages, &mut visiting, &mut prepared)?;
    if prepared.len() != packages.len() {
        let path = packages
            .iter()
            .find(|(key, _)| !prepared.contains_key(*key))
            .map(|(_, package)| package.path.clone())
            .expect("different map lengths imply one unused key");
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local package directory {} is not reachable from the root package",
            path.display()
        )));
    }
    if let Some(path) = unused_overrides.keys().next() {
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

fn apply_editor_overrides(
    sources: AuthorPackageSourcesV1,
    package_path: &NormalizedRelativePath,
    overrides: &mut BTreeMap<PathBuf, String>,
) -> Result<AuthorPackageSourcesV1, PackagePreparationError> {
    let (manifest, files) = sources.into_parts();
    let files = files
        .into_iter()
        .map(|file| {
            let project_path = PathBuf::from(package_path.as_str()).join(file.path().as_str());
            let bytes = if file.role() == BundleRoleV1::ModelSource {
                overrides
                    .remove(&project_path)
                    .map_or_else(|| file.bytes().to_vec(), String::into_bytes)
            } else {
                file.bytes().to_vec()
            };
            SourceFileV1::new(file.path().clone(), file.role(), bytes)
        })
        .collect();
    AuthorPackageSourcesV1::new(manifest, files).map_err(PackagePreparationError::Contract)
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

fn normalize_project_manifest(
    manifest: &LocalProjectManifest,
) -> Result<(String, Vec<(String, NormalizedRelativePath)>), PackagePreparationError> {
    if manifest.schema != PROJECT_SCHEMA {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "unsupported local project schema `{}`",
            manifest.schema
        )));
    }
    if manifest.sources.is_empty() || manifest.sources.len() > MAX_LOCAL_PACKAGE_DIRECTORIES_V1 {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local project must contain between 1 and {MAX_LOCAL_PACKAGE_DIRECTORIES_V1} sources"
        )));
    }
    validate_local_name("root source", &manifest.root)?;
    if !manifest.sources.contains_key(&manifest.root) {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local project root source `{}` is not declared in `sources`",
            manifest.root
        )));
    }
    let mut paths = Vec::with_capacity(manifest.sources.len());
    let mut portable_paths = BTreeMap::new();
    for (name, source) in &manifest.sources {
        validate_local_name("source", name)?;
        let path = NormalizedRelativePath::parse(&source.path).map_err(|error| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "local project source `{name}` has invalid path `{}`: {error}",
                source.path
            ))
        })?;
        if let Some(previous) = portable_paths.insert(path.as_str().to_ascii_lowercase(), name) {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "local project sources `{previous}` and `{name}` use portability-colliding paths"
            )));
        }
        paths.push((name.clone(), path));
    }
    for (alias, source) in &manifest.dependencies {
        validate_local_name("dependency alias", alias)?;
        validate_local_name("dependency source", source)?;
        if source == &manifest.root || !manifest.sources.contains_key(source) {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "dependency alias `{alias}` names unknown non-root source `{source}`"
            )));
        }
    }
    Ok((manifest.root.clone(), paths))
}

fn validate_local_name(kind: &str, value: &str) -> Result<(), PackagePreparationError> {
    let name = QualifiedName::parse(value).map_err(|error| {
        PackagePreparationError::LocalDirectoryGraph(format!(
            "invalid local project {kind} `{value}`: {error}"
        ))
    })?;
    if name.as_str().contains('.') {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "local project {kind} `{value}` must be one identifier"
        )));
    }
    Ok(())
}

fn validate_project_dependencies(
    manifest: &LocalProjectManifest,
    root_key: &PackageKey,
    source_keys: &BTreeMap<String, PackageKey>,
    packages: &BTreeMap<PackageKey, LocalPackageSource>,
) -> Result<(), PackagePreparationError> {
    let root = packages
        .get(root_key)
        .expect("root source was indexed as a package");
    if root.sources.manifest().dependencies().len() != manifest.dependencies.len() {
        return Err(PackagePreparationError::LocalDirectoryGraph(
            "eqiora.toml dependencies must map every direct root-package dependency exactly once"
                .to_owned(),
        ));
    }
    for requirement in root.sources.manifest().dependencies() {
        let alias = requirement.alias().as_str();
        let source = manifest.dependencies.get(alias).ok_or_else(|| {
            PackagePreparationError::LocalDirectoryGraph(format!(
                "eqiora.toml is missing root-package dependency alias `{alias}`"
            ))
        })?;
        let actual = source_keys
            .get(source)
            .expect("normalized dependency source exists");
        let expected = requirement.target();
        if actual.0 != expected.name || actual.1 != expected.version {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "dependency alias `{alias}` maps to `{}@{}`, expected `{}@{}`",
                actual.0, actual.1, expected.name, expected.version
            )));
        }
    }
    Ok(())
}

fn publish_project_lock(
    project: &Dir,
    resolution: &ResolutionRecordV1,
) -> Result<(), PackagePreparationError> {
    let bytes = resolution.canonical_json()?;
    match project.symlink_metadata(PROJECT_LOCK) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "{PROJECT_LOCK} is not a regular file"
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "cannot inspect {PROJECT_LOCK}: {error}"
            )));
        }
    }

    for _ in 0..128 {
        let sequence = NEXT_LOCK_STAGE.fetch_add(1, Ordering::Relaxed);
        let stage = format!(
            ".eqiora.lock.stage-{:x}-{sequence:016x}",
            std::process::id()
        );
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No)
            .nonblock(true);
        match project.open_with(&stage, &options) {
            Ok(mut file) => {
                let result = file
                    .write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .and_then(|()| {
                        drop(file);
                        project.rename(&stage, project, PROJECT_LOCK)
                    });
                if let Err(error) = result {
                    let _ = project.remove_file(&stage);
                    return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                        "cannot publish {PROJECT_LOCK} atomically: {error}"
                    )));
                }
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                    "cannot create {PROJECT_LOCK} staging file: {error}"
                )));
            }
        }
    }
    Err(PackagePreparationError::LocalDirectoryGraph(format!(
        "cannot reserve a {PROJECT_LOCK} staging name"
    )))
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
    for requirement in package.sources.manifest().dependencies() {
        let target = requirement.target();
        let target_key = (target.name.clone(), target.version.clone());
        let target_source = packages.get(&target_key).ok_or_else(|| {
            PackagePreparationError::MissingDependency {
                declaring: key.0.clone(),
                target: Box::new(target.clone()),
            }
        })?;
        let child = prepare_local_package(&target_key, packages, visiting, prepared)?;
        let actual = child.release.package_identity()?;
        if &actual != target {
            return Err(PackagePreparationError::IdentityMismatch {
                declaring: key.0.clone(),
                expected: Box::new(target.clone()),
                actual: Box::new(actual),
                path: target_source.path.clone(),
            });
        }
        dependencies.extend(child.dependencies.clone());
        dependencies.insert(target.clone(), child.release);
    }

    let release = prepare_package_release_v1(
        package.sources.clone(),
        &dependencies.values().cloned().collect::<Vec<_>>(),
    )
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
        AuthorManifestV1, BundleEntryV1, BundleRoleV1, DependencyRequirementV1,
        DirectoryPackageStore, ExactVersion, NormalizedRelativePath, SourceFileV1,
    };

    use super::*;
    use crate::package::PackagedModelDocument;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    const SOURCE_PATH: &str = "src/main.eqi";

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create(name: &str) -> Self {
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
        dependencies: Vec<DependencyRequirementV1>,
    ) -> AuthorPackageSourcesV1 {
        let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse(name).expect("package name"),
            ExactVersion::parse("1.0.0").expect("version"),
            dependencies,
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("author manifest");
        AuthorPackageSourcesV1::new(
            manifest,
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                source.as_bytes().to_vec(),
            )],
        )
        .expect("author sources")
    }

    fn write_package(path: &std::path::Path, sources: &AuthorPackageSourcesV1) {
        fs::create_dir(path.join("src")).expect("create source directory");
        fs::write(
            path.join("package.json"),
            sources
                .manifest()
                .canonical_json()
                .expect("canonical manifest"),
        )
        .expect("write manifest");
        fs::write(path.join(SOURCE_PATH), sources.files()[0].bytes()).expect("write source");
    }

    fn exact_dependency(alias: &str, release: &PackageReleaseV1) -> DependencyRequirementV1 {
        DependencyRequirementV1::new(
            QualifiedName::parse(alias).expect("alias"),
            release.package_identity().expect("package identity"),
        )
        .expect("dependency")
    }

    fn write_project(fixture: &TestDirectory, sources: &[&str], dependencies: &[(&str, &str)]) {
        let mut manifest =
            String::from("schema = \"eqiora.project.v1\"\nroot = \"root\"\n\n[dependencies]\n");
        for (alias, source) in dependencies {
            manifest.push_str(&format!("{alias} = \"{source}\"\n"));
        }
        for source in sources {
            manifest.push_str(&format!("\n[sources.{source}]\npath = \"{source}\"\n"));
        }
        fs::write(fixture.0.join(PROJECT_MANIFEST), manifest).expect("write project manifest");
    }

    #[test]
    fn local_project_locks_deterministically_and_reopens_offline() {
        let fixture = TestDirectory::create("complete");
        let library_path = fixture.child("library");
        let auxiliary_path = fixture.child("auxiliary");
        let root_path = fixture.child("root");
        let first_store = fixture.child("store-first");
        let second_store = fixture.child("store-second");

        let library_sources = author_sources(
            "org.example.Library",
            "public model Shared { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
            vec![],
        );
        let library_release =
            prepare_package_release_v1(library_sources.clone(), &[]).expect("library release");
        write_package(&library_path, &library_sources);

        let auxiliary_sources = author_sources(
            "org.example.Auxiliary",
            "public model Other {}",
            vec![exact_dependency("base", &library_release)],
        );
        let auxiliary_release = prepare_package_release_v1(
            auxiliary_sources.clone(),
            std::slice::from_ref(&library_release),
        )
        .expect("auxiliary release");
        write_package(&auxiliary_path, &auxiliary_sources);

        let root_sources = author_sources(
            "org.example.Root",
            "import org.example.Library.main as library; model Local {}",
            vec![
                exact_dependency("library", &library_release),
                exact_dependency("auxiliary", &auxiliary_release),
            ],
        );
        write_package(&root_path, &root_sources);
        write_project(
            &fixture,
            &["root", "library", "auxiliary"],
            &[("auxiliary", "auxiliary"), ("library", "library")],
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
        let root_path = fixture.child("root");

        let library_sources = author_sources(
            "org.example.EditorLibrary",
            "public component Resistor {}",
            vec![],
        );
        let library_release =
            prepare_package_release_v1(library_sources.clone(), &[]).expect("library release");
        write_package(&library_path, &library_sources);
        let root_source = "import org.example.EditorLibrary.main as library; model Main { instance load: library.Resistor(); }";
        let root_sources = author_sources(
            "org.example.EditorRoot",
            root_source,
            vec![exact_dependency("library", &library_release)],
        );
        write_package(&root_path, &root_sources);
        write_project(&fixture, &["root", "library"], &[("library", "library")]);

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
    fn changed_local_content_cannot_impersonate_required_identity() {
        let fixture = TestDirectory::create("mismatch");
        let dependency_path = fixture.child("dependency");
        let root_path = fixture.child("root");
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
            vec![exact_dependency("dependency", &admitted_release)],
        );
        write_package(&root_path, &root_sources);
        let admitted_sources = author_sources(
            "org.example.Dependency",
            "public model Shared { parameter gain: 1 = 1; }",
            vec![],
        );
        write_package(&dependency_path, &admitted_sources);
        write_project(
            &fixture,
            &["root", "dependency"],
            &[("dependency", "dependency")],
        );
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

        let error = resolve_local_package_project_v1(&fixture.0, &store)
            .expect_err("changed content must fail exact identity matching");
        assert!(matches!(
            error,
            PackagePreparationError::IdentityMismatch { .. }
        ));
        assert_eq!(
            fs::read(fixture.0.join(PROJECT_LOCK)).expect("previous lock remains"),
            previous_lock
        );
    }

    #[test]
    fn project_manifest_rejects_path_escape_and_unknown_fields_before_lock() {
        let fixture = TestDirectory::create("invalid-project");
        let store = fixture.child("store");
        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "schema = \"eqiora.project.v1\"\nroot = \"root\"\nextra = true\n\n[dependencies]\n\n[sources.root]\npath = \"../root\"\n",
        )
        .expect("write invalid manifest");

        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert!(!fixture.0.join(PROJECT_LOCK).exists());

        fs::write(
            fixture.0.join(PROJECT_MANIFEST),
            "schema = \"eqiora.project.v1\"\nroot = \"root\"\n\n[dependencies]\n\n[sources.root]\npath = \"../root\"\n",
        )
        .expect("write escaping manifest");
        assert!(resolve_local_package_project_v1(&fixture.0, &store).is_err());
        assert!(!fixture.0.join(PROJECT_LOCK).exists());
    }
}
