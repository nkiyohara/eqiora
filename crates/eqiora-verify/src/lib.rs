//! Deterministic verification-case discovery, validation, and evidence execution.
//!
//! Manifests select one of a closed set of typed evidence targets. They never
//! contain an executable command line or arguments, so repository data cannot
//! select an arbitrary process.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};

mod manifest;
pub use manifest::PythonEvidenceRunner;
use manifest::{CaseContract, load_repository};
#[cfg(test)]
use manifest::{CaseManifest, load_repository_with_targets};

const REPORT_SCHEMA: &str = "eqiora.verification-report/v5";
const CAPABILITY_INDEX_SCHEMA: &str = "eqiora.capability-evidence-index/v3";

/// Deterministic capability-to-evidence projection derived from validated cases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityEvidenceIndex {
    /// Stable index schema identifier.
    pub schema: &'static str,
    /// Exact capability filter, when present.
    pub selected_capability: Option<String>,
    /// Whether repository validation and selection succeeded.
    pub success: bool,
    /// Entries ordered by capability and then case ID.
    pub entries: Vec<CapabilityEvidenceEntry>,
    /// Repository-level discovery, validation, or selection failures.
    pub errors: Vec<String>,
}

impl CapabilityEvidenceIndex {
    /// Render the index without weakening its deterministic ordering.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut rendered = String::new();
        for entry in &self.entries {
            let evidence = entry.evidence.as_ref().map_or_else(
                || "no executable evidence".to_owned(),
                EvidenceTarget::human_label,
            );
            rendered.push_str(&format!(
                "{} -> {} [{}; {}; {}]\n",
                entry.capability,
                entry.case,
                entry.status.as_str(),
                entry.reference_kind,
                evidence
            ));
        }
        for error in &self.errors {
            rendered.push_str(&format!("error: {error}\n"));
        }
        rendered.push_str(&format!(
            "{} {} capability evidence entr{}\n",
            if self.success { "indexed" } else { "failed" },
            self.entries.len(),
            if self.entries.len() == 1 { "y" } else { "ies" }
        ));
        rendered
    }
}

/// One capability claim and the exact case contract that supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityEvidenceEntry {
    /// Stable capability identifier declared by the case manifest.
    pub capability: String,
    /// Stable verification case ID.
    pub case: String,
    /// Repository-relative authoritative manifest path.
    pub manifest: String,
    /// Declared case maturity.
    pub status: Status,
    /// Independent reference or falsification strategy.
    pub reference_kind: String,
    /// Reusable central-contract conformance kits consumed by this case.
    pub conformance_kits: Vec<String>,
    /// Structured evidence target, when declared.
    pub evidence: Option<EvidenceTarget>,
}

/// Repository operation represented by a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandKind {
    /// List case contracts.
    List,
    /// Validate without executing evidence.
    Check,
    /// Validate and execute evidence.
    Run,
}

/// Behavior after the first evidence failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionPolicy {
    /// Stop starting unseen targets after the first evidence failure.
    FailFast,
    /// Run every selected executable case.
    KeepGoing,
}

/// One deterministic verification request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    command: CommandKind,
    case: Option<String>,
    policy: ExecutionPolicy,
    environment: Option<EvidenceEnvironment>,
    runner_kind: Option<RunnerKind>,
    jobs: usize,
}

impl Request {
    /// Construct a verification request.
    #[must_use]
    pub fn new(command: CommandKind, case: Option<String>, policy: ExecutionPolicy) -> Self {
        Self {
            command,
            case,
            policy,
            environment: None,
            runner_kind: None,
            jobs: 1,
        }
    }

    /// Restrict execution to evidence declared for one exact environment.
    #[must_use]
    pub fn for_environment(mut self, environment: EvidenceEnvironment) -> Self {
        self.environment = Some(environment);
        self
    }

    /// Restrict execution to evidence using one exact runner kind.
    #[must_use]
    pub fn for_runner_kind(mut self, runner_kind: RunnerKind) -> Self {
        self.runner_kind = Some(runner_kind);
        self
    }

    /// Bound the number of evidence targets executing at once.
    #[must_use]
    pub fn with_jobs(mut self, jobs: usize) -> Self {
        self.jobs = jobs.max(1);
        self
    }
}

/// Versioned, machine-readable result of one repository operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationReport {
    /// Stable report schema identifier.
    pub schema: &'static str,
    /// Requested operation.
    pub command: CommandKind,
    /// Failure policy used for execution.
    pub policy: ExecutionPolicy,
    /// Exact case filter, when present.
    pub selected_case: Option<String>,
    /// Exact evidence environment filter, when present.
    pub selected_environment: Option<EvidenceEnvironment>,
    /// Exact evidence runner-kind filter, when present.
    pub selected_runner_kind: Option<RunnerKind>,
    /// Overall validation and execution result.
    pub success: bool,
    /// Deterministically ordered case results.
    pub cases: Vec<CaseReport>,
    /// Repository-level discovery or validation failures.
    pub errors: Vec<String>,
}

impl VerificationReport {
    /// Human-oriented rendering. Child output is shown only for failed cases.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut rendered = String::new();
        for case in &self.cases {
            let duration = case
                .duration_ms
                .map_or_else(String::new, |duration| format!("; {duration} ms"));
            rendered.push_str(&format!(
                "{:<11} {} [{}{}]\n",
                case.outcome.as_human(),
                case.id,
                case.status.as_str(),
                duration
            ));
            if let Some(message) = &case.message {
                rendered.push_str(&format!("  {message}\n"));
            }
            if case.outcome == Outcome::Failed {
                append_child_stream(&mut rendered, "stdout", case.stdout.as_deref());
                append_child_stream(&mut rendered, "stderr", case.stderr.as_deref());
            }
        }
        for error in &self.errors {
            rendered.push_str(&format!("error: {error}\n"));
        }
        rendered.push_str(&format!(
            "{} {} case(s)\n",
            if self.success { "completed" } else { "failed" },
            self.cases.len()
        ));
        rendered
    }
}

fn append_child_stream(rendered: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        rendered.push_str(&format!("  child {label}:\n"));
        for line in value.lines() {
            rendered.push_str(&format!("    {line}\n"));
        }
    }
}

/// Result for one case. Child streams are fields, never mixed into report JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaseReport {
    /// Stable case ID.
    pub id: String,
    /// Repository-relative manifest path.
    pub manifest: String,
    /// Declared maturity.
    pub status: Status,
    /// Result of the requested operation.
    pub outcome: Outcome,
    /// Structured evidence target, when declared.
    pub evidence: Option<EvidenceTarget>,
    /// Monotonic wall-clock duration of the evidence target, when it started.
    /// Serialized as `null` when absent, like every other optional field on
    /// this report: one object must not carry two encodings of absence.
    pub duration_ms: Option<u64>,
    /// Child exit code, when a process started and returned one.
    pub exit_code: Option<i32>,
    /// Captured child stdout.
    pub stdout: Option<String>,
    /// Captured child stderr.
    pub stderr: Option<String>,
    /// Stable explanatory message.
    pub message: Option<String>,
}

/// Case maturity accepted by the repository contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Candidate with no frozen contract.
    Proposed,
    /// Contract exists but evidence is not executable.
    Specified,
    /// Evidence is executable but its full claim is not yet verified.
    Implemented,
    /// Quantitative verification evidence passes.
    Verified,
    /// External or experimental validation evidence passes.
    Validated,
}

impl Status {
    fn is_executable(self) -> bool {
        matches!(self, Self::Implemented | Self::Verified | Self::Validated)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Specified => "specified",
            Self::Implemented => "implemented",
            Self::Verified => "verified",
            Self::Validated => "validated",
        }
    }
}

/// Per-case operation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Returned by `list`.
    Listed,
    /// Fully validated without execution.
    Checked,
    /// Evidence process succeeded.
    Passed,
    /// Evidence process failed or could not start.
    Failed,
    /// Not executable at its declared maturity.
    NotRunnable,
    /// Runnable but omitted after a fail-fast failure.
    Skipped,
    /// Runnable but outside an explicitly selected evidence filter.
    NotSelected,
}

impl Outcome {
    fn as_human(self) -> &'static str {
        match self {
            Self::Listed => "listed",
            Self::Checked => "checked",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::NotRunnable => "not-runnable",
            Self::Skipped => "skipped",
            Self::NotSelected => "not-selected",
        }
    }
}

/// Environment in which an evidence target is allowed to make its claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceEnvironment {
    /// Host execution requiring no selected physical accelerator topology.
    #[default]
    HostCpu,
    /// One-host MPI execution over an explicitly selected physical CUDA topology.
    PhysicalMpiCuda,
}

impl EvidenceEnvironment {
    fn is_host_cpu(&self) -> bool {
        *self == Self::HostCpu
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::HostCpu => "host-cpu",
            Self::PhysicalMpiCuda => "physical-mpi-cuda",
        }
    }
}

/// Closed execution mechanism used by an evidence target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunnerKind {
    /// Run a compiled workspace Cargo integration-test target.
    Cargo,
    /// Run the repository-owned installed-wheel Python gate.
    PythonInstalledWheel,
}

impl RunnerKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::PythonInstalledWheel => "python-installed-wheel",
        }
    }
}

/// One of the closed, shell-free evidence targets a case manifest may select.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EvidenceTarget {
    /// One workspace Cargo integration-test target.
    Cargo(CargoEvidenceTarget),
    /// One repository-owned Python installed-wheel gate.
    PythonInstalledWheel(PythonInstalledWheelEvidenceTarget),
}

impl EvidenceTarget {
    fn human_label(&self) -> String {
        let label = match self {
            Self::Cargo(target) => format!("{}/{}", target.package, target.test),
            Self::PythonInstalledWheel(target) => {
                format!("{}/{}", target.runner.as_str(), target.script)
            }
        };
        let environment = self.environment();
        if environment.is_host_cpu() {
            label
        } else {
            format!("{label}@{}", environment.as_str())
        }
    }

    fn environment(&self) -> EvidenceEnvironment {
        match self {
            Self::Cargo(target) => target.environment,
            Self::PythonInstalledWheel(target) => target.environment,
        }
    }

    fn runner_kind(&self) -> RunnerKind {
        match self {
            Self::Cargo(_) => RunnerKind::Cargo,
            Self::PythonInstalledWheel(_) => RunnerKind::PythonInstalledWheel,
        }
    }
}

/// A workspace Cargo integration-test evidence target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CargoEvidenceTarget {
    /// Workspace package name.
    pub package: String,
    /// Cargo integration-test target name.
    pub test: String,
    /// Explicit Cargo features required by this evidence target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// Optional case-relative evidence artifact checked before execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Exact environment required to execute this evidence target.
    #[serde(default, skip_serializing_if = "EvidenceEnvironment::is_host_cpu")]
    pub environment: EvidenceEnvironment,
}

/// A repository-owned Python installed-wheel evidence target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythonInstalledWheelEvidenceTarget {
    /// Fixed runner identity; no command or argument fields are accepted.
    pub runner: PythonEvidenceRunner,
    /// Normalized repository-relative path to one regular `.py` file.
    pub script: String,
    /// Exact environment required to execute this evidence target.
    #[serde(default, skip_serializing_if = "EvidenceEnvironment::is_host_cpu")]
    pub environment: EvidenceEnvironment,
}

/// Captured outcome of one exact evidence target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceOutput {
    /// Monotonic wall-clock duration in whole milliseconds, absent on spawn failure.
    pub duration_ms: Option<u64>,
    /// Process exit code, absent when the process could not start or was signaled.
    pub exit_code: Option<i32>,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Spawn failure, when applicable.
    pub start_error: Option<String>,
}

impl EvidenceOutput {
    fn succeeded(&self) -> bool {
        self.start_error.is_none() && self.exit_code == Some(0)
    }
}

/// Execution seam used by the system runner and deterministic unit fakes.
pub trait EvidenceRunner: Sync {
    /// Compile one exact Cargo `(package, feature-set)` group.
    ///
    /// A successful implementation retains the emitted executable paths for
    /// subsequent [`Self::run`] calls. Returning an output marks the complete
    /// group as a build failure. Non-Cargo runners may keep the default.
    fn build_cargo_group(
        &self,
        _root: &Path,
        _targets: &[EvidenceTarget],
    ) -> Option<EvidenceOutput> {
        None
    }

    /// Run exactly one already-validated evidence target.
    fn run(&self, root: &Path, target: &EvidenceTarget) -> EvidenceOutput;
}

/// Shell-free runner for the closed set of system evidence targets.
#[derive(Debug, Clone)]
pub struct SystemEvidenceRunner {
    cargo: OsString,
    python: OsString,
    prepared: Arc<Mutex<Vec<PreparedCargoTarget>>>,
}

#[derive(Debug, Clone)]
struct PreparedCargoTarget {
    target: EvidenceTarget,
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
            prepared: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cargo_build_command(&self, root: &Path, targets: &[EvidenceTarget]) -> Command {
        let Some(EvidenceTarget::Cargo(first)) = targets.first() else {
            panic!("Cargo build group must contain a Cargo evidence target");
        };
        let key = CargoBuildGroup::from_target(first);
        let tests = targets
            .iter()
            .map(|target| match target {
                EvidenceTarget::Cargo(target) => target.test.as_str(),
                EvidenceTarget::PythonInstalledWheel(_) => {
                    panic!("Cargo build group must contain only Cargo evidence targets")
                }
            })
            .collect::<BTreeSet<_>>();
        let mut command = Command::new(&self.cargo);
        command.args([
            "test",
            "--locked",
            "-p",
            &key.package,
            "--no-run",
            "--message-format=json",
        ]);
        for test in tests {
            command.args(["--test", test]);
        }
        if !key.features.is_empty() {
            command.arg("--features").arg(key.features.join(","));
        }
        command.current_dir(root);
        command
    }

    fn command(&self, root: &Path, target: &EvidenceTarget) -> Result<Command, String> {
        let mut command = match target {
            EvidenceTarget::Cargo(target) => {
                let prepared = self.prepared.lock().unwrap();
                let executable = prepared
                    .iter()
                    .find(|prepared| prepared.target == EvidenceTarget::Cargo(target.clone()))
                    .map(|prepared| prepared.executable.clone())
                    .ok_or_else(|| {
                        format!(
                            "Cargo evidence target `{}/{}` has no prepared executable",
                            target.package, target.test
                        )
                    })?;
                let mut command = Command::new(executable);
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

    fn record_executables(
        &self,
        targets: &[EvidenceTarget],
        stdout: &[u8],
        stderr: &[u8],
    ) -> Result<(), String> {
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
            if let Some(existing) = prepared
                .iter_mut()
                .find(|prepared_target| prepared_target.target == *target)
            {
                existing.executable = executable;
                existing.build_stderr = build_stderr;
            } else {
                prepared.push(PreparedCargoTarget {
                    target: target.clone(),
                    executable,
                    build_stderr,
                });
            }
        }
        Ok(())
    }

    fn prepared_build_stderr(&self, target: &EvidenceTarget) -> String {
        self.prepared
            .lock()
            .unwrap()
            .iter()
            .find(|prepared| prepared.target == *target)
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
            append_stderr(
                &mut stderr,
                &format!(
                    "error: test failed, to rerun pass `-p {} --test {}`\n",
                    target.package, target.test
                ),
            );
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
                let stderr = self.completed_stderr(target, &output.stderr, output.status.success());
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

/// Discover, validate, select, and optionally run repository cases.
#[must_use]
pub fn execute(root: &Path, request: &Request, runner: &dyn EvidenceRunner) -> VerificationReport {
    let mut report = VerificationReport {
        schema: REPORT_SCHEMA,
        command: request.command,
        policy: request.policy,
        selected_case: request.case.clone(),
        selected_environment: request.environment,
        selected_runner_kind: request.runner_kind,
        success: false,
        cases: Vec::new(),
        errors: Vec::new(),
    };
    let contracts = match load_repository(root) {
        Ok(contracts) => contracts,
        Err(error) => {
            report.errors.push(error);
            return report;
        }
    };
    let selected = match select_cases(contracts, request.case.as_deref()) {
        Ok(selected) => selected,
        Err(error) => {
            report.errors.push(error);
            return report;
        }
    };

    report.cases = execute_selected(root, selected, request, runner);
    report.success = report.errors.is_empty()
        && report
            .cases
            .iter()
            .all(|case| case.outcome != Outcome::Failed);
    report
}

/// Validate every case manifest and derive a capability-to-evidence index.
///
/// An exact capability filter is applied only after the complete repository
/// contract has passed validation, so an unrelated malformed case cannot be
/// hidden by selection.
#[must_use]
pub fn capability_evidence_index(
    root: &Path,
    selected_capability: Option<&str>,
) -> CapabilityEvidenceIndex {
    let mut index = CapabilityEvidenceIndex {
        schema: CAPABILITY_INDEX_SCHEMA,
        selected_capability: selected_capability.map(str::to_owned),
        success: false,
        entries: Vec::new(),
        errors: Vec::new(),
    };
    let contracts = match load_repository(root) {
        Ok(contracts) => contracts,
        Err(error) => {
            index.errors.push(error);
            return index;
        }
    };
    index = build_capability_evidence_index(contracts, selected_capability);
    index
}

fn build_capability_evidence_index(
    contracts: Vec<CaseContract>,
    selected_capability: Option<&str>,
) -> CapabilityEvidenceIndex {
    let mut entries = contracts
        .into_iter()
        .flat_map(|contract| {
            contract
                .capabilities
                .iter()
                .filter(|capability| {
                    selected_capability.is_none_or(|selected| selected == capability.as_str())
                })
                .map(|capability| CapabilityEvidenceEntry {
                    capability: capability.clone(),
                    case: contract.id.clone(),
                    manifest: contract.manifest.clone(),
                    status: contract.status,
                    reference_kind: contract.reference_kind.clone(),
                    conformance_kits: contract.conformance_kits.clone(),
                    evidence: contract.evidence.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then_with(|| left.case.cmp(&right.case))
    });

    let mut errors = Vec::new();
    match selected_capability {
        Some(selected) if entries.is_empty() => {
            errors.push(format!("unknown verification capability `{selected}`"));
        }
        _ => {}
    }
    CapabilityEvidenceIndex {
        schema: CAPABILITY_INDEX_SCHEMA,
        selected_capability: selected_capability.map(str::to_owned),
        success: errors.is_empty(),
        entries,
        errors,
    }
}

fn execute_selected(
    root: &Path,
    selected: Vec<CaseContract>,
    request: &Request,
    runner: &dyn EvidenceRunner,
) -> Vec<CaseReport> {
    let mut cases = Vec::with_capacity(selected.len());
    let mut targets = Vec::<EvidenceTarget>::new();
    for contract in &selected {
        let mut case = CaseReport::from_contract(contract);
        match request.command {
            CommandKind::List => case.outcome = Outcome::Listed,
            CommandKind::Check => case.outcome = Outcome::Checked,
            CommandKind::Run if !contract.status.is_executable() => {
                case.outcome = Outcome::NotRunnable;
                case.message = Some("case status does not declare executable evidence".to_owned());
            }
            CommandKind::Run
                if request.environment.is_some_and(|selected| {
                    contract
                        .evidence
                        .as_ref()
                        .is_some_and(|target| target.environment() != selected)
                }) =>
            {
                let required = contract
                    .evidence
                    .as_ref()
                    .expect("executable contracts were validated with evidence")
                    .environment();
                case.outcome = Outcome::NotSelected;
                case.message = Some(format!(
                    "evidence requires `{}` execution",
                    required.as_str()
                ));
            }
            CommandKind::Run
                if request.runner_kind.is_some_and(|selected| {
                    contract
                        .evidence
                        .as_ref()
                        .is_some_and(|target| target.runner_kind() != selected)
                }) =>
            {
                let required = contract
                    .evidence
                    .as_ref()
                    .expect("executable contracts were validated with evidence")
                    .runner_kind();
                case.outcome = Outcome::NotSelected;
                case.message = Some(format!(
                    "evidence requires `{}` runner kind",
                    required.as_str()
                ));
            }
            CommandKind::Run => {
                let target = contract
                    .evidence
                    .as_ref()
                    .expect("executable contracts were validated with evidence");
                if !targets.contains(target) {
                    targets.push(target.clone());
                }
            }
        }
        cases.push(case);
    }
    if request.command != CommandKind::Run {
        return cases;
    }

    let mut completed = vec![None; targets.len()];
    let mut groups = BTreeMap::<CargoBuildGroup, Vec<usize>>::new();
    for (index, target) in targets.iter().enumerate() {
        if let EvidenceTarget::Cargo(target) = target {
            groups
                .entry(CargoBuildGroup::from_target(target))
                .or_default()
                .push(index);
        }
    }
    for (group, indices) in groups {
        let group_targets = indices
            .iter()
            .map(|index| targets[*index].clone())
            .collect::<Vec<_>>();
        if let Some(output) = runner.build_cargo_group(root, &group_targets) {
            for index in indices {
                completed[index] = Some(TargetCompletion::BuildFailed {
                    group: group.clone(),
                    output: output.clone(),
                });
            }
        }
    }

    let runnable = targets
        .iter()
        .enumerate()
        .filter(|(index, _)| completed[*index].is_none())
        .map(|(index, target)| (index, target.clone()))
        .collect::<Vec<_>>();
    for (index, output) in run_targets(root, runnable, request, runner) {
        completed[index] = Some(TargetCompletion::Executed(output));
    }

    for case in &mut cases {
        if case.outcome != Outcome::Checked {
            continue;
        }
        let target = case
            .evidence
            .as_ref()
            .expect("selected executable cases were validated with evidence");
        let index = targets
            .iter()
            .position(|candidate| candidate == target)
            .expect("selected target was indexed");
        match &completed[index] {
            Some(TargetCompletion::Executed(output)) => {
                project_evidence_output(case, output);
            }
            Some(TargetCompletion::BuildFailed { group, output }) => {
                project_build_failure(case, group, output);
            }
            None => {
                case.outcome = Outcome::Skipped;
                case.message = Some("not run after fail-fast evidence failure".to_owned());
            }
        }
    }
    cases
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CargoBuildGroup {
    package: String,
    features: Vec<String>,
}

impl CargoBuildGroup {
    fn from_target(target: &CargoEvidenceTarget) -> Self {
        let mut features = target.features.clone();
        features.sort();
        features.dedup();
        Self {
            package: target.package.clone(),
            features,
        }
    }

    fn label(&self) -> String {
        format!("{} features=[{}]", self.package, self.features.join(","))
    }
}

#[derive(Debug, Clone)]
enum TargetCompletion {
    Executed(EvidenceOutput),
    BuildFailed {
        group: CargoBuildGroup,
        output: EvidenceOutput,
    },
}

fn run_targets(
    root: &Path,
    targets: Vec<(usize, EvidenceTarget)>,
    request: &Request,
    runner: &dyn EvidenceRunner,
) -> Vec<(usize, EvidenceOutput)> {
    if targets.is_empty() {
        return Vec::new();
    }
    let jobs = request.jobs.min(targets.len()).max(1);
    let mut completed = Vec::with_capacity(targets.len());
    let cursor = Mutex::new((0, false));
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        for _ in 0..jobs {
            let sender = sender.clone();
            let cursor = &cursor;
            let targets = &targets;
            scope.spawn(move || {
                loop {
                    let next = {
                        let mut cursor = cursor.lock().unwrap();
                        if cursor.1 || cursor.0 == targets.len() {
                            None
                        } else {
                            let target = targets[cursor.0].clone();
                            cursor.0 += 1;
                            Some(target)
                        }
                    };
                    let Some((index, target)) = next else {
                        break;
                    };
                    let output = runner.run(root, &target);
                    if !output.succeeded() && request.policy == ExecutionPolicy::FailFast {
                        cursor.lock().unwrap().1 = true;
                    }
                    sender
                        .send((index, output))
                        .expect("execution receiver remains alive");
                }
            });
        }
        drop(sender);
        completed.extend(receiver);
    });
    completed
}

fn project_build_failure(case: &mut CaseReport, group: &CargoBuildGroup, output: &EvidenceOutput) {
    case.duration_ms = None;
    case.exit_code = output.exit_code;
    case.stdout = Some(output.stdout.clone());
    case.stderr = Some(output.stderr.clone());
    case.outcome = Outcome::Failed;
    let reason = output.start_error.clone().unwrap_or_else(|| {
        format!(
            "Cargo build exited with {}",
            output
                .exit_code
                .map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
        )
    });
    case.message = Some(format!(
        "Cargo evidence group `{}` failed to compile: {reason}",
        group.label()
    ));
}

fn project_evidence_output(case: &mut CaseReport, output: &EvidenceOutput) -> bool {
    case.duration_ms = output.duration_ms;
    case.exit_code = output.exit_code;
    case.stdout = Some(output.stdout.clone());
    case.stderr = Some(output.stderr.clone());
    if output.succeeded() {
        case.outcome = Outcome::Passed;
        true
    } else {
        case.outcome = Outcome::Failed;
        case.message = output.start_error.clone().or_else(|| {
            Some(format!(
                "evidence target exited with {}",
                output
                    .exit_code
                    .map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
            ))
        });
        false
    }
}

impl CaseReport {
    fn from_contract(contract: &CaseContract) -> Self {
        Self {
            id: contract.id.clone(),
            manifest: contract.manifest.clone(),
            status: contract.status,
            outcome: Outcome::Checked,
            evidence: contract.evidence.clone(),
            duration_ms: None,
            exit_code: None,
            stdout: None,
            stderr: None,
            message: None,
        }
    }
}

fn select_cases(
    contracts: Vec<CaseContract>,
    selected: Option<&str>,
) -> Result<Vec<CaseContract>, String> {
    let Some(selected) = selected else {
        return Ok(contracts);
    };
    let mut matching = contracts
        .into_iter()
        .filter(|contract| contract.id == selected)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        Err(format!("unknown verification case ID `{selected}`"))
    } else {
        Ok(std::mem::take(&mut matching))
    }
}

#[cfg(test)]
mod tests;
