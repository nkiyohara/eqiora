//! One immutable, validated proposal and the existing project transaction.

use super::*;

/// A frozen package update awaiting explicit publication.
///
/// The proposal preserves authored requests and exact selected content. Commit
/// installs those bytes without selection or acquisition and rejects stale project
/// manifest/lock bytes.
pub struct ProjectUpdate {
    project: Dir,
    before_manifest: Vec<u8>,
    before_lock: Option<Vec<u8>>,
    manifest: Vec<u8>,
    lock: lock::ProjectLock,
    releases: Vec<PackageReleaseV1>,
}

impl ProjectUpdate {
    /// Exact semantic closure selected and validated by this proposal.
    #[must_use]
    pub fn resolution(&self) -> &ResolutionRecordV1 {
        &self.lock.resolution
    }

    /// Canonical request/selection proposal, identical to the lock committed later.
    ///
    /// # Errors
    /// Returns an encoding failure.
    pub fn lock_bytes(&self) -> Result<Vec<u8>, PackagePreparationError> {
        self.lock.bytes()
    }

    /// Deterministic package/request paths explaining each selected dependency.
    #[must_use]
    pub fn explanation(&self) -> String {
        let mut result = String::from(
            "Canonical package order; descending SemVer precedence; first complete solution.\n",
        );
        for pin in &self.lock.requests {
            use std::fmt::Write;
            writeln!(
                &mut result,
                "{}@{} -> {}@{} => {}",
                pin.declaring, pin.declaring_version, pin.dependency, pin.request, pin.selected
            )
            .expect("String write");
        }
        result
    }

    /// Publish this exact proposal through the existing recoverable transaction.
    ///
    /// # Errors
    /// Rejects stale manifest/lock bytes, invalid or unavailable store, installation
    /// failure, or publication failure. A failed transaction retains the previous pair.
    pub fn commit(
        self,
        store_root: impl Into<PathBuf>,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let _guard = transaction::write_guard(&self.project).map_err(io_error)?;
        transaction::recover(&self.project).map_err(io_error)?;
        if transaction::read(&self.project, PROJECT_MANIFEST, MAX_PROJECT_MANIFEST_BYTES)
            .map_err(io_error)?
            != self.before_manifest
            || read_optional_lock(&self.project)? != self.before_lock
        {
            return Err(git::error(
                "project changed since update preview; prepare a new explicit update",
            ));
        }
        let store_root = store_root.into();
        let installer = DirectoryPackageInstaller::open_ambient(&store_root).map_err(|source| {
            PackagePreparationError::Installation {
                store_root: store_root.clone(),
                source,
            }
        })?;
        for release in &self.releases {
            let _receipt = installer.install(release).map_err(|source| {
                PackagePreparationError::Installation {
                    store_root: store_root.clone(),
                    source,
                }
            })?;
        }
        transaction::commit(&self.project, &self.manifest, &self.lock.bytes()?)
            .map_err(io_error)?;
        Ok(self.lock.resolution)
    }
}

impl PackagedModelDocument {
    /// Freeze explicitly configured candidates and preview a validated global update.
    /// No store installation or manifest/lock publication occurs during preview.
    ///
    /// # Errors
    /// Rejects acquisition, request conflicts, ambiguous contents and compiler admission failures.
    pub fn preview_local_package_project_v1(
        project_root: impl Into<PathBuf>,
    ) -> Result<ProjectUpdate, PackagePreparationError> {
        preview(project_root.into(), |_| Ok(false))
    }
}

pub(super) fn preview(
    project_path: PathBuf,
    edit: impl FnOnce(&mut LocalProjectManifest) -> Result<bool, PackagePreparationError>,
) -> Result<ProjectUpdate, PackagePreparationError> {
    let project = open_project_root(&project_path)?;
    let guard = transaction::write_guard(&project).map_err(io_error)?;
    transaction::recover(&project).map_err(io_error)?;
    let before_manifest = transaction::read(&project, PROJECT_MANIFEST, MAX_PROJECT_MANIFEST_BYTES)
        .map_err(io_error)?;
    let before_lock = read_optional_lock(&project)?;
    let mut manifest = before_manifest.clone();
    let mut candidate = decode_project_manifest(&before_manifest)?;
    if edit(&mut candidate)? {
        manifest = toml::to_string_pretty(&candidate)
            .map_err(|error| git::error(&error.to_string()))?
            .into_bytes();
    }
    let prepared = prepare_local_package_project(
        project,
        &project_path,
        LocalProjectOverrides {
            manifest: Some(candidate),
            allow_git: true,
            ..Default::default()
        },
    )?;
    let mut releases = prepared
        .root
        .dependencies
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let resolution = ResolutionRecordV1::from_exact_releases(&prepared.root.release, &releases)?;
    releases.push(prepared.root.release);
    let lock = lock::ProjectLock::new(resolution, prepared.git, prepared.requests)?;
    drop(guard);
    Ok(ProjectUpdate {
        project: prepared.project,
        before_manifest,
        before_lock,
        manifest,
        lock,
        releases,
    })
}

fn read_optional_lock(project: &Dir) -> Result<Option<Vec<u8>>, PackagePreparationError> {
    match transaction::read(project, PROJECT_LOCK, MAX_PROJECT_LOCK_BYTES) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    }
}
fn io_error(error: std::io::Error) -> PackagePreparationError {
    git::error(&format!("project transaction failed: {error}"))
}
