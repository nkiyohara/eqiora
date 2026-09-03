//! Explicit, capability-rooted author source admission.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use cap_std::fs::Dir;

use crate::directory_io::{
    DirectoryFileError, DirectoryRootError, open_ambient_directory, read_bounded_regular_file,
    validate_directory,
};
use crate::source::MAX_TOTAL_BYTES;
use crate::{
    AuthorManifestV1, AuthorPackageSourcesV1, ContractError, NormalizedRelativePath, SourceFileV1,
};

const MANIFEST_PATH: &str = "package.json";

#[derive(Clone, Copy, Debug)]
struct AuthorPackageDirectoryLimits {
    manifest_bytes: usize,
    source_file_bytes: usize,
    source_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
struct ReadBudget {
    bytes: usize,
    resource: AuthorPackageDirectoryResource,
    observed_offset: u64,
    reported_limit: u64,
}

const V1_LIMITS: AuthorPackageDirectoryLimits = AuthorPackageDirectoryLimits {
    manifest_bytes: 16 * 1024 * 1024,
    source_file_bytes: MAX_TOTAL_BYTES,
    source_bytes: MAX_TOTAL_BYTES,
};

/// Resource category enforced while reading one retained author source directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthorPackageDirectoryResource {
    /// Canonical `package.json` bytes.
    ManifestBytes,
    /// Bytes read from one inventoried source file.
    SourceFileBytes,
    /// Aggregate bytes read from all inventoried source files.
    SourceTotalBytes,
    /// Directory entries visited during local-project discovery.
    ProjectEntries,
    /// Directory components below a local-project source root.
    ProjectDirectoryDepth,
    /// `.eqi` files admitted during local-project discovery.
    ProjectSourceFiles,
}

/// Failure while reading one exact package inventory or discovering local
/// project sources from a directory capability.
#[derive(Debug)]
pub enum AuthorPackageDirectoryError {
    /// Opening or inspecting the supplied root capability failed.
    RootIo {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A handle-relative filesystem operation below the retained root failed.
    EntryIo {
        /// Author-root-relative component or file being opened or read.
        path: NormalizedRelativePath,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The manifest or admitted source inventory violated a package contract.
    Contract(ContractError),
    /// The caller-supplied root handle does not identify a directory.
    RootNotDirectory {
        /// Ambient root path, when the adapter opened it for the caller.
        path: Option<PathBuf>,
    },
    /// An inventoried entry is not a regular file.
    NonRegularFile {
        /// Author-root-relative path rejected after no-follow open.
        path: NormalizedRelativePath,
    },
    /// A directory-read resource bound was exceeded.
    LimitExceeded {
        /// Author-root-relative entry whose bound was exceeded.
        path: NormalizedRelativePath,
        /// Resource whose bound was exceeded.
        resource: AuthorPackageDirectoryResource,
        /// Observed count or byte length.
        observed: u64,
        /// Maximum accepted count or byte length.
        limit: u64,
    },
    /// A bounded owned buffer could not be reserved.
    Allocation {
        /// Package-relative entry, or `None` for the inventory vector.
        path: Option<NormalizedRelativePath>,
        /// Allocation failure reported by the standard collection contract.
        source: std::collections::TryReserveError,
    },
}

impl std::fmt::Display for AuthorPackageDirectoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootIo { path, source } => match path {
                Some(path) => write!(
                    formatter,
                    "cannot open author source root {}: {source}",
                    path.display()
                ),
                None => write!(formatter, "cannot inspect author source root: {source}"),
            },
            Self::EntryIo { path, source } => {
                write!(formatter, "cannot read author source path {path}: {source}")
            }
            Self::Contract(error) => write!(formatter, "author source contract failed: {error}"),
            Self::RootNotDirectory { path } => match path {
                Some(path) => write!(
                    formatter,
                    "author source root {} must be a directory",
                    path.display()
                ),
                None => formatter.write_str("author source root must be a directory"),
            },
            Self::NonRegularFile { path } => {
                write!(
                    formatter,
                    "author source path {path} must be a regular file"
                )
            }
            Self::LimitExceeded {
                path,
                resource,
                observed,
                limit,
            } => {
                let name = match resource {
                    AuthorPackageDirectoryResource::ManifestBytes => "manifest bytes",
                    AuthorPackageDirectoryResource::SourceFileBytes => "source-file bytes",
                    AuthorPackageDirectoryResource::SourceTotalBytes => "total source bytes",
                    AuthorPackageDirectoryResource::ProjectEntries => "project entries",
                    AuthorPackageDirectoryResource::ProjectDirectoryDepth => {
                        "project directory depth"
                    }
                    AuthorPackageDirectoryResource::ProjectSourceFiles => "project source files",
                };
                write!(
                    formatter,
                    "author source path {path} has {observed} {name}, exceeding the limit {limit}"
                )
            }
            Self::Allocation { path, source } => {
                if let Some(path) = path {
                    write!(
                        formatter,
                        "cannot allocate author source path {path}: {source}"
                    )
                } else {
                    write!(
                        formatter,
                        "cannot allocate author source inventory: {source}"
                    )
                }
            }
        }
    }
}

impl std::error::Error for AuthorPackageDirectoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootIo { source, .. } | Self::EntryIo { source, .. } => Some(source),
            Self::Contract(error) => Some(error),
            Self::Allocation { source, .. } => Some(source),
            Self::RootNotDirectory { .. }
            | Self::NonRegularFile { .. }
            | Self::LimitExceeded { .. } => None,
        }
    }
}

impl From<ContractError> for AuthorPackageDirectoryError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

/// One explicit author source root retained as a directory capability.
///
/// Package admission reads only `package.json` and its normalized inventory.
/// Local-project discovery is a separate explicit method. Neither operation
/// performs another ambient lookup or follows a symbolic link below the root.
#[derive(Clone, Debug)]
pub struct AuthorPackageDirectory {
    root: Arc<Dir>,
}

impl AuthorPackageDirectory {
    /// Retain a caller-opened directory capability without acquiring ambient
    /// filesystem authority.
    ///
    /// # Errors
    ///
    /// Returns a path-aware error when the supplied handle does not identify
    /// a directory.
    pub fn try_from_dir(root: Dir) -> Result<Self, AuthorPackageDirectoryError> {
        if let Err(error) = validate_directory(&root) {
            return Err(match error {
                DirectoryRootError::Io(source) => {
                    AuthorPackageDirectoryError::RootIo { path: None, source }
                }
                DirectoryRootError::NotDirectory => {
                    AuthorPackageDirectoryError::RootNotDirectory { path: None }
                }
            });
        }
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// Open and retain one explicit author source root with ambient authority.
    ///
    /// The caller-selected path, including any ancestor resolution, is the one
    /// ambient lookup. Its final component is opened without following a
    /// symbolic link. Subsequent reads remain handle-relative to this retained
    /// directory.
    ///
    /// # Errors
    ///
    /// Returns a path-aware I/O or file-kind error when the root cannot be
    /// opened as a non-symlink directory.
    pub fn open_ambient(root: impl Into<PathBuf>) -> Result<Self, AuthorPackageDirectoryError> {
        let root_path = root.into();
        let root = open_ambient_directory(&root_path).map_err(|error| match error {
            DirectoryRootError::Io(source) => AuthorPackageDirectoryError::RootIo {
                path: Some(root_path.clone()),
                source,
            },
            DirectoryRootError::NotDirectory => AuthorPackageDirectoryError::RootNotDirectory {
                path: Some(root_path.clone()),
            },
        })?;
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// Read the closed manifest and exactly its inventoried files.
    ///
    /// Each path component is opened relative to the preceding retained
    /// directory handle without following symbolic links. File metadata is
    /// checked before allocation and the read itself is capped to detect
    /// concurrent growth beyond its budget. The returned source identity binds
    /// the bytes actually read; this operation is not an atomic snapshot of
    /// all files and does not prevent concurrent in-place mutation.
    ///
    /// # Errors
    ///
    /// Returns a path-aware I/O, file-kind, resource, manifest, inventory, or
    /// UTF-8 contract error. No partial author input is returned.
    pub fn read_sources(&self) -> Result<AuthorPackageSourcesV1, AuthorPackageDirectoryError> {
        self.read_sources_with_limits(V1_LIMITS)
    }

    /// Discover a bounded, deterministic inventory of local `.eqi` sources.
    ///
    /// Symbolic links and non-portable names fail closed, regular non-`.eqi`
    /// files are ignored, and every nested directory is opened relative to the
    /// retained capability without following links. Discovery admits at most
    /// 100,000 entries, 64 nested components, 10,000 sources, 16 MiB per
    /// source, and 256 MiB in aggregate.
    ///
    /// # Errors
    ///
    /// Returns a path-aware I/O, file-kind, portability, UTF-8, allocation, or
    /// resource-limit error. No partial inventory is returned.
    pub fn discover_project_sources(
        &self,
    ) -> Result<BTreeMap<NormalizedRelativePath, String>, AuthorPackageDirectoryError> {
        crate::project_directory::discover_project_sources(&self.root)
    }

    fn read_sources_with_limits(
        &self,
        limits: AuthorPackageDirectoryLimits,
    ) -> Result<AuthorPackageSourcesV1, AuthorPackageDirectoryError> {
        let manifest_path = NormalizedRelativePath::parse(MANIFEST_PATH)?;
        let manifest_limit = u64::try_from(limits.manifest_bytes).unwrap_or(u64::MAX);
        let manifest_bytes = self.read_regular_file(
            &manifest_path,
            ReadBudget {
                bytes: limits.manifest_bytes,
                resource: AuthorPackageDirectoryResource::ManifestBytes,
                observed_offset: 0,
                reported_limit: manifest_limit,
            },
        )?;
        let manifest = AuthorManifestV1::from_json(&manifest_bytes)?;
        let mut files = Vec::new();
        files
            .try_reserve_exact(manifest.bundle().len())
            .map_err(|source| AuthorPackageDirectoryError::Allocation { path: None, source })?;
        let mut remaining = limits.source_bytes;
        for entry in manifest.bundle() {
            let budget = if limits.source_file_bytes <= remaining {
                ReadBudget {
                    bytes: limits.source_file_bytes,
                    resource: AuthorPackageDirectoryResource::SourceFileBytes,
                    observed_offset: 0,
                    reported_limit: u64::try_from(limits.source_file_bytes).unwrap_or(u64::MAX),
                }
            } else {
                ReadBudget {
                    bytes: remaining,
                    resource: AuthorPackageDirectoryResource::SourceTotalBytes,
                    observed_offset: u64::try_from(limits.source_bytes.saturating_sub(remaining))
                        .unwrap_or(u64::MAX),
                    reported_limit: u64::try_from(limits.source_bytes).unwrap_or(u64::MAX),
                }
            };
            let bytes = self.read_regular_file(entry.path(), budget)?;
            remaining = remaining
                .checked_sub(bytes.len())
                .ok_or_else(|| ContractError::new("package author source byte count overflow"))?;
            files.push(SourceFileV1::new(entry.path().clone(), entry.role(), bytes));
        }
        AuthorPackageSourcesV1::new(manifest, files).map_err(Into::into)
    }

    fn read_regular_file(
        &self,
        path: &NormalizedRelativePath,
        budget: ReadBudget,
    ) -> Result<Vec<u8>, AuthorPackageDirectoryError> {
        read_bounded_regular_file(&self.root, path, budget.bytes).map_err(|error| match error {
            DirectoryFileError::RootIo(source) => {
                AuthorPackageDirectoryError::RootIo { path: None, source }
            }
            DirectoryFileError::Io { path, source } => {
                AuthorPackageDirectoryError::EntryIo { path, source }
            }
            DirectoryFileError::NonRegularFile { path } => {
                AuthorPackageDirectoryError::NonRegularFile { path }
            }
            DirectoryFileError::LimitExceeded { path, observed, .. } => {
                limit_exceeded(&path, observed, budget)
            }
            DirectoryFileError::Allocation { path, source } => {
                AuthorPackageDirectoryError::Allocation {
                    path: Some(path),
                    source,
                }
            }
        })
    }
}

fn limit_exceeded(
    path: &NormalizedRelativePath,
    observed_bytes: u64,
    budget: ReadBudget,
) -> AuthorPackageDirectoryError {
    AuthorPackageDirectoryError::LimitExceeded {
        path: path.clone(),
        resource: budget.resource,
        observed: budget.observed_offset.saturating_add(observed_bytes),
        limit: budget.reported_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cap_std::ambient_authority;

    use crate::{BundleEntryV1, BundleRoleV1, ExactVersion, QualifiedName};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "eqiora-package-author-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn manifest(entries: &[(&str, BundleRoleV1)]) -> AuthorManifestV1 {
        AuthorManifestV1::new(
            QualifiedName::parse("org.example.DirectoryInput").expect("package name"),
            ExactVersion::parse("1.0.0").expect("package version"),
            vec![],
            entries
                .iter()
                .map(|(path, role)| {
                    BundleEntryV1::new(
                        NormalizedRelativePath::parse(*path).expect("bundle path"),
                        *role,
                    )
                })
                .collect(),
        )
        .expect("author manifest")
    }

    fn write_manifest(root: &TestDirectory, entries: &[(&str, BundleRoleV1)]) -> Vec<u8> {
        let bytes = manifest(entries)
            .canonical_json()
            .expect("canonical manifest");
        fs::write(root.0.join(MANIFEST_PATH), &bytes).expect("write manifest");
        bytes
    }

    fn write_entry(root: &TestDirectory, path: &str, bytes: &[u8]) {
        let path = root.0.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create entry parent");
        }
        fs::write(path, bytes).expect("write entry");
    }

    fn write_valid_package(root: &TestDirectory) {
        write_entry(root, "src/main.eqi", b"model Main {}");
        write_manifest(root, &[("src/main.eqi", BundleRoleV1::ModelSource)]);
    }

    #[cfg(unix)]
    fn bind_socket_entry(root: &TestDirectory) -> std::os::unix::net::UnixListener {
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::net::UnixListener;

        let intended = root.0.join("main.eqi");
        #[cfg(target_os = "linux")]
        let listener = {
            use std::os::fd::AsRawFd;

            let parent = fs::File::open(&root.0).expect("open socket parent");
            let through_parent =
                PathBuf::from(format!("/proc/self/fd/{}/main.eqi", parent.as_raw_fd()));
            let listener = UnixListener::bind(through_parent)
                .expect("create Unix-domain socket entry through retained parent");
            drop(parent);
            listener
        };
        #[cfg(not(target_os = "linux"))]
        let listener = UnixListener::bind(&intended).expect("create Unix-domain socket entry");

        assert!(
            fs::symlink_metadata(&intended)
                .expect("inspect intended socket entry")
                .file_type()
                .is_socket(),
            "the socket must exist at the exact inventoried path"
        );
        listener
    }

    #[test]
    fn ambient_and_caller_owned_directory_capabilities_admit_the_same_sources() {
        let directory = TestDirectory::create("constructors");
        write_valid_package(&directory);

        let ambient = AuthorPackageDirectory::open_ambient(&directory.0)
            .expect("open explicit ambient root")
            .read_sources()
            .expect("load ambient package");
        let root = Dir::open_ambient_dir(&directory.0, ambient_authority())
            .expect("caller opens directory");
        let retained = AuthorPackageDirectory::try_from_dir(root)
            .expect("retain caller directory")
            .read_sources()
            .expect("load retained package");
        assert_eq!(ambient, retained);

        let regular_file = fs::File::open(directory.0.join(MANIFEST_PATH)).expect("open file");
        assert!(matches!(
            AuthorPackageDirectory::try_from_dir(Dir::from_std_file(regular_file)),
            Err(AuthorPackageDirectoryError::RootNotDirectory { .. })
        ));
    }

    #[test]
    fn loading_reads_only_the_closed_manifest_inventory() {
        let directory = TestDirectory::create("closed-inventory");
        fs::write(directory.0.join("unlisted.bin"), b"before").expect("write decoy first");
        write_valid_package(&directory);
        let package = AuthorPackageDirectory::open_ambient(&directory.0).expect("open package");
        let first = package.read_sources().expect("first load");

        fs::write(directory.0.join("unlisted.bin"), b"changed and larger").expect("replace decoy");
        fs::write(
            directory.0.join("another-unlisted.eqi"),
            b"invalid bytes: \xff",
        )
        .expect("write second decoy");
        assert_eq!(package.read_sources().expect("second load"), first);
    }

    #[test]
    fn missing_or_non_regular_inventory_entries_fail_closed_with_the_path() {
        let missing_manifest = TestDirectory::create("missing-manifest");
        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&missing_manifest.0)
                .expect("open root")
                .read_sources(),
            Err(AuthorPackageDirectoryError::EntryIo { path, .. })
                if path.as_str() == MANIFEST_PATH
        ));

        let missing_source = TestDirectory::create("missing-source");
        write_manifest(
            &missing_source,
            &[("src/missing.eqi", BundleRoleV1::ModelSource)],
        );
        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&missing_source.0)
                .expect("open root")
                .read_sources(),
            Err(AuthorPackageDirectoryError::EntryIo { path, .. })
                if matches!(path.as_str(), "src" | "src/missing.eqi")
        ));

        let non_regular = TestDirectory::create("non-regular");
        fs::create_dir(non_regular.0.join("source.eqi")).expect("create directory entry");
        write_manifest(&non_regular, &[("source.eqi", BundleRoleV1::ModelSource)]);
        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&non_regular.0)
                .expect("open root")
                .read_sources(),
            Err(AuthorPackageDirectoryError::NonRegularFile { path })
                if path.as_str() == "source.eqi"
        ));
    }

    #[test]
    fn injected_resource_limits_bound_manifest_inventory_and_source_reads() {
        let directory = TestDirectory::create("limits");
        write_entry(&directory, "a.eqi", b"abc");
        write_entry(&directory, "b.eqi", b"def");
        let manifest_bytes = write_manifest(
            &directory,
            &[
                ("a.eqi", BundleRoleV1::ModelSource),
                ("b.eqi", BundleRoleV1::ModelSource),
            ],
        );
        let package = AuthorPackageDirectory::open_ambient(&directory.0).expect("open package");

        assert!(matches!(
            package.read_sources_with_limits(AuthorPackageDirectoryLimits {
                manifest_bytes: manifest_bytes.len() - 1,
                source_file_bytes: 3,
                source_bytes: 6,
            }),
            Err(AuthorPackageDirectoryError::LimitExceeded {
                path,
                resource: AuthorPackageDirectoryResource::ManifestBytes,
                observed,
                limit,
            }) if path.as_str() == MANIFEST_PATH
                && observed == u64::try_from(manifest_bytes.len()).expect("manifest length")
                && limit == u64::try_from(manifest_bytes.len() - 1).expect("manifest limit")
        ));
        assert!(matches!(
            package.read_sources_with_limits(AuthorPackageDirectoryLimits {
                manifest_bytes: manifest_bytes.len(),
                source_file_bytes: 3,
                source_bytes: 5,
            }),
            Err(AuthorPackageDirectoryError::LimitExceeded {
                path,
                resource: AuthorPackageDirectoryResource::SourceTotalBytes,
                observed: 6,
                limit: 5,
            }) if path.as_str() == "b.eqi"
        ));

        assert!(matches!(
            package.read_sources_with_limits(AuthorPackageDirectoryLimits {
                manifest_bytes: manifest_bytes.len(),
                source_file_bytes: 2,
                source_bytes: 6,
            }),
            Err(AuthorPackageDirectoryError::LimitExceeded {
                path,
                resource: AuthorPackageDirectoryResource::SourceFileBytes,
                observed: 3,
                limit: 2,
            }) if path.as_str() == "a.eqi"
        ));
    }

    #[test]
    fn invalid_model_source_utf8_remains_a_source_contract_failure() {
        let directory = TestDirectory::create("utf8");
        write_entry(&directory, "main.eqi", &[0xff]);
        write_manifest(&directory, &[("main.eqi", BundleRoleV1::ModelSource)]);
        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&directory.0)
                .expect("open root")
                .read_sources(),
            Err(AuthorPackageDirectoryError::Contract(_))
        ));
    }

    #[test]
    fn malformed_manifest_never_reaches_inventory_reads() {
        let directory = TestDirectory::create("malformed-manifest");
        write_entry(&directory, "main.eqi", b"model Main {}");
        fs::write(
            directory.0.join(MANIFEST_PATH),
            br#"{"schema":"eqiora.author-manifest.v1","name":"org.example.Bad","version":"1.0.0","dependencies":[],"bundle":[{"path":"main.eqi","role":"executable_plugin"}]}"#,
        )
        .expect("write malformed manifest");
        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&directory.0)
                .expect("open root")
                .read_sources(),
            Err(AuthorPackageDirectoryError::Contract(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn no_path_component_may_redirect_through_a_symbolic_link() {
        use std::os::unix::fs::symlink;

        let manifest_link = TestDirectory::create("manifest-symlink");
        write_entry(&manifest_link, "main.eqi", b"model Main {}");
        let manifest_bytes = manifest(&[("main.eqi", BundleRoleV1::ModelSource)])
            .canonical_json()
            .expect("manifest bytes");
        fs::write(manifest_link.0.join("manifest-target.json"), manifest_bytes)
            .expect("write manifest target");
        symlink("manifest-target.json", manifest_link.0.join(MANIFEST_PATH))
            .expect("create manifest symlink");
        assert!(
            AuthorPackageDirectory::open_ambient(&manifest_link.0)
                .expect("open root")
                .read_sources()
                .is_err()
        );

        let final_link = TestDirectory::create("final-symlink");
        write_entry(&final_link, "target.eqi", b"model Main {}");
        symlink("target.eqi", final_link.0.join("main.eqi")).expect("create final symlink");
        write_manifest(&final_link, &[("main.eqi", BundleRoleV1::ModelSource)]);
        assert!(
            AuthorPackageDirectory::open_ambient(&final_link.0)
                .expect("open root")
                .read_sources()
                .is_err()
        );

        let intermediate_link = TestDirectory::create("intermediate-symlink");
        let target = intermediate_link.0.join("target");
        fs::create_dir(&target).expect("create target directory");
        fs::write(target.join("main.eqi"), b"model Main {}").expect("write target source");
        symlink("target", intermediate_link.0.join("src")).expect("create intermediate symlink");
        write_manifest(
            &intermediate_link,
            &[("src/main.eqi", BundleRoleV1::ModelSource)],
        );
        assert!(
            AuthorPackageDirectory::open_ambient(&intermediate_link.0)
                .expect("open root")
                .read_sources()
                .is_err()
        );

        let root_target = TestDirectory::create("root-symlink-target");
        write_valid_package(&root_target);
        let root_link = root_target.0.with_extension("link");
        symlink(&root_target.0, &root_link).expect("create root symlink");
        assert!(AuthorPackageDirectory::open_ambient(&root_link).is_err());
        fs::remove_file(root_link).expect("remove root symlink");
    }

    #[cfg(unix)]
    #[test]
    fn special_file_input_fails_without_entering_a_blocking_read() {
        let directory = TestDirectory::create("special-file");
        let _listener = bind_socket_entry(&directory);
        write_manifest(&directory, &[("main.eqi", BundleRoleV1::ModelSource)]);
        assert!(
            AuthorPackageDirectory::open_ambient(&directory.0)
                .expect("open root")
                .read_sources()
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_root_cannot_be_redirected_by_replacing_its_path() {
        let directory = TestDirectory::create("root-replacement");
        write_valid_package(&directory);
        let package = AuthorPackageDirectory::open_ambient(&directory.0).expect("open package");
        let expected = package.read_sources().expect("original package");
        let moved = directory.0.with_extension("moved");

        fs::rename(&directory.0, &moved).expect("move retained root");
        fs::create_dir(&directory.0).expect("create replacement root");
        write_entry(&directory, "src/main.eqi", b"model Replacement {}");
        write_manifest(&directory, &[("src/main.eqi", BundleRoleV1::ModelSource)]);
        assert_eq!(package.read_sources().expect("retained package"), expected);

        fs::remove_dir_all(&directory.0).expect("remove replacement root");
        fs::rename(moved, &directory.0).expect("restore original root for cleanup");
    }
}
