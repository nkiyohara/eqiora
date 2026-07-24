use std::path::PathBuf;
use std::sync::Arc;

use cap_std::fs::Dir;

use crate::canonical::MAX_CANONICAL_JSON_BYTES;
use crate::directory_io::{
    DirectoryFileError, DirectoryRootError, open_ambient_directory, read_bounded_regular_file,
    validate_directory,
};
use crate::{NormalizedRelativePath, PackageStore, SourceBundleDigest, StoreError};

/// A caller-rooted local store backed by an open directory capability.
///
/// The root and entries are opened without following a symlink in their final
/// path component. After construction, entries are opened relative to the
/// retained root handle, so renaming or replacing the caller's root path cannot
/// redirect a load. Only `<root>/<expected-digest>.json` can be read.
///
/// A load returns bytes read from one opened entry; it is not a filesystem
/// snapshot and does not prevent concurrent in-place mutation. The resolver
/// independently decodes and revalidates every returned release identity.
#[derive(Clone, Debug)]
pub struct DirectoryPackageStore {
    root: Arc<Dir>,
}

impl DirectoryPackageStore {
    /// Retain a caller-opened directory capability without acquiring ambient
    /// filesystem authority.
    ///
    /// # Errors
    ///
    /// Returns a typed root error when the supplied handle cannot be inspected
    /// as a directory.
    pub fn try_from_dir(root: Dir) -> Result<Self, StoreError> {
        if let Err(error) = validate_directory(&root) {
            return Err(match error {
                DirectoryRootError::Io(source) => StoreError::RootIo { path: None, source },
                DirectoryRootError::NotDirectory => StoreError::RootNotDirectory { path: None },
            });
        }
        Ok(Self {
            root: Arc::new(root),
        })
    }

    /// Open and retain one explicit store root with ambient authority.
    ///
    /// The caller-selected path is the only ambient lookup. Its final
    /// component is opened without following a symbolic link. Every later
    /// load remains relative to the retained directory handle.
    ///
    /// # Errors
    ///
    /// Returns a path-aware root error when the explicit root cannot be opened
    /// as a non-symlink directory.
    pub fn open_ambient(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root_path = root.into();
        let root = open_ambient_directory(&root_path).map_err(|error| match error {
            DirectoryRootError::Io(source) => StoreError::RootIo {
                path: Some(root_path.clone()),
                source,
            },
            DirectoryRootError::NotDirectory => StoreError::RootNotDirectory {
                path: Some(root_path.clone()),
            },
        })?;
        Ok(Self {
            root: Arc::new(root),
        })
    }

    fn exact_path(expected: SourceBundleDigest) -> Result<NormalizedRelativePath, StoreError> {
        NormalizedRelativePath::parse(format!("{expected}.json")).map_err(StoreError::Contract)
    }
}

impl PackageStore for DirectoryPackageStore {
    fn load_exact(
        &self,
        expected: SourceBundleDigest,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let path = Self::exact_path(expected)?;
        let limit = max_bytes.min(MAX_CANONICAL_JSON_BYTES);
        match read_bounded_regular_file(&self.root, &path, limit) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(DirectoryFileError::RootIo(source)) => {
                Err(StoreError::RootIo { path: None, source })
            }
            Err(DirectoryFileError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(DirectoryFileError::Io { source, .. }) => Err(StoreError::EntryIo {
                digest: expected,
                source,
            }),
            Err(DirectoryFileError::NonRegularFile { .. }) => {
                Err(StoreError::NonRegularEntry(expected))
            }
            Err(DirectoryFileError::LimitExceeded {
                observed, limit, ..
            }) => Err(StoreError::ReleaseTooLarge {
                digest: expected,
                observed,
                limit,
            }),
            Err(DirectoryFileError::Allocation { source, .. }) => Err(StoreError::Allocation {
                digest: expected,
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cap_std::ambient_authority;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create(_label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("eqp-{}-{nonce:x}", std::process::id()));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn directory_store_reads_only_the_exact_digest_entry() {
        let directory = TestDirectory::create("exact");
        let digest = SourceBundleDigest::parse(&"12".repeat(32)).expect("digest");
        let expected = directory.0.join(format!("{digest}.json"));
        fs::write(&expected, b"exact bytes").expect("write entry");
        fs::write(directory.0.join("other.json"), b"other bytes").expect("write decoy");

        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        assert_eq!(
            store.load_exact(digest, 64).expect("load"),
            Some(b"exact bytes".to_vec())
        );
        let absent = SourceBundleDigest::parse(&"13".repeat(32)).expect("absent digest");
        assert_eq!(store.load_exact(absent, 64).expect("load absent"), None);
    }

    #[test]
    fn ambient_and_caller_owned_directory_capabilities_load_the_same_entry() {
        let directory = TestDirectory::create("constructors");
        let digest = SourceBundleDigest::parse(&"23".repeat(32)).expect("digest");
        fs::write(directory.0.join(format!("{digest}.json")), b"exact bytes").expect("write entry");

        let ambient = DirectoryPackageStore::open_ambient(&directory.0)
            .expect("open ambient store")
            .load_exact(digest, 64)
            .expect("load ambient entry");
        let root = Dir::open_ambient_dir(&directory.0, ambient_authority())
            .expect("caller opens directory");
        let retained = DirectoryPackageStore::try_from_dir(root)
            .expect("retain caller directory")
            .load_exact(digest, 64)
            .expect("load retained entry");
        assert_eq!(ambient, retained);

        let regular_file =
            fs::File::open(directory.0.join(format!("{digest}.json"))).expect("open regular file");
        assert!(matches!(
            DirectoryPackageStore::try_from_dir(Dir::from_std_file(regular_file)),
            Err(StoreError::RootNotDirectory { path: None })
        ));
    }

    #[test]
    fn directory_store_rejects_oversize_entries() {
        let directory = TestDirectory::create("oversize");
        let digest = SourceBundleDigest::parse(&"34".repeat(32)).expect("digest");
        fs::write(directory.0.join(format!("{digest}.json")), b"12345").expect("write entry");

        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        assert!(matches!(
            store.load_exact(digest, 4),
            Err(StoreError::ReleaseTooLarge {
                digest: actual,
                observed: 5,
                limit: 4,
            }) if actual == digest
        ));
    }

    #[test]
    fn directory_store_rejects_non_regular_entries() {
        let directory = TestDirectory::create("non-regular");
        let digest = SourceBundleDigest::parse(&"45".repeat(32)).expect("digest");
        fs::create_dir(directory.0.join(format!("{digest}.json"))).expect("create directory entry");

        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        assert!(matches!(
            store.load_exact(digest, 64),
            Err(StoreError::NonRegularEntry(actual)) if actual == digest
        ));
    }

    #[cfg(unix)]
    #[test]
    fn directory_store_does_not_follow_entry_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::create("entry-symlink");
        let digest = SourceBundleDigest::parse(&"56".repeat(32)).expect("digest");
        let target = directory.0.join("target.json");
        fs::write(&target, b"target bytes").expect("write target");
        symlink(&target, directory.0.join(format!("{digest}.json"))).expect("create symlink");

        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        assert!(store.load_exact(digest, 64).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn directory_store_does_not_follow_root_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::create("root-symlink");
        let link = directory.0.with_extension("link");
        symlink(&directory.0, &link).expect("create symlink");

        assert!(DirectoryPackageStore::open_ambient(&link).is_err());
        fs::remove_file(link).expect("remove symlink");
    }

    #[cfg(unix)]
    #[test]
    fn directory_store_rejects_special_entries_without_blocking() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        use rustix::fs::{CWD, Mode, mkfifoat};

        let directory = TestDirectory::create("special-entry");
        let digest = SourceBundleDigest::parse(&"67".repeat(32)).expect("digest");
        mkfifoat(
            CWD,
            directory.0.join(format!("{digest}.json")),
            Mode::RUSR | Mode::WUSR,
        )
        .expect("create FIFO entry");

        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || sender.send(store.load_exact(digest, 64)));
        let result = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("opening a FIFO entry must not block");
        reader
            .join()
            .expect("FIFO reader thread")
            .expect("send FIFO result");
        match result.expect_err("special entry must fail closed") {
            StoreError::NonRegularEntry(actual) | StoreError::EntryIo { digest: actual, .. } => {
                assert_eq!(actual, digest);
            }
            error => panic!("unexpected special-entry failure: {error}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn directory_store_cannot_be_redirected_by_replacing_its_root_path() {
        let directory = TestDirectory::create("root-replacement");
        let moved = directory.0.with_extension("moved");
        let digest = SourceBundleDigest::parse(&"78".repeat(32)).expect("digest");
        let name = format!("{digest}.json");
        fs::write(directory.0.join(&name), b"original").expect("write original entry");
        let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open store");

        fs::rename(&directory.0, &moved).expect("move opened root");
        fs::create_dir(&directory.0).expect("create replacement root");
        fs::write(directory.0.join(&name), b"replacement").expect("write replacement entry");

        assert_eq!(
            store.load_exact(digest, 64).expect("load"),
            Some(b"original".to_vec())
        );

        fs::remove_file(directory.0.join(name)).expect("remove replacement entry");
        fs::remove_dir(&directory.0).expect("remove replacement root");
        fs::rename(moved, &directory.0).expect("restore original root for cleanup");
    }
}
