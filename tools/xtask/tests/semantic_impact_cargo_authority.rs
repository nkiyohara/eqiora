#[path = "../src/semantic_impact.rs"]
mod semantic_impact;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::OnceLock;

use semantic_impact::{
    AnalysisCaps, AnalysisFailure, AnalysisHead, CargoAuthorityAnalysis, CargoAuthorityRecord,
    CargoAuthorityRequest, CargoContentRecord, CargoDependencyKind, CargoGraphAuthority,
    CargoTargetKind, CaseId, CfgProfile, CommitRevision, Completeness, CoverageAuthority,
    CoverageCertificate, ExactAtom, ExactBytes, ExactOverlay, ExactQueryDomain, ExactRepoPath,
    FullGitOid, GitMode, NonEmptySortedSet, NonEmptySortedVec, OverlayEntry, OverlayStatus,
    RevisionIdentity, RevisionPoint, RevisionSide, Sha256, SortedSet, analyze_cargo_authority,
};
use serde_json::{Map, Value};

const C1_COMMIT: &str = "8e9aec96d5170cb7ab7b7a5f52281e0a0ef09582";
const C1_TREE: &str = "9994b7e1414447d9781f53608032c93f48cb91d6";
const ZERO_OID: &str = "0000000000000000000000000000000000000000";

const P_MANIFEST: &[u8] = b"[package]\nname = \"issue483-provider-shadow\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\nproc-macro = true\n";
const LF: &[u8] = b"\n";
const MAIN: &[u8] = b"fn main() {}\n";
const T_MANIFEST: &[u8] = b"[package]\nname = \"issue483-target-matrix\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\nautolib = false\nbuild = \"build.rs\"\n\n[lib]\n\n[[bin]]\nname = \"matrix-bin\"\n\n[[test]]\nname = \"matrix-test\"\n\n[[example]]\nname = \"matrix-example\"\n\n[[bench]]\nname = \"matrix-bench\"\n";
const T_MANIFEST_NO_LIB: &[u8] = b"[package]\nname = \"issue483-target-matrix\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\nautolib = false\nbuild = \"build.rs\"\n\n[[bin]]\nname = \"matrix-bin\"\n\n[[test]]\nname = \"matrix-test\"\n\n[[example]]\nname = \"matrix-example\"\n\n[[bench]]\nname = \"matrix-bench\"\n";

const RAW_DIFF_BYTES: usize = 0;
const RAW_DIFF_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const TREE_LISTING_RECORDS: usize = 2149;
const TREE_LISTING_BYTES: u64 = 243830;
const TREE_LISTING_SHA256: &str =
    "16f22eb7c5a6efafca7a0a75b15c45d051d94787c27e5bdf7c487bf4a085b618";

const R_BASE_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/r-base-test.jsonl"
);
const R_BASE_TEST_COUNT: usize = 432;
const R_BASE_TEST_BYTES: usize = 291019;
const R_BASE_TEST_SHA256: &str = "5a2a542c353dfd7eabb68cce96bf1e66413742370bbff49ce1c783f965db86b0";
const R_BASE_NONTEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/r-base-nontest.jsonl"
);
const R_BASE_NONTEST_COUNT: usize = 426;
const R_BASE_NONTEST_BYTES: usize = 286344;
const R_BASE_NONTEST_SHA256: &str =
    "2a5b29e110157ae8a02b1be503678f43ad84dbb6a5604037081175fcd275fac7";
const R_HEAD_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/r-head-test.jsonl"
);
const R_HEAD_TEST_COUNT: usize = 432;
const R_HEAD_TEST_BYTES: usize = 291019;
const R_HEAD_TEST_SHA256: &str = "b5e08b1a882c835b03d417c147a3360ca7b82c2681a9b7487cd65d1387bd6ccb";
const R_HEAD_NONTEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/r-head-nontest.jsonl"
);
const R_HEAD_NONTEST_COUNT: usize = 426;
const R_HEAD_NONTEST_BYTES: usize = 286344;
const R_HEAD_NONTEST_SHA256: &str =
    "5052cacfe644d512449ce74c22efb6269be26aebb99a94c46b9c2b8a68e565f4";
const P_HEAD_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/p-head-test.jsonl"
);
const P_HEAD_TEST_COUNT: usize = 435;
const P_HEAD_TEST_BYTES: usize = 334083;
const P_HEAD_TEST_SHA256: &str = "fcbd9fbbe006b61c1e3000925d0314875b40d854d49c052603b07cb060ddf29d";
const P_HEAD_NONTEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/p-head-nontest.jsonl"
);
const P_HEAD_NONTEST_COUNT: usize = 429;
const P_HEAD_NONTEST_BYTES: usize = 328831;
const P_HEAD_NONTEST_SHA256: &str =
    "eadb8ec78c5e88cf169e00fdec2bb3843399ed815de39cc296102a9f580479a0";
const P_BIN_HEAD_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test.jsonl"
);
const P_BIN_HEAD_TEST_COUNT: usize = 436;
const P_BIN_HEAD_TEST_BYTES: usize = 335071;
const P_BIN_HEAD_TEST_SHA256: &str =
    "4a244170e6c04e0475ebd468fb162f05812edfd12eae2670980360dd36125e58";
const T_ALL_HEAD_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/t-all-head-test.jsonl"
);
const T_ALL_HEAD_TEST_COUNT: usize = 440;
const T_ALL_HEAD_TEST_BYTES: usize = 339005;
const T_ALL_HEAD_TEST_SHA256: &str =
    "d18018adce2c1f3fec86e813f3e49ec414825fb4226db08ff872a1df8d877fd8";
const T_AUTO_HEAD_TEST: &str = include_str!(
    "../../../verify/quality/semantic-impact-cargo-authority/fixtures/t-auto-head-test.jsonl"
);
const T_AUTO_HEAD_TEST_COUNT: usize = 439;
const T_AUTO_HEAD_TEST_BYTES: usize = 338031;
const T_AUTO_HEAD_TEST_SHA256: &str =
    "f81c7bbb75bb2236f2211dac7121f83c92fc20a05cf5bca362348a98ab9bb877";

#[derive(Clone, Copy)]
enum OverlayCase {
    P,
    PMissing,
    PBin,
    TAll,
    TAuto,
    TAmbiguous,
}

struct ExpectedStreams {
    r_base_test: SortedSet<CargoAuthorityRecord>,
    r_base_nontest: SortedSet<CargoAuthorityRecord>,
    r_head_test: SortedSet<CargoAuthorityRecord>,
    r_head_nontest: SortedSet<CargoAuthorityRecord>,
    p_head_test: SortedSet<CargoAuthorityRecord>,
    p_head_nontest: SortedSet<CargoAuthorityRecord>,
    p_bin_head_test: SortedSet<CargoAuthorityRecord>,
    t_all_head_test: SortedSet<CargoAuthorityRecord>,
    t_auto_head_test: SortedSet<CargoAuthorityRecord>,
}

static STREAMS: OnceLock<ExpectedStreams> = OnceLock::new();

fn atom(value: &str) -> ExactAtom {
    ExactAtom(value.to_owned())
}

fn path(value: &str) -> ExactRepoPath {
    ExactRepoPath(value.to_owned())
}

fn oid(value: &str) -> FullGitOid {
    FullGitOid(value.to_owned())
}

fn sha(value: &str) -> Sha256 {
    Sha256(value.to_owned())
}

fn set<T: Ord>(values: impl IntoIterator<Item = T>) -> SortedSet<T> {
    values.into_iter().collect::<BTreeSet<_>>()
}

fn nonempty<T: Ord>(values: impl IntoIterator<Item = T>) -> NonEmptySortedSet<T> {
    NonEmptySortedSet(set(values))
}

fn c1() -> CommitRevision {
    CommitRevision {
        commit: oid(C1_COMMIT),
        tree: oid(C1_TREE),
    }
}

fn commit_identity() -> RevisionIdentity {
    RevisionIdentity::Commit {
        commit: oid(C1_COMMIT),
        tree: oid(C1_TREE),
    }
}

fn overlay_identity(digest: &str) -> RevisionIdentity {
    RevisionIdentity::Overlay {
        base_commit: oid(C1_COMMIT),
        base_tree: oid(C1_TREE),
        overlay_sha256: sha(digest),
    }
}

fn profile(test: bool) -> CfgProfile {
    let mut cfg_atoms = vec![
        atom("debug_assertions"),
        atom("target_arch=\"x86_64\""),
        atom("target_endian=\"little\""),
        atom("target_env=\"gnu\""),
        atom("target_family=\"unix\""),
        atom("target_os=\"linux\""),
        atom("target_pointer_width=\"64\""),
        atom("unix"),
    ];
    if test {
        cfg_atoms.push(atom("test"));
    }
    CfgProfile {
        target_triple: atom("x86_64-unknown-linux-gnu"),
        enabled_features: set([]),
        cfg_atoms: set(cfg_atoms),
    }
}

fn profiles(two: bool) -> NonEmptySortedSet<CfgProfile> {
    if two {
        nonempty([profile(true), profile(false)])
    } else {
        nonempty([profile(true)])
    }
}

fn caps() -> AnalysisCaps {
    AnalysisCaps {
        max_commit_refs: 2,
        max_commit_ref_bytes_each: 40,
        max_changed_facts: 3000,
        max_repo_path_bytes: 4096,
        max_changed_path_and_raw_diff_bytes: 8388608,
        max_git_tree_entries_per_side: 200000,
        max_git_tree_listing_bytes_per_side: 33554432,
        max_rust_files_per_side: 2048,
        max_rust_source_blob_bytes: 1048576,
        max_rust_source_bytes_per_side: 67108864,
        max_case_manifests_per_side: 512,
        max_case_manifest_bytes: 262144,
        max_case_manifest_bytes_per_side: 2097152,
        max_cargo_manifests_per_side: 512,
        max_cargo_manifest_bytes: 1048576,
        max_cargo_manifest_bytes_per_side: 8388608,
        max_cargo_lock_bytes_per_side: 8388608,
        max_cargo_authority_bytes_per_side: 67108864,
        max_workspace_packages_per_side: 256,
        max_cargo_targets_per_side: 4096,
        max_atom_bytes: 4096,
        max_derived_atom_bytes: 33554432,
        max_facade_inventory_bytes_per_side: 8388608,
        max_facade_entries_per_side: 4096,
        max_planner_bytes: 16777216,
        max_source_items_per_side: 1000000,
        max_dependency_edges_per_side: 2000000,
        max_canonical_node_bytes: 134217728,
        max_canonical_edge_bytes: 268435456,
        max_reason_chains: 1000000,
        max_hops_per_reason_chain: 64,
        max_retained_reason_hops: 4000000,
        max_canonical_reason_bytes: 268435456,
        max_analyzer_output_bytes: 67108864,
        max_complete_plan_output_bytes: 100663296,
        max_abstract_work: 1073741824,
    }
}

fn added(path_value: &str, blob: &str, bytes: &[u8]) -> OverlayEntry {
    OverlayEntry {
        status: OverlayStatus::Added,
        path: path(path_value),
        base_mode: GitMode::Absent,
        base_blob: oid(ZERO_OID),
        head_mode: GitMode::Regular,
        head_blob: oid(blob),
        head_bytes: Some(ExactBytes(bytes.to_vec())),
    }
}

fn overlay(case: OverlayCase) -> ExactOverlay {
    let entries = match case {
        OverlayCase::P => vec![
            added(
                "crates/issue483-provider-shadow/Cargo.toml",
                "81752de613bfa482355cee87168da3f585bf9410",
                P_MANIFEST,
            ),
            added(
                "crates/issue483-provider-shadow/src/lib.rs",
                "8b137891791fe96927ad78e64b0aad7bded08bdc",
                LF,
            ),
        ],
        OverlayCase::PMissing => vec![added(
            "crates/issue483-provider-shadow/Cargo.toml",
            "81752de613bfa482355cee87168da3f585bf9410",
            P_MANIFEST,
        )],
        OverlayCase::PBin => vec![
            added(
                "crates/issue483-provider-shadow/Cargo.toml",
                "81752de613bfa482355cee87168da3f585bf9410",
                P_MANIFEST,
            ),
            added(
                "crates/issue483-provider-shadow/src/lib.rs",
                "8b137891791fe96927ad78e64b0aad7bded08bdc",
                LF,
            ),
            added(
                "crates/issue483-provider-shadow/src/main.rs",
                "f328e4d9d04c31d0d70d16d21a07d1613be9d577",
                MAIN,
            ),
        ],
        OverlayCase::TAll | OverlayCase::TAmbiguous => {
            let mut values = t_entries(T_MANIFEST, "712f0adcb76c7d8309217334bfcd0edc829f53de");
            if matches!(case, OverlayCase::TAmbiguous) {
                values.push(added(
                    "crates/issue483-target-matrix/tests/matrix-test/main.rs",
                    "8b137891791fe96927ad78e64b0aad7bded08bdc",
                    LF,
                ));
            }
            values
        }
        OverlayCase::TAuto => t_entries(
            T_MANIFEST_NO_LIB,
            "808c77fbc39630f82cdebab16a4f26a9b1a8fc2a",
        ),
    };
    ExactOverlay {
        base: c1(),
        entries: NonEmptySortedVec(entries),
        overlay_sha256: sha(overlay_digest(case)),
    }
}

fn t_entries(manifest: &[u8], manifest_blob: &str) -> Vec<OverlayEntry> {
    vec![
        added(
            "crates/issue483-target-matrix/Cargo.toml",
            manifest_blob,
            manifest,
        ),
        added(
            "crates/issue483-target-matrix/benches/matrix-bench.rs",
            "8b137891791fe96927ad78e64b0aad7bded08bdc",
            LF,
        ),
        added(
            "crates/issue483-target-matrix/build.rs",
            "f328e4d9d04c31d0d70d16d21a07d1613be9d577",
            MAIN,
        ),
        added(
            "crates/issue483-target-matrix/examples/matrix-example.rs",
            "f328e4d9d04c31d0d70d16d21a07d1613be9d577",
            MAIN,
        ),
        added(
            "crates/issue483-target-matrix/src/bin/matrix-bin.rs",
            "f328e4d9d04c31d0d70d16d21a07d1613be9d577",
            MAIN,
        ),
        added(
            "crates/issue483-target-matrix/src/lib.rs",
            "8b137891791fe96927ad78e64b0aad7bded08bdc",
            LF,
        ),
        added(
            "crates/issue483-target-matrix/tests/matrix-test.rs",
            "8b137891791fe96927ad78e64b0aad7bded08bdc",
            LF,
        ),
    ]
}

fn overlay_digest(case: OverlayCase) -> &'static str {
    match case {
        OverlayCase::P => "da4cb2ed18cbad6c09b57a691346a68d666c75a1aafa2f656f36a753f8477abc",
        OverlayCase::PMissing => "ef04cf08ba2a88965866c056a884ecaea709b6e8b70a3618a811c4b2918c78bd",
        OverlayCase::PBin => "85848986b2a7191260c72737b60a862969ab3a4474773d6dbaa9ece67bc5afd4",
        OverlayCase::TAll => "86a46fbe0be4687d0d5bcdee8aae5b10771cc19e3cf8d00dea42492a03e4027d",
        OverlayCase::TAuto => "5e17230b69bd80035c1dfa9527be0d877e5774c86c649ff33fcdd9423d100d1d",
        OverlayCase::TAmbiguous => {
            "4b56c0424016f41a7492629f8f6598fb3b12135af7b433a13f4263965a1ec7c6"
        }
    }
}

fn request_commit(caps: AnalysisCaps) -> CargoAuthorityRequest {
    CargoAuthorityRequest {
        base: c1(),
        head: AnalysisHead::Commit(c1()),
        cfg_profiles: profiles(true),
        caps,
    }
}

fn request_overlay(case: OverlayCase, caps: AnalysisCaps) -> CargoAuthorityRequest {
    CargoAuthorityRequest {
        base: c1(),
        head: AnalysisHead::Overlay(overlay(case)),
        cfg_profiles: profiles(matches!(case, OverlayCase::P)),
        caps,
    }
}

fn request_p(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::P, caps)
}
fn request_p_missing(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::PMissing, caps)
}
fn request_p_bin(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::PBin, caps)
}
fn request_t_all(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::TAll, caps)
}
fn request_t_auto(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::TAuto, caps)
}
fn request_t_ambiguous(caps: AnalysisCaps) -> CargoAuthorityRequest {
    request_overlay(OverlayCase::TAmbiguous, caps)
}

fn object(value: &Value) -> &Map<String, Value> {
    value.as_object().expect("sealed JSON object")
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    object(value).get(name).expect("sealed JSON field")
}

fn text(value: &Value) -> &str {
    value.as_str().expect("sealed JSON string")
}

fn parse_revision(value: &Value) -> RevisionPoint {
    let side = match text(field(value, "side")) {
        "base" => RevisionSide::Base,
        "head" => RevisionSide::Head,
        _ => panic!("unsealed revision side"),
    };
    RevisionPoint {
        side,
        identity: parse_identity(field(value, "identity")),
    }
}

fn parse_identity(value: &Value) -> RevisionIdentity {
    match text(field(value, "kind")) {
        "commit" => RevisionIdentity::Commit {
            commit: oid(text(field(value, "commit"))),
            tree: oid(text(field(value, "tree"))),
        },
        "overlay" => RevisionIdentity::Overlay {
            base_commit: oid(text(field(value, "base_commit"))),
            base_tree: oid(text(field(value, "base_tree"))),
            overlay_sha256: sha(text(field(value, "overlay_sha256"))),
        },
        _ => panic!("unsealed revision identity"),
    }
}

fn parse_profile(value: &Value) -> CfgProfile {
    CfgProfile {
        target_triple: atom(text(field(value, "target_triple"))),
        enabled_features: set(field(value, "enabled_features")
            .as_array()
            .expect("features")
            .iter()
            .map(|value| atom(text(value)))),
        cfg_atoms: set(field(value, "cfg_atoms")
            .as_array()
            .expect("cfg atoms")
            .iter()
            .map(|value| atom(text(value)))),
    }
}

fn parse_mode(value: &Value) -> GitMode {
    match text(value) {
        "000000" => GitMode::Absent,
        "100644" => GitMode::Regular,
        "100755" => GitMode::Executable,
        "120000" => GitMode::Symlink,
        "160000" => GitMode::Gitlink,
        _ => panic!("unsealed Git mode"),
    }
}

fn parse_content(value: &Value) -> CargoContentRecord {
    let entry = field(value, "entry");
    CargoContentRecord {
        revision: parse_revision(field(entry, "revision")),
        path: path(text(field(entry, "path"))),
        mode: parse_mode(field(entry, "mode")),
        blob: oid(text(field(entry, "blob"))),
        byte_len: field(entry, "byte_len").as_u64().expect("byte length"),
        content_sha256: sha(text(field(value, "content_sha256"))),
    }
}

fn parse_target_kind(value: &Value) -> CargoTargetKind {
    match text(value) {
        "library" => CargoTargetKind::Library,
        "binary" => CargoTargetKind::Binary,
        "integration-test" => CargoTargetKind::IntegrationTest,
        "example" => CargoTargetKind::Example,
        "benchmark" => CargoTargetKind::Benchmark,
        "build-script" => CargoTargetKind::BuildScript,
        "proc-macro" => CargoTargetKind::ProcMacro,
        _ => panic!("unsealed target kind"),
    }
}

fn parse_dependency_kind(value: &Value) -> CargoDependencyKind {
    match text(value) {
        "normal" => CargoDependencyKind::Normal,
        "build" => CargoDependencyKind::Build,
        "development" => CargoDependencyKind::Development,
        _ => panic!("unsealed dependency kind"),
    }
}

fn optional_atom(value: &Value) -> Option<ExactAtom> {
    if value.is_null() {
        None
    } else {
        Some(atom(text(value)))
    }
}

fn parse_record(line: &str) -> CargoAuthorityRecord {
    let value: Value = serde_json::from_str(line).expect("accepted sealed JSONL record");
    match text(field(&value, "kind")) {
        "cargo-manifest-input" => {
            CargoAuthorityRecord::Manifest(parse_content(field(&value, "content")))
        }
        "cargo-lock-input" => CargoAuthorityRecord::Lock(parse_content(field(&value, "content"))),
        "cargo-package" => {
            let value = field(&value, "value");
            CargoAuthorityRecord::Package {
                revision: parse_revision(field(value, "revision")),
                package_id: atom(text(field(value, "package_id"))),
                package_name: atom(text(field(value, "package_name"))),
                manifest_path: path(text(field(value, "manifest_path"))),
            }
        }
        "cargo-target" => {
            let value = field(&value, "value");
            let target = field(value, "target");
            CargoAuthorityRecord::Target {
                revision: parse_revision(field(target, "revision")),
                package_id: atom(text(field(target, "package_id"))),
                target_id: atom(text(field(target, "target_id"))),
                cfg_profile: parse_profile(field(target, "cfg_profile")),
                target_kind: parse_target_kind(field(value, "target_kind")),
                crate_root: path(text(field(value, "crate_root"))),
            }
        }
        "cargo-dependency" => {
            let value = field(&value, "value");
            CargoAuthorityRecord::Dependency {
                revision: parse_revision(field(value, "revision")),
                dependent_package_id: atom(text(field(value, "dependent_package_id"))),
                dependency_package_id: atom(text(field(value, "dependency_package_id"))),
                dependency_kind: parse_dependency_kind(field(value, "dependency_kind")),
                rename: optional_atom(field(value, "rename")),
                optional: field(value, "optional").as_bool().expect("optional"),
                active_features: set(field(value, "active_features")
                    .as_array()
                    .expect("active features")
                    .iter()
                    .map(|value| atom(text(value)))),
                cfg_expression: optional_atom(field(value, "cfg_expression")),
                cfg_value: field(value, "cfg_value").as_bool().expect("cfg value"),
            }
        }
        _ => panic!("unsealed Cargo record"),
    }
}

fn parse_stream(
    stream: &str,
    count: usize,
    byte_len: usize,
    digest: &str,
) -> SortedSet<CargoAuthorityRecord> {
    assert_eq!(stream.len(), byte_len, "sealed stream byte length");
    assert_eq!(
        sha256_hex(stream.as_bytes()),
        digest,
        "sealed stream SHA-256"
    );
    let body = stream
        .strip_suffix('\n')
        .expect("LF-terminated sealed stream");
    let lines = body.split('\n').collect::<Vec<_>>();
    assert_eq!(lines.len(), count, "sealed stream record count");
    let records = set(lines.into_iter().map(parse_record));
    assert_eq!(
        records.len(),
        count,
        "sealed stream duplicate-free typed array"
    );
    records
}

fn streams() -> &'static ExpectedStreams {
    STREAMS.get_or_init(|| ExpectedStreams {
        r_base_test: parse_stream(
            R_BASE_TEST,
            R_BASE_TEST_COUNT,
            R_BASE_TEST_BYTES,
            R_BASE_TEST_SHA256,
        ),
        r_base_nontest: parse_stream(
            R_BASE_NONTEST,
            R_BASE_NONTEST_COUNT,
            R_BASE_NONTEST_BYTES,
            R_BASE_NONTEST_SHA256,
        ),
        r_head_test: parse_stream(
            R_HEAD_TEST,
            R_HEAD_TEST_COUNT,
            R_HEAD_TEST_BYTES,
            R_HEAD_TEST_SHA256,
        ),
        r_head_nontest: parse_stream(
            R_HEAD_NONTEST,
            R_HEAD_NONTEST_COUNT,
            R_HEAD_NONTEST_BYTES,
            R_HEAD_NONTEST_SHA256,
        ),
        p_head_test: parse_stream(
            P_HEAD_TEST,
            P_HEAD_TEST_COUNT,
            P_HEAD_TEST_BYTES,
            P_HEAD_TEST_SHA256,
        ),
        p_head_nontest: parse_stream(
            P_HEAD_NONTEST,
            P_HEAD_NONTEST_COUNT,
            P_HEAD_NONTEST_BYTES,
            P_HEAD_NONTEST_SHA256,
        ),
        p_bin_head_test: parse_stream(
            P_BIN_HEAD_TEST,
            P_BIN_HEAD_TEST_COUNT,
            P_BIN_HEAD_TEST_BYTES,
            P_BIN_HEAD_TEST_SHA256,
        ),
        t_all_head_test: parse_stream(
            T_ALL_HEAD_TEST,
            T_ALL_HEAD_TEST_COUNT,
            T_ALL_HEAD_TEST_BYTES,
            T_ALL_HEAD_TEST_SHA256,
        ),
        t_auto_head_test: parse_stream(
            T_AUTO_HEAD_TEST,
            T_AUTO_HEAD_TEST_COUNT,
            T_AUTO_HEAD_TEST_BYTES,
            T_AUTO_HEAD_TEST_SHA256,
        ),
    })
}

fn graph(
    revision: RevisionPoint,
    cfg_profile: CfgProfile,
    records: SortedSet<CargoAuthorityRecord>,
    count: u64,
    digest: &str,
) -> CargoGraphAuthority {
    CargoGraphAuthority {
        revision: revision.clone(),
        cfg_profile: cfg_profile.clone(),
        records,
        certificate: CoverageCertificate {
            domain: ExactQueryDomain::CargoGraph {
                revision,
                cfg_profile,
            },
            authority: CoverageAuthority::CargoManifestGraph,
            canonical_input_sha256: sha(digest),
            examined_items: count,
            complete: true,
        },
    }
}

fn point(side: RevisionSide, identity: RevisionIdentity) -> RevisionPoint {
    RevisionPoint { side, identity }
}

fn analysis(
    head: RevisionIdentity,
    graphs: impl IntoIterator<Item = CargoGraphAuthority>,
) -> CargoAuthorityAnalysis {
    CargoAuthorityAnalysis {
        base: commit_identity(),
        head,
        graphs: nonempty(graphs),
        completeness: Completeness::Complete,
        precise_cases: set::<CaseId>([]),
        unknowns: set::<ExactRepoPath>([]),
    }
}

fn expected_commit() -> CargoAuthorityAnalysis {
    let values = streams();
    analysis(
        commit_identity(),
        [
            graph(
                point(RevisionSide::Base, commit_identity()),
                profile(true),
                values.r_base_test.clone(),
                432,
                R_BASE_TEST_SHA256,
            ),
            graph(
                point(RevisionSide::Base, commit_identity()),
                profile(false),
                values.r_base_nontest.clone(),
                426,
                R_BASE_NONTEST_SHA256,
            ),
            graph(
                point(RevisionSide::Head, commit_identity()),
                profile(true),
                values.r_head_test.clone(),
                432,
                R_HEAD_TEST_SHA256,
            ),
            graph(
                point(RevisionSide::Head, commit_identity()),
                profile(false),
                values.r_head_nontest.clone(),
                426,
                R_HEAD_NONTEST_SHA256,
            ),
        ],
    )
}

fn expected_p() -> CargoAuthorityAnalysis {
    let values = streams();
    let identity = overlay_identity(overlay_digest(OverlayCase::P));
    analysis(
        identity.clone(),
        [
            graph(
                point(RevisionSide::Base, commit_identity()),
                profile(true),
                values.r_base_test.clone(),
                432,
                R_BASE_TEST_SHA256,
            ),
            graph(
                point(RevisionSide::Base, commit_identity()),
                profile(false),
                values.r_base_nontest.clone(),
                426,
                R_BASE_NONTEST_SHA256,
            ),
            graph(
                point(RevisionSide::Head, identity.clone()),
                profile(true),
                values.p_head_test.clone(),
                435,
                P_HEAD_TEST_SHA256,
            ),
            graph(
                point(RevisionSide::Head, identity),
                profile(false),
                values.p_head_nontest.clone(),
                429,
                P_HEAD_NONTEST_SHA256,
            ),
        ],
    )
}

fn expected_single(
    case: OverlayCase,
    records: SortedSet<CargoAuthorityRecord>,
    count: u64,
    digest: &str,
) -> CargoAuthorityAnalysis {
    let identity = overlay_identity(overlay_digest(case));
    analysis(
        identity.clone(),
        [
            graph(
                point(RevisionSide::Base, commit_identity()),
                profile(true),
                streams().r_base_test.clone(),
                432,
                R_BASE_TEST_SHA256,
            ),
            graph(
                point(RevisionSide::Head, identity),
                profile(true),
                records,
                count,
                digest,
            ),
        ],
    )
}

fn expected_p_bin() -> CargoAuthorityAnalysis {
    expected_single(
        OverlayCase::PBin,
        streams().p_bin_head_test.clone(),
        436,
        P_BIN_HEAD_TEST_SHA256,
    )
}
fn expected_t_all() -> CargoAuthorityAnalysis {
    expected_single(
        OverlayCase::TAll,
        streams().t_all_head_test.clone(),
        440,
        T_ALL_HEAD_TEST_SHA256,
    )
}
fn expected_t_auto() -> CargoAuthorityAnalysis {
    expected_single(
        OverlayCase::TAuto,
        streams().t_auto_head_test.clone(),
        439,
        T_AUTO_HEAD_TEST_SHA256,
    )
}

fn assert_success(
    request: CargoAuthorityRequest,
    expected: &CargoAuthorityAnalysis,
    label: &str,
) -> CargoAuthorityAnalysis {
    match analyze_cargo_authority(request) {
        Ok(actual) => {
            assert!(&actual == expected, "whole typed result mismatch: {label}");
            actual
        }
        Err(_) => panic!("ordinary positive rejected: {label}"),
    }
}

fn assert_failure(request: CargoAuthorityRequest, expected: AnalysisFailure, label: &str) {
    match analyze_cargo_authority(request) {
        Err(actual) => assert!(actual == expected, "wrong typed terminal: {label}"),
        Ok(_) => panic!("expected typed terminal but succeeded: {label}"),
    }
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = input.to_vec();
    let bit_len = (data.len() as u64)
        .checked_mul(8)
        .expect("SHA-256 input length");
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, bytes) in chunk.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes(bytes.try_into().expect("word"));
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(value);
        }
    }
    let mut output = [0u8; 32];
    for (slot, value) in output.chunks_exact_mut(4).zip(h) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
    output
}

fn sha256_hex(input: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in sha256(input) {
        write!(&mut output, "{byte:02x}").expect("hex");
    }
    output
}

fn mode_text(mode: &GitMode) -> &'static [u8] {
    match mode {
        GitMode::Absent => b"000000",
        GitMode::Regular => b"100644",
        GitMode::Executable => b"100755",
        GitMode::Symlink => b"120000",
        GitMode::Gitlink => b"160000",
    }
}

fn carrier(entries: &NonEmptySortedVec<OverlayEntry>) -> Vec<u8> {
    let mut output = Vec::new();
    for entry in &entries.0 {
        output.push(match entry.status {
            OverlayStatus::Added => b'A',
            _ => panic!("this S1 oracle admits Added carriers only"),
        });
        output.extend_from_slice(&(entry.path.0.len() as u32).to_be_bytes());
        output.extend_from_slice(entry.path.0.as_bytes());
        output.extend_from_slice(mode_text(&entry.base_mode));
        output.extend_from_slice(entry.base_blob.0.as_bytes());
        output.extend_from_slice(mode_text(&entry.head_mode));
        output.extend_from_slice(entry.head_blob.0.as_bytes());
        let bytes = entry
            .head_bytes
            .as_ref()
            .map_or(&[][..], |value| value.0.as_slice());
        output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        output.extend_from_slice(bytes);
    }
    output
}

fn assert_literal_and_carrier_seals() {
    let literals = [
        (
            P_MANIFEST,
            120,
            "79a5e2abade035cf70b80aad7d638b15a0f65f0539ad9829bcc58c6c110a5181",
        ),
        (
            LF,
            1,
            "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b",
        ),
        (
            MAIN,
            13,
            "536e506bb90914c243a12b397b9a998f85ae2cbd9ba02dfd03a9e155ca5ca0f4",
        ),
        (
            T_MANIFEST,
            265,
            "556b7ed0a880193f3d7637795e0f05bfebb25c435baed1b0929c8d1f41e9248b",
        ),
        (
            T_MANIFEST_NO_LIB,
            258,
            "4f6f3dab8487845ad290bcbd281264ec3a7cab324a7e7632296af6c849e6f9f3",
        ),
    ];
    for (bytes, length, digest) in literals {
        assert_eq!(bytes.len(), length, "literal byte length");
        assert_eq!(sha256_hex(bytes), digest, "literal SHA-256");
    }
    let cases = [
        (OverlayCase::P, 415),
        (OverlayCase::PMissing, 267),
        (OverlayCase::PBin, 576),
        (OverlayCase::TAll, 1370),
        (OverlayCase::TAuto, 1363),
        (OverlayCase::TAmbiguous, 1531),
    ];
    for (case, length) in cases {
        let overlay = overlay(case);
        let bytes = carrier(&overlay.entries);
        assert_eq!(bytes.len(), length, "Added carrier byte length");
        assert_eq!(
            sha256_hex(&bytes),
            overlay_digest(case),
            "Added carrier SHA-256"
        );
        assert_eq!(
            overlay.overlay_sha256.0,
            overlay_digest(case),
            "overlay identity binding"
        );
    }
    assert_eq!(RAW_DIFF_BYTES, 0);
    assert_eq!(
        sha256_hex(&[]),
        RAW_DIFF_SHA256,
        "empty commit raw-diff seal"
    );
    assert_eq!(TREE_LISTING_RECORDS, 2149);
    assert_eq!(TREE_LISTING_BYTES, 243830);
    assert_eq!(
        TREE_LISTING_SHA256,
        "16f22eb7c5a6efafca7a0a75b15c45d051d94787c27e5bdf7c487bf4a085b618"
    );
}

type RequestBuilder = fn(AnalysisCaps) -> CargoAuthorityRequest;
type CapSetter = fn(&mut AnalysisCaps, u64);

fn check_cap(
    builder: RequestBuilder,
    expected: &CargoAuthorityAnalysis,
    n: u64,
    setter: CapSetter,
    label: &str,
) {
    let mut at_n = caps();
    setter(&mut at_n, n);
    assert_success(builder(at_n), expected, label);
    let mut below = caps();
    setter(&mut below, n - 1);
    assert_failure(
        builder(below),
        AnalysisFailure::CapBeforeSafeFallback,
        label,
    );
    let mut zero = caps();
    setter(&mut zero, 0);
    assert_failure(builder(zero), AnalysisFailure::CapBeforeSafeFallback, label);
}

fn check_failure_after_carrier(
    builder: RequestBuilder,
    later: AnalysisFailure,
    n: u64,
    label: &str,
) {
    let mut at_n = caps();
    at_n.max_changed_path_and_raw_diff_bytes = n;
    assert_failure(builder(at_n), later, label);
    let mut below = caps();
    below.max_changed_path_and_raw_diff_bytes = n - 1;
    assert_failure(
        builder(below),
        AnalysisFailure::CapBeforeSafeFallback,
        label,
    );
    let mut zero = caps();
    zero.max_changed_path_and_raw_diff_bytes = 0;
    assert_failure(builder(zero), AnalysisFailure::CapBeforeSafeFallback, label);
}

fn set_commit_refs(c: &mut AnalysisCaps, n: u64) {
    c.max_commit_refs = n;
}
fn set_commit_ref_bytes(c: &mut AnalysisCaps, n: u64) {
    c.max_commit_ref_bytes_each = n;
}
fn set_changed_facts(c: &mut AnalysisCaps, n: u64) {
    c.max_changed_facts = n;
}
fn set_path_bytes(c: &mut AnalysisCaps, n: u64) {
    c.max_repo_path_bytes = n;
}
fn set_tree_entries(c: &mut AnalysisCaps, n: u64) {
    c.max_git_tree_entries_per_side = n;
}
fn set_tree_listing(c: &mut AnalysisCaps, n: u64) {
    c.max_git_tree_listing_bytes_per_side = n;
}
fn set_manifest_count(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_manifests_per_side = n;
}
fn set_manifest_single(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_manifest_bytes = n;
}
fn set_manifest_total(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_manifest_bytes_per_side = n;
}
fn set_lock_bytes(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_lock_bytes_per_side = n;
}
fn set_authority_bytes(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_authority_bytes_per_side = n;
}
fn set_packages(c: &mut AnalysisCaps, n: u64) {
    c.max_workspace_packages_per_side = n;
}
fn set_targets(c: &mut AnalysisCaps, n: u64) {
    c.max_cargo_targets_per_side = n;
}
fn set_atom_bytes(c: &mut AnalysisCaps, n: u64) {
    c.max_atom_bytes = n;
}
fn set_derived_atoms(c: &mut AnalysisCaps, n: u64) {
    c.max_derived_atom_bytes = n;
}
fn set_declarations(c: &mut AnalysisCaps, n: u64) {
    c.max_dependency_edges_per_side = n;
}

fn check_carrier_caps(
    p: &CargoAuthorityAnalysis,
    p_bin: &CargoAuthorityAnalysis,
    t_all: &CargoAuthorityAnalysis,
    t_auto: &CargoAuthorityAnalysis,
) {
    check_cap(
        request_p,
        p,
        415,
        |c, n| c.max_changed_path_and_raw_diff_bytes = n,
        "P carrier cap",
    );
    check_failure_after_carrier(
        request_p_missing,
        AnalysisFailure::RequiredCoverageMissing,
        267,
        "P missing carrier cap",
    );
    check_cap(
        request_p_bin,
        p_bin,
        576,
        |c, n| c.max_changed_path_and_raw_diff_bytes = n,
        "P bin carrier cap",
    );
    check_cap(
        request_t_all,
        t_all,
        1370,
        |c, n| c.max_changed_path_and_raw_diff_bytes = n,
        "T all carrier cap",
    );
    check_cap(
        request_t_auto,
        t_auto,
        1363,
        |c, n| c.max_changed_path_and_raw_diff_bytes = n,
        "T autolib carrier cap",
    );
    check_failure_after_carrier(
        request_t_ambiguous,
        AnalysisFailure::AuthorityConflict,
        1531,
        "T ambiguous carrier cap",
    );
}

fn atom_products(analysis: &CargoAuthorityAnalysis) -> (u64, u64) {
    let mut maximum = 0u64;
    let mut side_sums = [0u64, 0u64];
    for graph in &analysis.graphs.0 {
        let side = if matches!(&graph.revision.side, RevisionSide::Base) {
            0
        } else {
            1
        };
        for record in &graph.records {
            let mut values = Vec::<&str>::new();
            match record {
                CargoAuthorityRecord::Manifest(_) | CargoAuthorityRecord::Lock(_) => {}
                CargoAuthorityRecord::Package {
                    package_id,
                    package_name,
                    ..
                } => values.extend([package_id.0.as_str(), package_name.0.as_str()]),
                CargoAuthorityRecord::Target {
                    package_id,
                    target_id,
                    cfg_profile,
                    ..
                } => {
                    values.extend([
                        package_id.0.as_str(),
                        target_id.0.as_str(),
                        cfg_profile.target_triple.0.as_str(),
                    ]);
                    values.extend(
                        cfg_profile
                            .enabled_features
                            .iter()
                            .map(|value| value.0.as_str()),
                    );
                    values.extend(cfg_profile.cfg_atoms.iter().map(|value| value.0.as_str()));
                }
                CargoAuthorityRecord::Dependency {
                    dependent_package_id,
                    dependency_package_id,
                    rename,
                    active_features,
                    cfg_expression,
                    ..
                } => {
                    values.extend([
                        dependent_package_id.0.as_str(),
                        dependency_package_id.0.as_str(),
                    ]);
                    if let Some(value) = rename {
                        values.push(value.0.as_str());
                    }
                    values.extend(active_features.iter().map(|value| value.0.as_str()));
                    if let Some(value) = cfg_expression {
                        values.push(value.0.as_str());
                    }
                }
            }
            for value in values {
                let length = value.len() as u64;
                maximum = maximum.max(length);
                side_sums[side] = side_sums[side]
                    .checked_add(length)
                    .expect("derived atom sum");
            }
        }
    }
    (maximum, side_sums[0].max(side_sums[1]))
}

fn check_all_other_caps(t_all: &CargoAuthorityAnalysis) {
    let (max_atom, derived_atoms) = atom_products(t_all);
    assert_eq!(max_atom, 250, "sealed maximum ExactAtom bytes");
    assert_eq!(derived_atoms, 122198, "sealed TEST graph ExactAtom bytes");
    let controls: [(u64, CapSetter, &str); 15] = [
        (2, set_commit_refs, "commit ref count"),
        (40, set_commit_ref_bytes, "commit ref bytes"),
        (7, set_changed_facts, "changed facts"),
        (120, set_path_bytes, "repository path bytes"),
        (2156, set_tree_entries, "logical tree entries"),
        (243830, set_tree_listing, "raw tree listing bytes"),
        (40, set_manifest_count, "Cargo manifest count"),
        (8354, set_manifest_single, "single Cargo manifest bytes"),
        (30880, set_manifest_total, "Cargo manifest bytes per side"),
        (86711, set_lock_bytes, "Cargo lock bytes"),
        (39, set_packages, "workspace packages"),
        (207, set_targets, "Cargo targets"),
        (max_atom, set_atom_bytes, "single atom bytes"),
        (derived_atoms, set_derived_atoms, "derived atom bytes"),
        (239, set_declarations, "normalized dependency declarations"),
    ];
    for (n, setter, label) in controls {
        check_cap(request_t_all, t_all, n, setter, label);
    }
}

fn check_field_18(
    commit: &CargoAuthorityAnalysis,
    p: &CargoAuthorityAnalysis,
    p_bin: &CargoAuthorityAnalysis,
    t_all: &CargoAuthorityAnalysis,
    t_auto: &CargoAuthorityAnalysis,
) {
    check_cap(
        request_commit,
        commit,
        577363,
        set_authority_bytes,
        "R field 18",
    );
    check_cap(
        request_p,
        p,
        662914,
        set_authority_bytes,
        "P field 18 persistent sum",
    );
    let mut largest = caps();
    largest.max_cargo_authority_bytes_per_side = 334083;
    assert_failure(
        request_p(largest),
        AnalysisFailure::CapBeforeSafeFallback,
        "P field 18 rejects largest individual stream",
    );
    check_cap(
        request_p_bin,
        p_bin,
        335071,
        set_authority_bytes,
        "P bin field 18",
    );
    check_cap(
        request_t_all,
        t_all,
        339005,
        set_authority_bytes,
        "T all field 18",
    );
    check_cap(
        request_t_auto,
        t_auto,
        338031,
        set_authority_bytes,
        "T autolib field 18",
    );
}

const P_PACKAGE_ID: &str = "cargo-package-v1:24:issue483-provider-shadow:5:0.0.0:42:crates/issue483-provider-shadow/Cargo.toml";
const P_PROC_TARGET_ID: &str = "cargo-target-v1:98:cargo-package-v1:24:issue483-provider-shadow:5:0.0.0:42:crates/issue483-provider-shadow/Cargo.toml:10:proc-macro:24:issue483_provider_shadow:42:crates/issue483-provider-shadow/src/lib.rs";
const P_LIBRARY_TARGET_ID: &str = "cargo-target-v1:98:cargo-package-v1:24:issue483-provider-shadow:5:0.0.0:42:crates/issue483-provider-shadow/Cargo.toml:7:library:24:issue483_provider_shadow:42:crates/issue483-provider-shadow/src/lib.rs";
const P_DASHED_PROC_TARGET_ID: &str = "cargo-target-v1:98:cargo-package-v1:24:issue483-provider-shadow:5:0.0.0:42:crates/issue483-provider-shadow/Cargo.toml:10:proc-macro:24:issue483-provider-shadow:42:crates/issue483-provider-shadow/src/lib.rs";
const P_ROOT: &str = "crates/issue483-provider-shadow/src/lib.rs";

const T_PACKAGE_ID: &str = "cargo-package-v1:22:issue483-target-matrix:5:0.0.0:40:crates/issue483-target-matrix/Cargo.toml";
const T_BINARY_TARGET_ID: &str = "cargo-target-v1:94:cargo-package-v1:22:issue483-target-matrix:5:0.0.0:40:crates/issue483-target-matrix/Cargo.toml:6:binary:10:matrix-bin:51:crates/issue483-target-matrix/src/bin/matrix-bin.rs";
const T_LIBRARY_TARGET_ID: &str = "cargo-target-v1:94:cargo-package-v1:22:issue483-target-matrix:5:0.0.0:40:crates/issue483-target-matrix/Cargo.toml:7:library:22:issue483_target_matrix:40:crates/issue483-target-matrix/src/lib.rs";
const T_BUILD_TARGET_ID: &str = "cargo-target-v1:94:cargo-package-v1:22:issue483-target-matrix:5:0.0.0:40:crates/issue483-target-matrix/Cargo.toml:12:build-script:18:build-script-build:38:crates/issue483-target-matrix/build.rs";
const T_TEST_TARGET_ID: &str = "cargo-target-v1:94:cargo-package-v1:22:issue483-target-matrix:5:0.0.0:40:crates/issue483-target-matrix/Cargo.toml:16:integration-test:11:matrix-test:50:crates/issue483-target-matrix/tests/matrix-test.rs";
const T_BINARY_ROOT: &str = "crates/issue483-target-matrix/src/bin/matrix-bin.rs";
const T_LIBRARY_ROOT: &str = "crates/issue483-target-matrix/src/lib.rs";
const T_BUILD_ROOT: &str = "crates/issue483-target-matrix/build.rs";
const T_TEST_ROOT: &str = "crates/issue483-target-matrix/tests/matrix-test.rs";

fn exact_target(
    case: OverlayCase,
    package_id: &str,
    target_id: &str,
    target_kind: CargoTargetKind,
    crate_root: &str,
) -> CargoAuthorityRecord {
    CargoAuthorityRecord::Target {
        revision: point(RevisionSide::Head, overlay_identity(overlay_digest(case))),
        package_id: atom(package_id),
        target_id: atom(target_id),
        cfg_profile: profile(true),
        target_kind,
        crate_root: path(crate_root),
    }
}

fn p_proc_macro_target() -> CargoAuthorityRecord {
    exact_target(
        OverlayCase::P,
        P_PACKAGE_ID,
        P_PROC_TARGET_ID,
        CargoTargetKind::ProcMacro,
        P_ROOT,
    )
}

fn p_library_sibling_target() -> CargoAuthorityRecord {
    exact_target(
        OverlayCase::P,
        P_PACKAGE_ID,
        P_LIBRARY_TARGET_ID,
        CargoTargetKind::Library,
        P_ROOT,
    )
}

fn t_binary_target(case: OverlayCase) -> CargoAuthorityRecord {
    exact_target(
        case,
        T_PACKAGE_ID,
        T_BINARY_TARGET_ID,
        CargoTargetKind::Binary,
        T_BINARY_ROOT,
    )
}

fn t_library_target(case: OverlayCase) -> CargoAuthorityRecord {
    exact_target(
        case,
        T_PACKAGE_ID,
        T_LIBRARY_TARGET_ID,
        CargoTargetKind::Library,
        T_LIBRARY_ROOT,
    )
}

fn t_build_target() -> CargoAuthorityRecord {
    exact_target(
        OverlayCase::TAll,
        T_PACKAGE_ID,
        T_BUILD_TARGET_ID,
        CargoTargetKind::BuildScript,
        T_BUILD_ROOT,
    )
}

fn t_test_target() -> CargoAuthorityRecord {
    exact_target(
        OverlayCase::TAll,
        T_PACKAGE_ID,
        T_TEST_TARGET_ID,
        CargoTargetKind::IntegrationTest,
        T_TEST_ROOT,
    )
}

fn head_test_graph_mutant<F>(
    case: OverlayCase,
    mut analysis: CargoAuthorityAnalysis,
    mut mutate: F,
) -> CargoAuthorityAnalysis
where
    F: FnMut(&mut SortedSet<CargoAuthorityRecord>),
{
    let revision = point(RevisionSide::Head, overlay_identity(overlay_digest(case)));
    let cfg_profile = profile(true);
    let graph = analysis
        .graphs
        .0
        .iter()
        .find(|graph| graph.revision == revision && graph.cfg_profile == cfg_profile)
        .cloned()
        .expect("exact Head TEST graph");
    assert!(analysis.graphs.0.remove(&graph));
    let mut changed = graph;
    mutate(&mut changed.records);
    assert!(analysis.graphs.0.insert(changed));
    analysis
}

fn take_record<F>(
    records: &mut SortedSet<CargoAuthorityRecord>,
    mut predicate: F,
) -> CargoAuthorityRecord
where
    F: FnMut(&CargoAuthorityRecord) -> bool,
{
    let record = records
        .iter()
        .find(|record| predicate(record))
        .cloned()
        .expect("mutant source record");
    assert!(records.remove(&record));
    record
}

fn take_exact_target(
    records: &mut SortedSet<CargoAuthorityRecord>,
    expected: &CargoAuthorityRecord,
) -> CargoAuthorityRecord {
    assert!(
        records.remove(expected),
        "exact added package/target/kind/root record must exist"
    );
    expected.clone()
}

fn reject_mutant(
    actual: &CargoAuthorityAnalysis,
    correct: &CargoAuthorityAnalysis,
    wrong: CargoAuthorityAnalysis,
    name: &str,
) {
    assert!(
        actual == correct,
        "mutant control did not reach its exact expected whole result: {name}"
    );
    assert!(
        &wrong != correct,
        "named mutant collapsed to the exact expected whole result: {name}"
    );
    assert!(
        actual != &wrong,
        "exact typed whole-result equality admitted named mutant: {name}"
    );
}

fn target_mutants(
    actual_p: &CargoAuthorityAnalysis,
    p: &CargoAuthorityAnalysis,
    actual_t_all: &CargoAuthorityAnalysis,
    t_all: &CargoAuthorityAnalysis,
    actual_t_auto: &CargoAuthorityAnalysis,
    t_auto: &CargoAuthorityAnalysis,
) {
    assert_failure(
        request_p_missing(caps()),
        AnalysisFailure::RequiredCoverageMissing,
        "proc-macro missing-root causal mutant",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            assert!(records.contains(&p_proc_macro_target()));
            assert!(records.insert(p_library_sibling_target()));
        }),
        "accidental normal-library sibling",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            take_exact_target(records, &p_proc_macro_target());
            assert!(records.insert(exact_target(
                OverlayCase::P,
                P_PACKAGE_ID,
                P_DASHED_PROC_TARGET_ID,
                CargoTargetKind::ProcMacro,
                P_ROOT,
            )));
        }),
        "dash normalization error",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            let CargoAuthorityRecord::Target {
                revision,
                package_id,
                target_id,
                cfg_profile,
                target_kind,
                ..
            } = p_proc_macro_target()
            else {
                unreachable!()
            };
            assert!(records.contains(&p_proc_macro_target()));
            assert!(records.insert(CargoAuthorityRecord::Target {
                revision,
                package_id,
                target_id,
                cfg_profile,
                target_kind,
                crate_root: path("crates/issue483-provider-shadow/src/lib/main.rs"),
            }));
        }),
        "second default root",
    );
    let mut exact_target_count_cap = caps();
    exact_target_count_cap.max_cargo_targets_per_side = 207;
    let merged = assert_success(
        request_t_all(exact_target_count_cap),
        t_all,
        "matrix-bin implicit/explicit merge at exact target cap",
    );
    assert!(actual_t_all == &merged);
    let matrix_binary = t_binary_target(OverlayCase::TAll);
    let exact_head = t_all
        .graphs
        .0
        .iter()
        .find(|graph| {
            graph.revision
                == point(
                    RevisionSide::Head,
                    overlay_identity(overlay_digest(OverlayCase::TAll)),
                )
                && graph.cfg_profile == profile(true)
        })
        .expect("T_ALL Head TEST graph");
    assert!(exact_head.records.contains(&matrix_binary));
    reject_mutant(
        actual_t_all,
        t_all,
        head_test_graph_mutant(OverlayCase::TAll, t_all.clone(), |records| {
            take_exact_target(records, &t_library_target(OverlayCase::TAll));
        }),
        "six-to-five target error",
    );
    reject_mutant(
        actual_t_all,
        t_all,
        head_test_graph_mutant(OverlayCase::TAll, t_all.clone(), |records| {
            take_exact_target(records, &t_build_target());
        }),
        "omitted build script",
    );
    reject_mutant(
        actual_t_all,
        t_all,
        head_test_graph_mutant(OverlayCase::TAll, t_all.clone(), |records| {
            let CargoAuthorityRecord::Target {
                revision,
                package_id,
                target_id,
                cfg_profile,
                target_kind,
                ..
            } = t_test_target()
            else {
                unreachable!()
            };
            assert!(records.contains(&t_test_target()));
            assert!(records.insert(CargoAuthorityRecord::Target {
                revision,
                package_id,
                target_id,
                cfg_profile,
                target_kind,
                crate_root: path("crates/issue483-target-matrix/tests/matrix-test/main.rs"),
            }));
        }),
        "ambiguous integration root",
    );
    assert_failure(
        request_t_ambiguous(caps()),
        AnalysisFailure::AuthorityConflict,
        "ambiguous integration-root causal mutant",
    );
    reject_mutant(
        actual_t_auto,
        t_auto,
        head_test_graph_mutant(OverlayCase::TAuto, t_auto.clone(), |records| {
            assert!(records.contains(&t_binary_target(OverlayCase::TAuto)));
            assert!(!records.contains(&t_library_target(OverlayCase::TAuto)));
            assert!(records.insert(t_library_target(OverlayCase::TAuto)));
        }),
        "unused autolib-off library treatment",
    );
}

fn is_added_manifest(record: &CargoAuthorityRecord) -> bool {
    matches!(record, CargoAuthorityRecord::Manifest(content) if content.path.0 == "crates/issue483-provider-shadow/Cargo.toml")
}

fn manifest_join_mutants(actual_p: &CargoAuthorityAnalysis, p: &CargoAuthorityAnalysis) {
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            let CargoAuthorityRecord::Manifest(mut content) =
                take_record(records, is_added_manifest)
            else {
                unreachable!()
            };
            content.path = path("suffix-only/issue483-provider-shadow/Cargo.toml");
            records.insert(CargoAuthorityRecord::Manifest(content));
        }),
        "suffix or package-name manifest proxy",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            let CargoAuthorityRecord::Manifest(mut content) =
                take_record(records, is_added_manifest)
            else {
                unreachable!()
            };
            content.content_sha256 =
                sha("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
            records.insert(CargoAuthorityRecord::Manifest(content));
        }),
        "manifest content identity mismatch",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            take_record(records, is_added_manifest);
        }),
        "zero matching manifest authority",
    );
    reject_mutant(
        actual_p,
        p,
        head_test_graph_mutant(OverlayCase::P, p.clone(), |records| {
            let CargoAuthorityRecord::Manifest(mut second) = records
                .iter()
                .find(|record| is_added_manifest(record))
                .cloned()
                .expect("manifest")
            else {
                unreachable!()
            };
            second.content_sha256 =
                sha("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
            records.insert(CargoAuthorityRecord::Manifest(second));
        }),
        "multiple matching manifest authority",
    );
    let mut unknown = p.clone();
    unknown
        .unknowns
        .insert(path("crates/issue483-provider-shadow/Cargo.toml"));
    reject_mutant(
        actual_p,
        p,
        unknown,
        "generic Unknown in place of typed manifest join",
    );
}

#[test]
fn repository_derived_cargo_authority_is_exact_and_bounded() {
    let expected_commit = expected_commit();
    let expected_p = expected_p();
    let expected_p_bin = expected_p_bin();
    let expected_t_all = expected_t_all();
    let expected_t_auto = expected_t_auto();

    let _actual_commit = assert_success(request_commit(caps()), &expected_commit, "R_COMMIT");
    let actual_p = assert_success(request_p(caps()), &expected_p, "P_POS");
    let _actual_p_bin = assert_success(request_p_bin(caps()), &expected_p_bin, "P_BIN_POS");
    let actual_t_all = assert_success(request_t_all(caps()), &expected_t_all, "T_ALL_POS");
    let actual_t_auto = assert_success(
        request_t_auto(caps()),
        &expected_t_auto,
        "T_AUTOLIB_OFF_POS",
    );

    assert_failure(
        request_p_missing(caps()),
        AnalysisFailure::RequiredCoverageMissing,
        "P_MISSING_ROOT",
    );
    assert_failure(
        request_t_ambiguous(caps()),
        AnalysisFailure::AuthorityConflict,
        "T_AMBIGUOUS_ROOT",
    );

    assert_literal_and_carrier_seals();
    let mut zero_raw = caps();
    zero_raw.max_changed_path_and_raw_diff_bytes = 0;
    assert_success(
        request_commit(zero_raw),
        &expected_commit,
        "zero-byte R_COMMIT raw diff",
    );
    check_carrier_caps(
        &expected_p,
        &expected_p_bin,
        &expected_t_all,
        &expected_t_auto,
    );
    check_all_other_caps(&expected_t_all);
    check_field_18(
        &expected_commit,
        &expected_p,
        &expected_p_bin,
        &expected_t_all,
        &expected_t_auto,
    );

    target_mutants(
        &actual_p,
        &expected_p,
        &actual_t_all,
        &expected_t_all,
        &actual_t_auto,
        &expected_t_auto,
    );
    manifest_join_mutants(&actual_p, &expected_p);
}
