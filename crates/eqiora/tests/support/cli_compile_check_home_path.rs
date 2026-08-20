use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

pub(crate) fn resolve_home_backed_tmpdir(tmpdir: &Path, home: &Path) -> Result<PathBuf> {
    if !tmpdir.is_absolute() || !home.is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "HOME and TMPDIR must be absolute",
        ));
    }

    let canonical_home = fs::canonicalize(home)?;
    if !canonical_home.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "HOME must resolve to a directory",
        ));
    }

    let canonical_tmpdir = fs::canonicalize(tmpdir)?;
    if !canonical_tmpdir.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "TMPDIR must resolve to a directory",
        ));
    }
    if !canonical_tmpdir.starts_with(&canonical_home) {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "TMPDIR must be backed by HOME",
        ));
    }

    let os_tmp = Path::new("/tmp");
    let canonical_os_tmp = fs::canonicalize(os_tmp)?;
    if tmpdir.starts_with(os_tmp) || canonical_tmpdir.starts_with(canonical_os_tmp) {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            "TMPDIR must not use the OS temporary directory",
        ));
    }

    Ok(canonical_tmpdir)
}

pub(crate) fn require_canonical_home_backed_tmpdir(tmpdir: &Path, home: &Path) -> Result<()> {
    let canonical_tmpdir = resolve_home_backed_tmpdir(tmpdir, home)?;
    if canonical_tmpdir != tmpdir {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "TMPDIR must use its canonical spelling",
        ));
    }
    Ok(())
}
