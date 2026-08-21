use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::process::{Command, Stdio};

use toml::Table;

use super::cargo::semantic_records;
use super::model::{
    AnalysisCaps, AnalysisFailure, AnalysisHead, CargoAuthorityRecord, CargoAuthorityRequest,
    CargoContentRecord, CargoGraphAuthority, CommitRevision, ExactAtom, ExactRepoPath, FullGitOid,
    GitMode, NonEmptySortedSet, RevisionIdentity, RevisionPoint, RevisionSide, SortedSet,
    build_graph, checked_charge, normalize_repo_path, validate_atom, validate_profile,
    validate_repo_path,
};

const C1_HEAD_COMMIT: &str = "8e9aec96d5170cb7ab7b7a5f52281e0a0ef09582";
const C1_HEAD_TREE: &str = "9994b7e1414447d9781f53608032c93f48cb91d6";

#[derive(Clone)]
pub(super) struct ManifestDocument {
    pub(super) path: ExactRepoPath,
    pub(super) table: Table,
}

pub(super) struct WorkspaceDocuments {
    pub(super) root: Table,
    pub(super) members: Vec<ManifestDocument>,
}

#[derive(Clone)]
pub(super) struct TreeEntry {
    pub(super) mode: GitMode,
    pub(super) blob: FullGitOid,
    pub(super) byte_len: u64,
    overlay_bytes: Option<Vec<u8>>,
}

#[derive(Clone)]
pub(super) struct TreeImage {
    entries: BTreeMap<String, TreeEntry>,
}

impl TreeImage {
    pub(super) fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    pub(super) fn is_regular(&self, path: &str) -> bool {
        self.entries
            .get(path)
            .is_some_and(|entry| entry.mode == GitMode::Regular)
    }

    pub(super) fn require_regular(&self, path: String) -> Result<String, AnalysisFailure> {
        (path.ends_with(".rs") && self.is_regular(&path))
            .then_some(path)
            .ok_or(AnalysisFailure::RequiredCoverageMissing)
    }

    pub(super) fn regular_paths(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.mode == GitMode::Regular)
            .map(|(path, _)| path.as_str())
    }

    pub(super) fn added_regular(
        &mut self,
        path: String,
        blob: FullGitOid,
        bytes: Vec<u8>,
    ) -> Result<(), AnalysisFailure> {
        if self.entries.contains_key(&path) {
            return Err(AnalysisFailure::InvalidOverlay);
        }
        self.entries.insert(
            path,
            TreeEntry {
                mode: GitMode::Regular,
                blob,
                byte_len: bytes.len() as u64,
                overlay_bytes: Some(bytes),
            },
        );
        Ok(())
    }

    pub(super) fn entry(&self, path: &str) -> Option<&TreeEntry> {
        self.entries.get(path)
    }
}

struct SideInput {
    documents: WorkspaceDocuments,
    content_records: SortedSet<CargoAuthorityRecord>,
}

pub(super) fn insert_authority_record(
    records: &mut SortedSet<CargoAuthorityRecord>,
    record: CargoAuthorityRecord,
    caps: &AnalysisCaps,
    derived_bytes: &mut u64,
) -> Result<(), AnalysisFailure> {
    if records.contains(&record) {
        return Ok(());
    }
    match &record {
        CargoAuthorityRecord::Manifest(_) | CargoAuthorityRecord::Lock(_) => {}
        CargoAuthorityRecord::Package {
            package_id,
            package_name,
            ..
        } => {
            charge_record_atom(package_id, caps, derived_bytes)?;
            charge_record_atom(package_name, caps, derived_bytes)?;
        }
        CargoAuthorityRecord::Target {
            package_id,
            target_id,
            cfg_profile,
            ..
        } => {
            charge_record_atom(package_id, caps, derived_bytes)?;
            charge_record_atom(target_id, caps, derived_bytes)?;
            charge_record_atom(&cfg_profile.target_triple, caps, derived_bytes)?;
            for atom in cfg_profile
                .enabled_features
                .iter()
                .chain(&cfg_profile.cfg_atoms)
            {
                charge_record_atom(atom, caps, derived_bytes)?;
            }
        }
        CargoAuthorityRecord::Dependency {
            dependent_package_id,
            dependency_package_id,
            rename,
            active_features,
            cfg_expression,
            ..
        } => {
            charge_record_atom(dependent_package_id, caps, derived_bytes)?;
            charge_record_atom(dependency_package_id, caps, derived_bytes)?;
            for atom in rename.iter().chain(active_features).chain(cfg_expression) {
                charge_record_atom(atom, caps, derived_bytes)?;
            }
        }
    }
    records.insert(record);
    Ok(())
}

fn charge_record_atom(
    atom: &ExactAtom,
    caps: &AnalysisCaps,
    derived_bytes: &mut u64,
) -> Result<(), AnalysisFailure> {
    validate_atom(&atom.0, caps.max_atom_bytes)?;
    *derived_bytes = checked_charge(*derived_bytes, atom.0.len(), caps.max_derived_atom_bytes)?;
    Ok(())
}

pub(crate) fn analyze_cargo_authority(
    request: CargoAuthorityRequest,
) -> Result<super::model::CargoAuthorityAnalysis, AnalysisFailure> {
    validate_request(&request)?;
    let overlay_carrier = match &request.head {
        AnalysisHead::Overlay(overlay) => Some(super::change::preflight_added_overlay(
            overlay,
            &request.caps,
        )?),
        AnalysisHead::Commit(_) => None,
    };
    let base_tree = load_tree(&request.base, &request.caps)?;
    let (head_tree, head_identity) = match (&request.head, overlay_carrier) {
        (AnalysisHead::Commit(commit), None) => {
            read_commit_diff(&request.base, commit, &request)?;
            (load_tree(commit, &request.caps)?, commit_identity(commit))
        }
        (AnalysisHead::Overlay(overlay), Some(carrier)) => (
            super::change::apply_added_overlay(&base_tree, overlay, &carrier, &request.caps)?,
            RevisionIdentity::Overlay {
                base_commit: overlay.base.commit.clone(),
                base_tree: overlay.base.tree.clone(),
                overlay_sha256: overlay.overlay_sha256.clone(),
            },
        ),
        _ => return Err(AnalysisFailure::InternalFailure),
    };
    check_logical_tree(&head_tree, &request)?;
    let base_identity = commit_identity(&request.base);
    let base_point = RevisionPoint {
        side: RevisionSide::Base,
        identity: base_identity.clone(),
    };
    let head_point = RevisionPoint {
        side: RevisionSide::Head,
        identity: head_identity.clone(),
    };
    let base_input = load_side(&base_tree, &base_point, &request)?;
    let head_input = load_side(&head_tree, &head_point, &request)?;
    let mut graphs = BTreeSet::new();
    let mut base_total = 0_u64;
    for profile in &request.cfg_profiles.0 {
        graphs.insert(graph_for_profile(
            &base_input,
            &base_tree,
            &base_point,
            profile,
            &request,
            &mut base_total,
        )?);
    }
    let mut head_total = 0_u64;
    for profile in &request.cfg_profiles.0 {
        graphs.insert(graph_for_profile(
            &head_input,
            &head_tree,
            &head_point,
            profile,
            &request,
            &mut head_total,
        )?);
    }
    super::change::finish_analysis(
        &request,
        base_identity,
        head_identity,
        NonEmptySortedSet(graphs),
    )
}

fn validate_request(request: &CargoAuthorityRequest) -> Result<(), AnalysisFailure> {
    if request.cfg_profiles.0.is_empty() {
        return Err(AnalysisFailure::InvalidRequest);
    }
    if request.caps.max_commit_refs < 2 || request.caps.max_commit_ref_bytes_each < 40 {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    validate_commit_shape(&request.base).map_err(|_| AnalysisFailure::InvalidBase)?;
    if request.base.commit.0 != C1_HEAD_COMMIT || request.base.tree.0 != C1_HEAD_TREE {
        return Err(AnalysisFailure::InvalidBase);
    }
    for profile in &request.cfg_profiles.0 {
        validate_profile(profile)?;
    }
    match &request.head {
        AnalysisHead::Commit(commit) => {
            validate_commit_shape(commit).map_err(|_| AnalysisFailure::InvalidHead)?;
            if commit.commit.0 != C1_HEAD_COMMIT || commit.tree.0 != C1_HEAD_TREE {
                return Err(AnalysisFailure::InvalidHead);
            }
        }
        AnalysisHead::Overlay(overlay) => {
            validate_commit_shape(&overlay.base).map_err(|_| AnalysisFailure::InvalidOverlay)?;
            if overlay.base != request.base || overlay.base.commit.0 != C1_HEAD_COMMIT {
                return Err(AnalysisFailure::InvalidOverlay);
            }
        }
    }
    validate_git_commit(&request.base)
}

fn validate_commit_shape(commit: &CommitRevision) -> Result<(), AnalysisFailure> {
    let valid = is_lower_hex(&commit.commit.0, 40) && is_lower_hex(&commit.tree.0, 40);
    valid.then_some(()).ok_or(AnalysisFailure::InvalidRequest)
}

fn validate_git_commit(commit: &CommitRevision) -> Result<(), AnalysisFailure> {
    let object = run_git_bounded(
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", commit.commit.0),
        ],
        64,
    )?;
    if trim_lf(&object) != commit.commit.0.as_bytes() {
        return Err(AnalysisFailure::InvalidBase);
    }
    let tree = run_git_bounded(&["rev-parse", &format!("{}^{{tree}}", commit.commit.0)], 64)?;
    if trim_lf(&tree) != commit.tree.0.as_bytes() {
        return Err(AnalysisFailure::InvalidBase);
    }
    Ok(())
}

fn read_commit_diff(
    base: &CommitRevision,
    head: &CommitRevision,
    request: &CargoAuthorityRequest,
) -> Result<(), AnalysisFailure> {
    let bytes = run_git_bounded(
        &[
            "diff-tree",
            "-r",
            "--raw",
            "-z",
            "--no-renames",
            &base.commit.0,
            &head.commit.0,
        ],
        request.caps.max_changed_path_and_raw_diff_bytes,
    )?;
    let changed = bytes.iter().filter(|byte| **byte == 0).count() as u64;
    if changed > request.caps.max_changed_facts {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    Ok(())
}

fn load_tree(
    commit: &CommitRevision,
    caps: &super::model::AnalysisCaps,
) -> Result<TreeImage, AnalysisFailure> {
    let output = run_git_bounded(
        &["ls-tree", "-rlz", "--full-tree", &commit.commit.0],
        caps.max_git_tree_listing_bytes_per_side,
    )?;
    let mut entries = BTreeMap::new();
    let mut count = 0_u64;
    for raw in output.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        count = count
            .checked_add(1)
            .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
        if count > caps.max_git_tree_entries_per_side {
            return Err(AnalysisFailure::CapBeforeSafeFallback);
        }
        let (path, entry) = parse_tree_record(raw, caps.max_repo_path_bytes)?;
        if entries.insert(path, entry).is_some() {
            return Err(AnalysisFailure::AuthorityConflict);
        }
    }
    Ok(TreeImage { entries })
}

fn parse_tree_record(raw: &[u8], path_cap: u64) -> Result<(String, TreeEntry), AnalysisFailure> {
    let tab = raw
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or(AnalysisFailure::InternalFailure)?;
    let header = std::str::from_utf8(&raw[..tab]).map_err(|_| AnalysisFailure::InternalFailure)?;
    let path = std::str::from_utf8(&raw[tab + 1..])
        .map_err(|_| AnalysisFailure::RequiredCoverageMissing)?
        .to_owned();
    validate_repo_path(&path, path_cap)?;
    let mut fields = header.split_whitespace();
    let mode = parse_mode(fields.next().ok_or(AnalysisFailure::InternalFailure)?)?;
    let object_type = fields.next().ok_or(AnalysisFailure::InternalFailure)?;
    let blob = fields.next().ok_or(AnalysisFailure::InternalFailure)?;
    let size = fields.next().ok_or(AnalysisFailure::InternalFailure)?;
    if fields.next().is_some() || !is_lower_hex(blob, 40) {
        return Err(AnalysisFailure::InternalFailure);
    }
    let byte_len = if object_type == "commit" && size == "-" {
        0
    } else {
        size.parse().map_err(|_| AnalysisFailure::InternalFailure)?
    };
    Ok((
        path,
        TreeEntry {
            mode,
            blob: FullGitOid(blob.to_owned()),
            byte_len,
            overlay_bytes: None,
        },
    ))
}

fn check_logical_tree(
    tree: &TreeImage,
    request: &CargoAuthorityRequest,
) -> Result<(), AnalysisFailure> {
    if tree.entries.len() as u64 > request.caps.max_git_tree_entries_per_side {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    Ok(())
}

fn load_side(
    tree: &TreeImage,
    revision: &RevisionPoint,
    request: &CargoAuthorityRequest,
) -> Result<SideInput, AnalysisFailure> {
    let mut manifest_total = 0_u64;
    let mut cache = BTreeMap::new();
    let mut content_records = BTreeSet::new();
    let root_bytes = read_tree_blob(
        tree,
        "Cargo.toml",
        request.caps.max_cargo_manifest_bytes,
        &mut manifest_total,
        request.caps.max_cargo_manifest_bytes_per_side,
        &mut cache,
    )?;
    let root = parse_manifest(root_bytes)?;
    content_records.insert(CargoAuthorityRecord::Manifest(content_identity(
        tree,
        "Cargo.toml",
        revision,
        root_bytes,
    )?));
    let manifest_paths = member_manifest_paths(tree, &root)?;
    if manifest_paths.len() as u64 + 1 > request.caps.max_cargo_manifests_per_side {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    let mut parsed = Vec::with_capacity(manifest_paths.len());
    for path in &manifest_paths {
        let bytes = read_tree_blob(
            tree,
            path,
            request.caps.max_cargo_manifest_bytes,
            &mut manifest_total,
            request.caps.max_cargo_manifest_bytes_per_side,
            &mut cache,
        )?;
        let table = parse_manifest(bytes)?;
        let content = content_identity(tree, path, revision, bytes)?;
        content_records.insert(CargoAuthorityRecord::Manifest(content));
        parsed.push(ManifestDocument {
            path: ExactRepoPath(path.clone()),
            table,
        });
    }
    if root.contains_key("patch") || root.contains_key("replace") {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    let mut lock_total = 0_u64;
    let lock = read_tree_blob(
        tree,
        "Cargo.lock",
        request.caps.max_cargo_lock_bytes_per_side,
        &mut lock_total,
        request.caps.max_cargo_lock_bytes_per_side,
        &mut cache,
    )?;
    content_records.insert(CargoAuthorityRecord::Lock(content_identity(
        tree,
        "Cargo.lock",
        revision,
        lock,
    )?));
    Ok(SideInput {
        documents: WorkspaceDocuments {
            root,
            members: parsed,
        },
        content_records,
    })
}

fn member_manifest_paths(tree: &TreeImage, root: &Table) -> Result<Vec<String>, AnalysisFailure> {
    let workspace = root
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let members = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let excludes = workspace
        .get("exclude")
        .map(string_array)
        .transpose()?
        .unwrap_or_default();
    let patterns = members
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(AnalysisFailure::RequiredCoverageMissing)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut paths: Vec<_> = tree
        .regular_paths()
        .filter(|path| *path != "Cargo.toml" && path.ends_with("/Cargo.toml"))
        .filter(|path| {
            patterns
                .iter()
                .any(|pattern| member_pattern_matches(pattern, path))
        })
        .filter(|path| {
            !excludes
                .iter()
                .any(|pattern| member_pattern_matches(pattern, path))
        })
        .map(str::to_owned)
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    Ok(paths)
}

fn graph_for_profile(
    side: &SideInput,
    tree: &TreeImage,
    revision: &RevisionPoint,
    profile: &super::model::CfgProfile,
    request: &CargoAuthorityRequest,
    side_total: &mut u64,
) -> Result<CargoGraphAuthority, AnalysisFailure> {
    let mut records = side.content_records.clone();
    records.extend(semantic_records(
        &side.documents,
        tree,
        revision,
        profile,
        &request.caps,
    )?);
    build_graph(
        revision.clone(),
        profile.clone(),
        records,
        side_total,
        request.caps.max_cargo_authority_bytes_per_side,
    )
}

fn read_tree_blob<'a>(
    tree: &TreeImage,
    path: &str,
    per_blob_cap: u64,
    aggregate: &mut u64,
    aggregate_cap: u64,
    cache: &'a mut BTreeMap<String, Vec<u8>>,
) -> Result<&'a [u8], AnalysisFailure> {
    let entry = tree
        .entry(path)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    if entry.mode != GitMode::Regular {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    if entry.byte_len > per_blob_cap {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    *aggregate = aggregate
        .checked_add(entry.byte_len)
        .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
    if *aggregate > aggregate_cap {
        return Err(AnalysisFailure::CapBeforeSafeFallback);
    }
    if !cache.contains_key(path) {
        let bytes = if let Some(bytes) = &entry.overlay_bytes {
            bytes.clone()
        } else {
            run_git_bounded(&["cat-file", "blob", &entry.blob.0], entry.byte_len)?
        };
        if bytes.len() as u64 != entry.byte_len {
            return Err(AnalysisFailure::InternalFailure);
        }
        cache.insert(path.to_owned(), bytes);
    }
    Ok(cache.get(path).expect("cached blob"))
}

fn content_identity(
    tree: &TreeImage,
    path: &str,
    revision: &RevisionPoint,
    bytes: &[u8],
) -> Result<CargoContentRecord, AnalysisFailure> {
    let entry = tree
        .entry(path)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    Ok(CargoContentRecord {
        revision: revision.clone(),
        path: ExactRepoPath(path.to_owned()),
        mode: entry.mode.clone(),
        blob: entry.blob.clone(),
        byte_len: entry.byte_len,
        content_sha256: super::change::sha256(bytes),
    })
}

fn parse_manifest(bytes: &[u8]) -> Result<Table, AnalysisFailure> {
    let text = std::str::from_utf8(bytes).map_err(|_| AnalysisFailure::RequiredCoverageMissing)?;
    toml::from_str(text).map_err(|_| AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn inherited_string<'a>(
    value: Option<&'a toml::Value>,
    workspace: &'a str,
) -> Result<&'a str, AnalysisFailure> {
    match value {
        Some(toml::Value::String(value)) => Ok(value),
        Some(toml::Value::Table(table))
            if table.len() == 1
                && table.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Ok(workspace)
        }
        _ => Err(AnalysisFailure::RequiredCoverageMissing),
    }
}

pub(super) fn required_table<'a>(
    table: &'a Table,
    key: &str,
) -> Result<&'a Table, AnalysisFailure> {
    table
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn required_string<'a>(table: &'a Table, key: &str) -> Result<&'a str, AnalysisFailure> {
    table
        .get(key)
        .map(expect_string)
        .transpose()?
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn expect_string(value: &toml::Value) -> Result<&str, AnalysisFailure> {
    value
        .as_str()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn expect_bool(value: &toml::Value) -> Result<bool, AnalysisFailure> {
    value
        .as_bool()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn optional_bool(table: &Table, key: &str) -> Result<Option<bool>, AnalysisFailure> {
    table.get(key).map(expect_bool).transpose()
}

pub(super) fn workspace_dependency(value: &toml::Value) -> Result<bool, AnalysisFailure> {
    let Some(table) = value.as_table() else {
        return Ok(false);
    };
    let Some(raw) = table.get("workspace") else {
        return Ok(false);
    };
    if !expect_bool(raw)?
        || table.keys().any(|key| {
            !matches!(
                key.as_str(),
                "workspace" | "features" | "default-features" | "optional"
            )
        })
    {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    Ok(true)
}

pub(super) fn auto_enabled(table: &Table, key: &str) -> Result<bool, AnalysisFailure> {
    optional_bool(table, key).map(|value| value.unwrap_or(true))
}

pub(super) fn string_set(value: &toml::Value) -> Result<BTreeSet<String>, AnalysisFailure> {
    let values = value
        .as_array()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let result: BTreeSet<_> = values
        .iter()
        .map(expect_string)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(str::to_owned)
        .collect();
    (result.len() == values.len())
        .then_some(result)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

pub(super) fn optional_string_set(
    table: &Table,
    key: &str,
) -> Result<Option<BTreeSet<String>>, AnalysisFailure> {
    table.get(key).map(string_set).transpose()
}

pub(super) fn explicit_target_root(
    directory: &str,
    table: &Table,
    default: &str,
    tree: &TreeImage,
    path_cap: u64,
) -> Result<String, AnalysisFailure> {
    let relative = table
        .get("path")
        .map(expect_string)
        .transpose()?
        .unwrap_or(default);
    let path = normalize_repo_path(directory, relative, path_cap)?;
    (path.ends_with(".rs") && tree.is_regular(&path))
        .then_some(path)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

fn run_git_bounded(args: &[&str], cap: u64) -> Result<Vec<u8>, AnalysisFailure> {
    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| AnalysisFailure::InternalFailure)?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or(AnalysisFailure::InternalFailure)?;
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stdout
            .read(&mut buffer)
            .map_err(|_| AnalysisFailure::InternalFailure)?;
        if read == 0 {
            break;
        }
        let next = (output.len() as u64)
            .checked_add(read as u64)
            .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
        if next > cap {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AnalysisFailure::CapBeforeSafeFallback);
        }
        output.extend_from_slice(&buffer[..read]);
    }
    child
        .wait()
        .map_err(|_| AnalysisFailure::InternalFailure)?
        .success()
        .then_some(output)
        .ok_or(AnalysisFailure::InternalFailure)
}

pub(super) fn git_blob_oid(bytes: &[u8]) -> FullGitOid {
    let header = format!("blob {}\0", bytes.len());
    let mut input = Vec::with_capacity(header.len() + bytes.len());
    input.extend_from_slice(header.as_bytes());
    input.extend_from_slice(bytes);
    FullGitOid(sha1_hex(&input))
}

fn sha1_hex(bytes: &[u8]) -> String {
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let padded_len = (bytes.len() + 9).div_ceil(64) * 64;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(bytes);
    padded.push(0x80);
    padded.resize(padded_len - 8, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    for block in padded.as_chunks::<64>().0 {
        sha1_block(&mut state, block);
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

fn sha1_block(state: &mut [u32; 5], block: &[u8]) {
    let mut words = [0_u32; 80];
    for (index, bytes) in block.as_chunks::<4>().0.iter().enumerate() {
        words[index] = u32::from_be_bytes(bytes.as_slice().try_into().expect("four bytes"));
    }
    for index in 16..80 {
        words[index] =
            (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                .rotate_left(1);
    }
    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (index, word) in words.into_iter().enumerate() {
        let (function, constant) = match index {
            0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
            20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
            _ => (b ^ c ^ d, 0xca62_c1d6),
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(function)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(word);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }
    for (value, addend) in state.iter_mut().zip([a, b, c, d, e]) {
        *value = value.wrapping_add(addend);
    }
}

fn member_pattern_matches(pattern: &str, manifest: &str) -> bool {
    let directory = manifest.strip_suffix("/Cargo.toml").unwrap_or(manifest);
    if let Some(prefix) = pattern.strip_suffix("/*") {
        directory
            .strip_prefix(&format!("{prefix}/"))
            .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
    } else {
        pattern == directory
    }
}

fn string_array(value: &toml::Value) -> Result<Vec<&str>, AnalysisFailure> {
    value
        .as_array()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(AnalysisFailure::RequiredCoverageMissing)
        })
        .collect()
}

fn parse_mode(mode: &str) -> Result<GitMode, AnalysisFailure> {
    match mode {
        "100644" => Ok(GitMode::Regular),
        "100755" => Ok(GitMode::Executable),
        "120000" => Ok(GitMode::Symlink),
        "160000" => Ok(GitMode::Gitlink),
        _ => Err(AnalysisFailure::RequiredCoverageMissing),
    }
}

fn commit_identity(commit: &CommitRevision) -> RevisionIdentity {
    RevisionIdentity::Commit {
        commit: commit.commit.clone(),
        tree: commit.tree.clone(),
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn trim_lf(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}
