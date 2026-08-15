use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct FullGitOid(pub(crate) String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Sha256(pub(crate) String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExactRepoPath(pub(crate) String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExactAtom(pub(crate) String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExactBytes(pub(crate) Vec<u8>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CaseId(pub(crate) String);

pub(crate) type SortedSet<T> = BTreeSet<T>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NonEmptySortedSet<T>(pub(crate) SortedSet<T>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NonEmptySortedVec<T>(pub(crate) Vec<T>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum GitMode {
    Absent,
    Regular,
    Executable,
    Symlink,
    Gitlink,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[expect(dead_code, reason = "frozen surface; S1 claims only Added replay")]
pub(crate) enum OverlayStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CommitRevision {
    pub(crate) commit: FullGitOid,
    pub(crate) tree: FullGitOid,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct OverlayEntry {
    pub(crate) status: OverlayStatus,
    pub(crate) path: ExactRepoPath,
    pub(crate) base_mode: GitMode,
    pub(crate) base_blob: FullGitOid,
    pub(crate) head_mode: GitMode,
    pub(crate) head_blob: FullGitOid,
    pub(crate) head_bytes: Option<ExactBytes>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExactOverlay {
    pub(crate) base: CommitRevision,
    pub(crate) entries: NonEmptySortedVec<OverlayEntry>,
    pub(crate) overlay_sha256: Sha256,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum AnalysisHead {
    Commit(CommitRevision),
    Overlay(ExactOverlay),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CfgProfile {
    pub(crate) target_triple: ExactAtom,
    pub(crate) enabled_features: SortedSet<ExactAtom>,
    pub(crate) cfg_atoms: SortedSet<ExactAtom>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RevisionSide {
    Base,
    Head,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RevisionIdentity {
    Commit {
        commit: FullGitOid,
        tree: FullGitOid,
    },
    Overlay {
        base_commit: FullGitOid,
        base_tree: FullGitOid,
        overlay_sha256: Sha256,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RevisionPoint {
    pub(crate) side: RevisionSide,
    pub(crate) identity: RevisionIdentity,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CargoTargetKind {
    Library,
    Binary,
    IntegrationTest,
    Example,
    Benchmark,
    BuildScript,
    ProcMacro,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CargoDependencyKind {
    Normal,
    Build,
    Development,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DependencyJoin {
    Workspace {
        package_id: String,
    },
    ExternalRegistry {
        package_name: String,
        version_requirement: String,
        registry: DefaultRegistry,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DefaultRegistry {
    CratesIo,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Declaration {
    pub(super) owner: String,
    pub(super) alias: String,
    pub(super) join: DependencyJoin,
    pub(super) rename: Option<String>,
    pub(super) kind: CargoDependencyKind,
    pub(super) cfg_expression: Option<String>,
    pub(super) optional: bool,
    pub(super) uses_default_features: bool,
    pub(super) requested_features: BTreeSet<String>,
}

#[derive(Clone)]
pub(super) struct DependencyTemplate {
    pub(super) join: DependencyJoin,
    pub(super) features: BTreeSet<String>,
    pub(super) uses_default_features: bool,
}

#[derive(Clone)]
pub(super) enum FeatureRef {
    Local(String),
    ActivateOptional(String),
    ForwardStrong(String, String),
    ForwardWeak(String, String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct NormalizedCargoTarget {
    pub(super) kind: CargoTargetKind,
    pub(super) name: String,
    pub(super) root: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CargoContentRecord {
    pub(crate) revision: RevisionPoint,
    pub(crate) path: ExactRepoPath,
    pub(crate) mode: GitMode,
    pub(crate) blob: FullGitOid,
    pub(crate) byte_len: u64,
    pub(crate) content_sha256: Sha256,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CargoAuthorityRecord {
    Manifest(CargoContentRecord),
    Lock(CargoContentRecord),
    Package {
        revision: RevisionPoint,
        package_id: ExactAtom,
        package_name: ExactAtom,
        manifest_path: ExactRepoPath,
    },
    Target {
        revision: RevisionPoint,
        package_id: ExactAtom,
        target_id: ExactAtom,
        cfg_profile: CfgProfile,
        target_kind: CargoTargetKind,
        crate_root: ExactRepoPath,
    },
    Dependency {
        revision: RevisionPoint,
        dependent_package_id: ExactAtom,
        dependency_package_id: ExactAtom,
        dependency_kind: CargoDependencyKind,
        rename: Option<ExactAtom>,
        optional: bool,
        active_features: SortedSet<ExactAtom>,
        cfg_expression: Option<ExactAtom>,
        cfg_value: bool,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ExactQueryDomain {
    CargoGraph {
        revision: RevisionPoint,
        cfg_profile: CfgProfile,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum CoverageAuthority {
    CargoManifestGraph,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CoverageCertificate {
    pub(crate) domain: ExactQueryDomain,
    pub(crate) authority: CoverageAuthority,
    pub(crate) canonical_input_sha256: Sha256,
    pub(crate) examined_items: u64,
    pub(crate) complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Completeness {
    Complete,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CargoGraphAuthority {
    pub(crate) revision: RevisionPoint,
    pub(crate) cfg_profile: CfgProfile,
    pub(crate) records: SortedSet<CargoAuthorityRecord>,
    pub(crate) certificate: CoverageCertificate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CargoAuthorityRequest {
    pub(crate) base: CommitRevision,
    pub(crate) head: AnalysisHead,
    pub(crate) cfg_profiles: NonEmptySortedSet<CfgProfile>,
    pub(crate) caps: AnalysisCaps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CargoAuthorityAnalysis {
    pub(crate) base: RevisionIdentity,
    pub(crate) head: RevisionIdentity,
    pub(crate) graphs: NonEmptySortedSet<CargoGraphAuthority>,
    pub(crate) completeness: Completeness,
    pub(crate) precise_cases: SortedSet<CaseId>,
    pub(crate) unknowns: SortedSet<ExactRepoPath>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnalysisCaps {
    pub(crate) max_commit_refs: u64,
    pub(crate) max_commit_ref_bytes_each: u64,
    pub(crate) max_changed_facts: u64,
    pub(crate) max_repo_path_bytes: u64,
    pub(crate) max_changed_path_and_raw_diff_bytes: u64,
    pub(crate) max_git_tree_entries_per_side: u64,
    pub(crate) max_git_tree_listing_bytes_per_side: u64,
    pub(crate) max_rust_files_per_side: u64,
    pub(crate) max_rust_source_blob_bytes: u64,
    pub(crate) max_rust_source_bytes_per_side: u64,
    pub(crate) max_case_manifests_per_side: u64,
    pub(crate) max_case_manifest_bytes: u64,
    pub(crate) max_case_manifest_bytes_per_side: u64,
    pub(crate) max_cargo_manifests_per_side: u64,
    pub(crate) max_cargo_manifest_bytes: u64,
    pub(crate) max_cargo_manifest_bytes_per_side: u64,
    pub(crate) max_cargo_lock_bytes_per_side: u64,
    pub(crate) max_cargo_authority_bytes_per_side: u64,
    pub(crate) max_workspace_packages_per_side: u64,
    pub(crate) max_cargo_targets_per_side: u64,
    pub(crate) max_atom_bytes: u64,
    pub(crate) max_derived_atom_bytes: u64,
    pub(crate) max_facade_inventory_bytes_per_side: u64,
    pub(crate) max_facade_entries_per_side: u64,
    pub(crate) max_planner_bytes: u64,
    pub(crate) max_source_items_per_side: u64,
    pub(crate) max_dependency_edges_per_side: u64,
    pub(crate) max_canonical_node_bytes: u64,
    pub(crate) max_canonical_edge_bytes: u64,
    pub(crate) max_reason_chains: u64,
    pub(crate) max_hops_per_reason_chain: u64,
    pub(crate) max_retained_reason_hops: u64,
    pub(crate) max_canonical_reason_bytes: u64,
    pub(crate) max_analyzer_output_bytes: u64,
    pub(crate) max_complete_plan_output_bytes: u64,
    pub(crate) max_abstract_work: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisFailure {
    InvalidRequest,
    InvalidBase,
    InvalidHead,
    InvalidOverlay,
    ConcurrentOverlayChange,
    AuthorityConflict,
    RequiredCoverageMissing,
    CapBeforeSafeFallback,
    InternalFailure,
}

pub(super) fn validate_profile(profile: &CfgProfile) -> Result<(), AnalysisFailure> {
    let mut common = BTreeSet::new();
    for atom in [
        "debug_assertions",
        "target_arch=\"x86_64\"",
        "target_endian=\"little\"",
        "target_env=\"gnu\"",
        "target_family=\"unix\"",
        "target_os=\"linux\"",
        "target_pointer_width=\"64\"",
        "unix",
    ] {
        common.insert(ExactAtom(atom.to_owned()));
    }
    let mut test = common.clone();
    test.insert(ExactAtom("test".to_owned()));
    let admitted = profile.target_triple.0 == "x86_64-unknown-linux-gnu"
        && profile.enabled_features.is_empty()
        && (profile.cfg_atoms == test || profile.cfg_atoms == common);
    admitted
        .then_some(())
        .ok_or(AnalysisFailure::InvalidRequest)
}

pub(super) fn validate_repo_path(path: &str, cap: u64) -> Result<(), AnalysisFailure> {
    let valid = !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\0')
        && !path.contains('\\')
        && !path
            .bytes()
            .any(|byte| byte == b':' || byte.is_ascii_control())
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    valid
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    check_atom_cap(path, cap)
}

pub(super) fn validate_atom(atom: &str, cap: u64) -> Result<(), AnalysisFailure> {
    (!atom.is_empty() && !atom.contains('\0'))
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    check_atom_cap(atom, cap)
}

pub(super) fn normalize_repo_path(
    directory: &str,
    relative: &str,
    cap: u64,
) -> Result<String, AnalysisFailure> {
    if relative.starts_with('/') || relative.contains('\\') || relative.contains('\0') {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in relative.split('/') {
        match part {
            "" | "." | ".." => return Err(AnalysisFailure::RequiredCoverageMissing),
            part => parts.push(part),
        }
    }
    let path = parts.join("/");
    validate_repo_path(&path, cap)?;
    Ok(path)
}

pub(super) fn validate_package_name(value: &str, cap: u64) -> Result<(), AnalysisFailure> {
    let mut bytes = value.bytes();
    let first = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric());
    let rest = bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    (first && rest)
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    check_atom_cap(value, cap)
}

pub(super) fn validate_target_name(value: &str, cap: u64) -> Result<(), AnalysisFailure> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| !byte.is_ascii_control() && !b":/\\".contains(&byte));
    valid
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    check_atom_cap(value, cap)
}

pub(super) fn validate_feature_atom(value: &str, cap: u64) -> Result<(), AnalysisFailure> {
    let mut bytes = value.bytes();
    let first = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    let rest = bytes.all(|byte| byte.is_ascii_alphanumeric() || b"_+.-".contains(&byte));
    (first && rest)
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    check_atom_cap(value, cap)
}

fn check_atom_cap(value: &str, cap: u64) -> Result<(), AnalysisFailure> {
    (value.len() as u64 <= cap)
        .then_some(())
        .ok_or(AnalysisFailure::CapBeforeSafeFallback)
}

pub(super) fn parse_feature_ref(value: &str, cap: u64) -> Result<FeatureRef, AnalysisFailure> {
    if let Some(alias) = value.strip_prefix("dep:") {
        validate_feature_atom(alias, cap)?;
        return Ok(FeatureRef::ActivateOptional(alias.to_owned()));
    }
    if let Some((alias, feature)) = value.split_once("?/") {
        validate_feature_atom(alias, cap)?;
        validate_feature_atom(feature, cap)?;
        return Ok(FeatureRef::ForwardWeak(
            alias.to_owned(),
            feature.to_owned(),
        ));
    }
    if let Some((alias, feature)) = value.split_once('/') {
        validate_feature_atom(alias, cap)?;
        validate_feature_atom(feature, cap)?;
        return Ok(FeatureRef::ForwardStrong(
            alias.to_owned(),
            feature.to_owned(),
        ));
    }
    validate_feature_atom(value, cap)?;
    Ok(FeatureRef::Local(value.to_owned()))
}

pub(super) fn external_dependency_join(
    version: &str,
    package: &str,
    cap: u64,
) -> Result<DependencyJoin, AnalysisFailure> {
    validate_atom(version, cap)?;
    validate_feature_atom(package, cap)?;
    if version.is_empty() {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    Ok(DependencyJoin::ExternalRegistry {
        package_name: package.to_owned(),
        version_requirement: version.to_owned(),
        registry: DefaultRegistry::CratesIo,
    })
}

pub(super) fn validate_edition(value: &str) -> Result<(), AnalysisFailure> {
    matches!(value, "2018" | "2021" | "2024")
        .then_some(())
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn validate_library_types(
    types: Option<&BTreeSet<String>>,
) -> Result<(), AnalysisFailure> {
    let admitted: BTreeSet<&str> = ["lib", "rlib", "dylib", "staticlib", "cdylib"]
        .into_iter()
        .collect();
    if types.is_some_and(|types| {
        types.is_empty() || types.iter().any(|item| !admitted.contains(item.as_str()))
    }) {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    Ok(())
}

pub(super) fn checked_charge(
    current: u64,
    amount: usize,
    cap: u64,
) -> Result<u64, AnalysisFailure> {
    let next = current
        .checked_add(amount as u64)
        .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
    (next <= cap)
        .then_some(next)
        .ok_or(AnalysisFailure::CapBeforeSafeFallback)
}

pub(super) fn package_id(name: &str, version: &str, manifest: &str) -> ExactAtom {
    ExactAtom(format!(
        "cargo-package-v1:{}:{name}:{}:{version}:{}:{manifest}",
        name.len(),
        version.len(),
        manifest.len()
    ))
}

pub(super) fn target_kind_atom(kind: &CargoTargetKind) -> &'static str {
    match kind {
        CargoTargetKind::Library => "library",
        CargoTargetKind::Binary => "binary",
        CargoTargetKind::IntegrationTest => "integration-test",
        CargoTargetKind::Example => "example",
        CargoTargetKind::Benchmark => "benchmark",
        CargoTargetKind::BuildScript => "build-script",
        CargoTargetKind::ProcMacro => "proc-macro",
    }
}

pub(super) fn target_id(
    package: &str,
    kind: &CargoTargetKind,
    name: &str,
    root: &str,
) -> ExactAtom {
    let kind = target_kind_atom(kind);
    ExactAtom(format!(
        "cargo-target-v1:{}:{package}:{}:{kind}:{}:{name}:{}:{root}",
        package.len(),
        kind.len(),
        name.len(),
        root.len()
    ))
}

pub(super) fn build_graph(
    revision: RevisionPoint,
    profile: CfgProfile,
    records: SortedSet<CargoAuthorityRecord>,
    side_total: &mut u64,
    cap: u64,
) -> Result<CargoGraphAuthority, AnalysisFailure> {
    let mut stream = Vec::new();
    for record in &records {
        let mut count = CountSink(0);
        write_record(record, &mut count);
        count.push("\n");
        *side_total = side_total
            .checked_add(count.0)
            .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
        if *side_total > cap {
            return Err(AnalysisFailure::CapBeforeSafeFallback);
        }
        let mut sink = ByteSink(&mut stream);
        write_record(record, &mut sink);
        sink.push("\n");
    }
    let certificate = CoverageCertificate {
        domain: ExactQueryDomain::CargoGraph {
            revision: revision.clone(),
            cfg_profile: profile.clone(),
        },
        authority: CoverageAuthority::CargoManifestGraph,
        canonical_input_sha256: super::change::sha256(&stream),
        examined_items: records.len() as u64,
        complete: true,
    };
    Ok(CargoGraphAuthority {
        revision,
        cfg_profile: profile,
        records,
        certificate,
    })
}

trait Sink {
    fn push(&mut self, value: &str);
}

struct CountSink(u64);

impl Sink for CountSink {
    fn push(&mut self, value: &str) {
        self.0 = self.0.saturating_add(value.len() as u64);
    }
}

struct ByteSink<'a>(&'a mut Vec<u8>);

impl Sink for ByteSink<'_> {
    fn push(&mut self, value: &str) {
        self.0.extend_from_slice(value.as_bytes());
    }
}

fn write_record(record: &CargoAuthorityRecord, out: &mut impl Sink) {
    match record {
        CargoAuthorityRecord::Manifest(content) => {
            out.push("{\"kind\":\"cargo-manifest-input\",\"content\":");
            write_content(content, out);
        }
        CargoAuthorityRecord::Lock(content) => {
            out.push("{\"kind\":\"cargo-lock-input\",\"content\":");
            write_content(content, out);
        }
        value @ CargoAuthorityRecord::Package { .. } => {
            out.push("{\"kind\":\"cargo-package\",\"value\":");
            write_package(value, out);
        }
        value @ CargoAuthorityRecord::Target { .. } => {
            out.push("{\"kind\":\"cargo-target\",\"value\":");
            write_target(value, out);
        }
        value @ CargoAuthorityRecord::Dependency { .. } => {
            out.push("{\"kind\":\"cargo-dependency\",\"value\":");
            write_dependency(value, out);
        }
    }
    out.push("}");
}

fn write_content(content: &CargoContentRecord, out: &mut impl Sink) {
    out.push("{\"entry\":{\"revision\":");
    write_revision(&content.revision, out);
    out.push(",\"path\":");
    write_json(&content.path.0, out);
    out.push(",\"mode\":");
    write_json(mode_atom(&content.mode), out);
    out.push(",\"blob\":");
    write_json(&content.blob.0, out);
    out.push(",\"byte_len\":");
    out.push(&content.byte_len.to_string());
    out.push("},\"content_sha256\":");
    write_json(&content.content_sha256.0, out);
    out.push("}");
}

fn write_package(value: &CargoAuthorityRecord, out: &mut impl Sink) {
    let CargoAuthorityRecord::Package {
        revision,
        package_id,
        package_name,
        manifest_path,
    } = value
    else {
        unreachable!("package record")
    };
    out.push("{\"revision\":");
    write_revision(revision, out);
    for (key, value) in [
        ("package_id", &package_id.0),
        ("package_name", &package_name.0),
        ("manifest_path", &manifest_path.0),
    ] {
        out.push(",\"");
        out.push(key);
        out.push("\":");
        write_json(value, out);
    }
    out.push("}");
}

fn write_target(value: &CargoAuthorityRecord, out: &mut impl Sink) {
    let CargoAuthorityRecord::Target {
        revision,
        package_id,
        target_id,
        cfg_profile,
        target_kind,
        crate_root,
    } = value
    else {
        unreachable!("target record")
    };
    out.push("{\"target\":{\"revision\":");
    write_revision(revision, out);
    out.push(",\"package_id\":");
    write_json(&package_id.0, out);
    out.push(",\"target_id\":");
    write_json(&target_id.0, out);
    out.push(",\"cfg_profile\":");
    write_profile(cfg_profile, out);
    out.push("},\"target_kind\":");
    write_json(target_kind_atom(target_kind), out);
    out.push(",\"crate_root\":");
    write_json(&crate_root.0, out);
    out.push("}");
}

fn write_dependency(value: &CargoAuthorityRecord, out: &mut impl Sink) {
    let CargoAuthorityRecord::Dependency {
        revision,
        dependent_package_id,
        dependency_package_id,
        dependency_kind,
        rename,
        optional,
        active_features,
        cfg_expression,
        cfg_value,
    } = value
    else {
        unreachable!("dependency record")
    };
    out.push("{\"revision\":");
    write_revision(revision, out);
    for (key, value) in [
        ("dependent_package_id", &dependent_package_id.0),
        ("dependency_package_id", &dependency_package_id.0),
    ] {
        out.push(",\"");
        out.push(key);
        out.push("\":");
        write_json(value, out);
    }
    out.push(",\"dependency_kind\":");
    write_json(dependency_kind_atom(dependency_kind), out);
    out.push(",\"rename\":");
    write_option_atom(rename, out);
    out.push(",\"optional\":");
    out.push(if *optional { "true" } else { "false" });
    out.push(",\"active_features\":");
    write_atom_set(active_features, out);
    out.push(",\"cfg_expression\":");
    write_option_atom(cfg_expression, out);
    out.push(",\"cfg_value\":");
    out.push(if *cfg_value { "true" } else { "false" });
    out.push("}");
}

fn write_revision(value: &RevisionPoint, out: &mut impl Sink) {
    out.push("{\"side\":");
    write_json(
        match value.side {
            RevisionSide::Base => "base",
            RevisionSide::Head => "head",
        },
        out,
    );
    out.push(",\"identity\":");
    write_identity(&value.identity, out);
    out.push("}");
}

fn write_identity(value: &RevisionIdentity, out: &mut impl Sink) {
    match value {
        RevisionIdentity::Commit { commit, tree } => {
            out.push("{\"kind\":\"commit\",\"commit\":");
            write_json(&commit.0, out);
            out.push(",\"tree\":");
            write_json(&tree.0, out);
        }
        RevisionIdentity::Overlay {
            base_commit,
            base_tree,
            overlay_sha256,
        } => {
            out.push("{\"kind\":\"overlay\",\"base_commit\":");
            write_json(&base_commit.0, out);
            out.push(",\"base_tree\":");
            write_json(&base_tree.0, out);
            out.push(",\"overlay_sha256\":");
            write_json(&overlay_sha256.0, out);
        }
    }
    out.push("}");
}

fn write_profile(value: &CfgProfile, out: &mut impl Sink) {
    out.push("{\"target_triple\":");
    write_json(&value.target_triple.0, out);
    out.push(",\"enabled_features\":");
    write_atom_set(&value.enabled_features, out);
    out.push(",\"cfg_atoms\":");
    write_atom_set(&value.cfg_atoms, out);
    out.push("}");
}

fn write_atom_set(values: &SortedSet<ExactAtom>, out: &mut impl Sink) {
    out.push("[");
    for (index, value) in values.iter().enumerate() {
        out.push(if index == 0 { "" } else { "," });
        write_json(&value.0, out);
    }
    out.push("]");
}

fn write_option_atom(value: &Option<ExactAtom>, out: &mut impl Sink) {
    if let Some(value) = value {
        write_json(&value.0, out);
    } else {
        out.push("null");
    }
}

fn write_json(value: &str, out: &mut impl Sink) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push("\"");
    for character in value.chars() {
        match character {
            '"' => out.push("\\\""),
            '\\' => out.push("\\\\"),
            '\u{0}'..='\u{1f}' => {
                let byte = character as u8;
                let escaped = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[(byte >> 4) as usize],
                    HEX[(byte & 15) as usize],
                ];
                out.push(std::str::from_utf8(&escaped).expect("ASCII escape"));
            }
            other => out.push(other.encode_utf8(&mut [0_u8; 4])),
        }
    }
    out.push("\"");
}

fn mode_atom(mode: &GitMode) -> &'static str {
    match mode {
        GitMode::Absent => "000000",
        GitMode::Regular => "100644",
        GitMode::Executable => "100755",
        GitMode::Symlink => "120000",
        GitMode::Gitlink => "160000",
    }
}

fn dependency_kind_atom(kind: &CargoDependencyKind) -> &'static str {
    match kind {
        CargoDependencyKind::Normal => "normal",
        CargoDependencyKind::Build => "build",
        CargoDependencyKind::Development => "development",
    }
}
