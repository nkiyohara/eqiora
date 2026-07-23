//! Atomic, no-clobber publication into one explicit local package store.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, File, OpenOptions};

use crate::directory_io::{DirectoryRootError, open_ambient_directory, validate_directory};
use crate::{
    ContractError, DirectoryPackageStore, PackageReleaseV1, PackageStore, SourceBundleDigest,
    StoreError,
};

const MAX_STAGE_CREATE_ATTEMPTS: u32 = 128;
static NEXT_STAGE_ID: AtomicU64 = AtomicU64::new(0);

/// Whether a call published content or accepted an identical occupant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageInstallDisposition {
    /// This call atomically created the exact digest entry.
    Installed,
    /// Canonically identical release content was already installed.
    AlreadyPresent,
}

/// Cleanup state of the same-directory staging name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageStageCleanup {
    /// No staging entry was needed because identical content already existed.
    NotNeeded,
    /// The staging name was removed after publication or a lost race.
    Removed,
    /// Publication committed, but removal of the non-resolver staging name
    /// failed. The error kind is retained for diagnostics; staging garbage
    /// collection remains a separate contract.
    Deferred(std::io::ErrorKind),
}

/// Must-use receipt for one package installation attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "inspect disposition and staging cleanup before discarding an installation receipt"]
pub struct PackageInstallReceipt {
    digest: SourceBundleDigest,
    disposition: PackageInstallDisposition,
    staging_cleanup: PackageStageCleanup,
}

impl PackageInstallReceipt {
    const fn new(
        digest: SourceBundleDigest,
        disposition: PackageInstallDisposition,
        staging_cleanup: PackageStageCleanup,
    ) -> Self {
        Self {
            digest,
            disposition,
            staging_cleanup,
        }
    }

    /// Source-bundle digest naming the accepted store entry.
    #[must_use]
    pub const fn digest(self) -> SourceBundleDigest {
        self.digest
    }

    /// Whether this call published or accepted existing content.
    #[must_use]
    pub const fn disposition(self) -> PackageInstallDisposition {
        self.disposition
    }

    /// Whether staging was unnecessary, removed, or left for policy cleanup.
    #[must_use]
    pub const fn staging_cleanup(self) -> PackageStageCleanup {
        self.staging_cleanup
    }

    /// Whether this call performed the no-clobber publication.
    #[must_use]
    pub const fn was_installed(self) -> bool {
        matches!(self.disposition, PackageInstallDisposition::Installed)
    }
}

/// Filesystem phase retained by a package-installation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PackageInstallIoPhase {
    /// Create one unique, same-directory staging entry.
    CreateStage,
    /// Write all canonical release bytes to the staging entry.
    WriteStage,
    /// Synchronize the complete staging file before publication.
    SynchronizeStage,
    /// Atomically add the final digest name without replacement.
    Publish,
}

impl std::fmt::Display for PackageInstallIoPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CreateStage => "create staging entry",
            Self::WriteStage => "write staging entry",
            Self::SynchronizeStage => "synchronize staging entry",
            Self::Publish => "publish exact digest entry",
        })
    }
}

/// Failure before or during one no-clobber package publication.
#[derive(Debug)]
#[non_exhaustive]
pub enum PackageInstallError {
    /// Opening or inspecting the supplied installation root failed.
    RootIo {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The caller-supplied root handle does not identify a directory.
    RootNotDirectory {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
    },
    /// The release could not produce its closed identity or canonical wire.
    Contract(ContractError),
    /// A handle-relative staging or publication operation failed.
    Io {
        /// Source-bundle digest selected before filesystem mutation.
        digest: SourceBundleDigest,
        /// Exact operation that failed.
        phase: PackageInstallIoPhase,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The filesystem cannot provide the required atomic no-clobber publish.
    UnsupportedAtomicPublish {
        /// Source-bundle digest that was not published.
        digest: SourceBundleDigest,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// Every bounded create-new staging name was already occupied.
    StageNameExhausted {
        /// Source-bundle digest that was not published.
        digest: SourceBundleDigest,
        /// Number of unique names attempted.
        attempts: u32,
    },
    /// An existing exact entry could not be read through the ordinary store.
    ExistingEntry {
        /// Source-bundle digest naming the occupied entry.
        digest: SourceBundleDigest,
        /// Typed bounded-store failure.
        source: StoreError,
    },
    /// An existing exact entry is not a valid package release.
    ExistingContract {
        /// Source-bundle digest naming the occupied entry.
        digest: SourceBundleDigest,
        /// Closed release-contract failure.
        source: ContractError,
    },
    /// An occupied exact name contains different canonical release content.
    DigestCollision(SourceBundleDigest),
}

impl std::fmt::Display for PackageInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootIo { path, source } => match path {
                Some(path) => write!(
                    formatter,
                    "cannot open package installation root {}: {source}",
                    path.display()
                ),
                None => write!(
                    formatter,
                    "cannot inspect package installation root: {source}"
                ),
            },
            Self::RootNotDirectory { path } => match path {
                Some(path) => write!(
                    formatter,
                    "package installation root {} must be a directory",
                    path.display()
                ),
                None => formatter.write_str("package installation root must be a directory"),
            },
            Self::Contract(error) => {
                write!(formatter, "package installation contract failed: {error}")
            }
            Self::Io {
                digest,
                phase,
                source,
            } => write!(formatter, "cannot {phase} for package `{digest}`: {source}"),
            Self::UnsupportedAtomicPublish { digest, source } => write!(
                formatter,
                "filesystem cannot atomically publish package `{digest}` without replacement: {source}"
            ),
            Self::StageNameExhausted { digest, attempts } => write!(
                formatter,
                "cannot create a unique staging entry for package `{digest}` after {attempts} attempts"
            ),
            Self::ExistingEntry { digest, source } => write!(
                formatter,
                "cannot classify existing package store entry `{digest}`: {source}"
            ),
            Self::ExistingContract { digest, source } => write!(
                formatter,
                "existing package store entry `{digest}` is invalid: {source}"
            ),
            Self::DigestCollision(digest) => write!(
                formatter,
                "existing package store entry `{digest}` has different canonical content"
            ),
        }
    }
}

impl std::error::Error for PackageInstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootIo { source, .. }
            | Self::Io { source, .. }
            | Self::UnsupportedAtomicPublish { source, .. } => Some(source),
            Self::Contract(source) | Self::ExistingContract { source, .. } => Some(source),
            Self::ExistingEntry { source, .. } => Some(source),
            Self::RootNotDirectory { .. }
            | Self::StageNameExhausted { .. }
            | Self::DigestCollision(_) => None,
        }
    }
}

impl From<ContractError> for PackageInstallError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

/// Publication-only API over one explicit mutation-capable store authority.
///
/// This adapter deliberately does not implement [`PackageStore`]. It cannot
/// resolve dependencies, discover packages, update a lock, delete an exact
/// entry, or replace existing content. Publication uses a complete,
/// synchronized same-directory staging file and a no-clobber hard link, so a
/// reader observes either no final digest name or one complete release wire.
///
/// On Unix, the stage is created with mode `0600`, which the published hard
/// link retains. V1 is therefore a single-principal local-store contract;
/// shared-store permission policy is deliberately separate.
///
/// A failed post-commit removal is reported as
/// [`PackageStageCleanup::Deferred`] in the successful receipt. The reserved
/// staging name is never a resolver candidate, and staging garbage collection
/// is outside this contract. Persistence of the directory entry across power
/// loss is also a separate durability contract.
#[derive(Clone, Debug)]
pub struct DirectoryPackageInstaller {
    root: Arc<Dir>,
}

impl DirectoryPackageInstaller {
    /// Retain a caller-opened installation root without ambient authority.
    ///
    /// # Errors
    ///
    /// Returns a typed root failure if the handle cannot be inspected as a
    /// directory.
    pub fn try_from_dir(root: Dir) -> Result<Self, PackageInstallError> {
        if let Err(error) = validate_directory(&root) {
            return Err(match error {
                DirectoryRootError::Io(source) => {
                    PackageInstallError::RootIo { path: None, source }
                }
                DirectoryRootError::NotDirectory => {
                    PackageInstallError::RootNotDirectory { path: None }
                }
            });
        }
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// Open and retain one caller-selected installation root.
    ///
    /// The final root component is opened without following a symbolic link.
    /// Later staging and publication remain relative to the retained handle.
    ///
    /// # Errors
    ///
    /// Returns a path-aware root failure if the explicit path cannot be opened
    /// as a non-symlink directory.
    pub fn open_ambient(root: impl Into<PathBuf>) -> Result<Self, PackageInstallError> {
        let root_path = root.into();
        let root = open_ambient_directory(&root_path).map_err(|error| match error {
            DirectoryRootError::Io(source) => PackageInstallError::RootIo {
                path: Some(root_path.clone()),
                source,
            },
            DirectoryRootError::NotDirectory => PackageInstallError::RootNotDirectory {
                path: Some(root_path.clone()),
            },
        })?;
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// Atomically publish one canonical release without replacing an entry.
    ///
    /// Equal existing content is idempotent and avoids staging. Any invalid or
    /// different content already occupying the exact digest name fails closed.
    ///
    /// # Errors
    ///
    /// Returns a typed contract, root, staging, publication, unsupported
    /// filesystem, existing-entry, or digest-collision failure. No error path
    /// intentionally creates or replaces the final digest name.
    pub fn install(
        &self,
        release: &PackageReleaseV1,
    ) -> Result<PackageInstallReceipt, PackageInstallError> {
        self.install_with_hooks(
            release,
            |file, bytes| file.write_all(bytes),
            |_| {},
            |root, name| root.remove_file(name),
        )
    }

    fn install_with_hooks(
        &self,
        release: &PackageReleaseV1,
        write_stage: impl FnOnce(&mut File, &[u8]) -> Result<(), std::io::Error>,
        before_publish: impl FnOnce(SourceBundleDigest),
        cleanup_stage: impl FnOnce(&Dir, &str) -> Result<(), std::io::Error>,
    ) -> Result<PackageInstallReceipt, PackageInstallError> {
        let digest = release.source_digest()?;
        let canonical = release.canonical_json()?;
        if self.existing_is_identical(digest, release)? {
            return Ok(PackageInstallReceipt::new(
                digest,
                PackageInstallDisposition::AlreadyPresent,
                PackageStageCleanup::NotNeeded,
            ));
        }
        let mut stage = self.create_stage(digest)?;
        write_stage(stage.file_mut(), &canonical).map_err(|source| PackageInstallError::Io {
            digest,
            phase: PackageInstallIoPhase::WriteStage,
            source,
        })?;
        stage
            .file_mut()
            .sync_all()
            .map_err(|source| PackageInstallError::Io {
                digest,
                phase: PackageInstallIoPhase::SynchronizeStage,
                source,
            })?;

        stage.close();
        before_publish(digest);
        let exact_name = format!("{digest}.json");
        match self.root.hard_link(stage.name(), &self.root, &exact_name) {
            Ok(()) => {
                let staging_cleanup = stage.cleanup_with(cleanup_stage);
                Ok(PackageInstallReceipt::new(
                    digest,
                    PackageInstallDisposition::Installed,
                    staging_cleanup,
                ))
            }
            Err(publish_error) => {
                let staging_cleanup = stage.cleanup_with(cleanup_stage);
                if self.existing_is_identical(digest, release)? {
                    return Ok(PackageInstallReceipt::new(
                        digest,
                        PackageInstallDisposition::AlreadyPresent,
                        staging_cleanup,
                    ));
                }
                if publish_error.kind() == std::io::ErrorKind::Unsupported {
                    Err(PackageInstallError::UnsupportedAtomicPublish {
                        digest,
                        source: publish_error,
                    })
                } else {
                    Err(PackageInstallError::Io {
                        digest,
                        phase: PackageInstallIoPhase::Publish,
                        source: publish_error,
                    })
                }
            }
        }
    }

    fn create_stage(&self, digest: SourceBundleDigest) -> Result<StagedEntry, PackageInstallError> {
        for _ in 0..MAX_STAGE_CREATE_ATTEMPTS {
            let sequence = NEXT_STAGE_ID.fetch_add(1, Ordering::Relaxed);
            let name = format!(".{digest}.stage-{:x}-{sequence:016x}", std::process::id());
            let mut options = OpenOptions::new();
            options
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No)
                .nonblock(true)
                .sync(false);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match self.root.open_with(&name, &options) {
                Ok(file) => {
                    return Ok(StagedEntry {
                        root: Arc::clone(&self.root),
                        name: Some(name),
                        file: Some(file),
                    });
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    return Err(PackageInstallError::Io {
                        digest,
                        phase: PackageInstallIoPhase::CreateStage,
                        source,
                    });
                }
            }
        }
        Err(PackageInstallError::StageNameExhausted {
            digest,
            attempts: MAX_STAGE_CREATE_ATTEMPTS,
        })
    }

    fn existing_is_identical(
        &self,
        digest: SourceBundleDigest,
        expected: &PackageReleaseV1,
    ) -> Result<bool, PackageInstallError> {
        let root = self
            .root
            .try_clone()
            .map_err(|source| PackageInstallError::ExistingEntry {
                digest,
                source: StoreError::RootIo { path: None, source },
            })?;
        let store = DirectoryPackageStore::try_from_dir(root)
            .map_err(|source| PackageInstallError::ExistingEntry { digest, source })?;
        let Some(bytes) = store
            .load_exact(digest, crate::canonical::MAX_CANONICAL_JSON_BYTES)
            .map_err(|source| PackageInstallError::ExistingEntry { digest, source })?
        else {
            return Ok(false);
        };
        let existing = PackageReleaseV1::from_json(&bytes)
            .map_err(|source| PackageInstallError::ExistingContract { digest, source })?;
        let actual_digest = existing
            .source_digest()
            .map_err(|source| PackageInstallError::ExistingContract { digest, source })?;
        if actual_digest == digest && &existing == expected {
            Ok(true)
        } else {
            Err(PackageInstallError::DigestCollision(digest))
        }
    }
}

struct StagedEntry {
    root: Arc<Dir>,
    name: Option<String>,
    file: Option<File>,
}

impl StagedEntry {
    fn name(&self) -> &str {
        self.name.as_deref().expect("live staging name")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("open staging file")
    }

    fn close(&mut self) {
        drop(self.file.take());
    }

    fn cleanup_with(
        mut self,
        cleanup: impl FnOnce(&Dir, &str) -> Result<(), std::io::Error>,
    ) -> PackageStageCleanup {
        self.close();
        let name = self.name.take().expect("live staging name");
        match cleanup(&self.root, &name) {
            Ok(()) => PackageStageCleanup::Removed,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                PackageStageCleanup::Removed
            }
            Err(error) => PackageStageCleanup::Deferred(error.kind()),
        }
    }
}

impl Drop for StagedEntry {
    fn drop(&mut self) {
        self.close();
        if let Some(name) = self.name.take() {
            let _ = self.root.remove_file(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cap_std::ambient_authority;

    use crate::{
        AuthorManifestV1, BundleEntryV1, BundleRoleV1, CanonicalDeclaration, DeclarationKindV1,
        ExactVersion, NormalizedRelativePath, QualifiedName, SemanticContentV1,
        SemanticDeclarationV1, SourceFileV1, VisibilityV1,
    };

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("eqpi-{}-{nonce:x}", std::process::id()));
            fs::create_dir(&path).expect("create installer test root");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = make_directory_writable(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_directory_writable(path: &Path) -> Result<(), std::io::Error> {
        let mut permissions = fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o700);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
    }

    fn test_release(body: &str) -> PackageReleaseV1 {
        let path = NormalizedRelativePath::parse("src/package.eqi").expect("path");
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse("org.example.Install").expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            vec![],
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        let semantic = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Main").expect("declaration"),
            DeclarationKindV1::Model,
            VisibilityV1::Public,
            CanonicalDeclaration::new(body).expect("canonical declaration"),
        )])
        .expect("semantic");
        PackageReleaseV1::new(
            manifest,
            semantic,
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                body.as_bytes().to_vec(),
            )],
        )
        .expect("release")
    }

    fn assert_receipt(
        outcome: PackageInstallReceipt,
        digest: SourceBundleDigest,
        disposition: PackageInstallDisposition,
        cleanup: PackageStageCleanup,
    ) {
        assert_eq!(outcome.digest(), digest);
        assert_eq!(outcome.disposition(), disposition);
        assert_eq!(outcome.staging_cleanup(), cleanup);
    }

    #[test]
    fn publication_is_atomic_idempotent_and_canonical() {
        let directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let installer =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("open installer");
        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");

        let first = installer
            .install_with_hooks(
                &release,
                |file, bytes| file.write_all(bytes),
                |expected| {
                    assert_eq!(expected, digest);
                    assert_eq!(
                        store
                            .load_exact(digest, usize::MAX)
                            .expect("pre-publish load"),
                        None
                    );
                },
                |root, name| root.remove_file(name),
            )
            .expect("install release");
        assert_receipt(
            first,
            digest,
            PackageInstallDisposition::Installed,
            PackageStageCleanup::Removed,
        );
        let repeated = installer
            .install_with_hooks(
                &release,
                |_, _| panic!("equal occupant must avoid staging"),
                |_| panic!("equal occupant must avoid publication"),
                |_, _| panic!("equal occupant must avoid staging cleanup"),
            )
            .expect("idempotent install");
        assert_receipt(
            repeated,
            digest,
            PackageInstallDisposition::AlreadyPresent,
            PackageStageCleanup::NotNeeded,
        );
        assert_eq!(
            store
                .load_exact(digest, usize::MAX)
                .expect("installed bytes"),
            Some(release.canonical_json().expect("canonical release"))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(directory.0.join(format!("{digest}.json")))
                .expect("published metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn ambient_and_caller_owned_installation_capabilities_are_equivalent() {
        let directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let root = Dir::open_ambient_dir(&directory.0, ambient_authority())
            .expect("caller opens installation root");
        let caller =
            DirectoryPackageInstaller::try_from_dir(root).expect("retain caller-owned root");
        assert_receipt(
            caller.install(&release).expect("caller-owned install"),
            digest,
            PackageInstallDisposition::Installed,
            PackageStageCleanup::Removed,
        );
        let ambient =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("open ambient installer");
        assert_receipt(
            ambient
                .install(&release)
                .expect("ambient idempotent install"),
            digest,
            PackageInstallDisposition::AlreadyPresent,
            PackageStageCleanup::NotNeeded,
        );

        let regular_file =
            fs::File::open(directory.0.join(format!("{digest}.json"))).expect("open release file");
        assert!(matches!(
            DirectoryPackageInstaller::try_from_dir(Dir::from_std_file(regular_file)),
            Err(PackageInstallError::RootNotDirectory { path: None })
        ));
    }

    #[test]
    fn failed_stage_write_never_creates_an_exact_entry() {
        let directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let installer =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("open installer");

        assert!(matches!(
            installer.install_with_hooks(
                &release,
                |_, _| Err(std::io::Error::other("injected stage failure")),
                |_| panic!("failed stage must not reach publication"),
                |root, name| root.remove_file(name),
            ),
            Err(PackageInstallError::Io {
                digest: actual,
                phase: PackageInstallIoPhase::WriteStage,
                ..
            }) if actual == digest
        ));
        assert!(!directory.0.join(format!("{digest}.json")).exists());
        assert_eq!(
            fs::read_dir(&directory.0).expect("list test root").count(),
            0
        );
    }

    #[test]
    fn concurrent_equal_installers_converge_without_replacement() {
        let directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let installer =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("open installer");
        let before_publish = Arc::new(Barrier::new(2));
        let run = |installer: DirectoryPackageInstaller,
                   release: PackageReleaseV1,
                   barrier: Arc<Barrier>| {
            thread::spawn(move || {
                installer.install_with_hooks(
                    &release,
                    |file, bytes| file.write_all(bytes),
                    |_| {
                        barrier.wait();
                    },
                    |root, name| root.remove_file(name),
                )
            })
        };
        let first = run(
            installer.clone(),
            release.clone(),
            Arc::clone(&before_publish),
        );
        let second = run(installer, release.clone(), before_publish);
        let first = first
            .join()
            .expect("first installer")
            .expect("first result");
        let second = second
            .join()
            .expect("second installer")
            .expect("second result");

        assert_eq!(first.digest(), digest);
        assert_eq!(second.digest(), digest);
        assert_eq!(first.staging_cleanup(), PackageStageCleanup::Removed);
        assert_eq!(second.staging_cleanup(), PackageStageCleanup::Removed);
        assert!(matches!(
            (first.disposition(), second.disposition()),
            (
                PackageInstallDisposition::Installed,
                PackageInstallDisposition::AlreadyPresent
            ) | (
                PackageInstallDisposition::AlreadyPresent,
                PackageInstallDisposition::Installed
            )
        ));
        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        assert_eq!(
            store
                .load_exact(digest, usize::MAX)
                .expect("installed bytes"),
            Some(release.canonical_json().expect("canonical release"))
        );
    }

    #[test]
    fn committed_cleanup_failure_is_visible_in_the_receipt_and_retry_is_idempotent() {
        let directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let installer =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("open installer");

        let installed = installer
            .install_with_hooks(
                &release,
                |file, bytes| file.write_all(bytes),
                |_| {},
                |_, _| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            )
            .expect("publication succeeds despite deferred cleanup");
        assert_receipt(
            installed,
            digest,
            PackageInstallDisposition::Installed,
            PackageStageCleanup::Deferred(std::io::ErrorKind::PermissionDenied),
        );
        assert_receipt(
            installer
                .install(&release)
                .expect("retry exact installation"),
            digest,
            PackageInstallDisposition::AlreadyPresent,
            PackageStageCleanup::NotNeeded,
        );

        let final_name = format!("{digest}.json");
        let stage_name = fs::read_dir(&directory.0)
            .expect("list deferred staging entry")
            .map(|entry| entry.expect("directory entry").file_name())
            .find(|name| name.to_string_lossy() != final_name)
            .expect("deferred staging name");
        fs::remove_file(directory.0.join(stage_name)).expect("clean deferred test stage");
    }

    #[test]
    fn target_appearing_after_preflight_is_reclassified_without_overwrite() {
        let equal_directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let canonical = release.canonical_json().expect("canonical release");
        let equal_installer = DirectoryPackageInstaller::open_ambient(&equal_directory.0)
            .expect("open equal-race installer");
        let equal = equal_installer
            .install_with_hooks(
                &release,
                |file, bytes| file.write_all(bytes),
                |_| {
                    fs::write(equal_directory.0.join(format!("{digest}.json")), &canonical)
                        .expect("publish equal racing entry");
                },
                |root, name| root.remove_file(name),
            )
            .expect("equal race is idempotent");
        assert_receipt(
            equal,
            digest,
            PackageInstallDisposition::AlreadyPresent,
            PackageStageCleanup::Removed,
        );

        let different_directory = TestDirectory::create();
        let different = test_release("model Main { relation x = 1; }\n")
            .canonical_json()
            .expect("different canonical release");
        let different_installer = DirectoryPackageInstaller::open_ambient(&different_directory.0)
            .expect("open different-race installer");
        assert!(matches!(
            different_installer.install_with_hooks(
                &release,
                |file, bytes| file.write_all(bytes),
                |_| {
                    fs::write(
                        different_directory.0.join(format!("{digest}.json")),
                        &different,
                    )
                    .expect("publish different racing entry");
                },
                |root, name| root.remove_file(name),
            ),
            Err(PackageInstallError::DigestCollision(actual)) if actual == digest
        ));
        assert_eq!(
            fs::read(different_directory.0.join(format!("{digest}.json")))
                .expect("read different racing entry"),
            different
        );
    }

    #[test]
    fn occupied_invalid_or_different_content_fails_closed() {
        let malformed_directory = TestDirectory::create();
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        fs::write(
            malformed_directory.0.join(format!("{digest}.json")),
            b"{not-json",
        )
        .expect("write malformed occupant");
        let malformed = DirectoryPackageInstaller::open_ambient(&malformed_directory.0)
            .expect("open malformed installer");
        assert!(matches!(
            malformed.install(&release),
            Err(PackageInstallError::ExistingContract { digest: actual, .. })
                if actual == digest
        ));

        let collision_directory = TestDirectory::create();
        let different = test_release("model Main { relation x = 1; }\n");
        fs::write(
            collision_directory.0.join(format!("{digest}.json")),
            different.canonical_json().expect("different release bytes"),
        )
        .expect("write dishonest occupant");
        let collision = DirectoryPackageInstaller::open_ambient(&collision_directory.0)
            .expect("open collision installer");
        assert!(matches!(
            collision.install(&release),
            Err(PackageInstallError::DigestCollision(actual)) if actual == digest
        ));
        assert_eq!(
            fs::read(collision_directory.0.join(format!("{digest}.json")))
                .expect("read unchanged occupant"),
            different.canonical_json().expect("different release bytes")
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_installation_root_cannot_be_redirected_by_path_replacement() {
        let directory = TestDirectory::create();
        let moved = directory.0.with_extension("moved");
        let release = test_release("model Main {}\n");
        let digest = release.source_digest().expect("digest");
        let installer =
            DirectoryPackageInstaller::open_ambient(&directory.0).expect("retain installer root");

        fs::rename(&directory.0, &moved).expect("move retained root");
        fs::create_dir(&directory.0).expect("create replacement root");
        assert_receipt(
            installer
                .install(&release)
                .expect("install into retained root"),
            digest,
            PackageInstallDisposition::Installed,
            PackageStageCleanup::Removed,
        );
        assert!(moved.join(format!("{digest}.json")).is_file());
        assert!(!directory.0.join(format!("{digest}.json")).exists());

        fs::remove_dir(&directory.0).expect("remove replacement root");
        fs::rename(moved, &directory.0).expect("restore retained root for cleanup");
    }
}
