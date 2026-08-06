// These process checks own the trailing-delimiter boundary independently of
// private Clap parsing or source-shape predicates.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::api::ModelDocument;

use super::ACCEPTED_BYTES;

const INVALID_COMMAND: &[u8] = b"eqiora: invalid command line\nusage: eqiora check <MODEL_PATH>\n";
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[allow(clippy::option_env_unwrap)]
fn binary() -> &'static OsStr {
    OsStr::new(
        option_env!("CARGO_BIN_EXE_eqiora")
            .expect("production binary is absent; oracle remains intentionally RED"),
    )
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let tmpdir = PathBuf::from(
            std::env::var_os("TMPDIR")
                .expect("TMPDIR must name an absolute home-backed scratch root"),
        );
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is required"));
        assert!(tmpdir.is_absolute());
        assert!(tmpdir.starts_with(&home));
        assert!(!tmpdir.starts_with("/tmp"));
        fs::create_dir_all(&tmpdir).unwrap();
        let path = tmpdir.join(format!(
            "eqiora-cli-trailing-bare-delimiter-{}-{}",
            std::process::id(),
            SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).expect("remove exact trailing-delimiter scratch root");
        }
    }
}

fn run(args: &[&str], cwd: &Path) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("COLUMNS", "17")
        .env("LC_ALL", "C")
        .env("LANG", "fr_FR.UTF-8")
        .output()
        .expect("run CLI candidate")
}

fn assert_output(output: &Output, code: i32, stdout: &[u8], stderr: &[u8]) {
    assert_eq!(output.status.code(), Some(code));
    assert_eq!(output.stdout, stdout);
    assert_eq!(output.stderr, stderr);
}

fn accepted_stdout(filename: &str) -> Vec<u8> {
    let document = ModelDocument::compile(
        filename,
        std::str::from_utf8(ACCEPTED_BYTES).expect("accepted fixture is exact UTF-8"),
    )
    .expect("accepted fixture compiles through the independent direct operation");
    format!("accepted {}\n", document.structural_fingerprint().unwrap()).into_bytes()
}

#[test]
fn root_help_with_trailing_bare_delimiter_is_invalid() {
    let scratch = Scratch::new();
    assert_output(
        &run(&["--help", "--"], scratch.path()),
        64,
        b"",
        INVALID_COMMAND,
    );
}

#[test]
fn version_with_trailing_bare_delimiter_is_invalid() {
    let scratch = Scratch::new();
    assert_output(
        &run(&["--version", "--"], scratch.path()),
        64,
        b"",
        INVALID_COMMAND,
    );
}

#[test]
fn check_help_with_trailing_bare_delimiter_is_invalid() {
    let scratch = Scratch::new();
    assert_output(
        &run(&["check", "--help", "--"], scratch.path()),
        64,
        b"",
        INVALID_COMMAND,
    );
}

#[test]
fn accepted_path_with_trailing_bare_delimiter_is_invalid() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("accepted-secret.eqi"), ACCEPTED_BYTES).unwrap();
    assert_output(
        &run(&["check", "accepted-secret.eqi", "--"], scratch.path()),
        64,
        b"",
        INVALID_COMMAND,
    );
}

#[test]
fn required_delimiter_and_literal_hyphen_filenames_remain_accepted() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("accepted-secret.eqi"), ACCEPTED_BYTES).unwrap();
    fs::write(scratch.path().join("-secret.eqi"), ACCEPTED_BYTES).unwrap();
    fs::write(scratch.path().join("--"), ACCEPTED_BYTES).unwrap();

    for filename in ["accepted-secret.eqi", "-secret.eqi", "--"] {
        let expected = accepted_stdout(filename);
        assert_output(
            &run(&["check", "--", filename], scratch.path()),
            0,
            &expected,
            b"",
        );
    }
}
