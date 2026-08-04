use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use super::{
    CARGO_LIBRARY_TEST_PREFIX, CargoEvidenceTarget, EvidenceEnvironment, EvidenceTarget,
    PythonInstalledWheelEvidenceTarget, Status,
};

/// The fixed runner identity for an installed-wheel Python evidence target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PythonEvidenceRunner {
    /// Run the repository-owned installed-wheel gate with Python.
    PythonInstalledWheel,
}

impl PythonEvidenceRunner {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::PythonInstalledWheel => "python-installed-wheel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum CargoLibraryEvidenceRunner {
    CargoLibraryTest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoLibraryEvidenceTarget {
    runner: CargoLibraryEvidenceRunner,
    package: String,
    test: String,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    table: Option<String>,
    #[serde(default)]
    environment: EvidenceEnvironment,
}

#[derive(Serialize)]
struct CargoLibraryEvidenceTargetRef<'a> {
    runner: CargoLibraryEvidenceRunner,
    package: &'a str,
    test: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    features: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<&'a str>,
    #[serde(skip_serializing_if = "EvidenceEnvironment::is_host_cpu")]
    environment: EvidenceEnvironment,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EvidenceTargetManifest {
    CargoLibrary(CargoLibraryEvidenceTarget),
    Cargo(CargoEvidenceTarget),
    PythonInstalledWheel(PythonInstalledWheelEvidenceTarget),
}

impl<'de> Deserialize<'de> for EvidenceTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match EvidenceTargetManifest::deserialize(deserializer)? {
            EvidenceTargetManifest::CargoLibrary(target) => {
                let CargoLibraryEvidenceTarget {
                    runner: CargoLibraryEvidenceRunner::CargoLibraryTest,
                    package,
                    test,
                    features,
                    table,
                    environment,
                } = target;
                Ok(Self::Cargo(CargoEvidenceTarget {
                    package,
                    test: format!("{CARGO_LIBRARY_TEST_PREFIX}{test}"),
                    features,
                    table,
                    environment,
                }))
            }
            EvidenceTargetManifest::Cargo(target) => {
                if target.test.starts_with(CARGO_LIBRARY_TEST_PREFIX) {
                    return Err(D::Error::custom(format!(
                        "evidence test names beginning with `{CARGO_LIBRARY_TEST_PREFIX}` are reserved for `runner = \"cargo-library-test\"`"
                    )));
                }
                Ok(Self::Cargo(target))
            }
            EvidenceTargetManifest::PythonInstalledWheel(target) => {
                Ok(Self::PythonInstalledWheel(target))
            }
        }
    }
}

impl Serialize for EvidenceTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Cargo(target) => match target.library_test_name() {
                Some(test) => CargoLibraryEvidenceTargetRef {
                    runner: CargoLibraryEvidenceRunner::CargoLibraryTest,
                    package: &target.package,
                    test,
                    features: &target.features,
                    table: target.table.as_deref(),
                    environment: target.environment,
                }
                .serialize(serializer),
                None => target.serialize(serializer),
            },
            Self::PythonInstalledWheel(target) => target.serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CaseContract {
    pub(super) id: String,
    pub(super) manifest: String,
    pub(super) status: Status,
    pub(super) reference_kind: String,
    pub(super) capabilities: Vec<String>,
    pub(super) conformance_kits: Vec<String>,
    pub(super) evidence: Option<EvidenceTarget>,
}

pub(super) fn load_repository(root: &Path) -> Result<Vec<CaseContract>, String> {
    let targets = discover_workspace_targets(root)?;
    load_repository_with_workspace_targets(root, &targets)
}

#[cfg(test)]
pub(super) fn load_repository_with_targets(
    root: &Path,
    workspace_targets: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<CaseContract>, String> {
    let workspace_targets = workspace_targets
        .iter()
        .map(|(package, integration_tests)| {
            (
                package.clone(),
                WorkspacePackageTargets {
                    integration_tests: integration_tests.clone(),
                    // Unit fixtures using this seam predate library evidence and
                    // model ordinary library crates. Production discovery never
                    // assumes this value.
                    has_library: true,
                },
            )
        })
        .collect();
    load_repository_with_workspace_targets(root, &workspace_targets)
}

fn load_repository_with_workspace_targets(
    root: &Path,
    workspace_targets: &BTreeMap<String, WorkspacePackageTargets>,
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
    workspace_targets: &BTreeMap<String, WorkspacePackageTargets>,
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
                let package_targets =
                    workspace_targets.get(&evidence.package).ok_or_else(|| {
                        format!(
                            "case `{}` names non-workspace evidence package `{}`",
                            manifest.id, evidence.package
                        )
                    })?;
                if let Some(test) = evidence.library_test_name() {
                    if !package_targets.has_library {
                        return Err(format!(
                            "case `{}` names workspace package `{}` without a library target",
                            manifest.id, evidence.package
                        ));
                    }
                    validate_library_test_name(test)?;
                } else {
                    if evidence.test.contains(':') {
                        return Err(format!(
                            "case `{}` names missing integration-test target `{}/{}`",
                            manifest.id, evidence.package, evidence.test
                        ));
                    }
                    validate_stable_name("evidence test", &evidence.test)?;
                    if !package_targets.integration_tests.contains(&evidence.test) {
                        return Err(format!(
                            "case `{}` names missing integration-test target `{}/{}`",
                            manifest.id, evidence.package, evidence.test
                        ));
                    }
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

fn validate_library_test_name(value: &str) -> Result<(), String> {
    let segments = value.split("::").collect::<Vec<_>>();
    let valid = segments.len() >= 2
        && segments.iter().all(|segment| {
            let bytes = segment.as_bytes();
            bytes.first().is_some_and(u8::is_ascii_lowercase)
                && bytes
                    .iter()
                    .skip(1)
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "library evidence test `{value}` is not an exact fully qualified lowercase Rust test name"
        ))
    }
}

#[derive(Debug, Clone)]
struct WorkspacePackageTargets {
    integration_tests: BTreeSet<String>,
    has_library: bool,
}

fn discover_workspace_targets(
    root: &Path,
) -> Result<BTreeMap<String, WorkspacePackageTargets>, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let metadata = read_cargo_metadata(&cargo, root)?;
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
        let has_library = package
            .targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == "lib"));
        let integration_tests = package
            .targets
            .into_iter()
            .filter(|target| target.kind.iter().any(|kind| kind == "test"))
            .map(|target| target.name)
            .collect();
        if result
            .insert(
                package.name.clone(),
                WorkspacePackageTargets {
                    integration_tests,
                    has_library,
                },
            )
            .is_some()
        {
            return Err(format!(
                "Cargo metadata repeats workspace package `{}`",
                package.name
            ));
        }
    }
    Ok(result)
}

fn read_cargo_metadata(cargo: &std::ffi::OsStr, root: &Path) -> Result<CargoMetadata, String> {
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
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot decode Cargo metadata v1: {error}"))
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

pub(super) mod cargo_library {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{EvidenceEnvironment, read_cargo_metadata};

    pub(crate) struct BuildArtifact {
        pub(crate) executable: PathBuf,
        pub(crate) diagnostics: String,
    }

    pub(crate) fn selected_workspace_package_id(
        cargo: &OsStr,
        root: &Path,
        package: &str,
    ) -> Result<String, String> {
        let metadata = read_cargo_metadata(cargo, root)?;
        let workspace_members = metadata
            .workspace_members
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut selected = metadata.packages.iter().filter(|candidate| {
            candidate.name == package && workspace_members.contains(candidate.id.as_str())
        });
        let candidate = selected
            .next()
            .ok_or_else(|| format!("Cargo metadata has no workspace package named `{package}`"))?;
        if selected.next().is_some() {
            return Err(format!(
                "Cargo metadata repeats workspace package `{package}`"
            ));
        }
        if !candidate
            .targets
            .iter()
            .any(|target| target.kind.iter().any(|kind| kind == "lib"))
        {
            return Err(format!(
                "Cargo workspace package `{package}` has no library target"
            ));
        }
        Ok(candidate.id.clone())
    }

    pub(crate) fn inspect_build_output(
        stdout: &[u8],
        stderr: &[u8],
        package: &str,
        selected_package_id: &str,
    ) -> Result<BuildArtifact, String> {
        let mut executables = Vec::new();
        let mut diagnostics = String::new();
        for line in String::from_utf8_lossy(stdout).lines() {
            let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if message["reason"] == "compiler-message" {
                if let Some(rendered) = message["message"]["rendered"].as_str() {
                    append(&mut diagnostics, rendered);
                }
                continue;
            }
            if message["reason"] == "compiler-artifact"
                && message["package_id"].as_str() == Some(selected_package_id)
                && message["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
                && let Some(executable) = message["executable"].as_str()
            {
                executables.push(PathBuf::from(executable));
            }
        }
        if executables.len() != 1 {
            return Err(format!(
                "Cargo did not report exactly one library executable for package `{package}` (found {})",
                executables.len()
            ));
        }
        append(&mut diagnostics, &stderr_without_progress(stderr));
        Ok(BuildArtifact {
            executable: executables.pop().expect("one executable was checked"),
            diagnostics,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_package_id(stdout: &[u8]) -> String {
        let package_ids = String::from_utf8_lossy(stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|message| {
                message["reason"] == "compiler-artifact"
                    && message["target"]["kind"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
            })
            .filter_map(|message| message["package_id"].as_str().map(str::to_owned))
            .filter(|package_id| package_id.starts_with("path+"))
            .collect::<BTreeSet<_>>();
        if package_ids.len() == 1 {
            package_ids.into_iter().next().expect("one ID was checked")
        } else {
            // Production always supplies the exact ID from Cargo metadata.
            // This sentinel only lets parser fixtures exercise rejection.
            String::new()
        }
    }

    pub(crate) fn preflight_test(
        root: &Path,
        executable: &Path,
        test: &str,
        environment: EvidenceEnvironment,
    ) -> Result<(), String> {
        match inventory_count(root, executable, test, false)? {
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
        let ignored = match inventory_count(root, executable, test, true)? {
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

    fn inventory_count(
        root: &Path,
        executable: &Path,
        test: &str,
        ignored: bool,
    ) -> Result<usize, String> {
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

    fn append(destination: &mut String, addition: &str) {
        if !addition.is_empty() {
            if !destination.is_empty() && !destination.ends_with('\n') {
                destination.push('\n');
            }
            destination.push_str(addition);
        }
    }

    fn stderr_without_progress(stderr: &[u8]) -> String {
        let stderr = String::from_utf8_lossy(stderr);
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
}

#[derive(Debug, Deserialize)]
pub(super) struct CaseManifest {
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
