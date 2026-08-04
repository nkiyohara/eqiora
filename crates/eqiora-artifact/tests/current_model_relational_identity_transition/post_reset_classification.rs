//! Full classifications for later identity-bearing evidence paths.

use super::*;
use std::sync::OnceLock;

const OWNER: &str = "interfaces.python-offline-model-package";
const CURRENT_ROLE: &str = "current-model-or-package-artifact";
const IMMUTABLE_ROLE: &str = "source-or-resolution-identity";

static ACCEPTED_CLASSIFICATION: OnceLock<Vec<PostResetClassified>> = OnceLock::new();
static ACCEPTED_RECORD: OnceLock<Value> = OnceLock::new();

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
    if let Some(accepted) = ACCEPTED_CLASSIFICATION.get() {
        if accepted != &entries {
            return Err("later classification: frozen rows changed within one run".to_owned());
        }
    } else {
        ACCEPTED_CLASSIFICATION
            .set(entries.clone())
            .map_err(|_| "later classification: cannot freeze accepted rows".to_owned())?;
        ACCEPTED_RECORD
            .set(classification.clone())
            .map_err(|_| "later classification: cannot freeze accepted record".to_owned())?;
    }
    Ok(entries)
}

pub(super) fn validate_runtime(entries: &[PostResetClassified]) -> Result<(), String> {
    let accepted = ACCEPTED_CLASSIFICATION
        .get()
        .ok_or_else(|| "later classification: accepted rows were not initialized".to_owned())?;
    if entries != accepted {
        let record = ACCEPTED_RECORD.get().ok_or_else(|| {
            "later classification: accepted record was not initialized".to_owned()
        })?;
        validate(entries, record)?;
        return Err("later classification: runtime table differs from accepted rows".to_owned());
    }
    Ok(())
}

fn transition_current_identities() -> BTreeSet<String> {
    let transition = crate::transition();
    ["offline-model-package", "typed-execution-lineage"]
        .into_iter()
        .flat_map(|name| {
            let fixture = crate::entry(&transition, "deterministic", name);
            let model = fixture[concat!("model_", "digest")]
                .as_str()
                .unwrap()
                .to_owned();
            let compilation = fixture["edges"]
                .as_array()
                .unwrap()
                .iter()
                .find(|edge| edge["artifact"] == "compilation.json")
                .and_then(|edge| edge["digest"].as_str())
                .unwrap()
                .to_owned();
            [model, compilation]
        })
        .collect()
}

fn all_transition_current_artifact_identities() -> BTreeSet<String> {
    crate::transition()["deterministic"]
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
        .collect()
}

fn exact_literal_map(entry: &PostResetClassified) -> BTreeMap<String, usize> {
    entry
        .identity_literals
        .iter()
        .map(|literal| (literal.value.clone(), literal.occurrences))
        .collect()
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
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    if paths.len() != entries.len() || entries.len() != frozen_counts.0 {
        return Err(
            "later classification: exactly three unique exact paths are required".to_owned(),
        );
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

pub(super) fn observe(bytes: &[u8]) -> Result<ClassifiedObservation, String> {
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
    })
}

pub(super) fn expected_observation(entry: &PostResetClassified) -> ClassifiedObservation {
    ClassifiedObservation {
        signals: entry.signals.clone(),
        same_line_lower_hex_identity_signal_lines: entry.same_line_lower_hex_identity_signal_lines,
        identity_literals: exact_literal_map(entry),
    }
}

pub(super) fn validate_observation(
    entry: &PostResetClassified,
    observed: &ClassifiedObservation,
) -> Result<(), String> {
    let expected = expected_observation(entry);
    if observed.signals != expected.signals
        || observed.same_line_lower_hex_identity_signal_lines
            != expected.same_line_lower_hex_identity_signal_lines
    {
        return Err(format!(
            "later classification: `{}` must carry exactly its recorded search signals",
            entry.path
        ));
    }
    if observed.identity_literals != expected.identity_literals {
        return Err(format!(
            "later classification: `{}` must carry exactly its recorded identity literal inventory",
            entry.path
        ));
    }
    Ok(())
}

pub(super) fn observe_repository(
    entries: &[PostResetClassified],
    root: &Path,
) -> BTreeMap<String, ClassifiedObservation> {
    entries
        .iter()
        .filter_map(|entry| {
            fs::read(root.join(&entry.path)).ok().map(|bytes| {
                let observed = observe(&bytes).unwrap_or_else(|error| {
                    panic!("cannot observe later-classified `{}`: {error}", entry.path)
                });
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
        validate_observation(entry, content)?;
    }
    Ok(())
}

impl Observed {
    pub(super) fn classifying(mut self, entry: &PostResetClassified) -> Self {
        self.exists.insert(entry.path.clone());
        self.discovered.insert(entry.path.clone());
        self.classified
            .insert(entry.path.clone(), expected_observation(entry));
        self
    }
}

fn assert_live_entry(entry: &PostResetClassified, root: &Path) {
    let bytes = fs::read(root.join(&entry.path)).unwrap_or_else(|error| {
        panic!(
            "classified evidence `{}` must be readable: {error}",
            entry.path
        )
    });
    validate_observation(entry, &observe(&bytes).unwrap()).unwrap();
}

#[test]
fn identity_bearing_tests_and_the_source_release_have_truthful_exact_classes() {
    let classification = classification();
    let entries = from_classification(&classification).unwrap();
    validate(&entries, &classification).unwrap();
    let contract = TransitionContract::from_classification();
    assert_eq!(contract.post_reset_classified, entries);
    let root = repository_root();

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
                && contract
                    .promotion
                    .iter()
                    .all(|row| row.source != entry.path && row.target != entry.path)
                && !ORACLE_FILES.contains(&entry.path.as_str()),
            "later classification of `{}` changes no historical or admission set",
            entry.path
        );
        assert_live_entry(entry, &root);
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
}

#[test]
fn extra_signals_and_fifth_or_different_current_identities_are_refused() {
    let classification = classification();
    let entries = from_classification(&classification).unwrap();
    let root = repository_root();
    let current = transition_current_identities();
    let foreign = all_transition_current_artifact_identities()
        .into_iter()
        .find(|identity| !current.contains(identity))
        .unwrap()
        .to_owned();

    for entry in &entries {
        let bytes = fs::read(root.join(&entry.path)).unwrap();
        let extra_signal = SEARCH_TOKENS
            .iter()
            .find(|signal| !entry.signals.iter().any(|expected| expected == **signal))
            .unwrap();
        let mut grown = bytes.clone();
        grown.extend_from_slice(format!("\n{extra_signal}\n").as_bytes());
        assert!(validate_observation(entry, &observe(&grown).unwrap()).is_err());

        if entry.class == "mixed-claim-surface" {
            let mut fifth = bytes.clone();
            fifth.extend_from_slice(format!("\n{}\n", "e".repeat(64)).as_bytes());
            assert!(validate_observation(entry, &observe(&fifth).unwrap()).is_err());

            let old = entry
                .identity_literals
                .iter()
                .find(|literal| literal.role == CURRENT_ROLE)
                .unwrap();
            let source = std::str::from_utf8(&bytes).unwrap();
            let changed = source.replacen(&old.value, &foreign, 1);
            assert_ne!(changed, source);
            assert!(validate_observation(entry, &observe(changed.as_bytes()).unwrap()).is_err());
        }
    }

    let release = entries
        .iter()
        .find(|entry| entry.class == "source-or-package-identity")
        .unwrap();
    let mut release_value: Value =
        serde_json::from_slice(&fs::read(root.join(&release.path)).unwrap()).unwrap();
    release_value.as_object_mut().unwrap().insert(
        concat!("model_", "sha256").to_owned(),
        Value::String(current.iter().next().unwrap().clone()),
    );
    let mutant = serde_json::to_vec(&release_value).unwrap();
    assert!(validate_observation(release, &observe(&mutant).unwrap()).is_err());
}
