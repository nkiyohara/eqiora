//! Internal bounded discovery for a retained author directory capability.

use std::collections::{BTreeMap, BTreeSet};

use cap_fs_ext::DirExt;
use cap_std::fs::Dir;

use crate::directory_io::{DirectoryFileError, read_bounded_regular_file};
use crate::{
    AuthorPackageDirectoryError, AuthorPackageDirectoryResource, ContractError,
    NormalizedRelativePath,
};

const MAX_ENTRIES: usize = 100_000;
const MAX_DIRECTORY_DEPTH: usize = 64;
const MAX_SOURCE_FILES: usize = 10_000;
const MAX_SOURCE_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 256 * 1024 * 1024;

pub(crate) fn discover_project_sources(
    root: &Dir,
) -> Result<BTreeMap<NormalizedRelativePath, String>, AuthorPackageDirectoryError> {
    let mut pending = BTreeSet::<NormalizedRelativePath>::new();
    let mut root_pending = true;
    let mut sources = BTreeMap::new();
    let mut entries_seen = 0_usize;
    let mut source_bytes = 0_usize;

    while root_pending || !pending.is_empty() {
        let parent = if root_pending {
            root_pending = false;
            None
        } else {
            pending.pop_first()
        };
        let directory = match &parent {
            None => root
                .try_clone()
                .map_err(|source| AuthorPackageDirectoryError::RootIo { path: None, source })?,
            Some(path) => root.open_dir_nofollow(path.as_str()).map_err(|source| {
                AuthorPackageDirectoryError::EntryIo {
                    path: path.clone(),
                    source,
                }
            })?,
        };
        let iterator = directory
            .entries()
            .map_err(|source| enumeration_error(parent.clone(), source))?;
        let mut entries = Vec::new();
        for entry in iterator {
            entries_seen = entries_seen.saturating_add(1);
            let entry = entry.map_err(|source| enumeration_error(parent.clone(), source))?;
            let path = child_path(parent.as_ref(), entry.file_name())?;
            check_limit(
                path.clone(),
                AuthorPackageDirectoryResource::ProjectEntries,
                entries_seen,
                MAX_ENTRIES,
            )?;
            entries
                .try_reserve(1)
                .map_err(|source| AuthorPackageDirectoryError::Allocation {
                    path: parent.clone(),
                    source,
                })?;
            entries.push((path, entry));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));

        for (path, entry) in entries {
            let file_type =
                entry
                    .file_type()
                    .map_err(|source| AuthorPackageDirectoryError::EntryIo {
                        path: path.clone(),
                        source,
                    })?;
            if file_type.is_symlink() {
                return Err(AuthorPackageDirectoryError::NonRegularFile { path });
            }
            if file_type.is_dir() {
                check_limit(
                    path.clone(),
                    AuthorPackageDirectoryResource::ProjectDirectoryDepth,
                    path.as_str().split('/').count(),
                    MAX_DIRECTORY_DEPTH,
                )?;
                pending.insert(path);
                continue;
            }
            if !path.as_str().ends_with(".eqi") {
                continue;
            }
            if !file_type.is_file() {
                return Err(AuthorPackageDirectoryError::NonRegularFile { path });
            }
            check_limit(
                path.clone(),
                AuthorPackageDirectoryResource::ProjectSourceFiles,
                sources.len().saturating_add(1),
                MAX_SOURCE_FILES,
            )?;
            let bytes = read_bounded_regular_file(root, &path, MAX_SOURCE_FILE_BYTES)
                .map_err(map_file_error)?;
            source_bytes = source_bytes.saturating_add(bytes.len());
            check_limit(
                path.clone(),
                AuthorPackageDirectoryResource::SourceTotalBytes,
                source_bytes,
                MAX_SOURCE_BYTES,
            )?;
            let source = String::from_utf8(bytes).map_err(|error| {
                AuthorPackageDirectoryError::Contract(ContractError::new(format!(
                    "project source {path} is not UTF-8: {error}"
                )))
            })?;
            sources.insert(path, source);
        }
    }
    Ok(sources)
}

fn enumeration_error(
    parent: Option<NormalizedRelativePath>,
    source: std::io::Error,
) -> AuthorPackageDirectoryError {
    match parent {
        Some(path) => AuthorPackageDirectoryError::EntryIo { path, source },
        None => AuthorPackageDirectoryError::RootIo { path: None, source },
    }
}

fn child_path(
    parent: Option<&NormalizedRelativePath>,
    name: std::ffi::OsString,
) -> Result<NormalizedRelativePath, AuthorPackageDirectoryError> {
    let name = name.into_string().map_err(|_| {
        AuthorPackageDirectoryError::Contract(ContractError::new(match parent {
            Some(parent) => format!("project source directory {parent} contains a non-UTF-8 name"),
            None => "project source root contains a non-UTF-8 name".to_owned(),
        }))
    })?;
    NormalizedRelativePath::parse(parent.map_or(name.clone(), |parent| format!("{parent}/{name}")))
        .map_err(Into::into)
}

fn check_limit(
    path: NormalizedRelativePath,
    resource: AuthorPackageDirectoryResource,
    observed: usize,
    limit: usize,
) -> Result<(), AuthorPackageDirectoryError> {
    if observed <= limit {
        return Ok(());
    }
    Err(AuthorPackageDirectoryError::LimitExceeded {
        path,
        resource,
        observed: u64::try_from(observed).unwrap_or(u64::MAX),
        limit: u64::try_from(limit).unwrap_or(u64::MAX),
    })
}

fn map_file_error(error: DirectoryFileError) -> AuthorPackageDirectoryError {
    match error {
        DirectoryFileError::RootIo(source) => {
            AuthorPackageDirectoryError::RootIo { path: None, source }
        }
        DirectoryFileError::Io { path, source } => {
            AuthorPackageDirectoryError::EntryIo { path, source }
        }
        DirectoryFileError::NonRegularFile { path } => {
            AuthorPackageDirectoryError::NonRegularFile { path }
        }
        DirectoryFileError::LimitExceeded {
            path,
            observed,
            limit,
        } => AuthorPackageDirectoryError::LimitExceeded {
            path,
            resource: AuthorPackageDirectoryResource::SourceFileBytes,
            observed,
            limit,
        },
        DirectoryFileError::Allocation { path, source } => {
            AuthorPackageDirectoryError::Allocation {
                path: Some(path),
                source,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::AuthorPackageDirectory;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "eqiora-project-source-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create project source test root");
            Self(path)
        }

        fn write(&self, path: &str, source: &[u8]) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().expect("source parent"))
                .expect("create source parent");
            fs::write(path, source).expect("write project source");
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_sorted_eqi_sources_and_ignores_regular_decoys() {
        let root = TestDirectory::create("inventory");
        root.write("z/main.eqi", b"model Main {}");
        root.write("a/part.eqi", b"public component Part {}");
        root.write("a/notes.txt", b"not Eqiora source");

        let sources = AuthorPackageDirectory::open_ambient(&root.0)
            .expect("open project root")
            .discover_project_sources()
            .expect("discover project sources");
        let paths = sources.keys().map(ToString::to_string).collect::<Vec<_>>();

        assert_eq!(paths, ["a/part.eqi", "z/main.eqi"]);
        assert_eq!(sources.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_without_following_them() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::create("symlink");
        root.write("target.eqi", b"model Hidden {}");
        symlink("target.eqi", root.0.join("main.eqi")).expect("create source symlink");

        assert!(matches!(
            AuthorPackageDirectory::open_ambient(&root.0)
                .expect("open project root")
                .discover_project_sources(),
            Err(AuthorPackageDirectoryError::NonRegularFile { path })
                if path.as_str() == "main.eqi"
        ));
    }

    #[test]
    fn project_resource_limit_rejects_the_first_excess() {
        let path = NormalizedRelativePath::parse("main.eqi").expect("test path");
        assert!(
            check_limit(
                path.clone(),
                AuthorPackageDirectoryResource::ProjectEntries,
                4,
                4,
            )
            .is_ok()
        );
        assert!(matches!(
            check_limit(path, AuthorPackageDirectoryResource::ProjectEntries, 5, 4,),
            Err(AuthorPackageDirectoryError::LimitExceeded {
                resource: AuthorPackageDirectoryResource::ProjectEntries,
                observed: 5,
                limit: 4,
                ..
            })
        ));
    }
}
