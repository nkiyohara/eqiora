//! Recoverable publication of the author manifest and its accepted lock.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_fs_ext::{
    DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt, OpenOptionsSyncExt,
};
use cap_std::fs::{Dir, OpenOptions};

const PENDING: &str = ".eqiora-project-pending";
const GUARD: &str = ".eqiora-project-write";
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) fn write_guard(project: &Dir) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .follow(FollowSymlinks::No)
        .nonblock(true);
    let file = project.open_with(GUARD, &options)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other(
            "project write guard is not a regular file",
        ));
    }
    let file = file.into_std();
    file.try_lock().map_err(std::io::Error::other)?;
    Ok(file)
}

pub(super) fn read_guard(project: &Dir) -> std::io::Result<Option<std::fs::File>> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = match project.open_with(GUARD, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other(
            "project write guard is not a regular file",
        ));
    }
    let file = file.into_std();
    file.try_lock_shared().map_err(std::io::Error::other)?;
    Ok(Some(file))
}

/// Read the accepted lock without repairing an interrupted publication.
/// The caller retains a shared guard for the duration of the read.
pub(super) fn accepted_lock(project: &Dir) -> std::io::Result<Vec<u8>> {
    match project.open_dir_nofollow(PENDING) {
        Ok(journal) if !committed(&journal)? => {
            read(&journal, "lock.before", super::MAX_PROJECT_LOCK_BYTES)
        }
        Ok(_) => read(project, super::PROJECT_LOCK, super::MAX_PROJECT_LOCK_BYTES),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            read(project, super::PROJECT_LOCK, super::MAX_PROJECT_LOCK_BYTES)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn require_complete(project: &Dir) -> std::io::Result<()> {
    match project.symlink_metadata(PENDING) {
        Ok(_) => Err(std::io::Error::other(
            "interrupted project update; run package lock to recover",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn read(project: &Dir, name: &str, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = project.open_with(name, &options)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(std::io::Error::other("invalid project transaction file"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::other(
            "project transaction file exceeds its limit",
        ));
    }
    Ok(bytes)
}

fn create(project: &Dir, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    create_with(project, name, bytes, |file, bytes| file.write_all(bytes))
}

fn create_with(
    project: &Dir,
    name: &str,
    bytes: &[u8],
    write: impl FnOnce(&mut cap_std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No)
        .nonblock(true);
    let mut file = project.open_with(name, &options)?;
    write(&mut file, bytes)?;
    file.sync_all()
}

fn sync(directory: &Dir) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .maybe_dir(true)
        .follow(FollowSymlinks::No);
    directory.open_with(".", &options)?.sync_all()
}

fn replace(project: &Dir, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    replace_with(project, name, bytes, |file, bytes| file.write_all(bytes))
}

fn replace_with(
    project: &Dir,
    name: &str,
    bytes: &[u8],
    write: impl FnOnce(&mut cap_std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    match project.symlink_metadata(name) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(std::io::Error::other(
                "project target is not a regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let stage = unique(".eqiora-project-file");
    let result = create_with(project, &stage, bytes, write)
        .and_then(|()| project.rename(&stage, project, name))
        .and_then(|()| sync(project));
    if result.is_err() {
        let _ = project.remove_file(&stage);
    }
    result
}

fn committed(journal: &Dir) -> std::io::Result<bool> {
    match read(journal, "committed", 0) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn discard(project: &Dir, journal: &Dir) -> std::io::Result<()> {
    // Detach the journal before removing any recovery input. An interrupted
    // cleanup must never look like an incomplete transaction.
    let garbage = unique(".eqiora-project-complete");
    project.rename(PENDING, project, &garbage)?;
    sync(project)?;
    remove_journal_files(journal)?;
    project.remove_dir(&garbage)
}

fn remove_journal_files(journal: &Dir) -> std::io::Result<()> {
    for name in ["manifest.before", "lock.before", "lock.absent", "committed"] {
        match journal.remove_file(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Caller holds the project write guard throughout recovery and publication.
pub(super) fn recover(project: &Dir) -> std::io::Result<()> {
    let journal = match project.open_dir_nofollow(PENDING) {
        Ok(journal) => journal,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !committed(&journal)? {
        let manifest = read(
            &journal,
            "manifest.before",
            super::MAX_PROJECT_MANIFEST_BYTES,
        )?;
        let lock = match read(&journal, "lock.before", super::MAX_PROJECT_LOCK_BYTES) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                read(&journal, "lock.absent", 0)?;
                None
            }
            Err(error) => return Err(error),
        };
        replace(project, super::PROJECT_MANIFEST, &manifest)?;
        if let Some(lock) = lock {
            replace(project, super::PROJECT_LOCK, &lock)?;
        } else {
            match project.remove_file(super::PROJECT_LOCK) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            sync(project)?;
        }
    }
    discard(project, &journal)
}

pub(super) fn commit(project: &Dir, manifest: &[u8], lock: &[u8]) -> std::io::Result<()> {
    commit_with(project, manifest, lock, || Ok(()))
}

fn commit_with(
    project: &Dir,
    manifest: &[u8],
    lock: &[u8],
    after_manifest: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<()> {
    commit_using(project, manifest, lock, after_manifest, |file, bytes| {
        file.write_all(bytes)
    })
}

pub(super) fn commit_using(
    project: &Dir,
    manifest: &[u8],
    lock: &[u8],
    after_manifest: impl FnOnce() -> std::io::Result<()>,
    write_lock: impl FnOnce(&mut cap_std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let before_manifest = read(
        project,
        super::PROJECT_MANIFEST,
        super::MAX_PROJECT_MANIFEST_BYTES,
    )?;
    let before_lock = match read(project, super::PROJECT_LOCK, super::MAX_PROJECT_LOCK_BYTES) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if manifest.len() > super::MAX_PROJECT_MANIFEST_BYTES
        || lock.len() > super::MAX_PROJECT_LOCK_BYTES
    {
        return Err(std::io::Error::other(
            "project update exceeds its file limits",
        ));
    }
    let stage = unique(".eqiora-project-stage");
    project.create_dir(&stage)?;
    let journal = project.open_dir_nofollow(&stage)?;
    let prepare = || {
        create(&journal, "manifest.before", &before_manifest)?;
        if let Some(bytes) = before_lock {
            create(&journal, "lock.before", &bytes)?;
        } else {
            create(&journal, "lock.absent", &[])?;
        }
        sync(&journal)?;
        project.rename(&stage, project, PENDING)
    };
    if let Err(error) = prepare() {
        let _ = remove_journal_files(&journal);
        let _ = project.remove_dir(&stage);
        return Err(error);
    }
    sync(project)?;

    let result = replace(project, super::PROJECT_MANIFEST, manifest)
        .and_then(|()| after_manifest())
        .and_then(|()| replace_with(project, super::PROJECT_LOCK, lock, write_lock))
        .and_then(|()| create(&journal, "committed", &[]))
        .and_then(|()| sync(&journal));
    if let Err(error) = result {
        // A failed commit-marker write is not an accepted transaction.
        match journal.remove_file("committed") {
            Ok(()) => sync(&journal)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        recover(project).map_err(|recovery| {
            std::io::Error::other(format!(
                "project update failed: {error}; recovery remains pending: {recovery}"
            ))
        })?;
        return Err(error);
    }
    // Both files are committed; cleanup failure leaves a recognizable journal.
    let _ = discard(project, &journal);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_publication_reads_old_lock_then_recovers() {
        let temp = super::super::tests::TestDirectory::create("transaction-interrupted");
        let project = Dir::open_ambient_dir(&temp.0, cap_std::ambient_authority()).unwrap();
        create(&project, super::super::PROJECT_MANIFEST, b"old manifest").unwrap();
        create(&project, super::super::PROJECT_LOCK, b"old lock").unwrap();
        let guard = write_guard(&project).unwrap();
        assert!(read_guard(&project).is_err());
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            commit_with(&project, b"new manifest", b"new lock", || {
                panic!("interrupted")
            })
        }));
        assert!(interrupted.is_err());
        drop(guard);
        {
            let _reader = read_guard(&project).unwrap();
            assert_eq!(accepted_lock(&project).unwrap(), b"old lock");
            assert!(require_complete(&project).is_err());
            assert!(write_guard(&project).is_err());
            assert_eq!(
                read(&project, super::super::PROJECT_MANIFEST, 100).unwrap(),
                b"new manifest"
            );
        }
        let _writer = write_guard(&project).unwrap();
        recover(&project).unwrap();
        assert_eq!(
            read(&project, super::super::PROJECT_MANIFEST, 100).unwrap(),
            b"old manifest"
        );
        assert_eq!(accepted_lock(&project).unwrap(), b"old lock");
        require_complete(&project).unwrap();
    }

    #[test]
    fn failed_pair_update_restores_both_files() {
        let temp = super::super::tests::TestDirectory::create("transaction-pair");
        let project = Dir::open_ambient_dir(&temp.0, cap_std::ambient_authority()).unwrap();
        create(&project, super::super::PROJECT_MANIFEST, b"old manifest").unwrap();
        create(&project, super::super::PROJECT_LOCK, b"old lock").unwrap();
        let _guard = write_guard(&project).unwrap();
        assert!(write_guard(&project).is_err());
        let result = commit_with(&project, b"new manifest", b"new lock", || {
            Err(std::io::Error::other("injected publication failure"))
        });
        assert!(result.is_err());
        assert_eq!(
            read(&project, super::super::PROJECT_MANIFEST, 100).unwrap(),
            b"old manifest"
        );
        assert_eq!(
            read(&project, super::super::PROJECT_LOCK, 100).unwrap(),
            b"old lock"
        );
        assert!(!project.exists(PENDING));
        commit(&project, b"new manifest", b"new lock").unwrap();
        assert_eq!(
            read(&project, super::super::PROJECT_MANIFEST, 100).unwrap(),
            b"new manifest"
        );
        assert_eq!(
            read(&project, super::super::PROJECT_LOCK, 100).unwrap(),
            b"new lock"
        );
    }

    #[test]
    fn failed_first_lock_preserves_its_absence() {
        let temp = super::super::tests::TestDirectory::create("transaction-first-lock");
        let project = Dir::open_ambient_dir(&temp.0, cap_std::ambient_authority()).unwrap();
        create(&project, super::super::PROJECT_MANIFEST, b"old manifest").unwrap();
        let _guard = write_guard(&project).unwrap();
        assert!(
            commit_with(&project, b"new manifest", b"new lock", || {
                Err(std::io::Error::other("injected failure"))
            })
            .is_err()
        );
        assert!(!project.exists(super::super::PROJECT_LOCK));
        assert_eq!(
            read(&project, super::super::PROJECT_MANIFEST, 100).unwrap(),
            b"old manifest"
        );
    }
}
