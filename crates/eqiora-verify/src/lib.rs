//! Deterministic verification-case discovery, validation, and evidence execution.
//!
//! Manifests select one of a closed set of typed evidence targets. They never
//! contain an executable command line or arguments, so repository data cannot
//! select an arbitrary process.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

const REPORT_SCHEMA: &str = "eqiora.verification-report/v4";
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
        }
    }

    /// Restrict execution to evidence declared for one exact environment.
    #[must_use]
    pub fn for_environment(mut self, environment: EvidenceEnvironment) -> Self {
        self.environment = Some(environment);
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
    /// Runnable but outside the explicitly selected evidence environment.
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

/// The closed runner identity for an installed-wheel Python evidence target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PythonEvidenceRunner {
    /// Run the repository-owned installed-wheel gate with Python.
    PythonInstalledWheel,
}

impl PythonEvidenceRunner {
    fn as_str(self) -> &'static str {
        match self {
            Self::PythonInstalledWheel => "python-installed-wheel",
        }
    }
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
pub trait EvidenceRunner {
    /// Run exactly one already-validated evidence target.
    fn run(&self, root: &Path, target: &EvidenceTarget) -> EvidenceOutput;
}

/// Shell-free runner for the closed set of system evidence targets.
#[derive(Debug, Clone)]
pub struct SystemEvidenceRunner {
    cargo: OsString,
    python: OsString,
}

impl SystemEvidenceRunner {
    /// Use `CARGO` and `PYTHON` when set, otherwise `cargo` and `python3`.
    #[must_use]
    pub fn from_environment() -> Self {
        Self {
            cargo: env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")),
            python: env::var_os("PYTHON").unwrap_or_else(|| OsString::from("python3")),
        }
    }

    fn command(&self, root: &Path, target: &EvidenceTarget) -> Command {
        let mut command = match target {
            EvidenceTarget::Cargo(target) => {
                let mut command = Command::new(&self.cargo);
                command.args([
                    "test",
                    "--locked",
                    "-p",
                    &target.package,
                    "--test",
                    &target.test,
                ]);
                if !target.features.is_empty() {
                    command.arg("--features").arg(target.features.join(","));
                }
                if target.environment == EvidenceEnvironment::PhysicalMpiCuda {
                    command.args(["--", "--ignored"]);
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
        command
    }
}

impl EvidenceRunner for SystemEvidenceRunner {
    fn run(&self, root: &Path, target: &EvidenceTarget) -> EvidenceOutput {
        let mut command = self.command(root, target);
        let started = Instant::now();
        match command.output() {
            Ok(output) => EvidenceOutput {
                duration_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                start_error: None,
            },
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

/// Discover, validate, select, and optionally run repository cases.
#[must_use]
pub fn execute(root: &Path, request: &Request, runner: &dyn EvidenceRunner) -> VerificationReport {
    let mut report = VerificationReport {
        schema: REPORT_SCHEMA,
        command: request.command,
        policy: request.policy,
        selected_case: request.case.clone(),
        selected_environment: request.environment,
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
    let mut completed_targets = Vec::<(EvidenceTarget, EvidenceOutput)>::new();
    let mut prior_failure = false;
    for contract in selected {
        let mut case = CaseReport::from_contract(&contract);
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
            CommandKind::Run => {
                let target = contract
                    .evidence
                    .as_ref()
                    .expect("executable contracts were validated with evidence");
                let completed = completed_targets
                    .iter()
                    .position(|(completed, _)| completed == target);
                if completed.is_none()
                    && prior_failure
                    && request.policy == ExecutionPolicy::FailFast
                {
                    case.outcome = Outcome::Skipped;
                    case.message = Some("not run after fail-fast evidence failure".to_owned());
                } else {
                    let completed = completed.unwrap_or_else(|| {
                        completed_targets.push((target.clone(), runner.run(root, target)));
                        completed_targets.len() - 1
                    });
                    if !project_evidence_output(&mut case, &completed_targets[completed].1) {
                        prior_failure = true;
                    }
                }
            }
        }
        cases.push(case);
    }
    cases
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseContract {
    id: String,
    manifest: String,
    status: Status,
    reference_kind: String,
    capabilities: Vec<String>,
    conformance_kits: Vec<String>,
    evidence: Option<EvidenceTarget>,
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

fn load_repository(root: &Path) -> Result<Vec<CaseContract>, String> {
    let targets = discover_workspace_targets(root)?;
    load_repository_with_targets(root, &targets)
}

fn load_repository_with_targets(
    root: &Path,
    workspace_targets: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<CaseContract>, String> {
    let verify_root = root.join("verify");
    let mut manifests = Vec::new();
    collect_manifests(&verify_root, &mut manifests)?;
    manifests.sort();
    if manifests.is_empty() {
        return Err("verify/ contains no case.toml contracts".to_owned());
    }

    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize {}: {error}", root.display()))?;
    let mut ids = BTreeSet::new();
    let mut contracts = Vec::with_capacity(manifests.len());
    for path in manifests {
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let manifest: CaseManifest = toml::from_str(&source)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        let contract =
            validate_manifest(&canonical_root, root, &path, manifest, workspace_targets)?;
        if !ids.insert(contract.id.clone()) {
            return Err(format!("duplicate verification case ID `{}`", contract.id));
        }
        contracts.push(contract);
    }
    contracts.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(contracts)
}

fn collect_manifests(directory: &Path, manifests: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "cannot inspect an entry below {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_manifests(&path, manifests)?;
        } else if file_type.is_file() && entry.file_name() == "case.toml" {
            manifests.push(path);
        }
    }
    Ok(())
}

fn validate_manifest(
    canonical_root: &Path,
    root: &Path,
    path: &Path,
    manifest: CaseManifest,
    workspace_targets: &BTreeMap<String, BTreeSet<String>>,
) -> Result<CaseContract, String> {
    let case_directory = path
        .parent()
        .ok_or_else(|| format!("{} has no case directory", path.display()))?;
    let area = case_directory
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} is not below verify/<area>/<case>", path.display()))?;
    let case = case_directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has a non-UTF-8 case name", path.display()))?;
    let expected_id = format!("{area}.{case}");
    if manifest.id != expected_id {
        return Err(format!(
            "{} declares ID `{}`, expected `{expected_id}` from its path",
            path.display(),
            manifest.id
        ));
    }
    if manifest.reference_kind.trim().is_empty() || manifest.capabilities.is_empty() {
        return Err(format!(
            "case `{}` requires reference_kind and at least one capability",
            manifest.id
        ));
    }
    let mut capabilities = BTreeSet::new();
    for capability in &manifest.capabilities {
        validate_stable_name("capability", capability)?;
        if !capabilities.insert(capability) {
            return Err(format!(
                "case `{}` repeats capability `{capability}`",
                manifest.id
            ));
        }
    }
    let mut conformance_kits = BTreeSet::new();
    for kit in &manifest.conformance_kits {
        validate_stable_name("conformance kit", kit)?;
        if !conformance_kits.insert(kit) {
            return Err(format!(
                "case `{}` repeats conformance kit `{kit}`",
                manifest.id
            ));
        }
    }
    for required in ["README.md", "models", "references", "expected"] {
        if !case_directory.join(required).exists() {
            return Err(format!(
                "case `{}` is missing required `{required}`",
                manifest.id
            ));
        }
    }

    if manifest.status.is_executable() && manifest.evidence.is_none() {
        return Err(format!(
            "executable case `{}` has no [evidence] contract",
            manifest.id
        ));
    }
    if let Some(evidence) = &manifest.evidence {
        match evidence {
            EvidenceTarget::Cargo(evidence) => {
                validate_stable_name("evidence package", &evidence.package)?;
                validate_stable_name("evidence test", &evidence.test)?;
                let mut features = BTreeSet::new();
                for feature in &evidence.features {
                    validate_stable_name("evidence feature", feature)?;
                    if !features.insert(feature) {
                        return Err(format!(
                            "case `{}` repeats evidence feature `{feature}`",
                            manifest.id
                        ));
                    }
                }
                let tests = workspace_targets.get(&evidence.package).ok_or_else(|| {
                    format!(
                        "case `{}` names non-workspace evidence package `{}`",
                        manifest.id, evidence.package
                    )
                })?;
                if !tests.contains(&evidence.test) {
                    return Err(format!(
                        "case `{}` names missing integration-test target `{}/{}`",
                        manifest.id, evidence.package, evidence.test
                    ));
                }
                if let Some(table) = &evidence.table {
                    validate_case_artifact(canonical_root, case_directory, table, &manifest.id)?;
                }
            }
            EvidenceTarget::PythonInstalledWheel(evidence) => {
                validate_repository_python_script(
                    canonical_root,
                    root,
                    &evidence.script,
                    &manifest.id,
                )?;
            }
        }
    }

    let relative = path.strip_prefix(root).map_err(|_| {
        format!(
            "verification manifest {} is outside repository root",
            path.display()
        )
    })?;
    Ok(CaseContract {
        id: manifest.id,
        manifest: relative.to_string_lossy().replace('\\', "/"),
        status: manifest.status,
        reference_kind: manifest.reference_kind,
        capabilities: manifest.capabilities,
        conformance_kits: manifest.conformance_kits,
        evidence: manifest.evidence,
    })
}

fn validate_repository_python_script(
    canonical_root: &Path,
    root: &Path,
    relative: &str,
    id: &str,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative_path
            .extension()
            .is_none_or(|extension| extension != "py")
    {
        return Err(format!(
            "case `{id}` Python evidence script must be a normalized repository-relative `.py` path"
        ));
    }
    let script = root.join(relative_path);
    let metadata = script.symlink_metadata().map_err(|error| {
        format!(
            "case `{id}` Python evidence script `{relative}` is missing or inaccessible: {error}"
        )
    })?;
    let canonical = script.canonicalize().map_err(|error| {
        format!("case `{id}` Python evidence script `{relative}` is inaccessible: {error}")
    })?;
    if !metadata.file_type().is_file() || !canonical.starts_with(canonical_root) {
        return Err(format!(
            "case `{id}` Python evidence script `{relative}` is not a regular repository file"
        ));
    }
    Ok(())
}

fn validate_case_artifact(
    canonical_root: &Path,
    case_directory: &Path,
    relative: &str,
    id: &str,
) -> Result<(), String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "case `{id}` evidence artifact must be a normalized relative path"
        ));
    }
    let artifact = case_directory.join(relative_path);
    let canonical = artifact.canonicalize().map_err(|error| {
        format!("case `{id}` evidence artifact `{relative}` is missing or inaccessible: {error}")
    })?;
    if !canonical.starts_with(canonical_root) || !canonical.is_file() {
        return Err(format!(
            "case `{id}` evidence artifact `{relative}` is not a repository file"
        ));
    }
    Ok(())
}

fn validate_stable_name(label: &str, value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || !bytes[0].is_ascii_lowercase()
        || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
        || !bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        Err(format!(
            "{label} `{value}` is not a stable lowercase identifier"
        ))
    } else {
        Ok(())
    }
}

fn discover_workspace_targets(root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(cargo)
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `cargo metadata`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot decode Cargo metadata v1: {error}"))?;
    let workspace_members = metadata
        .workspace_members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    for package in metadata
        .packages
        .into_iter()
        .filter(|package| workspace_members.contains(&package.id))
    {
        let tests = package
            .targets
            .into_iter()
            .filter(|target| target.kind.iter().any(|kind| kind == "test"))
            .map(|target| target.name)
            .collect();
        if result.insert(package.name.clone(), tests).is_some() {
            return Err(format!(
                "Cargo metadata repeats workspace package `{}`",
                package.name
            ));
        }
    }
    Ok(result)
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CaseManifest {
    id: String,
    status: Status,
    reference_kind: String,
    capabilities: Vec<String>,
    #[serde(default)]
    conformance_kits: Vec<String>,
    evidence: Option<EvidenceTarget>,
    #[serde(flatten)]
    _extensions: BTreeMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                env::temp_dir().join(format!("eqiora-verify-test-{}-{nonce}", std::process::id()));
            fs::create_dir_all(root.join("verify/area/case/models")).unwrap();
            fs::create_dir_all(root.join("verify/area/case/references")).unwrap();
            fs::create_dir_all(root.join("verify/area/case/expected")).unwrap();
            fs::write(root.join("verify/area/case/README.md"), "case\n").unwrap();
            fs::write(root.join("verify/area/case/expected/evidence.csv"), "x,y\n").unwrap();
            fs::create_dir_all(root.join("tools/ci")).unwrap();
            fs::write(
                root.join("tools/ci/python_evidence.py"),
                "print('evidence')\n",
            )
            .unwrap();
            Self { root }
        }

        fn write_manifest(&self, source: &str) {
            fs::write(self.root.join("verify/area/case/case.toml"), source).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn targets() -> BTreeMap<String, BTreeSet<String>> {
        BTreeMap::from([(
            "eqiora-numerics".to_owned(),
            BTreeSet::from(["evidence_test".to_owned()]),
        )])
    }

    fn valid_manifest() -> &'static str {
        r#"
id = "area.case"
status = "verified"
reference_kind = "analytic"
capabilities = ["convergence", "conservation"]
conformance_kits = ["scalar-convergence-v1"]

[evidence]
package = "eqiora-numerics"
test = "evidence_test"
features = ["evidence-runtime"]
table = "expected/evidence.csv"
"#
    }

    fn valid_python_manifest() -> &'static str {
        r#"
id = "area.case"
status = "verified"
reference_kind = "framework-contract"
capabilities = ["python-autograd"]

[evidence]
runner = "python-installed-wheel"
script = "tools/ci/python_evidence.py"
"#
    }

    #[test]
    fn repository_contract_rejects_duplicate_index_keys_and_missing_artifact() {
        let fixture = Fixture::new();
        fixture.write_manifest(&valid_manifest().replace(
            "[\"convergence\", \"conservation\"]",
            "[\"convergence\", \"convergence\"]",
        ));
        let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
        assert!(error.contains("repeats capability"));

        fixture.write_manifest(&valid_manifest().replace(
            "[\"scalar-convergence-v1\"]",
            "[\"scalar-convergence-v1\", \"scalar-convergence-v1\"]",
        ));
        let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
        assert!(error.contains("repeats conformance kit"));

        fixture.write_manifest(
            &valid_manifest().replace("expected/evidence.csv", "expected/missing.csv"),
        );
        let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
        assert!(error.contains("missing or inaccessible"));
    }

    #[test]
    fn unknown_status_and_arbitrary_evidence_command_are_rejected() {
        assert!(
            toml::from_str::<CaseManifest>(&valid_manifest().replace("verified", "unknown-status"))
                .is_err()
        );
        assert!(
            toml::from_str::<CaseManifest>(&format!(
                "{}\ncommand = \"sh -c anything\"\n",
                valid_manifest()
            ))
            .is_err()
        );
        assert!(
            toml::from_str::<CaseManifest>(&format!(
                "{}\nargs = [\"--arbitrary\"]\n",
                valid_python_manifest()
            ))
            .is_err()
        );
        assert!(
            toml::from_str::<CaseManifest>(&valid_manifest().replace(
                "features = [\"evidence-runtime\"]",
                "features = [\"evidence-runtime\"]\nenvironment = \"unknown\"",
            ))
            .is_err()
        );
    }

    #[test]
    fn installed_wheel_python_target_is_closed_and_repository_owned() {
        let fixture = Fixture::new();
        fixture.write_manifest(valid_python_manifest());
        let contracts = load_repository_with_targets(&fixture.root, &targets()).unwrap();
        assert!(matches!(
            contracts[0].evidence,
            Some(EvidenceTarget::PythonInstalledWheel(
                PythonInstalledWheelEvidenceTarget {
                    runner: PythonEvidenceRunner::PythonInstalledWheel,
                    ref script,
                    ..
                }
            )) if script == "tools/ci/python_evidence.py"
        ));

        for invalid in [
            "../python_evidence.py",
            "/tmp/python_evidence.py",
            "tools/ci/python_evidence",
            "tools/ci/python_evidence.txt",
        ] {
            fixture.write_manifest(
                &valid_python_manifest().replace("tools/ci/python_evidence.py", invalid),
            );
            let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
            assert!(
                error.contains(
                    "Python evidence script must be a normalized repository-relative `.py` path"
                ),
                "{error}"
            );
        }

        fixture.write_manifest(
            &valid_python_manifest().replace("tools/ci/python_evidence.py", "tools/ci/missing.py"),
        );
        let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
        assert!(error.contains("missing or inaccessible"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn installed_wheel_python_target_rejects_a_script_symlink() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        symlink(
            fixture.root.join("tools/ci/python_evidence.py"),
            fixture.root.join("tools/ci/python_evidence_link.py"),
        )
        .unwrap();
        fixture.write_manifest(&valid_python_manifest().replace(
            "tools/ci/python_evidence.py",
            "tools/ci/python_evidence_link.py",
        ));
        let error = load_repository_with_targets(&fixture.root, &targets()).unwrap_err();
        assert!(error.contains("not a regular repository file"), "{error}");
    }

    #[test]
    fn target_serialization_preserves_the_existing_cargo_json_shape() {
        assert_eq!(REPORT_SCHEMA, "eqiora.verification-report/v4");
        assert_eq!(
            CAPABILITY_INDEX_SCHEMA,
            "eqiora.capability-evidence-index/v3"
        );
        let target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "evidence_test".to_owned(),
            features: Vec::new(),
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        });
        assert_eq!(
            serde_json::to_value(target).unwrap(),
            serde_json::json!({
                "package": "eqiora",
                "test": "evidence_test"
            })
        );

        let physical = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "physical_test".to_owned(),
            features: vec!["mpi-cuda".to_owned()],
            table: None,
            environment: EvidenceEnvironment::PhysicalMpiCuda,
        });
        assert_eq!(
            serde_json::to_value(physical).unwrap(),
            serde_json::json!({
                "package": "eqiora",
                "test": "physical_test",
                "features": ["mpi-cuda"],
                "environment": "physical-mpi-cuda"
            })
        );
    }

    #[test]
    fn system_runner_builds_only_closed_shell_free_commands() {
        let runner = SystemEvidenceRunner {
            cargo: OsString::from("cargo-evidence"),
            python: OsString::from("python-evidence"),
        };
        let root = Path::new("/repository");
        let cargo_target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "registered_case".to_owned(),
            features: vec!["one".to_owned(), "two".to_owned()],
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        });
        let cargo = runner.command(root, &cargo_target);
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
                "--test",
                "registered_case",
                "--features",
                "one,two",
            ]
        );
        assert_eq!(cargo.get_current_dir(), Some(root));

        let physical_target = EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora".to_owned(),
            test: "physical_case".to_owned(),
            features: vec!["mpi-cuda".to_owned()],
            table: None,
            environment: EvidenceEnvironment::PhysicalMpiCuda,
        });
        let physical = runner.command(root, &physical_target);
        assert_eq!(physical.get_program(), "cargo-evidence");
        assert_eq!(
            physical
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "test",
                "--locked",
                "-p",
                "eqiora",
                "--test",
                "physical_case",
                "--features",
                "mpi-cuda",
                "--",
                "--ignored",
            ]
        );
        assert_eq!(physical.get_current_dir(), Some(root));

        let python_target =
            EvidenceTarget::PythonInstalledWheel(PythonInstalledWheelEvidenceTarget {
                runner: PythonEvidenceRunner::PythonInstalledWheel,
                script: "tools/ci/python_evidence.py".to_owned(),
                environment: EvidenceEnvironment::HostCpu,
            });
        let python = runner.command(root, &python_target);
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
    fn canonical_repository_is_sorted_by_case_id() {
        let fixture = Fixture::new();
        fixture.write_manifest(valid_manifest());
        for (area, case) in [("zeta", "last"), ("alpha", "first")] {
            let directory = fixture.root.join(format!("verify/{area}/{case}"));
            fs::create_dir_all(directory.join("models")).unwrap();
            fs::create_dir_all(directory.join("references")).unwrap();
            fs::create_dir_all(directory.join("expected")).unwrap();
            fs::write(directory.join("README.md"), "case\n").unwrap();
            fs::write(
                directory.join("case.toml"),
                format!(
                    "id = \"{area}.{case}\"\nstatus = \"specified\"\nreference_kind = \"analytic\"\ncapabilities = [\"one\"]\n"
                ),
            )
            .unwrap();
        }
        let contracts = load_repository_with_targets(&fixture.root, &targets()).unwrap();
        let ids = contracts
            .into_iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["alpha.first", "area.case", "zeta.last"]);
    }

    struct FakeRunner {
        outputs: RefCell<VecDeque<EvidenceOutput>>,
        targets: RefCell<Vec<EvidenceTarget>>,
    }

    impl FakeRunner {
        fn new(outputs: impl IntoIterator<Item = EvidenceOutput>) -> Self {
            Self {
                outputs: RefCell::new(outputs.into_iter().collect()),
                targets: RefCell::new(Vec::new()),
            }
        }

        fn targets(&self) -> Vec<EvidenceTarget> {
            self.targets.borrow().clone()
        }
    }

    impl EvidenceRunner for FakeRunner {
        fn run(&self, _root: &Path, target: &EvidenceTarget) -> EvidenceOutput {
            self.targets.borrow_mut().push(target.clone());
            self.outputs.borrow_mut().pop_front().unwrap()
        }
    }

    fn successful_output() -> EvidenceOutput {
        EvidenceOutput {
            duration_ms: Some(7),
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            start_error: None,
        }
    }

    fn cargo_target() -> EvidenceTarget {
        EvidenceTarget::Cargo(CargoEvidenceTarget {
            package: "eqiora-numerics".to_owned(),
            test: "evidence_test".to_owned(),
            features: vec!["evidence-runtime".to_owned()],
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        })
    }

    fn contract_with_target(id: &str, evidence: EvidenceTarget) -> CaseContract {
        CaseContract {
            id: id.to_owned(),
            manifest: format!("verify/area/{id}/case.toml"),
            status: Status::Verified,
            reference_kind: "analytic".to_owned(),
            capabilities: vec!["convergence".to_owned()],
            conformance_kits: vec!["scalar-convergence-v1".to_owned()],
            evidence: Some(evidence),
        }
    }

    fn contract(id: &str) -> CaseContract {
        contract_with_target(id, cargo_target())
    }

    #[test]
    fn environment_selection_is_explicit_and_does_not_weaken_full_execution() {
        let mut physical = contract("physical");
        let Some(EvidenceTarget::Cargo(target)) = physical.evidence.as_mut() else {
            panic!("fixture has Cargo evidence");
        };
        target.environment = EvidenceEnvironment::PhysicalMpiCuda;

        let selected = execute_selected(
            Path::new("."),
            vec![physical.clone(), contract("portable")],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast)
                .for_environment(EvidenceEnvironment::HostCpu),
            &FakeRunner::new([successful_output()]),
        );
        assert_eq!(
            selected.iter().map(|case| case.outcome).collect::<Vec<_>>(),
            [Outcome::NotSelected, Outcome::Passed]
        );
        assert_eq!(
            selected[0].message.as_deref(),
            Some("evidence requires `physical-mpi-cuda` execution")
        );

        let full = execute_selected(
            Path::new("."),
            vec![physical],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast),
            &FakeRunner::new([EvidenceOutput {
                duration_ms: Some(11),
                exit_code: Some(29),
                stdout: String::new(),
                stderr: String::new(),
                start_error: None,
            }]),
        );
        assert_eq!(full[0].outcome, Outcome::Failed);
    }

    #[test]
    fn capability_index_is_manifest_derived_sorted_and_exactly_filterable() {
        let fixture = Fixture::new();
        fixture.write_manifest(valid_manifest());
        let contracts = load_repository_with_targets(&fixture.root, &targets()).unwrap();

        let all = build_capability_evidence_index(contracts.clone(), None);
        assert!(all.success);
        assert_eq!(
            all.entries
                .iter()
                .map(|entry| entry.capability.as_str())
                .collect::<Vec<_>>(),
            ["conservation", "convergence"]
        );
        assert!(all.entries.iter().all(|entry| {
            entry.case == "area.case"
                && entry.reference_kind == "analytic"
                && entry.conformance_kits == ["scalar-convergence-v1"]
                && entry.evidence.as_ref().is_some_and(|evidence| {
                    matches!(
                        evidence,
                        EvidenceTarget::Cargo(CargoEvidenceTarget { package, test, .. })
                            if package == "eqiora-numerics" && test == "evidence_test"
                    )
                })
        }));

        let selected = build_capability_evidence_index(contracts.clone(), Some("convergence"));
        assert!(selected.success);
        assert_eq!(selected.entries.len(), 1);
        assert_eq!(selected.entries[0].capability, "convergence");

        let unknown = build_capability_evidence_index(contracts, Some("not-declared"));
        assert!(!unknown.success);
        assert!(unknown.entries.is_empty());
        assert_eq!(
            unknown.errors,
            ["unknown verification capability `not-declared`"]
        );
    }

    #[test]
    fn identical_targets_execute_once_and_preserve_distinct_case_reports() {
        let runner = FakeRunner::new([EvidenceOutput {
            duration_ms: Some(37),
            exit_code: Some(0),
            stdout: "shared-out".to_owned(),
            stderr: "shared-err".to_owned(),
            start_error: None,
        }]);
        let reports = execute_selected(
            Path::new("."),
            vec![contract("a"), contract("b")],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::KeepGoing),
            &runner,
        );

        assert_eq!(runner.targets(), [cargo_target()]);
        assert_eq!(
            reports
                .iter()
                .map(|case| (case.id.as_str(), case.outcome))
                .collect::<Vec<_>>(),
            [("a", Outcome::Passed), ("b", Outcome::Passed)]
        );
        assert_eq!(reports[0].stdout.as_deref(), Some("shared-out"));
        assert_eq!(reports[1].stdout.as_deref(), Some("shared-out"));
        assert_eq!(reports[0].stderr.as_deref(), Some("shared-err"));
        assert_eq!(reports[1].stderr.as_deref(), Some("shared-err"));
        assert_eq!(reports[0].duration_ms, Some(37));
        assert_eq!(reports[1].duration_ms, Some(37));

        let report = VerificationReport {
            schema: REPORT_SCHEMA,
            command: CommandKind::Run,
            policy: ExecutionPolicy::KeepGoing,
            selected_case: None,
            selected_environment: None,
            success: true,
            cases: reports,
            errors: Vec::new(),
        };
        assert!(report.render_human().contains("a [verified; 37 ms]"));
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["cases"][0]["duration_ms"], 37);
        assert_eq!(json["cases"][1]["duration_ms"], 37);
    }

    #[test]
    fn non_executed_outcomes_omit_duration() {
        let runner = FakeRunner::new([]);
        let listed = execute_selected(
            Path::new("."),
            vec![contract("listed")],
            &Request::new(CommandKind::List, None, ExecutionPolicy::FailFast),
            &runner,
        );
        let checked = execute_selected(
            Path::new("."),
            vec![contract("checked")],
            &Request::new(CommandKind::Check, None, ExecutionPolicy::FailFast),
            &runner,
        );

        let mut not_runnable_contract = contract("not-runnable");
        not_runnable_contract.status = Status::Specified;
        let not_runnable = execute_selected(
            Path::new("."),
            vec![not_runnable_contract],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast),
            &runner,
        );

        let mut physical_contract = contract("not-selected");
        let Some(EvidenceTarget::Cargo(target)) = physical_contract.evidence.as_mut() else {
            panic!("fixture has Cargo evidence");
        };
        target.environment = EvidenceEnvironment::PhysicalMpiCuda;
        let not_selected = execute_selected(
            Path::new("."),
            vec![physical_contract],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast)
                .for_environment(EvidenceEnvironment::HostCpu),
            &runner,
        );
        assert!(runner.targets().is_empty());

        let mut distinct = cargo_target();
        let EvidenceTarget::Cargo(target) = &mut distinct else {
            panic!("fixture has Cargo evidence");
        };
        target.test = "distinct_test".to_owned();
        let fail_fast_runner = FakeRunner::new([EvidenceOutput {
            duration_ms: Some(19),
            exit_code: Some(17),
            stdout: String::new(),
            stderr: String::new(),
            start_error: None,
        }]);
        let fail_fast = execute_selected(
            Path::new("."),
            vec![
                contract("failed"),
                contract_with_target("skipped", distinct),
            ],
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast),
            &fail_fast_runner,
        );

        let reports = [
            &listed[0],
            &checked[0],
            &not_runnable[0],
            &not_selected[0],
            &fail_fast[1],
        ];
        assert_eq!(
            reports.map(|case| case.outcome),
            [
                Outcome::Listed,
                Outcome::Checked,
                Outcome::NotRunnable,
                Outcome::NotSelected,
                Outcome::Skipped,
            ]
        );
        for case in reports {
            assert_eq!(case.duration_ms, None);
            let json = serde_json::to_value(case).unwrap();
            assert_eq!(json["duration_ms"], serde_json::Value::Null, "{json}");
        }
    }

    #[test]
    fn every_target_field_participates_in_exact_identity() {
        let cargo_base = CargoEvidenceTarget {
            package: "package-a".to_owned(),
            test: "test-a".to_owned(),
            features: vec!["feature-a".to_owned(), "feature-b".to_owned()],
            table: None,
            environment: EvidenceEnvironment::HostCpu,
        };
        let mut targets = vec![EvidenceTarget::Cargo(cargo_base.clone())];
        for target in [
            CargoEvidenceTarget {
                package: "package-b".to_owned(),
                ..cargo_base.clone()
            },
            CargoEvidenceTarget {
                test: "test-b".to_owned(),
                ..cargo_base.clone()
            },
            CargoEvidenceTarget {
                features: vec!["feature-b".to_owned(), "feature-a".to_owned()],
                ..cargo_base.clone()
            },
            CargoEvidenceTarget {
                table: Some("expected/evidence.csv".to_owned()),
                ..cargo_base.clone()
            },
            CargoEvidenceTarget {
                environment: EvidenceEnvironment::PhysicalMpiCuda,
                ..cargo_base
            },
        ] {
            targets.push(EvidenceTarget::Cargo(target));
        }

        let python_base = PythonInstalledWheelEvidenceTarget {
            runner: PythonEvidenceRunner::PythonInstalledWheel,
            script: "tools/ci/python_a.py".to_owned(),
            environment: EvidenceEnvironment::HostCpu,
        };
        targets.extend([
            EvidenceTarget::PythonInstalledWheel(python_base.clone()),
            EvidenceTarget::PythonInstalledWheel(PythonInstalledWheelEvidenceTarget {
                script: "tools/ci/python_b.py".to_owned(),
                ..python_base.clone()
            }),
            EvidenceTarget::PythonInstalledWheel(PythonInstalledWheelEvidenceTarget {
                environment: EvidenceEnvironment::PhysicalMpiCuda,
                ..python_base
            }),
        ]);

        let contracts = targets
            .iter()
            .enumerate()
            .map(|(index, target)| contract_with_target(&format!("case-{index}"), target.clone()))
            .collect();
        let runner = FakeRunner::new((0..targets.len()).map(|_| successful_output()));
        let reports = execute_selected(
            Path::new("."),
            contracts,
            &Request::new(CommandKind::Run, None, ExecutionPolicy::KeepGoing),
            &runner,
        );

        assert!(reports.iter().all(|case| case.outcome == Outcome::Passed));
        assert_eq!(runner.targets(), targets);
    }

    #[test]
    fn shared_failure_forms_are_projected_without_hiding_affected_cases() {
        for output in [
            EvidenceOutput {
                duration_ms: Some(23),
                exit_code: Some(17),
                stdout: "exit-out".to_owned(),
                stderr: "exit-err".to_owned(),
                start_error: None,
            },
            EvidenceOutput {
                duration_ms: Some(29),
                exit_code: None,
                stdout: "signal-out".to_owned(),
                stderr: "signal-err".to_owned(),
                start_error: None,
            },
            EvidenceOutput {
                duration_ms: None,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                start_error: Some("cannot start evidence target: unavailable".to_owned()),
            },
        ] {
            let runner = FakeRunner::new([output.clone()]);
            let reports = execute_selected(
                Path::new("."),
                vec![contract("a"), contract("b")],
                &Request::new(CommandKind::Run, None, ExecutionPolicy::KeepGoing),
                &runner,
            );

            assert_eq!(runner.targets(), [cargo_target()]);
            assert!(reports.iter().all(|case| case.outcome == Outcome::Failed));
            for report in reports {
                assert_eq!(report.duration_ms, output.duration_ms);
                assert_eq!(report.exit_code, output.exit_code);
                assert_eq!(report.stdout.as_deref(), Some(output.stdout.as_str()));
                assert_eq!(report.stderr.as_deref(), Some(output.stderr.as_str()));
                assert_eq!(
                    report.message,
                    output.start_error.clone().or_else(|| Some(format!(
                        "evidence target exited with {}",
                        output
                            .exit_code
                            .map_or_else(|| "no exit code".to_owned(), |code| code.to_string())
                    )))
                );
                let json = serde_json::to_value(report).unwrap();
                match output.duration_ms {
                    Some(duration_ms) => assert_eq!(json["duration_ms"], duration_ms),
                    None => assert_eq!(json["duration_ms"], serde_json::Value::Null, "{json}"),
                }
            }
        }
    }

    #[test]
    fn fail_fast_projects_completed_targets_and_skips_only_unseen_targets() {
        let failure = EvidenceOutput {
            duration_ms: Some(31),
            exit_code: Some(17),
            stdout: "child-out".to_owned(),
            stderr: "child-err".to_owned(),
            start_error: None,
        };
        let mut distinct = cargo_target();
        let EvidenceTarget::Cargo(target) = &mut distinct else {
            panic!("fixture has Cargo evidence");
        };
        target.test = "distinct_test".to_owned();
        let cases = vec![
            contract("a"),
            contract("b"),
            contract_with_target("c", distinct.clone()),
            contract("d"),
        ];

        let fail_fast_runner = FakeRunner::new([failure.clone()]);
        let fail_fast = execute_selected(
            Path::new("."),
            cases.clone(),
            &Request::new(CommandKind::Run, None, ExecutionPolicy::FailFast),
            &fail_fast_runner,
        );
        assert_eq!(fail_fast_runner.targets(), [cargo_target()]);
        assert_eq!(
            fail_fast
                .iter()
                .map(|case| case.outcome)
                .collect::<Vec<_>>(),
            [
                Outcome::Failed,
                Outcome::Failed,
                Outcome::Skipped,
                Outcome::Failed
            ]
        );
        assert_eq!(
            fail_fast[2].message.as_deref(),
            Some("not run after fail-fast evidence failure")
        );

        let keep_going_runner = FakeRunner::new([failure, successful_output()]);
        let keep_going = execute_selected(
            Path::new("."),
            cases,
            &Request::new(CommandKind::Run, None, ExecutionPolicy::KeepGoing),
            &keep_going_runner,
        );
        assert_eq!(keep_going_runner.targets(), [cargo_target(), distinct]);
        assert_eq!(
            keep_going
                .iter()
                .map(|case| case.outcome)
                .collect::<Vec<_>>(),
            [
                Outcome::Failed,
                Outcome::Failed,
                Outcome::Passed,
                Outcome::Failed
            ]
        );
    }
}
