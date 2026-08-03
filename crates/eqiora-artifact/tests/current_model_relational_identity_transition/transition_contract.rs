//! The frozen two-state transition contract of RFC 0083 and the repository
//! sweep that feeds it.
//!
//! This is the private support module of the integration test
//! `current_model_relational_identity_transition`, included with `#[path]` so
//! the case keeps exactly one Cargo integration-test target. The split is by
//! responsibility: the test root owns every embedded literal and the identities
//! derived from it, and this half owns everything that reads the working tree —
//! the candidate sweep, the retired/preserved/required sets, the promotion
//! table, the post-reset forbidden-token scopes, and the state predicate over
//! them. Neither half is under the 2,000-line test ceiling by accident; the
//! ceiling is what forced the split, and no ledger entry was added for it.
//!
//! The split has one consequence the sweep itself has to carry: this case's
//! executable oracle is now two files rather than one, and both are excluded
//! from the candidate sweep by exact path. See `ORACLE_FILES`.

use crate::{CLASSIFICATION, frozen, frozen_inventory, raw_sha256};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn has_lower_hex_identity(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if !(lower.contains("model") || lower.contains("transaction")) {
        return false;
    }
    line.as_bytes().windows(64).any(|window| {
        window
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    })
}

/// The exact spellings the sweep searches for, beside the same-line lower-hex-64
/// identity rule. A recorded admission signal must be one of these.
const SEARCH_TOKENS: [&str; 11] = [
    "eqiora.model-envelope/v",
    "eqiora.model-transaction-envelope/v",
    "model_sha256",
    "modelSha256",
    "model_digest",
    "modelDigest",
    "ModelEnvelopeV",
    "ModelTransactionEnvelopeV",
    "ExactModelCodec",
    "compile_exact",
    "exact_codec",
];

fn carries_model_search_signal(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    SEARCH_TOKENS.iter().any(|token| text.contains(token))
        || text.lines().any(has_lower_hex_identity)
}

/// What one admitted later path's bytes spell: the search tokens they contain,
/// in `SEARCH_TOKENS` order, and how many of their lines freeze a Model-derived
/// lower-hex-64 identity. The live tree and every synthetic state read it here.
fn observe_admitted(bytes: &[u8]) -> (Vec<String>, usize) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return (Vec::new(), 0);
    };
    let signals = (SEARCH_TOKENS.iter())
        .filter(|token| text.contains(**token))
        .map(|token| (*token).to_owned())
        .collect();
    let literals = text.lines().filter(|line| has_lower_hex_identity(line));
    (signals, literals.count())
}

/// Trees the sweep does not enter, by exact relative path.
///
/// The second is maturin's packaging staging directory: building the Python
/// extension copies checked-in example resources into it, so a tree that has
/// run the gate carries an untracked build copy of an already classified Model
/// resource there. Excluding one exact generated directory keeps the sweep over
/// checked-in content; it grants no path permission to appear or disappear.
const EXCLUDED_TREES: [&str; 2] = [
    "verify/artifacts/current-model-relational-identity-transition",
    "bindings/python/python/eqiora/examples",
];

/// This case's own executable oracle, by exact path.
///
/// Both files spell the tokens the sweep searches for, so a sweep that read
/// them would report the oracle as an unclassified candidate of itself. The
/// exclusion is these two exact paths and nothing else: not this module's
/// directory, not a suffix rule, and emphatically not "every test". A third
/// executor file beside them is returned to this oracle, not given the
/// exclusion.
const ORACLE_FILES: [&str; 2] = [
    "crates/eqiora-artifact/tests/current_model_relational_identity_transition.rs",
    "crates/eqiora-artifact/tests/current_model_relational_identity_transition/\
     transition_contract.rs",
];

/// One walk of the working tree, answering both questions it can answer: which
/// paths carry the Model search signal, and what the scopes contain. Two
/// walkers would be two chances to disagree.
struct Sweep<'a> {
    scopes: &'a [ForbiddenScope],
    found: BTreeSet<String>,
    content: BTreeMap<String, String>,
}

fn scan_tree(root: &Path, directory: &Path, sweep: &mut Sweep<'_>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot scan {}: {error}", directory.display()))
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if entry.file_type().unwrap().is_dir() {
            if [".git", "target", "node_modules", "dist", "__pycache__"]
                .iter()
                .any(|name| entry.file_name() == *name)
                || EXCLUDED_TREES.contains(&relative.as_str())
            {
                continue;
            }
            scan_tree(root, &path, sweep);
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        if !ORACLE_FILES.contains(&relative.as_str()) && carries_model_search_signal(&bytes) {
            sweep.found.insert(relative.clone());
        }
        if sweep.scopes.iter().any(|scope| scope.covers(&relative))
            && let Ok(text) = String::from_utf8(bytes)
        {
            sweep.content.insert(relative, text);
        }
    }
}

/// Any digest that is not a promoted one. Its only role in a synthetic state is
/// to stand for "this live path has not been overwritten by its staged source".
const SYNTHETIC_UNPROMOTED_DIGEST: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// One staged file and the exact live path it is promoted to. The frozen digest
/// is what stops promotion from substituting meaning: the live target must
/// carry the staged bytes, not an equivalent rewrite of them.
struct Promotion {
    source: String,
    target: String,
    bytes: usize,
    sha256: String,
    target_exists_pre_reset: bool,
}

/// Evidence whose bytes survive the reset at a different path.
///
/// It is preserved evidence in neither state, which is why it is separated
/// from `preserved_evidence`: listing it there would make the pre state demand
/// a path the reset removes, or the post state demand one it never creates.
struct PromotedEvidence {
    pre_reset: String,
    post_reset: String,
}

/// One later product path admitted by exact path, and the exact signal it is
/// admitted as carrying. Admission is a permission, never an obligation: the
/// path may be absent, and a post-reset state carrying none of these is
/// accepted exactly as it was before this class existed. A path that does exist
/// must spell exactly `signals`, freeze exactly `identity_literals` — zero —
/// Model identities, and not exist before the reset at all.
struct PostResetAdmitted {
    path: String,
    signals: Vec<String>,
    identity_literals: usize,
}

/// One narrowly frozen product-source scope and the tokens the reset deletes
/// from it. Post-reset only: `pre_reset_occurrence` says which of them the
/// pre-reset tree carries.
struct ForbiddenScope {
    name: String,
    paths: Vec<String>,
    exclude_names: Vec<String>,
    exclude_paths: BTreeSet<String>,
    forbidden: Vec<String>,
}

/// `*` matches any run of bytes within one path segment. Nothing else.
fn matches_name(pattern: &str, name: &str) -> bool {
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == name;
    }
    let Some(mut rest) = name.strip_prefix(parts[0]) else {
        return false;
    };
    for middle in &parts[1..parts.len() - 1] {
        let Some(at) = rest.find(middle) else {
            return false;
        };
        rest = &rest[at + middle.len()..];
    }
    let tail = parts[parts.len() - 1];
    rest.len() >= tail.len() && rest.ends_with(tail)
}

/// The frozen scope dialect: an exact path, or a literal prefix followed by
/// `**/` for any depth and a final-segment name pattern. No other construct is
/// accepted, so a scope cannot be quietly widened by a cleverer glob.
fn matches_glob(pattern: &str, path: &str) -> bool {
    match pattern.split_once("**/") {
        Some((prefix, name)) => path.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty() && matches_name(name, rest.rsplit_once('/').map_or(rest, |(_, n)| n))
        }),
        None => match (pattern.rsplit_once('/'), path.rsplit_once('/')) {
            (Some((pattern_dir, name)), Some((path_dir, candidate))) => {
                pattern_dir == path_dir && matches_name(name, candidate)
            }
            (None, None) => matches_name(pattern, path),
            _ => false,
        },
    }
}

impl ForbiddenScope {
    fn covers(&self, path: &str) -> bool {
        let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
        !self.exclude_paths.contains(path)
            && !self
                .exclude_names
                .iter()
                .any(|pattern| matches_name(pattern, name))
            && self.paths.iter().any(|pattern| matches_glob(pattern, path))
    }

    /// Which of this scope's forbidden tokens the observed content spells, and
    /// which it does not, in the scope's own declaration order.
    fn occurrence(&self, content: &BTreeMap<String, String>) -> (Vec<String>, Vec<String>) {
        let covered = content
            .iter()
            .filter(|(path, _)| self.covers(path))
            .map(|(_, source)| source)
            .collect::<Vec<_>>();
        self.forbidden
            .iter()
            .cloned()
            .partition(|token| covered.iter().any(|source| source.contains(token.as_str())))
    }
}

/// The post-reset product-source contract. Path existence cannot see a private
/// historical branch left inside a file the reset preserves; this can.
///
/// Exact substring matching, deliberately: recognizing a `#[cfg(test)]` module
/// or a comment would need a permissive textual parser, and a parser that
/// guesses what a scope contains is the thing this contract exists to replace.
fn scan_forbidden_tokens(
    scopes: &[ForbiddenScope],
    content: &BTreeMap<String, String>,
) -> Result<(), String> {
    for scope in scopes {
        for (path, source) in content {
            if !scope.covers(path) {
                continue;
            }
            if let Some(token) = scope
                .forbidden
                .iter()
                .find(|token| source.contains(token.as_str()))
            {
                return Err(format!(
                    "post-reset: forbidden product token `{token}` survives in `{path}` under \
                     scope `{}`; the reset deletes the branch, not only the file that hosted it",
                    scope.name
                ));
            }
        }
    }
    Ok(())
}

/// The frozen two-state transition contract of the RFC 0083 epoch reset.
///
/// `retired` is the complete set of paths the reset removes; every other frozen
/// inventory path is preserved, and an in-place migration may stop one matching
/// the search signal without removing it. `required_post_reset` is the complete
/// set of paths the reset may add: eleven byte-frozen promotions — ten staged
/// control-v2 sources and the historical cylinder — plus two unversioned Rust
/// wire owners required by existence alone. `post_reset_admitted` is neither of
/// those: it permits a later product path without ever requiring one.
///
/// There is no third state and no sentinel. A repository in which a proper
/// nonempty subset of `retired` is missing is mid-flight, not reset.
struct TransitionContract {
    inventory: BTreeSet<String>,
    retired: BTreeSet<String>,
    retired_outside_inventory: BTreeSet<String>,
    required_post_reset: BTreeSet<String>,
    existence_only_post_reset: BTreeSet<String>,
    preserved_evidence: BTreeSet<String>,
    promoted_evidence: Vec<PromotedEvidence>,
    promotion: Vec<Promotion>,
    post_reset_admitted: Vec<PostResetAdmitted>,
    forbidden: Vec<ForbiddenScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionState {
    PreReset,
    PostReset,
}

/// One observed repository state, real or synthetic: which frozen paths exist,
/// which paths carry the Model search signal, and the digest of each path the
/// promotion table names.
#[derive(Clone)]
struct Observed {
    exists: BTreeSet<String>,
    discovered: BTreeSet<String>,
    digests: BTreeMap<String, String>,
    content: BTreeMap<String, String>,
    /// What `observe_admitted` read from each admitted later path that exists.
    /// Absent is how "not there" is spelled, and that is an accepted state.
    admitted: BTreeMap<String, (Vec<String>, usize)>,
}

fn classification() -> Value {
    serde_json::from_slice(frozen(CLASSIFICATION)).unwrap()
}

fn frozen_paths(transition: &Value, key: &str) -> BTreeSet<String> {
    transition[key]
        .as_array()
        .unwrap_or_else(|| panic!("the frozen transition contract must name `{key}`"))
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect()
}

fn frozen_list(scope: &Value, key: &str) -> Vec<String> {
    scope[key]
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

impl TransitionContract {
    fn from_classification() -> Self {
        let classification = classification();
        let search = &classification["search"];
        let transition = &search["transition"];
        Self {
            inventory: frozen_inventory(),
            retired: frozen_paths(transition, "retired"),
            retired_outside_inventory: frozen_paths(transition, "retired_outside_inventory"),
            required_post_reset: frozen_paths(transition, "required_post_reset"),
            existence_only_post_reset: frozen_paths(
                transition,
                "required_post_reset_without_frozen_bytes",
            ),
            preserved_evidence: frozen_paths(transition, "preserved_evidence"),
            promoted_evidence: transition["promoted_evidence"]
                .as_array()
                .expect("the transition contract must name its promoted evidence")
                .iter()
                .map(|entry| PromotedEvidence {
                    pre_reset: entry["pre_reset"].as_str().unwrap().to_owned(),
                    post_reset: entry["post_reset"].as_str().unwrap().to_owned(),
                })
                .collect(),
            promotion: transition["promotion"]
                .as_array()
                .expect("the transition contract must name its promotion table")
                .iter()
                .map(|entry| Promotion {
                    source: entry["source"].as_str().unwrap().to_owned(),
                    target: entry["target"].as_str().unwrap().to_owned(),
                    bytes: entry["bytes"].as_u64().unwrap() as usize,
                    sha256: entry["sha256"].as_str().unwrap().to_owned(),
                    target_exists_pre_reset: entry["target_exists_pre_reset"].as_bool().unwrap(),
                })
                .collect(),
            post_reset_admitted: transition["post_reset_admitted"]
                .as_array()
                .expect("the transition contract must name what it admits after the reset")
                .iter()
                .map(|entry| PostResetAdmitted {
                    path: entry["path"].as_str().unwrap().to_owned(),
                    signals: frozen_list(entry, "signals"),
                    identity_literals: entry["identity_literals"].as_u64().unwrap() as usize,
                })
                .collect(),
            forbidden: search["forbidden_product_tokens"]["scopes"]
                .as_array()
                .expect("the forbidden-token contract must declare its scopes")
                .iter()
                .map(|scope| ForbiddenScope {
                    name: scope["name"].as_str().unwrap().to_owned(),
                    paths: frozen_list(scope, "paths"),
                    exclude_names: frozen_list(scope, "exclude_names"),
                    exclude_paths: frozen_list(scope, "exclude_paths").into_iter().collect(),
                    forbidden: frozen_list(scope, "forbidden"),
                })
                .collect(),
        }
    }

    /// The frozen inventory minus everything the reset retires.
    fn preserved(&self) -> BTreeSet<String> {
        self.inventory.difference(&self.retired).cloned().collect()
    }

    /// Every path this contract makes a statement about, in either state.
    fn mentioned(&self) -> BTreeSet<String> {
        let mut paths = self.inventory.clone();
        paths.extend(self.retired.iter().cloned());
        paths.extend(self.required_post_reset.iter().cloned());
        paths.extend(self.preserved_evidence.iter().cloned());
        for promotion in &self.promotion {
            paths.insert(promotion.source.clone());
            paths.insert(promotion.target.clone());
        }
        // Mentioned but never required: naming them here is what lets both
        // states see whether one is present.
        paths.extend(self.post_reset_admitted.iter().map(|e| e.path.clone()));
        paths
    }

    fn check_pre_reset(&self, observed: &Observed) -> Result<(), String> {
        for path in &self.inventory {
            if !observed.exists.contains(path) {
                return Err(format!(
                    "pre-reset: frozen inventory path `{path}` is missing"
                ));
            }
        }
        if observed.discovered != self.inventory {
            let unexpected = observed
                .discovered
                .difference(&self.inventory)
                .collect::<Vec<_>>();
            let unmatched = self
                .inventory
                .difference(&observed.discovered)
                .collect::<Vec<_>>();
            return Err(format!(
                "pre-reset: discovered candidates must equal the frozen inventory exactly; \
                 unclassified {unexpected:?}, no longer matching {unmatched:?}"
            ));
        }
        for promotion in &self.promotion {
            let observed_source = observed.digests.get(&promotion.source).ok_or_else(|| {
                format!(
                    "pre-reset: no digest observed for staged source `{}`",
                    promotion.source
                )
            })?;
            if observed_source != &promotion.sha256 {
                return Err(format!(
                    "pre-reset: staged source `{}` must carry its frozen {}-byte digest {}, \
                     observed {observed_source}",
                    promotion.source, promotion.bytes, promotion.sha256
                ));
            }
            let target_exists = observed.exists.contains(&promotion.target);
            if target_exists != promotion.target_exists_pre_reset {
                return Err(format!(
                    "pre-reset: promotion target `{}` must{} exist before the reset",
                    promotion.target,
                    if promotion.target_exists_pre_reset {
                        ""
                    } else {
                        " not"
                    }
                ));
            }
            if target_exists {
                let observed_target = observed.digests.get(&promotion.target).ok_or_else(|| {
                    format!(
                        "pre-reset: no digest observed for live target `{}`",
                        promotion.target
                    )
                })?;
                if observed_target == &promotion.sha256 {
                    return Err(format!(
                        "pre-reset: live target `{}` already carries its promoted bytes, so the \
                         repository is neither wholly before nor wholly after the reset",
                        promotion.target
                    ));
                }
            }
        }
        for path in &self.existence_only_post_reset {
            if observed.exists.contains(path) {
                return Err(format!(
                    "pre-reset: post-reset owner `{path}` already exists; the unversioned wire \
                     owners are created by the reset, not before it"
                ));
            }
        }
        for admitted in &self.post_reset_admitted {
            if observed.exists.contains(&admitted.path) {
                return Err(format!(
                    "pre-reset: post-reset-admitted path `{}` already exists; admission covers a \
                     product path created after the reset, and a pre-reset tree that carries one \
                     is mid-flight",
                    admitted.path
                ));
            }
        }
        Ok(())
    }

    fn check_post_reset(&self, observed: &Observed) -> Result<(), String> {
        // Named evidence first, so its removal is reported as what it is.
        for path in &self.preserved_evidence {
            if !observed.exists.contains(path) {
                return Err(format!(
                    "post-reset: preserved evidence `{path}` was removed; historical negative \
                     specimens and retained separate-family goldens survive the reset"
                ));
            }
        }
        for evidence in &self.promoted_evidence {
            if !observed.exists.contains(&evidence.post_reset) {
                return Err(format!(
                    "post-reset: promoted evidence `{}` never arrived at `{}`; its bytes move \
                     to their new owner, they are not deleted with the path that hosted them",
                    evidence.pre_reset, evidence.post_reset
                ));
            }
        }
        for path in &self.preserved() {
            if !observed.exists.contains(path) {
                return Err(format!(
                    "post-reset: preserved inventory path `{path}` was removed; only a frozen \
                     retired path may disappear"
                ));
            }
        }
        for path in &self.required_post_reset {
            if !observed.exists.contains(path) {
                return Err(format!("post-reset: required target `{path}` is missing"));
            }
        }
        for promotion in &self.promotion {
            let observed_target = observed.digests.get(&promotion.target).ok_or_else(|| {
                format!(
                    "post-reset: no digest observed for promoted target `{}`",
                    promotion.target
                )
            })?;
            if observed_target != &promotion.sha256 {
                return Err(format!(
                    "post-reset: promoted target `{}` must carry the staged {}-byte source \
                     digest {}, observed {observed_target}; promotion copies bytes and never \
                     substitutes meaning",
                    promotion.target, promotion.bytes, promotion.sha256
                ));
            }
        }
        // Containment-only: an absent admitted path is skipped, so no later
        // capability is required for acceptance. A present one must still be
        // what it was admitted as.
        for admitted in &self.post_reset_admitted {
            if !observed.exists.contains(&admitted.path) {
                continue;
            }
            let (signals, literals) = observed.admitted.get(&admitted.path).ok_or_else(|| {
                format!(
                    "post-reset: admitted path `{}` exists but no content was observed for it",
                    admitted.path
                )
            })?;
            if signals != &admitted.signals {
                return Err(format!(
                    "post-reset: admitted path `{}` must carry exactly its recorded search signal \
                     {:?}, observed {signals:?}; a path that spells something else returns here",
                    admitted.path, admitted.signals
                ));
            }
            if literals != &admitted.identity_literals {
                return Err(format!(
                    "post-reset: admitted path `{}` freezes {literals} Model-derived identity \
                     literal against the recorded {}; a path that pins an identity is a fixture",
                    admitted.path, admitted.identity_literals
                ));
            }
        }
        let mut admissible = self.preserved();
        admissible.extend(self.required_post_reset.iter().cloned());
        admissible.extend(self.post_reset_admitted.iter().map(|e| e.path.clone()));
        let unclassified = observed
            .discovered
            .difference(&admissible)
            .collect::<Vec<_>>();
        if !unclassified.is_empty() {
            return Err(format!(
                "post-reset: unclassified new signal-bearing paths {unclassified:?}; a new \
                 Model-bearing path is returned to this oracle, never admitted here"
            ));
        }
        scan_forbidden_tokens(&self.forbidden, &observed.content)
    }
}

/// The complete transition predicate. Nothing else decides which state a
/// repository is in, and no state is inferred from one witness path.
fn classify_transition(
    contract: &TransitionContract,
    observed: &Observed,
) -> Result<TransitionState, String> {
    if let Some(path) = observed.discovered.difference(&observed.exists).next() {
        return Err(format!(
            "incoherent observation: `{path}` carries the search signal but does not exist"
        ));
    }
    let surviving = contract
        .retired
        .intersection(&observed.exists)
        .collect::<Vec<_>>();
    if surviving.len() == contract.retired.len() {
        contract
            .check_pre_reset(observed)
            .map(|()| TransitionState::PreReset)
    } else if surviving.is_empty() {
        contract
            .check_post_reset(observed)
            .map(|()| TransitionState::PostReset)
    } else {
        Err(format!(
            "partial transition: {} of {} retired paths survive, including `{}`; the frozen \
             retired set lands wholly or not at all",
            surviving.len(),
            contract.retired.len(),
            surviving[0]
        ))
    }
}

/// A synthetic product source written the way the reset leaves it.
///
/// It deliberately spells every token `deliberately_permitted` names, so a
/// contract that over-forbids fails here rather than in the implementation
/// lane. The last two entries sit outside every scope: the negative corpus and
/// the historical record must keep naming what the reset removed.
fn clean_post_reset_product_source() -> BTreeMap<String, String> {
    [
        (
            "crates/eqiora-artifact/src/model_wire.rs",
            "pub const MODEL_SCHEMA: &str = \"eqiora.model-envelope/v8\";\n\
             impl ModelEnvelope {\n    fn encode(node: &KernelNode) -> Wire { todo!() }\n\
             \x20   fn ensure(&self) -> Result<(), Diagnostic> { Ok(()) }\n}\n",
        ),
        (
            "crates/eqiora-artifact/src/model_transaction_wire.rs",
            "pub const TRANSACTION_SCHEMA: &str = \"eqiora.model-transaction-envelope/v8\";\n",
        ),
        (
            "crates/eqiora-api/src/control/compile.rs",
            "const PROTOCOL: &str = \"eqiora.control/v2\";\n\
             const RETIRED: &str = \"eqiora.control/v1\";\n\
             const COMMAND: &str = \"model.compile-check/v1\";\n",
        ),
        (
            "bindings/python/python/eqiora/__init__.pyi",
            "def compile(source: str) -> Model: ...\ndef replay(model: Model) -> Run: ...\n",
        ),
        (
            "studio/src/control-protocol.ts",
            "export const PROTOCOL = 'eqiora.control/v2';\n",
        ),
        (
            "verify/interfaces/control-plane-compile-check/models/forbidden-model-wire-v2.json",
            "{\"modelWire\":{\"schema\":\"eqiora.model-envelope/v7\"},\"requiredFeatures\":[]}\n",
        ),
        (
            "crates/eqiora-artifact/tests/model_wire_negative_corpus.rs",
            "// ModelEnvelopeV7 specimens must be rejected by the current decoder.\n",
        ),
    ]
    .into_iter()
    .map(|(path, source)| (path.to_owned(), source.to_owned()))
    .collect()
}

/// Synthetic bytes for each admitted later path, written the way an ordinary
/// current-owner consumer is: naming the `model_digest` its caller already
/// holds, freezing nothing. Mutation material, not a specification of those
/// files — it is what makes the predicates below non-vacuous on a checkout
/// where none of the paths exists, since every state reaches `observe_admitted`
/// through real bytes exactly as the live tree does.
const ADMITTED_AS_RECORDED: [(&str, &str); 3] = [
    (
        "crates/eqiora-python/src/trajectory.rs",
        "pub fn trajectory(model_digest: &str) -> PyResult<Trajectory> {\n    \
         Trajectory::open(model_digest)\n}\n",
    ),
    (
        "bindings/python/python/eqiora/trajectory.pyi",
        "class Trajectory:\n    model_digest: str\n",
    ),
    (
        "crates/eqiora-python/src/result.rs",
        "pub fn result(model_digest: &str) -> PyResult<Result> {\n    Result::open(model_digest)\n}\n",
    ),
];

impl Observed {
    /// The working tree, looked up path by path with one sweep that answers
    /// both the candidate question and the product-source one.
    fn from_repository(contract: &TransitionContract, root: &Path) -> Self {
        let mut sweep = Sweep {
            scopes: &contract.forbidden,
            found: BTreeSet::new(),
            content: BTreeMap::new(),
        };
        scan_tree(root, root, &mut sweep);
        let mut exists = sweep.found.clone();
        exists.extend(
            contract
                .mentioned()
                .into_iter()
                .filter(|path| root.join(path).exists()),
        );
        let mut digests = BTreeMap::new();
        for promotion in &contract.promotion {
            for path in [&promotion.source, &promotion.target] {
                if let Ok(bytes) = fs::read(root.join(path)) {
                    digests.insert(path.clone(), raw_sha256(&bytes));
                }
            }
        }
        let mut admitted = BTreeMap::new();
        for entry in &contract.post_reset_admitted {
            if let Ok(bytes) = fs::read(root.join(&entry.path)) {
                admitted.insert(entry.path.clone(), observe_admitted(&bytes));
            }
        }
        Self {
            exists,
            discovered: sweep.found,
            digests,
            content: sweep.content,
            admitted,
        }
    }

    /// The exact state this contract was frozen against. Its content map is
    /// empty on purpose: the forbidden-token contract is post-reset only, and
    /// what the pre-reset checkout spells is frozen under
    /// `pre_reset_occurrence` instead.
    fn exact_pre_reset(contract: &TransitionContract) -> Self {
        let mut exists = contract.inventory.clone();
        exists.extend(contract.retired.iter().cloned());
        let mut digests = BTreeMap::new();
        for promotion in &contract.promotion {
            digests.insert(promotion.source.clone(), promotion.sha256.clone());
            if promotion.target_exists_pre_reset {
                exists.insert(promotion.target.clone());
                digests.insert(
                    promotion.target.clone(),
                    SYNTHETIC_UNPROMOTED_DIGEST.to_owned(),
                );
            }
        }
        Self {
            discovered: contract.inventory.clone(),
            exists,
            digests,
            content: BTreeMap::new(),
            admitted: BTreeMap::new(),
        }
    }

    /// One complete post-reset state a structurally correct reset may produce:
    /// the maximal one, in which every preserved path still matches the sweep.
    ///
    /// It is not *the* observed post-reset signal set, and no test may read it
    /// as one. A preserved path migrated in place may stop carrying a Model
    /// signal, which is admissible and which `no_longer_matching` exercises;
    /// the post-reset predicate bounds which paths exist and contains — never
    /// equals — the discovered set.
    fn maximal_post_reset(contract: &TransitionContract) -> Self {
        let mut exists = contract.preserved();
        exists.extend(contract.required_post_reset.iter().cloned());
        let digests = contract
            .promotion
            .iter()
            .map(|promotion| (promotion.target.clone(), promotion.sha256.clone()))
            .collect();
        let discovered = exists.clone();
        Self {
            exists,
            discovered,
            digests,
            content: clean_post_reset_product_source(),
            admitted: BTreeMap::new(),
        }
    }

    /// One admitted later path present with the given bytes: whether it is
    /// discovered, what it spells, and what it freezes all come from content.
    fn admitting(mut self, path: &str, source: &str) -> Self {
        self.exists.insert(path.to_owned());
        if carries_model_search_signal(source.as_bytes()) {
            self.discovered.insert(path.to_owned());
        }
        self.admitted
            .insert(path.to_owned(), observe_admitted(source.as_bytes()));
        self
    }

    fn without(mut self, paths: &[&str]) -> Self {
        for path in paths {
            assert!(
                self.exists.remove(*path),
                "the synthetic state must actually contain `{path}` before removing it"
            );
            self.discovered.remove(*path);
            self.digests.remove(*path);
        }
        self
    }

    fn with(mut self, paths: &[&str]) -> Self {
        for path in paths {
            assert!(
                self.exists.insert((*path).to_owned()),
                "the synthetic state must not already contain `{path}`"
            );
        }
        self
    }

    fn signalling(mut self, paths: &[&str]) -> Self {
        for path in paths {
            self.exists.insert((*path).to_owned());
            self.discovered.insert((*path).to_owned());
        }
        self
    }

    fn no_longer_matching(mut self, paths: &[&str]) -> Self {
        for path in paths {
            assert!(
                self.discovered.remove(*path),
                "the synthetic state must actually discover `{path}` before it stops matching"
            );
        }
        self
    }

    fn with_digest(mut self, path: &str, digest: &str) -> Self {
        self.digests.insert(path.to_owned(), digest.to_owned());
        self
    }

    fn with_source(mut self, path: &str, source: &str) -> Self {
        self.content.insert(path.to_owned(), source.to_owned());
        self
    }
}

fn refused(result: Result<TransitionState, String>, because: &str) {
    let reason = result.expect_err("this state must be refused");
    assert!(
        reason.contains(because),
        "the refusal must name `{because}`, got: {reason}"
    );
}

#[test]
fn the_sweep_excludes_exactly_this_cases_two_executor_files() {
    let root = repository_root();
    let inventory = frozen_inventory();
    let declared = classification();
    let search = &declared["search"];

    // Declared where the classification records the sweep, so a third excluded
    // file would have to be written down before it could hide.
    assert_eq!(
        frozen_list(search, "excluded_paths"),
        ORACLE_FILES.map(str::to_owned).to_vec(),
        "the declared self-exclusion must be exactly this case's two executor files"
    );
    assert_eq!(
        frozen_list(search, "excluded_trees"),
        EXCLUDED_TREES.map(str::to_owned).to_vec()
    );

    for path in ORACLE_FILES {
        let bytes = fs::read(root.join(path))
            .unwrap_or_else(|error| panic!("excluded executor `{path}` must exist: {error}"));
        // Load-bearing: each half spells the tokens the sweep searches for, so
        // without this exclusion the oracle would report itself.
        assert!(
            carries_model_search_signal(&bytes),
            "`{path}` must carry the search signal, or excluding it claims something it need not"
        );
        assert!(
            !inventory.contains(path),
            "`{path}` is excluded from the sweep and must not also be a classified candidate"
        );
    }

    // Exact, and only exact: every other test file in the crate — including the
    // sibling Model oracle — stays a classified inventory member.
    for path in [
        "crates/eqiora-artifact/tests/current_model_wire_oracle.rs",
        "crates/eqiora-artifact/tests/model_v8_wire.rs",
        "crates/eqiora-artifact/tests/realization_v4_wire.rs",
    ] {
        assert!(!ORACLE_FILES.contains(&path));
        assert!(
            inventory.contains(path),
            "`{path}` is an ordinary test file and must remain a classified candidate"
        );
    }
    assert!(
        EXCLUDED_TREES
            .iter()
            .all(|tree| !tree.contains("/tests") && !tree.ends_with("tests")),
        "no excluded tree may stand in for a rule about test directories"
    );
}

#[test]
fn the_frozen_transition_contract_partitions_the_repository() {
    let contract = TransitionContract::from_classification();
    let classification = classification();
    let frozen_transition = &classification["search"]["transition"];
    let count = |key: &str| frozen_transition[key].as_u64().unwrap() as usize;

    // Counts are frozen beside the sets, so a silently shortened list fails.
    assert_eq!(contract.retired.len(), count("retired_path_count"));
    assert_eq!(contract.preserved().len(), count("preserved_path_count"));
    assert_eq!(
        contract.required_post_reset.len(),
        count("required_post_reset_path_count")
    );

    // Two states, one partition — and the partition is of the inventory, not of
    // those three counts. 34 retired inventory paths and 304 preserved ones
    // cover the 338 candidates exactly; the other 10 retired paths carry no
    // search signal and were never inventory members, and the 13 required paths
    // are post-reset additions and one in-place replacement.
    let retired_inventory = contract.retired.intersection(&contract.inventory).count();
    assert_eq!(
        retired_inventory + contract.preserved().len(),
        contract.inventory.len()
    );
    assert_eq!(
        retired_inventory,
        count("retired_inside_the_inventory_count")
    );
    assert_eq!(
        retired_inventory + contract.retired_outside_inventory.len(),
        count("retired_path_count")
    );
    assert_eq!(
        contract
            .retired
            .difference(&contract.inventory)
            .cloned()
            .collect::<BTreeSet<_>>(),
        contract.retired_outside_inventory,
        "a retired path outside the inventory carries no search signal and must be listed as such"
    );
    assert_eq!(
        contract
            .required_post_reset
            .intersection(&contract.inventory)
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "verify/interfaces/control-plane-compile-check/expected/contract.json".to_owned()
        ]),
        "exactly one required path already exists as an inventory member; the reset replaces its \
         bytes in place and adds the other twelve"
    );
    assert!(
        contract.preserved_evidence.is_subset(&contract.preserved()),
        "invariant preserved evidence exists at the same path in both states, so no member \
         may also be retired"
    );

    // Evidence whose location changes is separated from the invariant kind, so
    // neither state has to be the union of two phases.
    for evidence in &contract.promoted_evidence {
        assert!(
            contract.retired.contains(&evidence.pre_reset),
            "promoted evidence {} must retire at its old path",
            evidence.pre_reset
        );
        assert!(
            contract.required_post_reset.contains(&evidence.post_reset),
            "promoted evidence {} must be required at its new path",
            evidence.post_reset
        );
        assert!(
            !contract.preserved_evidence.contains(&evidence.pre_reset)
                && !contract.preserved_evidence.contains(&evidence.post_reset),
            "promoted evidence is invariant at neither path: {}",
            evidence.pre_reset
        );
        assert!(
            contract.promotion.iter().any(|promotion| {
                promotion.source == evidence.pre_reset && promotion.target == evidence.post_reset
            }),
            "promoted evidence {} must carry frozen bytes across, not merely a path pair",
            evidence.pre_reset
        );
    }

    // Promotion is an injective map from retired staging sources onto the byte-
    // frozen half of what the reset may add; the rest is required by existence.
    let targets = contract
        .promotion
        .iter()
        .map(|promotion| promotion.target.clone())
        .collect::<BTreeSet<_>>();
    assert!(
        targets.is_disjoint(&contract.existence_only_post_reset),
        "a target either carries frozen bytes or is required by existence, never both"
    );
    assert_eq!(
        targets
            .union(&contract.existence_only_post_reset)
            .cloned()
            .collect::<BTreeSet<_>>(),
        contract.required_post_reset
    );
    assert_eq!(targets.len(), contract.promotion.len());
    assert_eq!(
        targets.len(),
        11,
        "eleven byte-frozen promotions: ten staged control-v2 sources and the historical cylinder"
    );
    assert_eq!(contract.existence_only_post_reset.len(), 2);

    // The unversioned wire owners are created by the reset and their content is
    // owned by the full-byte oracle, so existence is all this case may require.
    for path in &contract.existence_only_post_reset {
        assert!(
            !contract.inventory.contains(path) && !contract.retired.contains(path),
            "{path} does not exist before the reset, so it is neither classified nor retired"
        );
    }
    assert_eq!(
        contract.existence_only_post_reset,
        BTreeSet::from([
            "crates/eqiora-artifact/src/model_transaction_wire.rs".to_owned(),
            "crates/eqiora-artifact/src/model_wire.rs".to_owned(),
        ]),
        "the reset folds the retired v8 wrappers and the surviving current encoding into exactly \
         these two unversioned owners"
    );
    for retired in [
        "crates/eqiora-artifact/src/model_v8.rs",
        "crates/eqiora-artifact/src/model_transaction_v8.rs",
    ] {
        assert!(
            contract.retired.contains(retired),
            "{retired} is a version-named source owner and retires with the epoch"
        );
    }
    for promotion in &contract.promotion {
        assert!(
            contract.retired.contains(&promotion.source),
            "staged source {} must retire once its live target exists",
            promotion.source
        );
        assert!(!contract.retired.contains(&promotion.target));
        assert_eq!(
            promotion.target_exists_pre_reset,
            contract.inventory.contains(&promotion.target),
            "{} is a new path exactly when it is not already classified",
            promotion.target
        );
        assert!(promotion.bytes > 0);
        assert_eq!(promotion.sha256.len(), 64);
        assert!(
            promotion
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }

    // Every frozen path is exact: no glob, suffix rule, or directory allowance.
    for path in contract.mentioned() {
        assert!(
            !path.is_empty()
                && !path.ends_with('/')
                && !path.contains('*')
                && !path.contains('?')
                && !path.contains("..")
                && !path.starts_with('/'),
            "frozen path `{path}` must name one exact file"
        );
    }
}

#[test]
fn the_repository_is_in_exactly_one_frozen_transition_state() {
    let contract = TransitionContract::from_classification();
    let observed = Observed::from_repository(&contract, &repository_root());
    for tree in EXCLUDED_TREES {
        assert!(
            !observed
                .discovered
                .iter()
                .any(|path| path.starts_with(&format!("{tree}/"))),
            "the sweep must not enter {tree}"
        );
    }
    for path in ORACLE_FILES {
        assert!(
            !observed.discovered.contains(path),
            "the sweep must not report its own executor `{path}`"
        );
    }
    assert_eq!(
        classify_transition(&contract, &observed),
        Ok(TransitionState::PostReset),
        "the working tree must be the complete post-reset state"
    );

    // Containment-only, so this holds whether or not the later Python
    // trajectory surface has landed. Neither path exists in this checkout,
    // which is why the synthetic states below carry the mutants.
    for entry in &contract.post_reset_admitted {
        assert_eq!(
            observed.exists.contains(&entry.path),
            observed.admitted.contains_key(&entry.path),
            "admitted path `{}` must be read exactly when it exists",
            entry.path
        );
    }

    // The same walk collects the scopes, so the token contract is wired to the
    // real tree and not only to synthetic content.
    for path in [
        "crates/eqiora-artifact/src/model/node.rs",
        "crates/eqiora-artifact/src/model_transaction.rs",
        "crates/eqiora-api/src/control/schema.rs",
        "bindings/python/python/eqiora/__init__.pyi",
        "examples/python/fixed_reference_fsi.py",
        "studio/src/control-protocol.ts",
        "studio/src-tauri/src/compile.rs",
    ] {
        assert!(
            observed.content.contains_key(path),
            "the declared scopes must reach `{path}` in the working tree"
        );
    }
    assert!(
        observed.content.len() >= 200,
        "scoped product source collapsed"
    );

    assert_eq!(
        scan_forbidden_tokens(&contract.forbidden, &observed.content),
        Ok(()),
        "the complete post-reset product source must spell no forbidden token"
    );
}

/// The observed tree spells none of the 102, and the frozen record still says
/// there was something to remove. The 98/4 partition was measured before the
/// reset, over a tree this checkout no longer is, so it is read as the frozen
/// record it is: checked against the token list the live scan consumes and
/// against this case's declared totals, never as a second tree scan.
#[test]
fn pre_reset_occurrence_remains_frozen_and_post_reset_is_clean() {
    let contract = TransitionContract::from_classification();
    let observed = Observed::from_repository(&contract, &repository_root());
    let classification = classification();
    let occurrence = &classification["search"]["forbidden_product_tokens"]["pre_reset_occurrence"];
    assert_eq!(occurrence["measured_state"], "pre_reset");

    let records = occurrence["scopes"]
        .as_array()
        .expect("the presence contract must record every declared scope");
    assert_eq!(records.len(), contract.forbidden.len());

    let mut recorded_present = 0;
    let mut prospective = Vec::new();
    for scope in &contract.forbidden {
        let record = records
            .iter()
            .find(|record| record["name"] == scope.name.as_str())
            .unwrap_or_else(|| panic!("scope `{}` must record its occurrence", scope.name));
        let (present, _) = scope.occurrence(&observed.content);
        assert_eq!(
            present,
            Vec::<String>::new(),
            "{} tokens surviving in the observed post-reset tree",
            scope.name
        );

        // The record is held against the token list the scan above consumes: it
        // must count that list, partition it, and name as prospective only
        // tokens the scope really forbids.
        let recorded = record["present"].as_u64().unwrap() as usize;
        let absent = frozen_list(record, "absent");
        assert_eq!(
            record["declared"].as_u64().unwrap() as usize,
            scope.forbidden.len(),
            "{} declared token count",
            scope.name
        );
        assert_eq!(
            recorded + absent.len(),
            scope.forbidden.len(),
            "{} record must partition the tokens the scan forbids",
            scope.name
        );
        for token in &absent {
            assert!(
                scope.forbidden.contains(token),
                "{} records `{token}` as prospective without forbidding it",
                scope.name
            );
        }
        recorded_present += recorded;
        prospective.extend(absent);
    }

    // 86 of 90 Rust tokens, all six Python and all six control tokens. The four
    // that are missing are prospective post-reset guards: they name the
    // per-generation entry points a renamed historical v2 branch would most
    // plausibly reappear under while the reset is being written. Forbidding
    // them after the reset costs nothing; claiming they were observed would be
    // false, so this case does not.
    assert_eq!(recorded_present, 98);
    assert_eq!(
        prospective,
        [
            "from_program_v2",
            "from_json_v2",
            "from_transaction_v2",
            "digest_v2"
        ]
    );
    assert_eq!(occurrence["present_token_count"].as_u64().unwrap(), 98);
    assert_eq!(occurrence["prospective_token_count"].as_u64().unwrap(), 4);
}

#[test]
fn the_exact_pre_reset_and_a_maximal_post_reset_state_are_accepted() {
    let contract = TransitionContract::from_classification();
    assert_eq!(
        classify_transition(&contract, &Observed::exact_pre_reset(&contract)),
        Ok(TransitionState::PreReset)
    );
    assert_eq!(
        classify_transition(&contract, &Observed::maximal_post_reset(&contract)),
        Ok(TransitionState::PostReset)
    );

    // A preserved path migrated in place may stop carrying a Model signal, so
    // the maximal state above is one valid post-reset state, not the only one.
    let migrated = Observed::maximal_post_reset(&contract).no_longer_matching(&[
        "crates/eqiora-api/src/lib.rs",
        "crates/eqiora-python/src/model.rs",
        "studio/src/control-protocol.ts",
        "docs/architecture.md",
        "verify/interfaces/current-authoring-profile/expected/profile.json",
    ]);
    assert_eq!(
        classify_transition(&contract, &migrated),
        Ok(TransitionState::PostReset)
    );
}

#[test]
fn a_partial_retirement_is_refused() {
    let contract = TransitionContract::from_classification();

    // One retired path gone: the reset has begun and has not finished.
    for path in [
        "crates/eqiora-api/src/codec.rs",
        "crates/eqiora-artifact/src/model_v2.rs",
        "bindings/python/python/eqiora/compatibility.py",
        "verify/interfaces/control-plane-compile-check/oracle/v2/schema/compile-v2.schema.json",
        "verify/interfaces/control-plane-compile-check/models/accepted-v1.json",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::exact_pre_reset(&contract).without(&[path]),
            ),
            "partial transition",
        );
    }

    // Two retired paths gone is the same refusal, not a nearer miss.
    refused(
        classify_transition(
            &contract,
            &Observed::exact_pre_reset(&contract).without(&[
                "crates/eqiora-api/src/codec.rs",
                "crates/eqiora-artifact/src/model_transaction_v7.rs",
            ]),
        ),
        "partial transition",
    );

    // The mirror: retired paths surviving an otherwise completed reset.
    refused(
        classify_transition(
            &contract,
            &Observed::maximal_post_reset(&contract).with(&["crates/eqiora-api/src/codec.rs"]),
        ),
        "partial transition",
    );
    refused(
        classify_transition(
            &contract,
            &Observed::maximal_post_reset(&contract).with(&[
                "schemas/control/compile-v1.schema.json",
                "crates/eqiora-artifact/tests/model_v8_wire.rs",
            ]),
        ),
        "partial transition",
    );
}

#[test]
fn deleting_preserved_evidence_after_the_reset_is_refused() {
    let contract = TransitionContract::from_classification();
    for path in [
        "verify/artifacts/current-model-canonical-identity/expected/historical/model-v1.json",
        "verify/artifacts/current-model-canonical-identity/expected/historical/model-v7.json",
        "verify/artifacts/current-model-canonical-identity/expected/historical/\
         model-transaction-v7.json",
        "verify/artifacts/realization-run-wire/expected/realization-v1.json",
        "crates/eqiora-artifact/tests/realization_v4_wire.rs",
        "crates/eqiora-artifact/src/realization_v5.rs",
        "crates/eqiora-artifact/src/spatial_trajectory_v3.rs",
        "verify/numerics/canonical-cartesian-poisson-cuda/artifacts/q1-fem-run.json",
        "verify/fsi/fixed-reference-cuda-solve-2d/artifacts/model.json",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).without(&[path]),
            ),
            "preserved evidence",
        );
    }

    // Any other preserved path is protected too, by its own frozen membership.
    for path in [
        "crates/eqiora-artifact/src/model.rs",
        "verify/interfaces/control-plane-compile-check/expected/contract.json",
        "docs/site/examples.md",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).without(&[path]),
            ),
            "preserved inventory path",
        );
    }

    // The superseded v7 cylinder is evidence in both states at different paths,
    // which is the failure only `promoted_evidence` can describe.
    refused(
        classify_transition(
            &contract,
            &Observed::maximal_post_reset(&contract).without(&[
                "verify/artifacts/current-model-canonical-identity/expected/historical/\
                 steady-flow-past-cylinder.model-v7.json",
            ]),
        ),
        "promoted evidence",
    );

    // Its unversioned sibling is an ordinary product example and does not move.
    refused(
        classify_transition(
            &contract,
            &Observed::maximal_post_reset(&contract)
                .without(&["examples/steady-flow-past-cylinder.model.json"]),
        ),
        "preserved inventory path",
    );
}

#[test]
fn a_missing_or_substituted_promotion_target_is_refused() {
    let contract = TransitionContract::from_classification();
    for path in [
        "schemas/control/compile-v2.schema.json",
        "verify/interfaces/control-plane-compile-check/expected/historical/compile-v1.schema.json",
        "verify/interfaces/control-plane-compile-check/models/retired-v1.json",
        "verify/interfaces/control-plane-compile-check/models/accepted-v2.json",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).without(&[path]),
            ),
            "required target",
        );
    }

    // The two unversioned wire owners are required by existence alone; this
    // case owns neither's content.
    for path in [
        "crates/eqiora-artifact/src/model_wire.rs",
        "crates/eqiora-artifact/src/model_transaction_wire.rs",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).without(&[path]),
            ),
            "required target",
        );
        // The mirror: they must not exist before the reset either, or the
        // repository is mid-flight rather than pre-reset.
        refused(
            classify_transition(
                &contract,
                &Observed::exact_pre_reset(&contract).with(&[path]),
            ),
            "created by the reset, not before it",
        );
    }

    // A target that exists but carries other bytes is a substituted meaning,
    // including a live path left at its pre-reset content.
    for path in [
        "schemas/control/compile-v2.schema.json",
        "verify/interfaces/control-plane-compile-check/models/retired-v1.json",
        "verify/interfaces/control-plane-compile-check/expected/contract.json",
        "verify/artifacts/current-model-canonical-identity/expected/historical/\
         steady-flow-past-cylinder.model-v7.json",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract)
                    .with_digest(path, SYNTHETIC_UNPROMOTED_DIGEST),
            ),
            "promotion copies bytes",
        );
    }

    // Before the reset the staged source itself is byte-frozen, so the bytes
    // that will be promoted cannot drift while they wait.
    refused(
        classify_transition(
            &contract,
            &Observed::exact_pre_reset(&contract).with_digest(
                "verify/interfaces/control-plane-compile-check/oracle/v2/schema/\
                 compile-v2.schema.json",
                SYNTHETIC_UNPROMOTED_DIGEST,
            ),
        ),
        "must carry its frozen",
    );
}

#[test]
fn an_unclassified_new_signal_path_is_refused() {
    let contract = TransitionContract::from_classification();

    // After the reset, an unlisted Model-bearing path is returned to this
    // oracle; only an exact `post_reset_admitted` entry is ever admitted.
    for path in [
        "crates/eqiora-artifact/src/model_epoch_ladder.rs",
        "verify/interfaces/control-plane-compile-check/models/accepted-v3.json",
        "crates/eqiora-api/tests/control_compile_v2.rs",
    ] {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).signalling(&[path]),
            ),
            "unclassified new signal-bearing",
        );
    }

    // Before the reset the same rule is exact inventory equality, in both
    // directions: a new candidate appears, or a frozen one stops matching.
    refused(
        classify_transition(
            &contract,
            &Observed::exact_pre_reset(&contract)
                .signalling(&["crates/eqiora-api/src/codec_v2.rs"]),
        ),
        "must equal the frozen inventory exactly",
    );
    refused(
        classify_transition(
            &contract,
            &Observed::exact_pre_reset(&contract)
                .no_longer_matching(&["crates/eqiora-artifact/src/model_v8.rs"]),
        ),
        "must equal the frozen inventory exactly",
    );

    // A second executor file beside this oracle's two is the same refusal: the
    // exclusion is exact, so a third file arrives as an unclassified candidate.
    refused(
        classify_transition(
            &contract,
            &Observed::exact_pre_reset(&contract).signalling(&[
                "crates/eqiora-artifact/tests/current_model_relational_identity_transition/\
                 second_support_module.rs",
            ]),
        ),
        "must equal the frozen inventory exactly",
    );
}

#[test]
fn the_forbidden_token_scopes_are_narrow_declared_and_complete() {
    let contract = TransitionContract::from_classification();
    let classification = classification();
    let declared = &classification["search"]["forbidden_product_tokens"];
    assert_eq!(declared["applies"], "post_reset_only");
    assert_eq!(
        declared["scope_count"].as_u64().unwrap() as usize,
        contract.forbidden.len()
    );
    let tokens: usize = contract
        .forbidden
        .iter()
        .map(|scope| scope.forbidden.len())
        .sum();
    assert_eq!(
        declared["forbidden_token_count"].as_u64().unwrap() as usize,
        tokens
    );

    for scope in &contract.forbidden {
        assert!(!scope.paths.is_empty() && !scope.forbidden.is_empty());
        let unique = scope.forbidden.iter().collect::<BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            scope.forbidden.len(),
            "{} repeats a token",
            scope.name
        );
        assert!(
            scope.forbidden.iter().all(|token| !token.is_empty()),
            "{} declares an empty token, which would forbid every file",
            scope.name
        );
    }

    // The whole removal inventory must be reachable, or the contract is a
    // sample. All 102 are forbidden however many the pre-reset tree spells.
    let all = contract
        .forbidden
        .iter()
        .flat_map(|scope| scope.forbidden.iter())
        .collect::<BTreeSet<_>>();
    for required in [
        "ModelEnvelopeV1",
        "ModelEnvelopeV7",
        "ModelEnvelopeV8",
        "ModelTransactionEnvelopeV8",
        "WireModelContentV2",
        "ExactModelCodec",
        "ModelArtifactGeneration",
        "AcceptedModelEnvelope",
        "VersionedModelTransactionEnvelope",
        "encode_v1",
        "encode_v8",
        "ensure_v8",
        "from_program_v8",
        "from_json_v2",
        "from_transaction_v8",
        "digest_v8",
        "reject_coordinate_dependency_before_v8",
        "exact_codec",
        "eqiora::compatibility",
        "eqiora.model-envelope/v7",
        "eqiora.model-transaction-envelope/v1",
        "eqiora.compatibility",
        "compile_exact",
        "define_exact",
        "replay_exact",
        "modelWire",
        "requiredFeatures",
        "model-wire/",
        "CompileFeatureV1",
        "COMPILE_FEATURE_V1",
        "MAX_COMPILE_REQUIRED_FEATURES_V1",
    ] {
        assert!(
            all.contains(&required.to_owned()),
            "{required} must be forbidden"
        );
    }

    // Persisted current names and the protocol the v2 decoder names in its
    // rejection diagnostic stay legal; forbidding them would delete evidence.
    let permitted = declared["deliberately_permitted"]
        .as_array()
        .expect("the contract must name what it deliberately permits")
        .iter()
        .map(|entry| entry["token"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        permitted,
        [
            "eqiora.control/v1",
            "eqiora.model-envelope/v8",
            "eqiora.model-transaction-envelope/v8",
            "model.compile-check/v1",
        ]
    );
    for token in &permitted {
        assert!(
            !all.contains(token),
            "{token} is deliberately permitted and must not also be forbidden"
        );
        assert!(
            !all.iter()
                .any(|forbidden| token.contains(forbidden.as_str())),
            "a forbidden token must not be a substring of the permitted {token}"
        );
    }

    // Narrowness is the point: the negative corpus, the retained bytes, the
    // kits, the RFC, and the docs stay unscanned.
    for unscanned in [
        "verify/interfaces/control-plane-compile-check/oracle/v2/schema/compile-v2.schema.json",
        "verify/interfaces/control-plane-compile-check/models/forbidden-model-wire-v2.json",
        "verify/interfaces/control-plane-compile-check/expected/contract.json",
        "verify/artifacts/current-model-canonical-identity/expected/historical/model-v7.json",
        "verify/artifacts/realization-run-wire/expected/realization-v1.json",
        "crates/eqiora-artifact/tests/current_model_wire_oracle.rs",
        "crates/eqiora-api/tests/control_compile_v1.rs",
        "bindings/python/tests/test_control_plane.py",
        "rfcs/0083-current-model-artifact-epoch.md",
        "docs/capability-matrix.md",
        "CHANGELOG.md",
        "schemas/control/compile-v2.schema.json",
        "studio/src/control-protocol.test.ts",
        "studio/src/state.spec.ts",
    ] {
        for scope in &contract.forbidden {
            assert!(
                !scope.covers(unscanned),
                "scope `{}` must not reach `{unscanned}`",
                scope.name
            );
        }
    }

    // This case's own executors are outside every scope too, so the tokens they
    // must spell to do their work are never a violation.
    for path in ORACLE_FILES {
        for scope in &contract.forbidden {
            assert!(
                !scope.covers(path),
                "scope `{}` must not reach this oracle's own executor `{path}`",
                scope.name
            );
        }
    }

    // The single test-only exclusion is scoped to the control tokens alone: a
    // control test may name a field it proves the v2 decoder rejects, while a
    // historical Model spelling in product Rust is a branch wherever it sits.
    let scope = |name: &str| {
        contract
            .forbidden
            .iter()
            .find(|scope| scope.name == name)
            .unwrap_or_else(|| panic!("scope `{name}` must be declared"))
    };
    assert!(!scope("control-product-source").covers("crates/eqiora-api/src/control/tests.rs"));
    assert!(scope("rust-product-source").covers("crates/eqiora-api/src/control/tests.rs"));

    // Every scope reaches the files the reset must actually clean.
    for (name, path) in [
        (
            "rust-product-source",
            "crates/eqiora-artifact/src/model/node.rs",
        ),
        (
            "rust-product-source",
            "crates/eqiora-artifact/src/model_transaction.rs",
        ),
        ("rust-product-source", "crates/eqiora/src/lib.rs"),
        ("rust-product-source", "studio/src-tauri/src/compile.rs"),
        (
            "python-product-source",
            "bindings/python/python/eqiora/__init__.pyi",
        ),
        (
            "python-product-source",
            "examples/python/fixed_reference_fsi.py",
        ),
        (
            "control-product-source",
            "crates/eqiora-api/src/control/schema.rs",
        ),
        ("control-product-source", "studio/src/control-protocol.ts"),
        ("control-product-source", "studio/src-tauri/src/compile.rs"),
    ] {
        assert!(
            scope(name).covers(path),
            "scope `{name}` must reach `{path}`"
        );
    }
}

#[test]
fn a_clean_post_reset_product_source_is_accepted_and_a_private_branch_is_refused() {
    let contract = TransitionContract::from_classification();
    assert_eq!(
        scan_forbidden_tokens(&contract.forbidden, &clean_post_reset_product_source()),
        Ok(())
    );

    // One violation per scope, each the kind path existence cannot see: a
    // private generation branch inside a preserved Rust file, an exact-codec
    // selector still exported by the Python surface, and a caller-selected
    // Model generation still accepted by the control plane. The last is one of
    // the four tokens absent from the pre-reset tree: a prospective guard is
    // enforced exactly like a token that exists today.
    for (path, source, token) in [
        (
            "crates/eqiora-artifact/src/model/node.rs",
            "impl WireNode {\n    pub(crate) fn encode_v3(node: &KernelNode) -> Result<Self, \
             Diagnostic> {\n        Self::encode(node, WireVersion::V3)\n    }\n}\n",
            "encode_v3",
        ),
        (
            "crates/eqiora-artifact/src/model_wire.rs",
            "pub const MODEL_SCHEMA: &str = \"eqiora.model-envelope/v8\";\n\
             fn admit_historical(schema: &str) -> bool { schema == \"eqiora.model-envelope/v7\" }\n",
            "eqiora.model-envelope/v7",
        ),
        (
            "crates/eqiora-artifact/src/model_wire.rs",
            "impl ModelEnvelope {\n    fn from_program_v2(program: &Program) -> Self { todo!() }\n}\n",
            "from_program_v2",
        ),
        (
            "crates/eqiora-api/src/package/model_document.rs",
            "pub struct ModelDocument { codec: ExactModelCodec }\n",
            "ExactModelCodec",
        ),
        (
            "bindings/python/python/eqiora/__init__.pyi",
            "def compile_exact(source: str, generation: int) -> Model: ...\n",
            "compile_exact",
        ),
        (
            "examples/python/fixed_reference_fsi.py",
            "from eqiora.compatibility import ExactModelCodec\n",
            "eqiora.compatibility",
        ),
        (
            "studio/src/control-protocol.ts",
            "export interface CompileRequest { modelWire?: string }\n",
            "modelWire",
        ),
        (
            "crates/eqiora-api/src/control/schema.rs",
            "const MAX_COMPILE_REQUIRED_FEATURES_V1: usize = 8;\n",
            "MAX_COMPILE_REQUIRED_FEATURES_V1",
        ),
    ] {
        let dirty = clean_post_reset_product_source()
            .into_iter()
            .chain([(path.to_owned(), source.to_owned())])
            .collect();
        let reason = scan_forbidden_tokens(&contract.forbidden, &dirty)
            .expect_err("a surviving product token must be refused");
        assert!(
            reason.contains(token) && reason.contains(path),
            "the refusal must name `{token}` in `{path}`, got: {reason}"
        );

        // And it is refused through the state predicate, not only in isolation.
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).with_source(path, source),
            ),
            "forbidden product token",
        );
    }

    // The same tokens outside the declared scopes are evidence, not a branch,
    // and must keep naming what the reset removed.
    for (path, source) in [
        (
            "crates/eqiora-api/src/control/tests.rs",
            "let request = json!({\"modelWire\": \"...\", \"requiredFeatures\": []});\n",
        ),
        (
            "studio/src/control-protocol.test.ts",
            "it('rejects modelWire', () => expect(parse({ modelWire: 'x' })).toThrow());\n",
        ),
        (
            "verify/artifacts/current-model-canonical-identity/expected/historical/model-v7.json",
            "{\"schema\":\"eqiora.model-envelope/v7\"}\n",
        ),
        (
            "crates/eqiora-artifact/tests/model_wire_negative_corpus.rs",
            "// ExactModelCodec and encode_v3 are named here only as history.\n",
        ),
        (
            "bindings/python/tests/test_control_plane.py",
            "def test_compile_exact_is_gone(): assert not hasattr(eqiora, 'compile_exact')\n",
        ),
    ] {
        let content = clean_post_reset_product_source()
            .into_iter()
            .chain([(path.to_owned(), source.to_owned())])
            .collect();
        assert_eq!(
            scan_forbidden_tokens(&contract.forbidden, &content),
            Ok(()),
            "`{path}` is outside every declared scope and must stay unscanned"
        );
    }
}

/// Admission adds a permission and nothing else: the historical record of the
/// reset is what it was, and no admitted path joins a single set inside it.
#[test]
fn a_later_product_path_is_admitted_by_exact_path_and_joins_no_frozen_set() {
    let contract = TransitionContract::from_classification();
    let classification = classification();
    let transition = &classification["search"]["transition"];
    let classes = classification["classes"].as_object().unwrap();
    let admitted = contract
        .post_reset_admitted
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        admitted,
        ADMITTED_AS_RECORDED
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect::<BTreeSet<_>>(),
        "the admitted set is exactly the paths this oracle derived, and the synthetic states \
         below mutate those same paths"
    );
    let count = transition["post_reset_admitted_path_count"]
        .as_u64()
        .unwrap();
    assert_eq!(admitted.len(), contract.post_reset_admitted.len());
    assert_eq!(admitted.len() as u64, count);

    // Every historical count is still its own. Listing an admitted path in the
    // inventory would claim it existed before the reset; listing one in
    // `required_post_reset` would claim the reset created it, and would make a
    // later capability a condition of the transition being accepted.
    assert_eq!(contract.inventory.len(), 338);
    assert_eq!(contract.retired.len(), 44);
    assert_eq!(contract.preserved().len(), 304);
    assert_eq!(contract.required_post_reset.len(), 13);
    assert_eq!(contract.preserved_evidence.len(), 40);
    for path in &admitted {
        assert!(
            !contract.inventory.contains(path)
                && !contract.retired.contains(path)
                && !contract.required_post_reset.contains(path)
                && !contract.preserved_evidence.contains(path)
                && !ORACLE_FILES.contains(&path.as_str())
                && (contract.promotion.iter())
                    .all(|row| row.source != *path && row.target != *path),
            "admitted `{path}` joins no frozen set: not the inventory, not `retired`, not \
             `required_post_reset`, not `preserved_evidence`, no promotion row, and not this \
             oracle's own executor files"
        );
    }

    // Each entry says what it is, who owns it, and why, and is admitted only for
    // a signal the sweep really searches for, spelled as the sweep spells it.
    for entry in transition["post_reset_admitted"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        let class = entry["class"].as_str().unwrap();
        assert!(classes.contains_key(class), "undeclared class {class}");
        for key in ["owner", "note"] {
            assert!(!entry[key].as_str().unwrap().is_empty(), "{path}: `{key}`");
        }
        let signals = frozen_list(entry, "signals");
        let places = (signals.iter())
            .map(|signal| SEARCH_TOKENS.iter().position(|token| token == signal))
            .collect::<Option<Vec<_>>>();
        assert!(
            !signals.is_empty()
                && places.is_some_and(|at| at.windows(2).all(|pair| pair[0] < pair[1])),
            "{path} must record a nonempty signal list the sweep would find, in its order and \
             without repeats"
        );
        assert_eq!(
            entry["identity_literals"].as_u64().unwrap(),
            0,
            "{path} is admitted as a consumer surface and may freeze no Model identity"
        );
    }
}

/// The admission predicate: optional, signal-bearing, identity-free, absent
/// before the reset, and exact.
#[test]
fn an_admitted_later_path_is_optional_signal_bearing_and_identity_free() {
    let contract = TransitionContract::from_classification();
    let classify = |observed: Observed| classify_transition(&contract, &observed);
    let reset = || Observed::maximal_post_reset(&contract);
    let all = |state: Observed| {
        (ADMITTED_AS_RECORDED.iter()).fold(state, |at, (path, bytes)| at.admitting(path, bytes))
    };
    let trajectory = |source: &str| reset().admitting(ADMITTED_AS_RECORDED[0].0, source);

    // Optional independently: every subset, including none and all, remains a
    // complete post-reset state.
    for mask in 0..(1 << ADMITTED_AS_RECORDED.len()) {
        let observed = ADMITTED_AS_RECORDED.iter().enumerate().fold(
            reset(),
            |state, (index, (path, source))| {
                if mask & (1 << index) == 0 {
                    state
                } else {
                    state.admitting(path, source)
                }
            },
        );
        assert_eq!(
            classify(observed),
            Ok(TransitionState::PostReset),
            "admitted subset mask {mask:#05b} must remain optional"
        );
    }

    // Signal. The first mutant is what path existence and the candidate
    // sweep can see: it exists, spells no signal at all, and is therefore never
    // discovered. The second spells one signal more than it was admitted for.
    for source in [
        "pub fn trajectory(handle: &Handle) -> PyResult<Trajectory> {\n    handle.open()\n}\n",
        "pub fn trajectory(model_digest: &str, model_sha256: &str) -> PyResult<Trajectory> {\n    \
         todo!()\n}\n",
    ] {
        refused(
            classify(trajectory(source)),
            "must carry exactly its recorded search signal",
        );
    }

    // Identity. The same consumer with one Model identity frozen into it is a
    // fixture, and a fixture is classified rather than admitted.
    let pinned = format!(
        "{}const PINNED_MODEL: &str = \"{}\";\n",
        ADMITTED_AS_RECORDED[0].1,
        "e".repeat(64)
    );
    refused(
        classify(trajectory(&pinned)),
        "Model-derived identity literal",
    );

    // Pre-reset. Admission describes a path created after the reset, so one
    // present before it is mid-flight — by existence alone, signal or not.
    for (path, _) in ADMITTED_AS_RECORDED {
        refused(
            classify(Observed::exact_pre_reset(&contract).with(&[path])),
            "admission covers a product path created after the reset",
        );
    }

    // Exact. Admission reaches only those exact paths: not a sibling in
    // the same directory, not a file below one of them, not another spelling of
    // the same name, and not the other extension of the same module.
    for path in [
        "crates/eqiora-python/src/trajectory_2d.rs",
        "crates/eqiora-python/src/trajectory/segment.rs",
        "crates/eqiora-python/src/result_2d.rs",
        "crates/eqiora-python/src/result/output.rs",
        "bindings/python/python/eqiora/trajectory.py",
        "bindings/python/python/eqiora/trajectory2.pyi",
    ] {
        refused(
            classify(all(reset()).signalling(&[path])),
            "unclassified new signal-bearing",
        );
    }
}
