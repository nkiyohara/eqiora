//! Shared handle-relative directory I/O invariants for package adapters.

use std::io::Read;
use std::path::Path;

use cap_fs_ext::{
    DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt, OpenOptionsSyncExt,
};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};

use crate::NormalizedRelativePath;

const READ_BUFFER_BYTES: usize = 16 * 1024;

#[derive(Debug)]
pub(crate) enum DirectoryRootError {
    Io(std::io::Error),
    NotDirectory,
}

#[derive(Debug)]
pub(crate) enum DirectoryFileError {
    RootIo(std::io::Error),
    Io {
        path: NormalizedRelativePath,
        source: std::io::Error,
    },
    NonRegularFile {
        path: NormalizedRelativePath,
    },
    LimitExceeded {
        path: NormalizedRelativePath,
        observed: u64,
        limit: u64,
    },
    Allocation {
        path: NormalizedRelativePath,
        source: std::collections::TryReserveError,
    },
}

pub(crate) fn validate_directory(root: &Dir) -> Result<(), DirectoryRootError> {
    let metadata = root.dir_metadata().map_err(DirectoryRootError::Io)?;
    if !metadata.is_dir() {
        return Err(DirectoryRootError::NotDirectory);
    }
    Ok(())
}

pub(crate) fn open_ambient_directory(path: &Path) -> Result<Dir, DirectoryRootError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .maybe_dir(true)
        .follow(FollowSymlinks::No)
        .nonblock(true);
    let root = File::open_ambient_with(path, &options, ambient_authority())
        .map_err(DirectoryRootError::Io)?;
    if !root.metadata().map_err(DirectoryRootError::Io)?.is_dir() {
        return Err(DirectoryRootError::NotDirectory);
    }
    Ok(Dir::from_std_file(root.into_std()))
}

pub(crate) fn read_bounded_regular_file(
    root: &Dir,
    path: &NormalizedRelativePath,
    max_bytes: usize,
) -> Result<Vec<u8>, DirectoryFileError> {
    read_bounded_regular_file_after_metadata(root, path, max_bytes, || {})
}

fn read_bounded_regular_file_after_metadata(
    root: &Dir,
    path: &NormalizedRelativePath,
    max_bytes: usize,
    after_metadata: impl FnOnce(),
) -> Result<Vec<u8>, DirectoryFileError> {
    let mut file = open_regular_file(root, path)?;
    let metadata = file.metadata().map_err(|source| DirectoryFileError::Io {
        path: path.clone(),
        source,
    })?;
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if metadata.len() > limit {
        return Err(DirectoryFileError::LimitExceeded {
            path: path.clone(),
            observed: metadata.len(),
            limit,
        });
    }
    after_metadata();
    let capacity =
        usize::try_from(metadata.len()).map_err(|_| DirectoryFileError::LimitExceeded {
            path: path.clone(),
            observed: metadata.len(),
            limit,
        })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|source| DirectoryFileError::Allocation {
            path: path.clone(),
            source,
        })?;

    let mut chunk = [0_u8; READ_BUFFER_BYTES];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let chunk_len = remaining.min(chunk.len());
        let read =
            read_retrying_interrupts(&mut file, &mut chunk[..chunk_len]).map_err(|source| {
                DirectoryFileError::Io {
                    path: path.clone(),
                    source,
                }
            })?;
        if read == 0 {
            return Ok(bytes);
        }
        let required =
            bytes
                .len()
                .checked_add(read)
                .ok_or_else(|| DirectoryFileError::LimitExceeded {
                    path: path.clone(),
                    observed: u64::MAX,
                    limit,
                })?;
        if required > bytes.capacity() {
            bytes
                .try_reserve_exact(required - bytes.len())
                .map_err(|source| DirectoryFileError::Allocation {
                    path: path.clone(),
                    source,
                })?;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }

    let mut extra = [0_u8; 1];
    let extra_read = read_retrying_interrupts(&mut file, &mut extra).map_err(|source| {
        DirectoryFileError::Io {
            path: path.clone(),
            source,
        }
    })?;
    if extra_read != 0 {
        return Err(DirectoryFileError::LimitExceeded {
            path: path.clone(),
            observed: limit.saturating_add(1),
            limit,
        });
    }
    Ok(bytes)
}

fn open_regular_file(
    root: &Dir,
    path: &NormalizedRelativePath,
) -> Result<File, DirectoryFileError> {
    let mut directory = root.try_clone().map_err(DirectoryFileError::RootIo)?;
    let mut segments = path.as_str().split('/').peekable();
    let mut traversed = String::new();
    while let Some(segment) = segments.next() {
        if !traversed.is_empty() {
            traversed.push('/');
        }
        traversed.push_str(segment);
        if segments.peek().is_none() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No).nonblock(true);
            let file = directory.open_with(segment, &options).map_err(|source| {
                DirectoryFileError::Io {
                    path: path.clone(),
                    source,
                }
            })?;
            if !file
                .metadata()
                .map_err(|source| DirectoryFileError::Io {
                    path: path.clone(),
                    source,
                })?
                .is_file()
            {
                return Err(DirectoryFileError::NonRegularFile { path: path.clone() });
            }
            return Ok(file);
        }

        let component_path = NormalizedRelativePath::parse(traversed.clone()).map_err(|error| {
            DirectoryFileError::Io {
                path: path.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, error),
            }
        })?;
        directory =
            directory
                .open_dir_nofollow(segment)
                .map_err(|source| DirectoryFileError::Io {
                    path: component_path,
                    source,
                })?;
    }

    Err(DirectoryFileError::Io {
        path: path.clone(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "normalized package path has no components",
        ),
    })
}

fn read_retrying_interrupts(file: &mut File, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
    loop {
        match file.read(buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cap_std::ambient_authority;

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("eqdio-{}-{nonce:x}", std::process::id()));
            fs::create_dir(&path).expect("create directory I/O test root");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn growth_after_metadata_is_detected_by_the_one_byte_probe() {
        let directory = TestDirectory::create();
        let entry = directory.0.join("entry.json");
        fs::write(&entry, b"1234").expect("write initial entry");
        let root = Dir::open_ambient_dir(&directory.0, ambient_authority())
            .expect("open caller-owned root");
        let path = NormalizedRelativePath::parse("entry.json").expect("normalized entry path");

        let error = read_bounded_regular_file_after_metadata(&root, &path, 5, || {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&entry)
                .expect("reopen entry after metadata");
            file.write_all(b"56").expect("grow entry beyond limit");
        })
        .expect_err("growth beyond the active limit must fail closed");

        assert!(matches!(
            error,
            DirectoryFileError::LimitExceeded {
                observed: 6,
                limit: 5,
                ..
            }
        ));
    }
}
