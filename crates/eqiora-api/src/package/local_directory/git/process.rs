//! Linux resource limits apply to Git and its inherited process group.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use super::super::{PackagePreparationError, error};

pub(super) const DEADLINE: Duration = Duration::from_secs(90);
const OUTPUT_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn run(
    home: &Path,
    repo: &Path,
    args: &[&str],
    deadline: Instant,
) -> Result<Vec<u8>, PackagePreparationError> {
    run_executable(home, repo, args, deadline, "/usr/bin/git", None)
}

pub(super) fn check_commit(
    home: &Path,
    repo: &Path,
    commit: &str,
    deadline: Instant,
) -> Result<bool, PackagePreparationError> {
    if !super::super::is_commit(commit) {
        return Err(error("invalid immutable commit probe"));
    }
    let input = format!("{commit}\n");
    let output = run_executable(
        home,
        repo,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        deadline,
        "/usr/bin/git",
        Some(input.as_bytes()),
    )?;
    if output == format!("{commit} missing\n").as_bytes() {
        return Ok(false);
    }
    if output == format!("{commit} commit\n").as_bytes() {
        return Ok(true);
    }
    Err(error(
        "immutable commit probe returned an invalid object response",
    ))
}

fn run_executable(
    home: &Path,
    repo: &Path,
    args: &[&str],
    deadline: Instant,
    executable: &str,
    input: Option<&[u8]>,
) -> Result<Vec<u8>, PackagePreparationError> {
    if input.is_some_and(|bytes| bytes.len() > 64) {
        return Err(error("contained Git input exceeds 64 bytes"));
    }
    let mut command = Command::new("/usr/bin/prlimit");
    command
        .args([
            "--as=1073741824",
            "--fsize=67108864",
            "--cpu=60",
            "--nofile=64",
            "--",
            executable,
        ])
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.askPass=",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.file.allow=never",
            "-c",
            "http.followRedirects=false",
            "-c",
            "http.lowSpeedLimit=1",
            "-c",
            "http.lowSpeedTime=15",
            "-c",
            "fetch.unpackLimit=0",
            "-c",
            "transfer.unpackLimit=0",
            "-c",
            "pack.threads=1",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "fetch.writeCommitGraph=false",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "core.pager=cat",
        ])
        .args(args)
        .current_dir(repo)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TEMPLATE_DIR", home)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| error("cannot start contained Git process"))?;
    let pid = rustix::process::Pid::from_raw(
        i32::try_from(child.id()).map_err(|_| error("invalid Git process group"))?,
    )
    .ok_or_else(|| error("invalid Git process group"))?;
    if let Some(input) = input {
        let written = child.stdin.take().expect("piped stdin").write_all(input);
        if written.is_err() {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            let _ = child.wait();
            return Err(error("cannot provide contained Git input"));
        }
    }
    let overflow = Arc::new(AtomicBool::new(false));
    let read = |mut stream: Box<dyn Read + Send>, flag: Arc<AtomicBool>, limit: u64| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream.by_ref().take(limit + 1).read_to_end(&mut bytes);
            if result.is_err() || bytes.len() as u64 > limit {
                flag.store(true, Ordering::Release);
            }
            bytes
        })
    };
    let stdout = read(
        Box::new(child.stdout.take().expect("piped stdout")),
        overflow.clone(),
        OUTPUT_BYTES,
    );
    let stderr = read(
        Box::new(child.stderr.take().expect("piped stderr")),
        overflow.clone(),
        64 * 1024,
    );
    let status = loop {
        if overflow.load(Ordering::Acquire) || Instant::now() >= deadline {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
            let _ = child.wait();
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
                break Some(status);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
                let _ = child.wait();
                break None;
            }
        }
    };
    let output = stdout
        .join()
        .map_err(|_| error("Git output reader failed"))?;
    let _discarded = stderr.join(); // Never expose transport-provided diagnostics or URLs.
    if !status.is_some_and(|status| status.success()) || overflow.load(Ordering::Acquire) {
        return Err(error(
            "Git operation rejected, unavailable or exceeded resource limits",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn commit_absence_requires_a_successful_bounded_batch_response() {
        let fixture =
            crate::package::local_directory::tests::TestDirectory::create("git-commit-probe");
        run(
            &fixture.0,
            &fixture.0,
            &["init", "--bare", "."],
            Instant::now() + DEADLINE,
        )
        .unwrap();
        let missing = "ab".repeat(20);
        assert!(
            !check_commit(&fixture.0, &fixture.0, &missing, Instant::now() + DEADLINE).unwrap()
        );
        assert!(check_commit(&fixture.0, &fixture.0, &missing, Instant::now()).is_err());
        assert!(check_commit(&fixture.0, &fixture.0, "HEAD", Instant::now() + DEADLINE).is_err());
    }

    #[test]
    fn contained_process_rejects_deadline_output_and_memory_excess_without_leaking_stderr() {
        let fixture =
            crate::package::local_directory::tests::TestDirectory::create("git-containment");
        let script = fixture.0.join("fake-git");
        for (body, deadline) in [
            ("sleep 10", Instant::now() + Duration::from_millis(20)),
            ("yes SECRET >&2", Instant::now() + Duration::from_secs(3)),
            (
                "exec /usr/bin/python3 -c 'x=bytearray(2147483648)'",
                Instant::now() + Duration::from_secs(3),
            ),
        ] {
            std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let error = run_executable(
                &fixture.0,
                &fixture.0,
                &[],
                deadline,
                script.to_str().unwrap(),
                None,
            )
            .unwrap_err()
            .to_string();
            assert!(!error.contains("SECRET"));
            assert!(error.contains("resource limits"));
        }
    }
}
