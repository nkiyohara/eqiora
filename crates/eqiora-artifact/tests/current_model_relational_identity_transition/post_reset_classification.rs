//! Full classifications for later identity-bearing evidence paths.

use super::*;

const RETIRED_TYPED_EXECUTION_IDENTITIES: &[u8] = include_bytes!(
    "../../../../verify/artifacts/current-model-relational-identity-transition/expected/deterministic/typed-execution-lineage/identities.json"
);
const CURRENT_TYPED_COMPILATION_IDENTITIES: &[u8] = include_bytes!(
    "../../../../verify/packages/typed-compilation-lineage/expected/identities.json"
);
use std::sync::OnceLock;

const OWNER: &str = "interfaces.python-offline-model-package";
const CURRENT_ROLE: &str = "current-model-or-package-artifact";
const IMMUTABLE_ROLE: &str = "source-or-resolution-identity";
const PYTHON_MIXED_PATH: &str = "bindings/python/tests/test_offline_model_package.py";
const RUST_MIXED_PATH: &str = "crates/eqiora-python/tests/python_offline_model_package.rs";

struct AcceptedClassificationState {
    identities: Vec<PostResetClassified>,
    diagnostic_record: Value,
}

static ACCEPTED_STATE: OnceLock<AcceptedClassificationState> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ClassifiedClaim {
    selector: String,
    class: String,
    disposition: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IdentityLiteral {
    value: String,
    role: String,
    occurrences: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PostResetClassified {
    pub(super) path: String,
    class: String,
    disposition: String,
    owner: String,
    signals: Vec<String>,
    same_line_lower_hex_identity_signal_lines: usize,
    current_model_identity_literals: usize,
    identity_literal_occurrences: usize,
    identity_literals: Vec<IdentityLiteral>,
    claims: Vec<ClassifiedClaim>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ClassifiedObservation {
    signals: Vec<String>,
    same_line_lower_hex_identity_signal_lines: usize,
    identity_literals: BTreeMap<String, usize>,
    current_compilation_slots: Option<[(String, String); 2]>,
}

fn string_field(value: &Value, key: &str, context: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("later classification: `{context}` must name string `{key}`"))
}

fn count_field(value: &Value, key: &str, context: &str) -> Result<usize, String> {
    value[key]
        .as_u64()
        .map(|count| count as usize)
        .ok_or_else(|| format!("later classification: `{context}` must name count `{key}`"))
}

fn string_list(value: &Value, key: &str, context: &str) -> Result<Vec<String>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("later classification: `{context}` must name list `{key}`"))?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                format!("later classification: `{context}.{key}` must contain only strings")
            })
        })
        .collect()
}

/// Load the authoritative exact classifications. The executor contains no
/// second path list or signal table: mutations are compared back to this
/// durable record.
pub(super) fn from_classification(
    classification: &Value,
) -> Result<Vec<PostResetClassified>, String> {
    let transition = &classification["search"]["transition"];
    transition["post_reset_classified"]
        .as_array()
        .ok_or_else(|| {
            "later classification: frozen transition must name `post_reset_classified`".to_owned()
        })?
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let context = format!("post_reset_classified[{index}]");
            let claims = row["claims"]
                .as_array()
                .ok_or_else(|| format!("later classification: `{context}` must name `claims`"))?
                .iter()
                .map(|claim| {
                    Ok(ClassifiedClaim {
                        selector: string_field(claim, "selector", &context)?,
                        class: string_field(claim, "class", &context)?,
                        disposition: string_field(claim, "disposition", &context)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let identity_literals = row["identity_literals"]
                .as_array()
                .ok_or_else(|| {
                    format!("later classification: `{context}` must name `identity_literals`")
                })?
                .iter()
                .map(|literal| {
                    Ok(IdentityLiteral {
                        value: string_field(literal, "value", &context)?,
                        role: string_field(literal, "role", &context)?,
                        occurrences: count_field(literal, "occurrences", &context)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(PostResetClassified {
                path: string_field(row, "path", &context)?,
                class: string_field(row, "class", &context)?,
                disposition: string_field(row, "disposition", &context)?,
                owner: string_field(row, "owner", &context)?,
                signals: string_list(row, "signals", &context)?,
                same_line_lower_hex_identity_signal_lines: count_field(
                    row,
                    "same_line_lower_hex_identity_signal_lines",
                    &context,
                )?,
                current_model_identity_literals: count_field(
                    row,
                    "current_model_identity_literals",
                    &context,
                )?,
                identity_literal_occurrences: count_field(
                    row,
                    "identity_literal_occurrences",
                    &context,
                )?,
                identity_literals,
                claims,
            })
        })
        .collect()
}

pub(super) fn validated_from_classification(
    classification: &Value,
) -> Result<Vec<PostResetClassified>, String> {
    let entries = from_classification(classification)?;
    validate(&entries, classification)?;
    initialize_accepted_state(&ACCEPTED_STATE, &entries, classification)?;
    Ok(entries)
}

fn initialize_accepted_state<'a>(
    cell: &'a OnceLock<AcceptedClassificationState>,
    identities: &[PostResetClassified],
    diagnostic_record: &Value,
) -> Result<&'a AcceptedClassificationState, String> {
    let accepted = cell.get_or_init(|| AcceptedClassificationState {
        identities: identities.to_vec(),
        diagnostic_record: diagnostic_record.clone(),
    });
    if accepted.identities != identities {
        return Err("later classification: frozen rows changed within one run".to_owned());
    }
    Ok(accepted)
}

fn validate_cached_runtime(
    cell: &OnceLock<AcceptedClassificationState>,
    entries: &[PostResetClassified],
) -> Result<(), String> {
    let accepted = cell
        .get()
        .ok_or_else(|| "later classification: accepted rows were not initialized".to_owned())?;
    if entries != accepted.identities {
        validate(entries, &accepted.diagnostic_record)?;
        return Err("later classification: runtime table differs from accepted rows".to_owned());
    }
    Ok(())
}

pub(super) fn validate_runtime(entries: &[PostResetClassified]) -> Result<(), String> {
    validate_cached_runtime(&ACCEPTED_STATE, entries)
}

fn transition_current_identities() -> BTreeSet<String> {
    let transition = crate::transition();
    let offline = crate::entry(&transition, "deterministic", "offline-model-package");
    let typed: Value = serde_json::from_slice(RETIRED_TYPED_EXECUTION_IDENTITIES).unwrap();
    [
        offline[concat!("model_", "digest")].as_str().unwrap(),
        offline["edges"].as_array().unwrap().iter()
            .find(|edge| edge["artifact"] == "compilation.json")
            .and_then(|edge| edge["digest"].as_str()).unwrap(),
        typed[concat!("model_", "sha256")].as_str().unwrap(),
        typed["package_compilation_sha256"].as_str().unwrap(),
    ]
        .into_iter().map(str::to_owned).collect()
}

fn all_transition_current_artifact_identities() -> BTreeSet<String> {
    let mut identities = crate::transition()["deterministic"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|fixture| {
            fixture[concat!("model_", "digest")]
                .as_str()
                .into_iter()
                .chain(
                    fixture["edges"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|edge| edge["digest"].as_str()),
                )
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    let typed: Value = serde_json::from_slice(RETIRED_TYPED_EXECUTION_IDENTITIES).unwrap();
    identities.extend(
        [concat!("model_", "sha256"), "package_compilation_sha256"]
            .into_iter()
            .map(|key| typed[key].as_str().unwrap().to_owned()),
    );
    identities
}

fn exact_literal_map(entry: &PostResetClassified) -> BTreeMap<String, usize> {
    entry
        .identity_literals
        .iter()
        .map(|literal| (literal.value.clone(), literal.occurrences))
        .collect()
}

#[rustfmt::skip]
fn historical_compilation_slots() -> Result<(String, String), String> {
    let transition = crate::transition();
    let extract = |name: &str, pointer: &str| {
        crate::entry(&transition, "deterministic", name)["edges"]
            .as_array()
            .and_then(|edges| edges.iter().find(|edge| edge["pointer"] == pointer))
            .and_then(|edge| edge["digest"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("current slots: `{name}` must freeze predecessor `{pointer}`"))
    };
    Ok((
        extract("offline-model-package", "/compilation_digest")?,
        serde_json::from_slice::<Value>(RETIRED_TYPED_EXECUTION_IDENTITIES)
            .map_err(|error| format!("current slots: invalid retired typed identities: {error}"))?
            ["package_compilation_sha256"]
            .as_str()
            .ok_or_else(|| "current slots: retired typed identities lack `/package_compilation_sha256`".to_owned())?
            .to_owned(),
    ))
}

#[rustfmt::skip]
fn validate_current_compilation_slots(
    entries: &[PostResetClassified],
    slots: &[(String, String); 2],
) -> Result<(), String> {
    let historical = historical_compilation_slots()?;
    if (slots[0].0.as_str(), slots[1].0.as_str()) != (historical.0.as_str(), historical.1.as_str()) {
        return Err("current slots: historical predecessors changed".to_owned());
    }
    for (name, pointer, value) in [
        ("offline-model-package.live", "/compilation_digest", &slots[0].1),
        ("typed-compilation-lineage.live", "/package_compilation_sha256", &slots[1].1),
    ] {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(format!("current slots: `{name}` `{pointer}` must be lowercase-hex-64"));
        }
    }
    let stale = [
        ("offline-model-package.live /compilation_digest", &slots[0]),
        ("typed-compilation-lineage.live /package_compilation_sha256", &slots[1]),
    ]
        .into_iter().filter_map(|(slot, (old, current))| (old == current).then_some(slot)).collect::<Vec<_>>();
    if !stale.is_empty() {
        return Err(format!("current slots: stale-alpha.1 {stale:?}"));
    }
    if slots[0].1 == slots[1].1 {
        return Err("current slots: delegated compilation values collapsed".to_owned());
    }
    let protected = entries.iter().filter(|entry| entry.class == "mixed-claim-surface")
        .flat_map(|entry| entry.identity_literals.iter().map(|literal| &literal.value)).collect::<BTreeSet<_>>();
    if protected.contains(&slots[0].1) || protected.contains(&slots[1].1) {
        return Err("current slots: delegated compilation value collides with alpha.1 history".to_owned());
    }
    Ok(())
}

#[rustfmt::skip]
fn compilation_slots_from_authorities(
    entries: &[PostResetClassified],
    offline: &Value,
    typed: &Value,
) -> Result<[(String, String); 2], String> {
    let read = |document: &Value, pointer: &str, name: &str| {
        crate::resolve(document, pointer)
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("current slots: `{name}` must expose string `{pointer}`"))
    };
    let historical = historical_compilation_slots()?;
    let slots = [
        (historical.0, read(offline, "/compilation_digest", "offline-model-package.live")?),
        (historical.1, read(typed, "/package_compilation_sha256", "typed-compilation-lineage.live")?),
    ];
    validate_current_compilation_slots(entries, &slots)?;
    Ok(slots)
}

#[rustfmt::skip]
fn current_compilation_slots(
    entries: &[PostResetClassified],
) -> Result<[(String, String); 2], String> {
    let fixture = |name| {
        crate::DETERMINISTIC.iter()
            .find(|fixture| fixture.name == name)
            .ok_or_else(|| format!("current slots: missing `{name}` deterministic authority"))
            .and_then(|fixture| serde_json::from_slice(fixture.live)
                .map_err(|error| format!("current slots: invalid `{name}.live`: {error}")))
    };
    let typed = serde_json::from_slice(CURRENT_TYPED_COMPILATION_IDENTITIES)
        .map_err(|error| format!("current slots: invalid typed-compilation live authority: {error}"))?;
    compilation_slots_from_authorities(entries, &fixture("offline-model-package")?, &typed)
}

#[rustfmt::skip]
fn synthetic_compilation_slots() -> [(String, String); 2] {
    let historical = historical_compilation_slots().unwrap();
    [(historical.0, "0".repeat(64)), (historical.1, "1".repeat(64))]
}

fn declared_count(transition: &Value, key: &str) -> Result<usize, String> {
    count_field(transition, key, "search.transition")
}

/// Validate a candidate table against both the durable rows and their semantic
/// roles. Equality to the JSON catches path, signal, class, and disposition
/// mutants; the semantic checks prevent a self-consistent but false JSON edit.
pub(super) fn validate(
    entries: &[PostResetClassified],
    classification: &Value,
) -> Result<(), String> {
    let expected = from_classification(classification)?;
    let transition = &classification["search"]["transition"];
    let classes = classification["classes"]
        .as_object()
        .ok_or_else(|| "later classification: missing frozen class vocabulary".to_owned())?;
    let dispositions = classification["dispositions"]
        .as_object()
        .ok_or_else(|| "later classification: missing frozen disposition vocabulary".to_owned())?;
    let frozen_counts = (
        declared_count(transition, "post_reset_classified_path_count")?,
        declared_count(
            transition,
            "post_reset_classified_mixed_assertion_surface_count",
        )?,
        declared_count(
            transition,
            "post_reset_classified_source_or_package_identity_path_count",
        )?,
        declared_count(
            transition,
            "post_reset_classified_model_derived_identity_literal_count",
        )?,
    );
    if frozen_counts != (3, 2, 1, 8) {
        return Err(format!(
            "later classification: frozen 3/2/1/8 counts changed to {frozen_counts:?}"
        ));
    }

    let paths = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    if paths.len() != entries.len() || entries.len() != frozen_counts.0 {
        return Err(
            "later classification: exactly three unique exact paths are required".to_owned(),
        );
    }
    let fixture_paths = transition["post_reset_fixture_admitted"]
        .as_array()
        .ok_or_else(|| {
            "later classification: frozen transition must name `post_reset_fixture_admitted`"
                .to_owned()
        })?
        .iter()
        .enumerate()
        .map(|(index, row)| {
            string_field(
                row,
                "path",
                &format!("post_reset_fixture_admitted[{index}]"),
            )
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    if let Some(overlap) = paths.intersection(&fixture_paths).next() {
        return Err(format!(
            "later classification: `{overlap}` overlaps `post_reset_fixture_admitted`; exact \
             classifications and fixture admissions are disjoint"
        ));
    }

    let transition_identities = transition_current_identities();
    let all_current_artifact_identities = all_transition_current_artifact_identities();
    if transition_identities.len() != 4 {
        return Err(
            "later classification: four frozen #114 current identities are required".to_owned(),
        );
    }
    let mut mixed_count = 0;
    let mut source_count = 0;
    let mut current_count = 0;

    for entry in entries {
        if entry.path.is_empty()
            || entry.path.starts_with('/')
            || entry.path.ends_with('/')
            || entry.path.contains('*')
            || entry.path.contains('?')
            || entry.path.contains("..")
        {
            return Err(format!(
                "later classification: `{}` is not one exact repository path",
                entry.path
            ));
        }
        if !classes.contains_key(&entry.class) || !dispositions.contains_key(&entry.disposition) {
            return Err(format!(
                "later classification: `{}` uses undeclared class or disposition",
                entry.path
            ));
        }
        if entry.owner != OWNER {
            return Err(format!(
                "later classification: `{}` must remain owned by `{OWNER}`",
                entry.path
            ));
        }
        let places = entry
            .signals
            .iter()
            .map(|signal| SEARCH_TOKENS.iter().position(|token| token == signal))
            .collect::<Option<Vec<_>>>();
        if places.is_none_or(|at| at.windows(2).any(|pair| pair[0] >= pair[1]))
            || (entry.signals.is_empty() && entry.same_line_lower_hex_identity_signal_lines == 0)
        {
            return Err(format!(
                "later classification: `{}` must record its exact ordered search signals",
                entry.path
            ));
        }

        let mut literal_values = BTreeSet::new();
        let mut literal_occurrences = 0;
        let mut entry_current_count = 0;
        let mut current_values = BTreeSet::new();
        for literal in &entry.identity_literals {
            if literal.value.len() != 64
                || !literal
                    .value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || literal.occurrences == 0
                || !literal_values.insert(literal.value.as_str())
                || ![CURRENT_ROLE, IMMUTABLE_ROLE].contains(&literal.role.as_str())
            {
                return Err(format!(
                    "later classification: `{}` has an invalid or repeated identity record",
                    entry.path
                ));
            }
            literal_occurrences += literal.occurrences;
            if literal.role == CURRENT_ROLE {
                entry_current_count += literal.occurrences;
                current_values.insert(literal.value.clone());
            } else if all_current_artifact_identities.contains(&literal.value) {
                return Err(format!(
                    "later classification: `{}` mislabels a current artifact identity as immutable input",
                    entry.path
                ));
            }
        }
        if literal_occurrences != entry.identity_literal_occurrences
            || entry_current_count != entry.current_model_identity_literals
        {
            return Err(format!(
                "later classification: `{}` identity totals disagree with its exact inventory",
                entry.path
            ));
        }
        current_count += entry_current_count;

        if entry.class == "mixed-claim-surface" {
            mixed_count += 1;
            let claims = entry
                .claims
                .iter()
                .map(|claim| (claim.class.as_str(), claim.disposition.as_str()))
                .collect::<BTreeSet<_>>();
            let required = BTreeSet::from([
                ("current-owner-assertion", "migrate"),
                ("source-or-package-identity", "migrate-in-place"),
            ]);
            if entry.disposition != "decompose-by-claim"
                || entry.current_model_identity_literals != 4
                || current_values != transition_identities
                || entry.claims.len() != 2
                || claims != required
                || entry.claims.iter().any(|claim| claim.selector.is_empty())
            {
                return Err(format!(
                    "later classification: test `{}` must decompose four exact current identities from immutable inputs",
                    entry.path
                ));
            }
        } else if entry.class == "source-or-package-identity" {
            source_count += 1;
            if entry.disposition != "migrate-in-place"
                || entry.current_model_identity_literals != 0
                || !entry.claims.is_empty()
                || entry
                    .identity_literals
                    .iter()
                    .any(|literal| literal.role != IMMUTABLE_ROLE)
            {
                return Err(
                    "later classification: the stored release must remain source/package identity only"
                        .to_owned(),
                );
            }
        } else {
            return Err(format!(
                "later classification: `{}` has an invalid top-level class",
                entry.path
            ));
        }
    }

    if (mixed_count, source_count, current_count)
        != (frozen_counts.1, frozen_counts.2, frozen_counts.3)
    {
        return Err(format!(
            "later classification: observed mixed/source/current totals were {mixed_count}/{source_count}/{current_count}"
        ));
    }
    if entries != expected {
        return Err(
            "later classification: runtime table differs from authoritative exact JSON rows"
                .to_owned(),
        );
    }
    Ok(())
}

fn lower_hex_identity_inventory(bytes: &[u8]) -> BTreeMap<String, usize> {
    let mut identities = BTreeMap::new();
    let mut start = 0;
    while start < bytes.len() {
        if !(bytes[start].is_ascii_digit() || (b'a'..=b'f').contains(&bytes[start])) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_digit() || (b'a'..=b'f').contains(&bytes[end]))
        {
            end += 1;
        }
        if end - start == 64 {
            *identities
                .entry(String::from_utf8(bytes[start..end].to_vec()).unwrap())
                .or_insert(0) += 1;
        }
        start = end;
    }
    identities
}

fn observe_historical(bytes: &[u8]) -> Result<ClassifiedObservation, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "later classification: classified evidence must be UTF-8".to_owned())?;
    Ok(ClassifiedObservation {
        signals: SEARCH_TOKENS
            .iter()
            .filter(|token| text.contains(**token))
            .map(|token| (*token).to_owned())
            .collect(),
        same_line_lower_hex_identity_signal_lines: text
            .lines()
            .filter(|line| has_lower_hex_identity(line))
            .count(),
        identity_literals: lower_hex_identity_inventory(bytes),
        current_compilation_slots: None,
    })
}

fn historical_expected_observation(entry: &PostResetClassified) -> ClassifiedObservation {
    ClassifiedObservation {
        signals: entry.signals.clone(),
        same_line_lower_hex_identity_signal_lines: entry.same_line_lower_hex_identity_signal_lines,
        identity_literals: exact_literal_map(entry),
        current_compilation_slots: None,
    }
}

fn validate_historical_observation(
    entry: &PostResetClassified,
    observed: &ClassifiedObservation,
) -> Result<(), String> {
    let expected = historical_expected_observation(entry);
    if observed.signals != expected.signals
        || observed.same_line_lower_hex_identity_signal_lines
            != expected.same_line_lower_hex_identity_signal_lines
    {
        return Err(format!(
            "later classification: `{}` must carry exactly its recorded search signals",
            entry.path
        ));
    }
    if observed.identity_literals != expected.identity_literals
        || observed.current_compilation_slots.is_some()
    {
        return Err(format!(
            "historical classification: `{}` must carry exactly its sealed identity inventory",
            entry.path
        ));
    }
    Ok(())
}

fn remove_historical_once(
    identities: &mut BTreeMap<String, usize>,
    value: &str,
    path: &str,
) -> Result<(), String> {
    if identities.remove(value) != Some(1) {
        return Err(format!(
            "live classification: `{path}` must delegate one recorded compilation occurrence"
        ));
    }
    Ok(())
}

#[rustfmt::skip]
fn live_expected_observation(
    entry: &PostResetClassified,
    slots: &[(String, String); 2],
) -> Result<ClassifiedObservation, String> {
    let mut expected = historical_expected_observation(entry);
    match entry.path.as_str() {
        PYTHON_MIXED_PATH => {
            remove_historical_once(&mut expected.identity_literals, &slots[0].0, &entry.path)?;
            remove_historical_once(&mut expected.identity_literals, &slots[1].0, &entry.path)?;
        }
        RUST_MIXED_PATH => {
            remove_historical_once(&mut expected.identity_literals, &slots[1].0, &entry.path)?;
        }
        _ => {
            return Err(format!(
                "live classification: `{}` has no current compilation projection",
                entry.path
            ));
        }
    }
    if expected.identity_literals.insert(slots[0].1.clone(), 1).is_some()
        || expected.identity_literals.insert(slots[1].1.clone(), 1).is_some() {
        return Err("live classification: delegated compilation identity collision".to_owned());
    }
    let total = expected.identity_literals.values().sum::<usize>();
    let required = if entry.path == PYTHON_MIXED_PATH { 9 } else { 10 };
    if total != required {
        return Err(format!("live classification: `{}` must contain exactly {required} occurrences", entry.path));
    }
    expected.current_compilation_slots = Some(slots.clone());
    Ok(expected)
}

#[rustfmt::skip]
fn validate_live_observation(
    entries: &[PostResetClassified],
    entry: &PostResetClassified,
    observed: &ClassifiedObservation,
) -> Result<(), String> {
    let slots = observed.current_compilation_slots.as_ref().ok_or_else(||
        format!("live classification: `{}` lacks current compilation authority", entry.path))?;
    validate_current_compilation_slots(entries, slots)?;
    if observed != &live_expected_observation(entry, slots)? {
        return Err(format!("live classification: `{}` differs from its exact delegated identity map", entry.path));
    }
    Ok(())
}

pub(super) fn observe_repository(
    entries: &[PostResetClassified],
    root: &Path,
) -> BTreeMap<String, ClassifiedObservation> {
    let slots = current_compilation_slots(entries).unwrap_or_else(|error| panic!("{error}"));
    entries
        .iter()
        .filter_map(|entry| {
            fs::read(root.join(&entry.path)).ok().map(|bytes| {
                let mut observed = observe_historical(&bytes).unwrap_or_else(|error| {
                    panic!("cannot observe later-classified `{}`: {error}", entry.path)
                });
                if [PYTHON_MIXED_PATH, RUST_MIXED_PATH].contains(&entry.path.as_str()) {
                    observed.current_compilation_slots = Some(slots.clone());
                    validate_live_observation(entries, entry, &observed)
                        .unwrap_or_else(|error| panic!("{error}"));
                } else {
                    validate_historical_observation(entry, &observed)
                        .unwrap_or_else(|error| panic!("{error}"));
                }
                (entry.path.clone(), observed)
            })
        })
        .collect()
}

pub(super) fn validate_pre_reset_absence(
    entries: &[PostResetClassified],
    exists: &BTreeSet<String>,
) -> Result<(), String> {
    if let Some(entry) = entries.iter().find(|entry| exists.contains(&entry.path)) {
        return Err(format!(
            "pre-reset: later-classified path `{}` already exists; its exact classification \
             records a post-reset capability, not historical inventory",
            entry.path
        ));
    }
    Ok(())
}

pub(super) fn validate_post_reset(
    entries: &[PostResetClassified],
    exists: &BTreeSet<String>,
    discovered: &BTreeSet<String>,
    observed: &BTreeMap<String, ClassifiedObservation>,
) -> Result<(), String> {
    let mut live_slots = None;
    for entry in entries.iter().filter(|entry| exists.contains(&entry.path)) {
        if !discovered.contains(&entry.path) {
            return Err(format!(
                "post-reset: later-classified path `{}` exists but no longer carries the signal \
                 for which it received an exact classification",
                entry.path
            ));
        }
        let content = observed.get(&entry.path).ok_or_else(|| {
            format!(
                "post-reset: later-classified path `{}` exists but no exact content observation \
                 was recorded",
                entry.path
            )
        })?;
        if [PYTHON_MIXED_PATH, RUST_MIXED_PATH].contains(&entry.path.as_str()) {
            validate_live_observation(entries, entry, content)?;
            if let Some(previous) = &live_slots
                && content.current_compilation_slots.as_ref() != Some(previous)
            {
                return Err(
                    "live classification: mixed surfaces disagree on current slots".to_owned(),
                );
            }
            live_slots = content.current_compilation_slots.clone();
        } else {
            validate_historical_observation(entry, content)?;
        }
    }
    Ok(())
}

impl Observed {
    pub(super) fn classifying(mut self, entry: &PostResetClassified) -> Self {
        self.exists.insert(entry.path.clone());
        self.discovered.insert(entry.path.clone());
        let observation = if [PYTHON_MIXED_PATH, RUST_MIXED_PATH].contains(&entry.path.as_str()) {
            live_expected_observation(entry, &synthetic_compilation_slots()).unwrap()
        } else {
            historical_expected_observation(entry)
        };
        self.classified.insert(entry.path.clone(), observation);
        self
    }
}

fn assert_live_entry(
    entries: &[PostResetClassified],
    entry: &PostResetClassified,
    slots: &[(String, String); 2],
    root: &Path,
) {
    let bytes = fs::read(root.join(&entry.path)).unwrap_or_else(|error| {
        panic!(
            "classified evidence `{}` must be readable: {error}",
            entry.path
        )
    });
    let mut observed = observe_historical(&bytes).unwrap();
    if [PYTHON_MIXED_PATH, RUST_MIXED_PATH].contains(&entry.path.as_str()) {
        observed.current_compilation_slots = Some(slots.clone());
        validate_live_observation(entries, entry, &observed).unwrap();
    } else {
        validate_historical_observation(entry, &observed).unwrap();
    }
}

#[test]
fn identity_bearing_tests_and_the_source_release_have_truthful_exact_classes() {
    let classification = classification();
    let entries = from_classification(&classification).unwrap();
    validate(&entries, &classification).unwrap();
    let contract = TransitionContract::from_classification();
    assert_eq!(contract.post_reset_classified, entries);

    for entry in &entries {
        assert!(
            !contract.inventory.contains(&entry.path)
                && !contract.retired.contains(&entry.path)
                && !contract.required_post_reset.contains(&entry.path)
                && !contract.preserved_evidence.contains(&entry.path)
                && !contract
                    .post_reset_admitted
                    .iter()
                    .any(|admitted| admitted.path == entry.path)
                && !contract
                    .post_reset_fixture_admitted
                    .iter()
                    .any(|admitted| admitted.path == entry.path)
                && contract
                    .promotion
                    .iter()
                    .all(|row| row.source != entry.path && row.target != entry.path)
                && !ORACLE_FILES.contains(&entry.path.as_str()),
            "later classification of `{}` changes no historical or admission set",
            entry.path
        );
        validate_historical_observation(entry, &historical_expected_observation(entry)).unwrap();
    }

    let slots = current_compilation_slots(&entries).unwrap();
    let root = repository_root();
    for entry in &entries {
        assert_live_entry(&entries, entry, &slots, &root);
    }

    let release_entry = entries
        .iter()
        .find(|entry| entry.class == "source-or-package-identity")
        .unwrap();
    let release_bytes = fs::read(root.join(&release_entry.path)).unwrap();
    let release: Value = serde_json::from_slice(&release_bytes).unwrap();
    assert_eq!(release["schema"], "eqiora.package-release.v1");
    assert_eq!(release["semantic"]["schema"], "eqiora.semantic-content.v1");
    assert_eq!(release["source"]["schema"], "eqiora.source-bundle.v1");
    let stem = Path::new(&release_entry.path)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(stem.len(), 64);
    assert!(
        stem.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

#[test]
fn accepted_identity_and_diagnostic_state_initializes_atomically_under_concurrency() {
    const THREADS: usize = 32;

    let classification = classification();
    let entries = from_classification(&classification).unwrap();
    let cell = OnceLock::new();
    let barrier = std::sync::Barrier::new(THREADS);
    let addresses = std::thread::scope(|scope| {
        let handles = (0..THREADS)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    let accepted =
                        initialize_accepted_state(&cell, &entries, &classification).unwrap();
                    assert_eq!(accepted.identities, entries);
                    assert_eq!(accepted.diagnostic_record, classification);
                    accepted as *const AcceptedClassificationState as usize
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<BTreeSet<_>>()
    });
    assert_eq!(
        addresses.len(),
        1,
        "every thread must observe one atomic cell"
    );
    validate_cached_runtime(&cell, &entries).unwrap();

    let mut mutant = entries.clone();
    mutant[0].class = "non-fixture-search-hit".to_owned();
    assert_eq!(
        validate_cached_runtime(&cell, &mutant),
        Err(format!(
            "later classification: `{}` has an invalid top-level class",
            mutant[0].path
        ))
    );
}

#[test]
fn omission_misclassification_and_path_proximity_are_refused() {
    let classification = classification();
    let entries = from_classification(&classification).unwrap();
    let contract = TransitionContract::from_classification();

    for mask in 0..(1 << entries.len()) {
        let observed = entries.iter().enumerate().fold(
            Observed::maximal_post_reset(&contract),
            |state, (index, entry)| {
                if mask & (1 << index) == 0 {
                    state
                } else {
                    state.classifying(entry)
                }
            },
        );
        assert_eq!(
            classify_transition(&contract, &observed),
            Ok(TransitionState::PostReset),
            "exact classified subset mask {mask:#05b}"
        );
    }
    for entry in &entries {
        refused(
            classify_transition(
                &contract,
                &Observed::maximal_post_reset(&contract).with(&[&entry.path]),
            ),
            "no longer carries the signal",
        );
        refused(
            classify_transition(
                &contract,
                &Observed::exact_pre_reset(&contract).with(&[&entry.path]),
            ),
            "later-classified path",
        );
    }

    for omitted in 0..entries.len() {
        let mut mutant = entries.clone();
        mutant.remove(omitted);
        assert!(validate(&mutant, &classification).is_err());
    }

    let mixed = entries
        .iter()
        .position(|entry| entry.class == "mixed-claim-surface")
        .unwrap();
    let source = entries
        .iter()
        .position(|entry| entry.class == "source-or-package-identity")
        .unwrap();
    let mut wrong_test = entries.clone();
    wrong_test[mixed].class = "non-fixture-search-hit".to_owned();
    wrong_test[mixed].disposition = "migrate-in-place".to_owned();
    assert!(validate(&wrong_test, &classification).is_err());

    let mut wrong_claim = entries.clone();
    wrong_claim[mixed].claims[1].disposition = "delegate".to_owned();
    assert!(validate(&wrong_claim, &classification).is_err());

    let mut wrong_release = entries.clone();
    wrong_release[source].class = "current-owner-assertion".to_owned();
    wrong_release[source].disposition = "migrate".to_owned();
    assert!(validate(&wrong_release, &classification).is_err());

    let mut runtime_mutant = TransitionContract::from_classification();
    runtime_mutant.post_reset_classified[mixed].class = "non-fixture-search-hit".to_owned();
    let present = entries[mixed].clone();
    refused(
        classify_transition(
            &runtime_mutant,
            &Observed::maximal_post_reset(&runtime_mutant).classifying(&present),
        ),
        "invalid top-level class",
    );

    const PROXIMATE: &str = "bindings/python/tests/test_offline_model_package_copy.py";
    refused(
        classify_transition(
            &contract,
            &Observed::maximal_post_reset(&contract).signalling(&[PROXIMATE]),
        ),
        "unclassified new signal-bearing",
    );
    let mut proximity_mutant = entries.clone();
    proximity_mutant[mixed].path = PROXIMATE.to_owned();
    assert!(validate(&proximity_mutant, &classification).is_err());

    let fixture_path =
        classification["search"]["transition"]["post_reset_fixture_admitted"][0]["path"]
            .as_str()
            .unwrap();
    let mut overlap_mutant = entries.clone();
    overlap_mutant[mixed].path = fixture_path.to_owned();
    assert_eq!(
        validate(&overlap_mutant, &classification),
        Err(format!(
            "later classification: `{fixture_path}` overlaps `post_reset_fixture_admitted`; \
             exact classifications and fixture admissions are disjoint"
        ))
    );
}

#[test]
#[rustfmt::skip]
fn historical_and_live_classification_causal_mutants_are_refused() {
    let classification = classification(); let entries = from_classification(&classification).unwrap(); validate(&entries, &classification).unwrap();
    let substitute = |observed: &mut ClassifiedObservation, from: &str, to: String| { let count = observed.identity_literals.remove(from).unwrap(); *observed.identity_literals.entry(to).or_insert(0) += count; };
    for entry in &entries {
        let positive = historical_expected_observation(entry); validate_historical_observation(entry, &positive).unwrap(); let mut mutants = Vec::new();
        let extra = SEARCH_TOKENS.iter().find(|signal| !entry.signals.iter().any(|expected| expected == **signal)).unwrap();
        let mut changed = positive.clone(); changed.signals.push((*extra).to_owned()); mutants.push(changed);
        let mut changed = positive.clone(); changed.same_line_lower_hex_identity_signal_lines += 1; mutants.push(changed);
        let mut changed = positive.clone(); changed.identity_literals.insert("e".repeat(64), 1); mutants.push(changed);
        let mut changed = positive.clone(); let old = entry.identity_literals.first().unwrap().value.clone(); substitute(&mut changed, &old, "f".repeat(64)); mutants.push(changed);
        for mutant in mutants { assert!(validate_historical_observation(entry, &mutant).is_err()); }
    }
    let live_document = |name| serde_json::from_slice::<Value>(crate::DETERMINISTIC.iter().find(|fixture| fixture.name == name).unwrap().live).unwrap();
    let (mut offline, mut typed) = (live_document("offline-model-package"), serde_json::from_slice::<Value>(CURRENT_TYPED_COMPILATION_IDENTITIES).unwrap());
    offline["compilation_digest"] = Value::String("0".repeat(64));
    typed["package_compilation_sha256"] = Value::String("1".repeat(64));
    let slots = compilation_slots_from_authorities(&entries, &offline, &typed).unwrap();
    let refuses = |offline: &Value, typed: &Value| assert!(compilation_slots_from_authorities(&entries, offline, typed).is_err());
    let mut missing = offline.clone(); missing.as_object_mut().unwrap().remove("compilation_digest"); refuses(&missing, &typed);
    let mut wrong = offline.clone(); let value = wrong.as_object_mut().unwrap().remove("compilation_digest").unwrap();
    wrong.as_object_mut().unwrap().insert("compilation_sha256".to_owned(), value); refuses(&wrong, &typed);
    for value in [Value::Null, Value::String("A".repeat(64)), Value::String("g".repeat(64)), Value::String("0".repeat(63)),
        Value::String(slots[0].0.clone()), Value::String(entries[0].identity_literals[0].value.clone())] {
        let mut mutant = offline.clone(); mutant["compilation_digest"] = value; refuses(&mutant, &typed);
    }
    let mut collapsed = typed.clone(); collapsed["package_compilation_sha256"] = Value::String(slots[0].1.clone()); refuses(&offline, &collapsed);
    for entry in entries.iter().filter(|entry| entry.class == "mixed-claim-surface") {
        let positive = live_expected_observation(entry, &slots).unwrap(); validate_live_observation(&entries, entry, &positive).unwrap(); let mut mutants = Vec::new();
        for (old, current, foreign) in [(&slots[0].0, &slots[0].1, "2".repeat(64)), (&slots[1].0, &slots[1].1, "3".repeat(64))] {
            let mut stale = positive.clone(); substitute(&mut stale, current, old.clone()); mutants.push(stale);
            let mut assertion_only = positive.clone(); substitute(&mut assertion_only, current, foreign); mutants.push(assertion_only);
            let mut omitted = positive.clone(); omitted.identity_literals.remove(current); mutants.push(omitted);
        }
        let mut added = positive.clone(); added.identity_literals.insert("4".repeat(64), 1); mutants.push(added);
        let mut duplicate = positive.clone(); *duplicate.identity_literals.get_mut(&slots[0].1).unwrap() += 1; mutants.push(duplicate);
        let mut collapsed = positive.clone(); substitute(&mut collapsed, &slots[1].1, slots[0].1.clone()); mutants.push(collapsed);
        for (role, foreign) in [(CURRENT_ROLE, "5".repeat(64)), (IMMUTABLE_ROLE, "6".repeat(64))] {
            let protected = entry.identity_literals.iter().find(|literal| literal.role == role && ![&slots[0].0, &slots[1].0].contains(&&literal.value)).unwrap();
            let mut drift = positive.clone(); substitute(&mut drift, &protected.value, foreign); mutants.push(drift);
        }
        let mut added_signal = positive.clone(); added_signal.signals.push("unrecorded-signal".to_owned()); mutants.push(added_signal);
        if !positive.signals.is_empty() {
            let mut removed = positive.clone(); removed.signals.pop(); mutants.push(removed);
            let mut reordered = positive.clone(); reordered.signals.reverse(); mutants.push(reordered);
        }
        let mut line_drift = positive.clone(); line_drift.same_line_lower_hex_identity_signal_lines += 1; mutants.push(line_drift);
        if entry.path == RUST_MIXED_PATH {
            for count in [0, 2, 3] {
                let mut retention = positive.clone(); if count == 0 { retention.identity_literals.remove(&slots[0].0); }
                else { retention.identity_literals.insert(slots[0].0.clone(), count); } mutants.push(retention);
            }
            let mut substituted = positive.clone(); substitute(&mut substituted, &slots[0].0, "7".repeat(64)); mutants.push(substituted);
        }
        for mutant in mutants { assert!(validate_live_observation(&entries, entry, &mutant).is_err()); }
    }
    let mixed = entries.iter().filter(|entry| entry.class == "mixed-claim-surface").collect::<Vec<_>>();
    let other = [(slots[0].0.clone(), "8".repeat(64)), (slots[1].0.clone(), "9".repeat(64))];
    validate_current_compilation_slots(&entries, &other).unwrap();
    let observed = BTreeMap::from([(mixed[0].path.clone(), live_expected_observation(mixed[0], &slots).unwrap()),
        (mixed[1].path.clone(), live_expected_observation(mixed[1], &other).unwrap())]);
    let present = observed.keys().cloned().collect::<BTreeSet<_>>();
    assert!(validate_post_reset(&entries, &present, &present, &observed).is_err());
}
