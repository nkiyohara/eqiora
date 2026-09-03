//! Exact local-directory package resolution and store preparation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use eqiora_package::{
    AuthorPackageDirectory, AuthorPackageSourcesV1, DirectoryPackageInstaller, ExactVersion,
    ModelPackageIdentityV1, PackageReleaseV1, QualifiedName, ResolutionRecordV1,
};

use super::{PackagePreparationError, PackagedModelDocument, prepare_package_release_v1};

const MAX_LOCAL_PACKAGE_DIRECTORIES_V1: usize = 65_536;

type PackageKey = (QualifiedName, ExactVersion);

#[derive(Clone)]
struct LocalPackageSource {
    path: PathBuf,
    sources: AuthorPackageSourcesV1,
}

#[derive(Clone)]
struct PreparedLocalPackage {
    release: PackageReleaseV1,
    dependencies: BTreeMap<ModelPackageIdentityV1, PackageReleaseV1>,
}

impl PackagedModelDocument {
    /// Maximum root-plus-dependency count admitted by local exact resolution.
    pub const MAX_LOCAL_PACKAGE_DIRECTORIES_V1: usize = MAX_LOCAL_PACKAGE_DIRECTORIES_V1;

    /// Resolve explicit local package directories and populate one offline store.
    ///
    /// Every directory crosses its bounded `package.json` inventory. Dependencies are
    /// prepared leaf-first through compiler-owned semantics, matched against their parents'
    /// exact identities, and normalized independently of caller order. The returned ordinary
    /// [`ResolutionRecordV1`] is the authority; the populated store remains a replaceable cache.
    ///
    /// This operation performs no project-manifest or lockfile I/O, Git/network access, version
    /// selection, environment lookup, or executable package hook. The complete closure must be
    /// supplied explicitly and may not contain unrelated directories.
    ///
    /// # Errors
    ///
    /// Returns a typed directory, graph, compiler-preparation, exact-identity, lock-derivation,
    /// or store-installation failure. No lock is returned until the closure is installed.
    pub fn resolve_local_package_directories_v1<R, D, I, P>(
        root: R,
        dependency_roots: I,
        store_root: P,
    ) -> Result<ResolutionRecordV1, PackagePreparationError>
    where
        R: Into<PathBuf>,
        D: Into<PathBuf>,
        I: IntoIterator<Item = D>,
        P: Into<PathBuf>,
    {
        resolve_local_package_directories_v1(root, dependency_roots, store_root)
    }
}

/// Resolve exact packages from explicit local directories and populate one local store.
///
/// Every directory is admitted through its bounded `package.json` inventory. Dependencies
/// are prepared leaf-first through compiler-owned semantics, matched against the exact
/// identities declared by their parents, and normalized independently of caller order.
/// The store is a replaceable cache populated with atomic no-clobber entries; the returned
/// canonical [`ResolutionRecordV1`] remains the authority for later offline compilation.
///
/// This operation performs no project-manifest or lockfile I/O, Git/network access, version
/// selection, environment lookup, or executable package hook. All dependency directories must
/// be supplied explicitly, and none may be unrelated to the root closure.
///
/// # Errors
///
/// Returns a typed directory, graph, compiler-preparation, exact-identity, lock-derivation, or
/// store-installation failure. No resolution record is returned until the complete closure is
/// prepared and installed.
fn resolve_local_package_directories_v1<R, D, I, P>(
    root: R,
    dependency_roots: I,
    store_root: P,
) -> Result<ResolutionRecordV1, PackagePreparationError>
where
    R: Into<PathBuf>,
    D: Into<PathBuf>,
    I: IntoIterator<Item = D>,
    P: Into<PathBuf>,
{
    let root = root.into();
    let mut paths = Vec::new();
    paths.push(root.clone());
    for dependency in dependency_roots {
        if paths.len() == MAX_LOCAL_PACKAGE_DIRECTORIES_V1 {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "local package closure has {} directories, exceeding the limit {MAX_LOCAL_PACKAGE_DIRECTORIES_V1}",
                paths.len() + 1
            )));
        }
        paths.push(dependency.into());
    }

    let mut packages = BTreeMap::<PackageKey, LocalPackageSource>::new();
    let mut root_key = None;
    for (index, path) in paths.into_iter().enumerate() {
        let directory = AuthorPackageDirectory::open_ambient(&path).map_err(|source| {
            PackagePreparationError::Directory {
                path: path.clone(),
                source,
            }
        })?;
        let sources =
            directory
                .read_sources()
                .map_err(|source| PackagePreparationError::Directory {
                    path: path.clone(),
                    source,
                })?;
        let key = (
            sources.manifest().name().clone(),
            sources.manifest().version().clone(),
        );
        if index == 0 {
            root_key = Some(key.clone());
        }
        if let Some(previous) = packages.get(&key) {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "local package `{}@{}` is supplied by both {} and {}",
                key.0,
                key.1,
                previous.path.display(),
                path.display()
            )));
        }
        packages.insert(key, LocalPackageSource { path, sources });
    }

    let root_key = root_key.expect("root path is always present");
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

    let dependencies = root_package
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let resolution = ResolutionRecordV1::from_exact_releases(&root_package.release, &dependencies)?;
    let store_root = store_root.into();
    let installer = DirectoryPackageInstaller::open_ambient(&store_root).map_err(|source| {
        PackagePreparationError::Installation {
            store_root: store_root.clone(),
            source,
        }
    })?;
    for release in dependencies
        .iter()
        .chain(std::iter::once(&root_package.release))
    {
        let _receipt =
            installer
                .install(release)
                .map_err(|source| PackagePreparationError::Installation {
                    store_root: store_root.clone(),
                    source,
                })?;
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

    #[test]
    fn local_directories_resolve_order_independently_and_compile_offline() {
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
            "model Local {}",
            vec![
                exact_dependency("library", &library_release),
                exact_dependency("auxiliary", &auxiliary_release),
            ],
        );
        write_package(&root_path, &root_sources);

        let first = resolve_local_package_directories_v1(
            &root_path,
            [&library_path, &auxiliary_path],
            &first_store,
        )
        .expect("first resolution");
        let second = resolve_local_package_directories_v1(
            &root_path,
            [&auxiliary_path, &library_path],
            &second_store,
        )
        .expect("permuted resolution");
        assert_eq!(
            first.canonical_json().expect("first lock"),
            second.canonical_json().expect("second lock")
        );

        let store = DirectoryPackageStore::open_ambient(&first_store).expect("offline store");
        let model = PackagedModelDocument::compile_locked(&store, &first, "library.Shared")
            .expect("compile imported public Model");
        model
            .compilation()
            .validate_against(&first)
            .expect("exact local resolution lineage");
        assert!(model.model().aliases().contains_key("law"));
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
        let changed_sources = author_sources(
            "org.example.Dependency",
            "public model Shared { parameter gain: 1 = 2; }",
            vec![],
        );
        write_package(&dependency_path, &changed_sources);
        let root_sources = author_sources(
            "org.example.Root",
            "model Local {}",
            vec![exact_dependency("dependency", &admitted_release)],
        );
        write_package(&root_path, &root_sources);

        let error = resolve_local_package_directories_v1(&root_path, [&dependency_path], &store)
            .expect_err("changed content must fail exact identity matching");
        assert!(matches!(
            error,
            PackagePreparationError::IdentityMismatch { .. }
        ));
        assert!(
            fs::read_dir(&store)
                .expect("read empty store")
                .next()
                .is_none()
        );
    }
}
