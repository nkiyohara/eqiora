//! Explicit materialization and read-only reopening of one accepted exact project closure.

use eqiora_package::{DirectoryPackageStore, ResolvedPackageGraph};

use super::*;

impl PackagedModelDocument {
    /// Add an exact bundled dependency through the ordinary manifest/lock transaction.
    ///
    /// # Errors
    /// Rejects unavailable releases, invalid closure or failed publication.
    pub fn add_bundled_package_dependency_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        name: &str,
        version: &str,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        update_local_package_project(project_root.into(), store_root.into(), |manifest| {
            manifest.dependencies.insert(
                name.to_owned(),
                LocalProjectDependency {
                    version: version.to_owned(),
                    path: None,
                    bundled: true,
                    git: None,
                },
            );
            Ok(true)
        })
    }

    /// Fetch the existing exact lock from explicitly declared local, bundled or Git sources.
    /// Neither authored requests nor the accepted lock are changed.
    ///
    /// # Errors
    /// Rejects missing or changed source content instead of implicitly updating the lock.
    pub fn fetch_local_package_project_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let project_path = project_root.into();
        let project = open_project_root(&project_path)?;
        let _guard = transaction::read_guard(&project).map_err(io_error)?;
        transaction::require_complete(&project).map_err(io_error)?;
        let lock = read_project_lock(&project)?;
        let prepared = prepare_local_package_project(
            project,
            &project_path,
            LocalProjectOverrides {
                allow_git: true,
                locked_git: Some(lock.git.clone()),
                ..Default::default()
            },
        )?;
        let dependencies = prepared
            .root
            .dependencies
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let actual =
            ResolutionRecordV1::from_exact_releases(&prepared.root.release, &dependencies)?;
        require_lock(&lock.resolution, &actual)?;
        let actual_lock = lock::ProjectLock::new(actual, prepared.git)?;
        if actual_lock.bytes()? != lock.bytes()? {
            return Err(git::error(
                "Git selections differ from accepted project lock",
            ));
        }
        install(
            &store_root.into(),
            dependencies
                .iter()
                .chain(std::iter::once(&prepared.root.release)),
        )?;
        Ok(lock.resolution)
    }

    /// Validate the authored root and all accepted package bytes in one explicit offline store.
    /// Dependency paths and bundled availability are not consulted during reopening.
    ///
    /// # Errors
    /// Rejects stale root sources/requests, missing closure, modified content or unsafe filesystem entries.
    pub fn open_local_package_project_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let (lock, _) = open(project_root.into(), store_root.into())?;
        Ok(lock)
    }

    /// Copy the accepted exact closure to an explicit offline store without rewriting identities.
    /// The existing no-clobber content-addressed installer owns every publication.
    ///
    /// # Errors
    /// Rejects a stale project, unavailable/modified source closure or conflicting destination entry.
    pub fn vendor_local_package_project_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let (lock, graph) = open(project_root.into(), store_root.into())?;
        install(
            &destination.into(),
            graph.packages().map(|(_, release)| release),
        )?;
        Ok(lock)
    }
}

fn open(
    project_path: PathBuf,
    store_path: PathBuf,
) -> Result<(ResolutionRecordV1, ResolvedPackageGraph), PackagePreparationError> {
    let project = open_project_root(&project_path)?;
    let _guard = transaction::read_guard(&project).map_err(io_error)?;
    transaction::require_complete(&project).map_err(io_error)?;
    let lock = read_project_lock(&project)?;
    let store = DirectoryPackageStore::open_ambient(&store_path)
        .map_err(|error| PackagePreparationError::LocalDirectoryGraph(error.to_string()))?;
    let graph = ExactResolver
        .resolve(&lock.resolution, &store)
        .map_err(PackagePreparationError::Resolution)?;
    let dependencies = graph
        .packages()
        .filter(|(identity, _)| *identity != graph.root())
        .map(|(_, release)| release.clone())
        .collect::<Vec<_>>();
    let mut overrides = LocalProjectOverrides {
        offline: true,
        locked_git: Some(lock.git.clone()),
        ..Default::default()
    };
    retain_releases(&mut overrides.prepared, &dependencies)?;
    let prepared = prepare_local_package_project(project, &project_path, overrides)?;
    let dependencies = prepared
        .root
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let actual = ResolutionRecordV1::from_exact_releases(&prepared.root.release, &dependencies)?;
    require_lock(&lock.resolution, &actual)?;
    Ok((lock.resolution, graph))
}

fn require_lock(
    expected: &ResolutionRecordV1,
    actual: &ResolutionRecordV1,
) -> Result<(), PackagePreparationError> {
    if expected != actual {
        return Err(PackagePreparationError::LocalDirectoryGraph("project sources or requests differ from accepted eqiora.lock; explicitly update the lock".to_owned()));
    }
    Ok(())
}

fn install<'a>(
    store_path: &Path,
    releases: impl Iterator<Item = &'a PackageReleaseV1>,
) -> Result<(), PackagePreparationError> {
    let error = |source| PackagePreparationError::Installation {
        store_root: store_path.to_owned(),
        source,
    };
    let installer = DirectoryPackageInstaller::open_ambient(store_path).map_err(error)?;
    for release in releases {
        let _receipt = installer.install(release).map_err(error)?;
    }
    Ok(())
}

pub(super) fn retain_releases(
    prepared: &mut BTreeMap<PackageKey, PreparedLocalPackage>,
    releases: &[PackageReleaseV1],
) -> Result<(), PackagePreparationError> {
    let releases = releases
        .iter()
        .map(|release| Ok((release.package_identity()?, release)))
        .collect::<Result<BTreeMap<_, _>, PackagePreparationError>>()?;
    for (identity, release) in &releases {
        let mut dependencies = BTreeMap::new();
        let mut pending = release
            .manifest()
            .dependencies()
            .iter()
            .map(|dependency| dependency.target().clone())
            .collect::<Vec<_>>();
        while let Some(target) = pending.pop() {
            if dependencies.contains_key(&target) {
                continue;
            }
            let child = releases.get(&target).ok_or_else(|| {
                PackagePreparationError::MissingDependency {
                    declaring: identity.name.clone(),
                    target: Box::new(target.clone()),
                }
            })?;
            pending.extend(
                child
                    .manifest()
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.target().clone()),
            );
            dependencies.insert(target, (*child).clone());
        }
        let key = (identity.name.clone(), identity.version.clone());
        if let Some(existing) = prepared.get(&key) {
            if existing.release != **release {
                return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                    "conflicting exact release `{}@{}`",
                    identity.name, identity.version
                )));
            }
        } else {
            prepared.insert(
                key,
                PreparedLocalPackage {
                    release: (*release).clone(),
                    dependencies,
                },
            );
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> PackagePreparationError {
    PackagePreparationError::LocalDirectoryGraph(error.to_string())
}
