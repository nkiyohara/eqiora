//! Shell-free execution for validated verification targets.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::{
    CargoBuildGroup, EvidenceEnvironment, EvidenceOutput, EvidenceRunner, EvidenceTarget,
    ExecutionKey, normalized_features,
};

/// Shell-free runner for the closed set of system evidence targets.
#[derive(Debug, Clone)]
pub struct SystemEvidenceRunner {
    cargo: OsString,
    python: OsString,
    prepared: Arc<Mutex<BTreeMap<ExecutionKey, PreparedCargoTarget>>>,
}

#[derive(Debug, Clone)]
struct PreparedCargoTarget {
    executable: PathBuf,
    build_stderr: String,
}

impl SystemEvidenceRunner {
    /// Use `CARGO` and `PYTHON` when set, otherwise `cargo` and `python3`.
    #[must_use]
    pub fn from_environment() -> Self {
        Self {
            cargo: env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")),
            python: env::var_os("PYTHON").unwrap_or_else(|| OsString::from("python3")),
            prepared: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn cargo_build_command(&self, root: &Path, targets: &[EvidenceTarget]) -> Command {
        let Some(EvidenceTarget::Cargo(first)) = targets.first() else {
            panic!("Cargo build group must contain a Cargo evidence target");
        };
        let key = CargoBuildGroup::from_target(first);
        let library = first.library_test_name().is_some();
        let mut command = Command::new(&self.cargo);
        command.args(["test", "--locked", "-p", &first.package]);
        if library {
            assert!(targets.iter().all(|target| matches!(
                target,
                EvidenceTarget::Cargo(target) if target.library_test_name().is_some()
            )));
            command.arg("--lib");
        }
        command.args(["--no-run", "--message-format=json"]);
        if !library {
            let tests = targets
                .iter()
                .map(|target| match target {
                    EvidenceTarget::Cargo(target) => target.test.as_str(),
                    EvidenceTarget::PythonInstalledWheel(_) => {
                        panic!("Cargo build group must contain only Cargo evidence targets")
                    }
                })
                .collect::<BTreeSet<_>>();
            for test in tests {
                command.args(["--test", test]);
            }
        }
        if !key.features.is_empty() {
            command.arg("--features").arg(key.features.join(","));
        }
        command.current_dir(root);
        command
    }

    fn command(&self, root: &Path, target: &EvidenceTarget) -> Result<Command, String> {
        let key = ExecutionKey::from_target(target);
        let mut command = match target {
            EvidenceTarget::Cargo(target) => {
                let prepared = self.prepared.lock().unwrap();
                let executable = prepared
                    .get(&key)
                    .map(|prepared| prepared.executable.clone())
                    .ok_or_else(|| {
                        format!(
                            "Cargo evidence execution `{}` has no prepared executable",
                            key.label()
                        )
                    })?;
                let mut command = Command::new(executable);
                if let Some(test) = target.library_test_name() {
                    command.args([test, "--exact"]);
                }
                if target.environment == EvidenceEnvironment::PhysicalMpiCuda {
                    command.arg("--ignored");
                }
                command
            }
            EvidenceTarget::PythonInstalledWheel(target) => {
                let mut command = Command::new(&self.python);
                command.arg(&target.script);
                command
            }
        };
        command.current_dir(root);
        Ok(command)
    }

    fn library_inventory_count(
        &self,
        root: &Path,
        target: &EvidenceTarget,
        ignored: bool,
    ) -> Result<usize, String> {
        let EvidenceTarget::Cargo(target_details) = target else {
            unreachable!("library inventory requires a Cargo target");
        };
        let test = target_details
            .library_test_name()
            .expect("library inventory requires a library target");
        let key = ExecutionKey::from_target(target);
        let executable = self
            .prepared
            .lock()
            .unwrap()
            .get(&key)
            .map(|prepared| prepared.executable.clone())
            .ok_or_else(|| {
                format!(
                    "Cargo evidence execution `{}` has no prepared executable",
                    key.label()
                )
            })?;
        let mut command = Command::new(executable);
        command.args([test, "--exact", "--list", "--format=terse"]);
        if ignored {
            command.arg("--ignored");
        }
        let output = command.current_dir(root).output().map_err(|error| {
            format!("cannot start library evidence inventory for `{test}`: {error}")
        })?;
        if !output.status.success() {
            return Err(format!(
                "library evidence inventory for `{test}` failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let expected = format!("{test}: test");
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| *line == expected)
            .count())
    }

    fn preflight_library_test(&self, root: &Path, target: &EvidenceTarget) -> Result<(), String> {
        let EvidenceTarget::Cargo(target_details) = target else {
            unreachable!("library preflight requires a Cargo target");
        };
        let test = target_details
            .library_test_name()
            .expect("library preflight requires a library target");
        let environment = target_details.environment;
        match self.library_inventory_count(root, target, false)? {
            1 => {}
            0 => {
                return Err(format!(
                    "library evidence test `{test}` is missing from the {} libtest inventory",
                    environment.as_str()
                ));
            }
            count => {
                return Err(format!(
                    "library evidence test `{test}` appears {count} times in the {} libtest inventory",
                    environment.as_str()
                ));
            }
        }
        let ignored = match self.library_inventory_count(root, target, true)? {
            0 => false,
            1 => true,
            count => {
                return Err(format!(
                    "library evidence test `{test}` appears {count} times in the ignored libtest inventory"
                ));
            }
        };
        match (environment, ignored) {
            (EvidenceEnvironment::HostCpu, false)
            | (EvidenceEnvironment::PhysicalMpiCuda, true) => Ok(()),
            (EvidenceEnvironment::HostCpu, true) => Err(format!(
                "library evidence test `{test}` is ignored but host-cpu library evidence requires a non-ignored test"
            )),
            (EvidenceEnvironment::PhysicalMpiCuda, false) => Err(format!(
                "library evidence test `{test}` is not ignored but physical-mpi-cuda library evidence requires an ignored test"
            )),
        }
    }

    fn record_executables(
        &self,
        targets: &[EvidenceTarget],
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<(), String> {
        if targets.first().is_some_and(|target| {
            matches!(
                target,
                EvidenceTarget::Cargo(target) if target.library_test_name().is_some()
            )
        }) {
            return self.record_library_executable(targets, stdout, stderr);
        }
        let expected = targets
            .iter()
            .map(|target| match target {
                EvidenceTarget::Cargo(target) => target.test.clone(),
                EvidenceTarget::PythonInstalledWheel(_) => {
                    panic!("Cargo build group must contain only Cargo evidence targets")
                }
            })
            .collect::<BTreeSet<_>>();
        let mut executables = BTreeMap::new();
        let mut target_diagnostics = BTreeMap::<String, String>::new();
        let mut shared_diagnostics = String::new();
        for line in String::from_utf8_lossy(stdout).lines() {
            let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if message["reason"] == "compiler-message" {
                let Some(rendered) = message["message"]["rendered"].as_str() else {
                    continue;
                };
                let Some(name) = message["target"]["name"].as_str() else {
                    append_stderr(&mut shared_diagnostics, rendered);
                    continue;
                };
                if expected.contains(name) {
                    append_stderr(
                        target_diagnostics.entry(name.to_owned()).or_default(),
                        rendered,
                    );
                } else {
                    append_stderr(&mut shared_diagnostics, rendered);
                }
                continue;
            }
            if message["reason"] != "compiler-artifact" {
                continue;
            }
            let is_test = message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "test"));
            if !is_test {
                continue;
            }
            let Some(name) = message["target"]["name"].as_str() else {
                continue;
            };
            let Some(executable) = message["executable"].as_str() else {
                continue;
            };
            if expected.contains(name) {
                executables.insert(name.to_owned(), PathBuf::from(executable));
            }
        }
        let missing = expected
            .iter()
            .filter(|test| !executables.contains_key(*test))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "Cargo did not report an executable for test target(s): {}",
                missing.join(", ")
            ));
        }
        let cargo_stderr = cargo_stderr_without_progress(&String::from_utf8_lossy(stderr));

        let mut prepared = self.prepared.lock().unwrap();
        for target in targets {
            let EvidenceTarget::Cargo(target_details) = target else {
                unreachable!("Cargo group was checked above");
            };
            let executable = executables
                .get(&target_details.test)
                .expect("all expected executables were checked")
                .clone();
            let mut build_stderr = shared_diagnostics.clone();
            if let Some(diagnostics) = target_diagnostics.get(&target_details.test) {
                append_stderr(&mut build_stderr, diagnostics);
            }
            append_stderr(&mut build_stderr, &cargo_stderr);
            let key = ExecutionKey::from_target(target);
            if let Some(existing) = prepared.get_mut(&key) {
                existing.executable = executable;
                existing.build_stderr = build_stderr;
            } else {
                prepared.insert(
                    key,
                    PreparedCargoTarget {
                        executable,
                        build_stderr,
                    },
                );
            }
        }
        Ok(())
    }

    fn record_library_executable(
        &self,
        targets: &[EvidenceTarget],
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<(), String> {
        let Some(EvidenceTarget::Cargo(first)) = targets.first() else {
            unreachable!("library build group requires a Cargo target");
        };
        let package = &first.package;
        if !targets.iter().all(|target| {
            matches!(
                target,
                EvidenceTarget::Cargo(target)
                    if target.package == *package && target.library_test_name().is_some()
            )
        }) {
            return Err("Cargo library build group contains mismatched targets".to_owned());
        }

        let mut executables = Vec::new();
        let mut diagnostics = String::new();
        for line in String::from_utf8_lossy(stdout).lines() {
            let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if message["reason"] == "compiler-message" {
                if let Some(rendered) = message["message"]["rendered"].as_str() {
                    append_stderr(&mut diagnostics, rendered);
                }
                continue;
            }
            if message["reason"] != "compiler-artifact"
                || !message["package_id"]
                    .as_str()
                    .is_some_and(|package_id| cargo_package_id_matches(package_id, package))
                || !message["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
            {
                continue;
            }
            if let Some(executable) = message["executable"].as_str() {
                executables.push(PathBuf::from(executable));
            }
        }
        if executables.len() != 1 {
            return Err(format!(
                "Cargo did not report exactly one library executable for package `{package}` (found {})",
                executables.len()
            ));
        }
        append_stderr(
            &mut diagnostics,
            &cargo_stderr_without_progress(&String::from_utf8_lossy(stderr)),
        );
        let executable = executables.pop().expect("one executable was checked");
        let mut prepared = self.prepared.lock().unwrap();
        for target in targets {
            prepared.insert(
                ExecutionKey::from_target(target),
                PreparedCargoTarget {
                    executable: executable.clone(),
                    build_stderr: diagnostics.clone(),
                },
            );
        }
        Ok(())
    }

    fn prepared_build_stderr(&self, target: &EvidenceTarget) -> String {
        let key = ExecutionKey::from_target(target);
        self.prepared
            .lock()
            .unwrap()
            .get(&key)
            .map_or_else(String::new, |prepared| prepared.build_stderr.clone())
    }

    fn completed_stderr(
        &self,
        target: &EvidenceTarget,
        child_stderr: &[u8],
        succeeded: bool,
    ) -> String {
        let mut stderr = self.prepared_build_stderr(target);
        append_stderr(&mut stderr, &String::from_utf8_lossy(child_stderr));
        if !succeeded && let EvidenceTarget::Cargo(target) = target {
            if let Some(test) = target.library_test_name() {
                let features = normalized_features(&target.features);
                let mut command = format!("cargo test --locked -p {} --lib", target.package);
                if !features.is_empty() {
                    command.push_str(&format!(" --features {}", features.join(",")));
                }
                command.push_str(&format!(" {test} -- --exact"));
                if target.environment == EvidenceEnvironment::PhysicalMpiCuda {
                    command.push_str(" --ignored");
                }
                append_stderr(
                    &mut stderr,
                    &format!("error: test failed, to rerun pass `{command}`\n"),
                );
            } else {
                append_stderr(
                    &mut stderr,
                    &format!(
                        "error: test failed, to rerun pass `-p {} --test {}`\n",
                        target.package, target.test
                    ),
                );
            }
        }
        stderr
    }
}

impl EvidenceRunner for SystemEvidenceRunner {
    fn build_cargo_group(&self, root: &Path, targets: &[EvidenceTarget]) -> Option<EvidenceOutput> {
        let output = match self.cargo_build_command(root, targets).output() {
            Ok(output) => output,
            Err(error) => {
                return Some(EvidenceOutput {
                    duration_ms: None,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    start_error: Some(format!("cannot start Cargo evidence build: {error}")),
                });
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Some(EvidenceOutput {
                duration_ms: None,
                exit_code: output.status.code(),
                stdout,
                stderr,
                start_error: None,
            });
        }
        if let Err(error) = self.record_executables(targets, &output.stdout, &output.stderr) {
            return Some(EvidenceOutput {
                duration_ms: None,
                exit_code: output.status.code(),
                stdout,
                stderr,
                start_error: Some(error),
            });
        }
        None
    }

    fn run(&self, root: &Path, target: &EvidenceTarget) -> EvidenceOutput {
        if matches!(
            target,
            EvidenceTarget::Cargo(target) if target.library_test_name().is_some()
        ) && let Err(error) = self.preflight_library_test(root, target)
        {
            return EvidenceOutput {
                duration_ms: None,
                exit_code: None,
                stdout: String::new(),
                stderr: self.prepared_build_stderr(target),
                start_error: Some(error),
            };
        }
        let mut command = match self.command(root, target) {
            Ok(command) => command,
            Err(error) => {
                return EvidenceOutput {
                    duration_ms: None,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    start_error: Some(error),
                };
            }
        };
        let started = Instant::now();
        match command.output() {
            Ok(output) => {
                let succeeded = output.status.success();
                let mut child_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                if !succeeded
                    && matches!(
                        target,
                        EvidenceTarget::Cargo(target) if target.library_test_name().is_some()
                    )
                {
                    append_stderr(&mut child_stderr, &String::from_utf8_lossy(&output.stdout));
                }
                let stderr = self.completed_stderr(target, child_stderr.as_bytes(), succeeded);
                EvidenceOutput {
                    duration_ms: Some(
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    ),
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr,
                    start_error: None,
                }
            }
            Err(error) => EvidenceOutput {
                duration_ms: None,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                start_error: Some(format!("cannot start evidence target: {error}")),
            },
        }
    }
}

fn cargo_package_id_matches(package_id: &str, package: &str) -> bool {
    let Some((source, fragment)) = package_id.rsplit_once('#') else {
        return false;
    };
    if let Some((name, _version)) = fragment.split_once('@') {
        name == package
    } else {
        source
            .trim_end_matches('/')
            .rsplit_once('/')
            .is_some_and(|(_, name)| name == package)
    }
}

fn append_stderr(stderr: &mut String, addition: &str) {
    if addition.is_empty() {
        return;
    }
    if !stderr.is_empty() && !stderr.ends_with('\n') {
        stderr.push('\n');
    }
    stderr.push_str(addition);
}

fn cargo_stderr_without_progress(stderr: &str) -> String {
    let mut retained = stderr
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            ![
                "Blocking waiting for file lock",
                "Checking ",
                "Compiling ",
                "Downloaded ",
                "Downloading ",
                "Finished ",
                "Fresh ",
                "Locking ",
                "Running ",
                "Updating ",
                "Waiting ",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !retained.is_empty() && stderr.ends_with('\n') {
        retained.push('\n');
    }
    retained
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CargoEvidenceTarget, PythonEvidenceRunner, PythonInstalledWheelEvidenceTarget};

    fn library_target(
        test: &str,
        features: &[&str],
        environment: EvidenceEnvironment,
    ) -> EvidenceTarget {
        let features = features
            .iter()
            .map(|feature| format!("\"{feature}\""))
            .collect::<Vec<_>>()
            .join(", ");
        toml::from_str(&format!(
            "runner = \"cargo-library-test\"\npackage = \"eqiora-numerics\"\ntest = \"{test}\"\nfeatures = [{features}]\nenvironment = \"{}\"\n",
            environment.as_str()
        ))
        .unwrap()
    }

    #[test]
    fn library_build_and_execution_commands_are_closed_exact_and_reproducible() {
        let runner = SystemEvidenceRunner {
            cargo: OsString::from("cargo-evidence"),
            python: OsString::from("python-evidence"),
            prepared: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let root = Path::new("/repository");
        let first = library_target(
            "private_parent::private_child::registered_evidence",
            &["one", "two"],
            EvidenceEnvironment::HostCpu,
        );
        let second = library_target(
            "private_parent::private_child::second_evidence",
            &["two", "one"],
            EvidenceEnvironment::HostCpu,
        );
        assert!(matches!(
            &first,
            EvidenceTarget::Cargo(CargoEvidenceTarget { test, .. })
                if test == "lib::private_parent::private_child::registered_evidence"
        ));

        let build = runner.cargo_build_command(root, &[first.clone(), second]);
        assert_eq!(build.get_program(), "cargo-evidence");
        assert_eq!(
            build
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "test",
                "--locked",
                "-p",
                "eqiora-numerics",
                "--lib",
                "--no-run",
                "--message-format=json",
                "--features",
                "one,two",
            ]
        );
        assert_eq!(build.get_current_dir(), Some(root));

        runner.prepared.lock().unwrap().insert(
            ExecutionKey::from_target(&first),
            PreparedCargoTarget {
                executable: PathBuf::from("/target/eqiora_numerics-lib"),
                build_stderr: "warning: retained library diagnostic\n".to_owned(),
            },
        );
        let command = runner.command(root, &first).unwrap();
        assert_eq!(command.get_program(), "/target/eqiora_numerics-lib");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "private_parent::private_child::registered_evidence",
                "--exact"
            ]
        );
        assert_eq!(command.get_current_dir(), Some(root));
        assert_eq!(
            runner.completed_stderr(
                &first,
                b"frozen private oracle failure\n\
                  test result: FAILED. 0 passed; 1 failed\n",
                false,
            ),
            "warning: retained library diagnostic\n\
             frozen private oracle failure\n\
             test result: FAILED. 0 passed; 1 failed\n\
             error: test failed, to rerun pass `cargo test --locked -p eqiora-numerics --lib --features one,two private_parent::private_child::registered_evidence -- --exact`\n"
        );

        let physical = library_target(
            "private_parent::private_child::ignored_evidence",
            &["mpi-cuda"],
            EvidenceEnvironment::PhysicalMpiCuda,
        );
        runner.prepared.lock().unwrap().insert(
            ExecutionKey::from_target(&physical),
            PreparedCargoTarget {
                executable: PathBuf::from("/target/eqiora_numerics-lib"),
                build_stderr: String::new(),
            },
        );
        let command = runner.command(root, &physical).unwrap();
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "private_parent::private_child::ignored_evidence",
                "--exact",
                "--ignored"
            ]
        );
        assert_eq!(
            runner.completed_stderr(&physical, b"physical failure\n", false),
            "physical failure\n\
             error: test failed, to rerun pass `cargo test --locked -p eqiora-numerics --lib --features mpi-cuda private_parent::private_child::ignored_evidence -- --exact --ignored`\n"
        );
    }

    #[test]
    fn library_artifact_discovery_accepts_only_one_selected_package_library_executable() {
        let target = library_target(
            "private_parent::private_child::registered_evidence",
            &[],
            EvidenceEnvironment::HostCpu,
        );
        let selected = serde_json::json!({
            "reason": "compiler-artifact",
            "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
            "target": {"name": "eqiora_numerics", "kind": ["lib"]},
            "executable": "/target/eqiora_numerics-lib"
        });
        let dependency = serde_json::json!({
            "reason": "compiler-artifact",
            "package_id": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0",
            "target": {"name": "serde", "kind": ["lib"]},
            "executable": "/target/serde-lib"
        });
        let diagnostic = serde_json::json!({
            "reason": "compiler-message",
            "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
            "target": {"name": "eqiora_numerics", "kind": ["lib"]},
            "message": {"rendered": "warning: selected library diagnostic\n"}
        });
        let stdout = [diagnostic, dependency.clone(), selected.clone()]
            .map(|message| message.to_string())
            .join("\n");
        let runner = SystemEvidenceRunner {
            cargo: OsString::from("cargo-evidence"),
            python: OsString::from("python-evidence"),
            prepared: Arc::new(Mutex::new(BTreeMap::new())),
        };
        runner
            .record_executables(
                std::slice::from_ref(&target),
                stdout.as_bytes(),
                b"   Compiling eqiora-numerics v0.0.0\nwarning: cargo summary\n",
            )
            .unwrap();
        {
            let prepared = runner.prepared.lock().unwrap();
            assert_eq!(prepared.len(), 1);
            let entry = prepared.values().next().unwrap();
            assert_eq!(
                entry.executable,
                PathBuf::from("/target/eqiora_numerics-lib")
            );
            assert_eq!(
                entry.build_stderr,
                "warning: selected library diagnostic\nwarning: cargo summary\n"
            );
        }

        let wrong_artifacts = [
            vec![serde_json::json!({
                "reason": "compiler-artifact",
                "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
                "target": {"name": "eqiora_numerics", "kind": ["test"]},
                "executable": "/target/eqiora_numerics-test"
            })],
            vec![serde_json::json!({
                "reason": "compiler-artifact",
                "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
                "target": {"name": "eqiora_numerics", "kind": ["bin"]},
                "executable": "/target/eqiora_numerics-bin"
            })],
            vec![dependency],
            vec![serde_json::json!({
                "reason": "compiler-artifact",
                "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
                "target": {"name": "eqiora_numerics", "kind": ["lib"]},
                "executable": null
            })],
            vec![
                selected.clone(),
                serde_json::json!({
                    "reason": "compiler-artifact",
                    "package_id": "path+file:///repository/eqiora-numerics#0.0.0",
                    "target": {"name": "eqiora_numerics", "kind": ["lib"]},
                    "executable": "/target/eqiora_numerics-lib-duplicate"
                }),
            ],
        ];
        for artifacts in wrong_artifacts {
            let runner = SystemEvidenceRunner {
                cargo: OsString::from("cargo-evidence"),
                python: OsString::from("python-evidence"),
                prepared: Arc::new(Mutex::new(BTreeMap::new())),
            };
            let stdout = artifacts
                .into_iter()
                .map(|message| message.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            let error = runner
                .record_executables(std::slice::from_ref(&target), stdout.as_bytes(), b"")
                .unwrap_err();
            assert!(error.contains("eqiora-numerics"), "{error}");
            assert!(error.contains("library"), "{error}");
            assert!(runner.prepared.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn system_runner_builds_only_closed_shell_free_commands() {
        let runner = SystemEvidenceRunner {
            cargo: OsString::from("cargo-evidence"),
            python: OsString::from("python-evidence"),
            prepared: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let root = Path::new("/repository");
        let cargo_target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "registered_case".to_owned(),
            features: vec!["one".to_owned(), "two".to_owned()],
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        });
        let cargo = runner.cargo_build_command(root, std::slice::from_ref(&cargo_target));
        assert_eq!(cargo.get_program(), "cargo-evidence");
        assert_eq!(
            cargo
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "test",
                "--locked",
                "-p",
                "eqiora",
                "--no-run",
                "--message-format=json",
                "--test",
                "registered_case",
                "--features",
                "one,two",
            ]
        );
        assert_eq!(cargo.get_current_dir(), Some(root));
        runner.prepared.lock().unwrap().insert(
            ExecutionKey::from_target(&cargo_target),
            PreparedCargoTarget {
                executable: PathBuf::from("/target/registered_case"),
                build_stderr: "warning: compile diagnostic\n".to_owned(),
            },
        );
        let mut equivalent_cargo_target = cargo_target.clone();
        let EvidenceTarget::Cargo(equivalent) = &mut equivalent_cargo_target else {
            unreachable!();
        };
        equivalent.features.reverse();
        equivalent.table = Some("expected/claim-only.csv".to_owned());
        let cargo = runner.command(root, &equivalent_cargo_target).unwrap();
        assert_eq!(cargo.get_program(), "/target/registered_case");
        assert_eq!(cargo.get_args().count(), 0);
        assert_eq!(
            runner.completed_stderr(
                &cargo_target,
                b"thread panicked\n\
                  test result: FAILED. 0 passed; 1 failed\n",
                false
            ),
            "warning: compile diagnostic\n\
             thread panicked\n\
             test result: FAILED. 0 passed; 1 failed\n\
             error: test failed, to rerun pass `-p eqiora --test registered_case`\n"
        );

        let physical_target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "physical_case".to_owned(),
            features: vec!["mpi-cuda".to_owned()],
            table: None,
            environment: EvidenceEnvironment::PhysicalMpiCuda,
        });
        let physical_build =
            runner.cargo_build_command(root, std::slice::from_ref(&physical_target));
        assert_eq!(physical_build.get_program(), "cargo-evidence");
        assert_eq!(
            physical_build
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "test",
                "--locked",
                "-p",
                "eqiora",
                "--no-run",
                "--message-format=json",
                "--test",
                "physical_case",
                "--features",
                "mpi-cuda",
            ]
        );
        runner.prepared.lock().unwrap().insert(
            ExecutionKey::from_target(&physical_target),
            PreparedCargoTarget {
                executable: PathBuf::from("/target/physical_case"),
                build_stderr: String::new(),
            },
        );
        let physical = runner.command(root, &physical_target).unwrap();
        assert_eq!(physical.get_program(), "/target/physical_case");
        assert_eq!(
            physical
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["--ignored"]
        );
        assert_eq!(physical.get_current_dir(), Some(root));

        let python_target =
            EvidenceTarget::PythonInstalledWheel(PythonInstalledWheelEvidenceTarget {
                runner: PythonEvidenceRunner::PythonInstalledWheel,
                script: "tools/ci/python_evidence.py".to_owned(),
                environment: EvidenceEnvironment::HostCpu,
            });
        let python = runner.command(root, &python_target).unwrap();
        assert_eq!(python.get_program(), "python-evidence");
        assert_eq!(
            python
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["tools/ci/python_evidence.py"]
        );
        assert_eq!(python.get_current_dir(), Some(root));
    }

    #[test]
    fn system_runner_retains_non_progress_group_build_diagnostics() {
        let runner = SystemEvidenceRunner {
            cargo: OsString::from("cargo-evidence"),
            python: OsString::from("python-evidence"),
            prepared: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "registered_case".to_owned(),
            features: Vec::new(),
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        });
        let stdout = [
            serde_json::json!({
                "reason": "compiler-message",
                "target": {"name": "eqiora"},
                "message": {"rendered": "warning: shared diagnostic\n"}
            }),
            serde_json::json!({
                "reason": "compiler-message",
                "target": {"name": "registered_case"},
                "message": {"rendered": "warning: target diagnostic\n"}
            }),
            serde_json::json!({
                "reason": "compiler-artifact",
                "target": {"name": "registered_case", "kind": ["test"]},
                "executable": "/target/registered_case"
            }),
        ]
        .map(|message| message.to_string())
        .join("\n");
        runner
            .record_executables(
                std::slice::from_ref(&target),
                stdout.as_bytes(),
                b"   Compiling eqiora v0.0.0\n\
                   Finished `test` profile target(s) in 0.01s\n\
                 warning: cargo summary\n",
            )
            .unwrap();

        let prepared = runner.prepared.lock().unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(
            prepared.values().next().unwrap().build_stderr,
            "warning: shared diagnostic\n\
             warning: target diagnostic\n\
             warning: cargo summary\n"
        );
    }
}
