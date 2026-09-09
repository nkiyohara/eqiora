//! Freeze author bytes and explicit transport inputs before version selection.

use super::*;

const MAX_FROZEN_BYTES: usize = 256 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Frozen {
    pub packages: BTreeMap<PackageKey, LocalPackageSource>,
    pub requests: selection::Inventory,
    pub prepared: BTreeMap<PackageKey, PreparedLocalPackage>,
    pub git: Vec<lock::GitPin>,
    indexed: BTreeMap<PathBuf, PackageKey>,
    bytes: usize,
}

impl Frozen {
    fn insert_source(
        &mut self,
        key: &PackageKey,
        package: LocalPackageSource,
        requests: selection::Requests,
    ) -> Result<(), PackagePreparationError> {
        self.charge(package.files.iter().map(|file| file.bytes().len()).sum())?;
        if let Some(previous) = self.packages.get(key) {
            if previous.entry != package.entry
                || previous.files != package.files
                || self.requests.get(key) != Some(&requests)
            {
                return Err(conflicting(key));
            }
            // Locations are diagnostic/editor provenance only, never a selection tie-break.
            if package.path < previous.path {
                self.packages.insert(key.clone(), package);
            }
        } else {
            if let Some(previous) = self.prepared.get(key)
                && (previous.release.manifest().entry().as_str() != package.entry
                    || previous.release.source().files() != package.files
                    || self.requests.get(key) != Some(&requests))
            {
                return Err(conflicting(key));
            }
            self.packages.insert(key.clone(), package);
        }
        self.requests.insert(key.clone(), requests);
        Ok(())
    }

    pub fn retain_releases(
        &mut self,
        releases: &[PackageReleaseV1],
        locked: Option<&[lock::RequestPin]>,
    ) -> Result<(), PackagePreparationError> {
        for release in releases {
            let key = (
                release.manifest().name().clone(),
                release.manifest().version().clone(),
            );
            let requests = release
                .manifest()
                .dependencies()
                .iter()
                .map(|dependency| {
                    let name = dependency.target().name.clone();
                    let authored = locked.and_then(|pins| {
                        pins.iter().find(|pin| {
                            pin.declaring == key.0
                                && pin.declaring_version == key.1
                                && pin.dependency == name
                        })
                    });
                    Ok((
                        name,
                        authored.map_or_else(
                            || VersionRequest::parse(dependency.target().version.as_str()),
                            |pin| Ok(pin.request.clone()),
                        )?,
                    ))
                })
                .collect::<Result<selection::Requests, PackagePreparationError>>()?;
            if let Some(previous) = self.packages.get(&key)
                && (previous.entry != release.manifest().entry().as_str()
                    || previous.files != release.source().files()
                    || self.requests.get(&key) != Some(&requests))
            {
                return Err(conflicting(&key));
            }
            self.requests.insert(key, requests);
            self.charge(
                release
                    .source()
                    .files()
                    .iter()
                    .map(|file| file.bytes().len())
                    .sum(),
            )?;
        }
        offline::retain_releases(&mut self.prepared, releases)
    }

    fn charge(&mut self, bytes: usize) -> Result<(), PackagePreparationError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| git::error("frozen candidate byte count overflow"))?;
        if self.bytes > MAX_FROZEN_BYTES || self.requests.len() >= MAX_LOCAL_PACKAGE_DIRECTORIES_V1
        {
            return Err(git::error(
                "explicit candidate inventory exceeds package or byte bounds",
            ));
        }
        Ok(())
    }
}

fn conflicting(key: &PackageKey) -> PackagePreparationError {
    git::error(&format!(
        "conflicting content for package `{}@{}` in explicit candidate inventory",
        key.0, key.1
    ))
}

pub(super) fn load(
    project: &Dir,
    project_path: &Path,
    relative_path: PathBuf,
    depth: usize,
    overrides: &mut LocalProjectOverrides,
    frozen: &mut Frozen,
) -> Result<PackageKey, PackagePreparationError> {
    let location = project_path.join(&relative_path);
    if let Some(key) = frozen.indexed.get(&location) {
        return Ok(key.clone());
    }
    if depth > MAX_LOCAL_DEPENDENCY_DEPTH
        || frozen.indexed.len() >= MAX_LOCAL_PACKAGE_DIRECTORIES_V1
    {
        return Err(git::error(
            "local candidate inventory exceeds package or dependency depth bounds",
        ));
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
    frozen.charge(
        toml::to_string(&manifest)
            .map_err(|_| git::error("cannot retain admitted candidate manifest"))?
            .len(),
    )?;
    let name = QualifiedName::parse(&manifest.package.name)?;
    let version = ExactVersion::parse(&manifest.package.version)?;
    let key = (name.clone(), version.clone());
    frozen.indexed.insert(location, key.clone());

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

    files.sort_by(|left, right| left.path().cmp(right.path()));
    if manifest.dependencies.len() > 4096 {
        return Err(git::error(
            "package request inventory exceeds 4096 dependencies",
        ));
    }
    let requests = manifest
        .dependencies
        .iter()
        .map(|(name, dependency)| {
            Ok((
                QualifiedName::parse(name)?,
                VersionRequest::parse(&dependency.version)?,
            ))
        })
        .collect::<Result<selection::Requests, PackagePreparationError>>()?;
    let package = LocalPackageSource {
        path: project_path.join(&relative_path),
        relative_path: relative_path.clone(),
        source_root,
        name,
        version,
        entry: manifest.package.entry,
        files,
        dependencies: BTreeMap::new(),
    };
    frozen.insert_source(&key, package, requests)?;
    for (declared_name, dependency) in manifest.dependencies {
        let dependency_name = QualifiedName::parse(&declared_name)?;
        let git_sources = dependency
            .sources
            .iter()
            .filter_map(|source| source.git.clone())
            .collect::<Vec<_>>();
        if dependency.sources.is_empty() || dependency.sources.len() > 4096 {
            return Err(git::error(
                "dependency requires a bounded nonempty explicit sources array",
            ));
        }
        for source in dependency.sources {
            if usize::from(source.path.is_some())
                + usize::from(source.bundled)
                + usize::from(source.git.is_some())
                != 1
            {
                return Err(git::error(
                    "candidate source requires exactly one path, bundled or Git source",
                ));
            }
            if let Some(path) = &source.path {
                let path = resolve_dependency_path(&relative_path, path)?;
                if overrides.confined && path.starts_with("..") {
                    return Err(git::error("Git dependency path escapes fetched repository"));
                }
            }
            if let Some(git) = &source.git {
                git.validate()?;
                if overrides.confined && !git.repository.starts_with("https://") {
                    return Err(git::error(
                        "fetched packages cannot authorize ambient local Git repositories",
                    ));
                }
            }
            if overrides.offline {
                continue;
            }
            let actual = if let Some(source) = &source.git {
                if !overrides.allow_git {
                    return Err(git::error(
                        "Git source requires explicit package fetch or update",
                    ));
                }
                if overrides.locked_versions.is_some() {
                    continue;
                }
                let (actual, commit) = git::freeze(
                    source,
                    None,
                    &project_path.join(&relative_path),
                    overrides,
                    frozen,
                )?;
                frozen.git.push(lock::GitPin {
                    declaring: key.0.clone(),
                    declaring_version: key.1.clone(),
                    dependency: dependency_name.clone(),
                    version: actual.1.clone(),
                    request: source.rev.clone(),
                    commit,
                    repository: source
                        .repository
                        .starts_with("https://")
                        .then(|| source.repository.clone()),
                });
                actual
            } else if source.bundled {
                let releases = super::super::standard::closure(&declared_name)?;
                let last = releases.last().expect("nonempty bundled closure");
                let actual = (
                    last.manifest().name().clone(),
                    last.manifest().version().clone(),
                );
                frozen.retain_releases(&releases, None)?;
                actual
            } else {
                let path = resolve_dependency_path(
                    &relative_path,
                    source.path.as_deref().expect("validated path"),
                )?;
                load(project, project_path, path, depth + 1, overrides, frozen)?
            };
            if actual.0 != dependency_name {
                return Err(git::error(&format!(
                    "candidate for `{declared_name}` declares `{}`; transport cannot rename a package",
                    actual.0
                )));
            }
        }
        if !overrides.offline
            && !git_sources.is_empty()
            && let Some(version) = overrides
                .locked_versions
                .as_ref()
                .and_then(|versions| versions.get(&dependency_name))
        {
            let expected = (dependency_name.clone(), version.clone());
            if !frozen.requests.contains_key(&expected) {
                let pins = overrides
                    .locked_git
                    .as_ref()
                    .into_iter()
                    .flatten()
                    .filter(|pin| {
                        pin.declaring == key.0
                            && pin.declaring_version == key.1
                            && pin.dependency == dependency_name
                            && pin.version == *version
                    })
                    .collect::<Vec<_>>();
                if pins.is_empty() {
                    // A newly configured transport may supply the already locked
                    // content; the exact record below still forbids any drift.
                    for source in &git_sources {
                        let (actual, _) = git::freeze(
                            source,
                            None,
                            &project_path.join(&relative_path),
                            overrides,
                            frozen,
                        )?;
                        if actual.0 != dependency_name {
                            return Err(git::error(
                                "Git transport cannot rename the locked package",
                            ));
                        }
                    }
                } else {
                    let actual = git::materialize(
                        &git_sources,
                        &pins,
                        &project_path.join(&relative_path),
                        overrides,
                        frozen,
                    )?;
                    if actual != expected {
                        return Err(git::error(
                            "accepted Git commit does not contain the exact locked package",
                        ));
                    }
                }
            }
        }
    }
    Ok(key)
}
