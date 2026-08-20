#![cfg(target_os = "linux")]

#[path = "support/cli_compile_check_home_path.rs"]
mod cli_compile_check_home_path;

use std::fs;
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use cli_compile_check_home_path::{
    require_canonical_home_backed_tmpdir, resolve_home_backed_tmpdir,
};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let submitted_tmpdir = PathBuf::from(
            std::env::var_os("TMPDIR")
                .expect("the repository scheduler must supply an existing TMPDIR"),
        );
        let submitted_home = PathBuf::from(std::env::var_os("HOME").expect("HOME must be present"));
        assert!(submitted_tmpdir.is_absolute());
        assert!(submitted_home.is_absolute());

        let authority = fs::canonicalize(&submitted_tmpdir)
            .expect("the scheduler TMPDIR must have existing backing");
        let home = fs::canonicalize(&submitted_home).expect("HOME must have existing backing");
        assert!(authority.is_dir());
        assert!(home.is_dir());
        assert!(authority.starts_with(&home));
        assert!(!authority.starts_with(Path::new("/tmp")));

        for attempt in 0..1_024 {
            let root = authority.join(format!(
                "eqiora-cli-home-path-oracle-{}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create exact owned oracle fixture: {error}"),
            }
        }
        panic!("no unique oracle fixture name remained")
    }

    fn directory(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(&path).expect("create owned oracle directory");
        assert_eq!(fs::canonicalize(&path).unwrap(), path);
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.exists() {
            fs::remove_dir_all(&self.root).expect("remove exact owned oracle fixture");
        }
    }
}

#[test]
fn canonical_home_backed_tmpdir_admission_is_causal() {
    let fixture = Fixture::new();

    // O0: a canonical candidate is accepted through a deliberately long HOME alias.
    let real_home = fixture.directory("o0-real-home");
    let candidate = fixture.directory("o0-real-home/canonical-tmpdir");
    let alias_home = fixture
        .root
        .join(format!("o0-home-alias-{}", "a".repeat(120)));
    symlink(&real_home, &alias_home).expect("create owned long HOME alias");
    assert_eq!(fs::canonicalize(&alias_home).unwrap(), real_home);
    assert_eq!(fs::canonicalize(&candidate).unwrap(), candidate);
    assert!(!candidate.starts_with(&alias_home));
    assert_eq!(
        resolve_home_backed_tmpdir(&candidate, &alias_home).expect("O0 stage 1"),
        candidate
    );
    require_canonical_home_backed_tmpdir(&candidate, &alias_home).expect("O0 stage 2");

    // N1: all backing predicates pass, but a canonical sibling is outside home.
    let n1_home = fixture.directory("n1-home");
    let n1_candidate = fixture.directory("n1-outside/tmpdir");
    assert_eq!(fs::canonicalize(&n1_home).unwrap(), n1_home);
    assert_eq!(fs::canonicalize(&n1_candidate).unwrap(), n1_candidate);
    assert!(!n1_candidate.starts_with(&n1_home));
    assert!(resolve_home_backed_tmpdir(&n1_candidate, &n1_home).is_err());

    // N2: a textual prefix is not component containment.
    let n2_home = fixture.directory("n2/home");
    let n2_candidate = fixture.directory("n2/home-prefix-trap/tmpdir");
    assert!(
        n2_candidate
            .as_os_str()
            .as_bytes()
            .starts_with(n2_home.as_os_str().as_bytes())
    );
    assert!(!n2_candidate.starts_with(&n2_home));
    assert_eq!(fs::canonicalize(&n2_candidate).unwrap(), n2_candidate);
    assert!(resolve_home_backed_tmpdir(&n2_candidate, &n2_home).is_err());

    // N3: stage 1 alone must reject an in-home spelling that resolves outside.
    let n3_home = fixture.directory("n3-home");
    let n3_outside = fixture.directory("n3-outside");
    let n3_target = fixture.directory("n3-outside/tmpdir");
    let n3_escape = n3_home.join("escape");
    symlink(&n3_outside, &n3_escape).expect("create owned escape symlink");
    let n3_candidate = n3_escape.join("tmpdir");
    assert!(n3_candidate.starts_with(&n3_home));
    assert_eq!(fs::canonicalize(&n3_candidate).unwrap(), n3_target);
    assert!(!n3_target.starts_with(&n3_home));
    assert!(resolve_home_backed_tmpdir(&n3_candidate, &n3_home).is_err());

    // N4: stage 1 proves backing and containment before stage 2 rejects spelling.
    let n4_home = fixture.directory("n4-home");
    let n4_target = fixture.directory("n4-home/target/tmpdir");
    let n4_alias = n4_home.join("alias");
    symlink(n4_home.join("target"), &n4_alias).expect("create owned inside-home symlink");
    let n4_candidate = n4_alias.join("tmpdir");
    assert_eq!(
        resolve_home_backed_tmpdir(&n4_candidate, &n4_home).expect("N4 stage 1"),
        n4_target
    );
    assert!(require_canonical_home_backed_tmpdir(&n4_candidate, &n4_home).is_err());

    // N5: containment under / is proven before the read-only /tmp denial.
    let os_root = Path::new("/");
    let os_tmp = Path::new("/tmp");
    assert!(os_root.is_absolute() && os_root.is_dir());
    assert!(os_tmp.is_absolute() && os_tmp.is_dir());
    let canonical_root = fs::canonicalize(os_root).unwrap();
    let canonical_tmp = fs::canonicalize(os_tmp).unwrap();
    assert!(canonical_tmp.starts_with(&canonical_root));
    assert!(resolve_home_backed_tmpdir(os_tmp, os_root).is_err());

    // Long/no-override pressure and the 107/108-byte socket boundary remain
    // owned by the byte-identical Issue #496 scheduler oracle.
}
