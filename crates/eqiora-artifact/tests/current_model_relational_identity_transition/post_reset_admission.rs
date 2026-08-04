//! Exact-path post-reset admission regressions for the transition contract.

use super::*;

#[derive(Clone)]
struct ExpectedAdmission {
    path: &'static str,
    class: &'static str,
    owner: &'static str,
    note: String,
    source: String,
}

fn rfc85_identity_free() -> Vec<ExpectedAdmission> {
    vec![
        ExpectedAdmission {
            path: "rfcs/0085-standalone-prescribed-dynamic-solid-artifacts.md",
            class: "current-owner-assertion",
            owner: "RFC 0085 standalone prescribed dynamic-solid artifact contract",
            note: "the accepted RFC owns the exact successor path split and names the current \
                   Model schema and digest edge without freezing any Model-derived identity \
                   literal. It is neither a reset product nor a required transition path."
                .to_owned(),
            source: format!(
                "const SCHEMA: &str = \"{}8\";\nconst EDGE: &str = \"{}\";\n",
                SEARCH_TOKENS[0], SEARCH_TOKENS[2]
            ),
        },
        ExpectedAdmission {
            path: "crates/eqiora-artifact/src/prescribed_dynamic_solid_realization.rs",
            class: "non-fixture-search-hit",
            owner: "the prescribed dynamic-solid Realization artifact owner",
            note: format!(
                "`{}` is the exact relational edge to the caller-owned current Model artifact; \
                 the module freezes no Model-derived identity literal and is neither a reset \
                 product nor a required transition path.",
                SEARCH_TOKENS[2]
            ),
            source: format!("struct Wire {{ {}: String }}\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "crates/eqiora/tests/prescribed_dynamic_solid_state_run_3d.rs",
            class: "non-fixture-search-hit",
            owner: "artifacts.prescribed-dynamic-solid-state-run-3d independent exact-artifact \
                    oracle",
            note: format!(
                "the independent exact-artifact oracle names `{}` as a canonical lineage key \
                 and mutation target but derives all expected identities from committed bytes; \
                 the Rust source freezes no Model-derived identity literal and is neither a \
                 reset product nor a required transition path.",
                SEARCH_TOKENS[2]
            ),
            source: format!(
                "fn stale_lineage() {{ let key = \"{}\"; assert!(!key.is_empty()); }}\n",
                SEARCH_TOKENS[2]
            ),
        },
        ExpectedAdmission {
            path: "verify/artifacts/prescribed-dynamic-solid-state-run-3d/references/\
                   derive_prescribed_dynamic_solid_state_run_3d.py",
            class: "non-fixture-search-hit",
            owner: "artifacts.prescribed-dynamic-solid-state-run-3d independent exact-artifact \
                    oracle",
            note: format!(
                "the independent standard-library derivation names the current Model schema and \
                 `{}` while re-deriving expected identities from canonical bytes; the script \
                 freezes no Model-derived identity literal and is neither a reset product nor a \
                 required transition path.",
                SEARCH_TOKENS[2]
            ),
            source: format!(
                "SCHEMA = \"{}8\"\nEDGE = \"{}\"\n",
                SEARCH_TOKENS[0], SEARCH_TOKENS[2]
            ),
        },
    ]
}

fn rfc85_fixtures() -> Vec<ExpectedAdmission> {
    let owner = "artifacts.prescribed-dynamic-solid-state-run-3d independent exact-artifact \
                 oracle";
    let mut rows = vec![ExpectedAdmission {
        path: "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/model.json",
        class: "delegated-current-owner-evidence",
        owner,
        note: "the exact current Model canonical-byte fixture names its schema and freezes no \
               same-line Model-derived lower-hex-64 identity; its bytes remain owned by the \
               standalone lineage oracle."
            .to_owned(),
        source: format!("{{\"schema\":\"{}8\"}}\n", SEARCH_TOKENS[0]),
    }];
    let named = [
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
             geometry-identity.json",
            "the exact Geometry identity fixture carries one same-line `{}` literal linking it \
             to the current Model; admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/realization.json",
            "the exact standalone-solid Realization fixture carries one same-line `{}` literal; \
             admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
             prior-displacement-snapshot.json",
            "the exact prior-displacement FieldSnapshot fixture carries one same-line `{}` \
             literal; admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
             prior-velocity-snapshot.json",
            "the exact prior-velocity FieldSnapshot fixture carries one same-line `{}` literal; \
             admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
             accepted-displacement-snapshot.json",
            "the exact accepted-displacement FieldSnapshot fixture carries one same-line `{}` \
             literal; admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
             accepted-velocity-snapshot.json",
            "the exact accepted-velocity FieldSnapshot fixture carries one same-line `{}` \
             literal; admission owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/prior-state.json",
            "the exact prior State fixture carries one same-line `{}` literal; admission owns \
             only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/accepted-state.json",
            "the exact accepted-next State fixture carries one same-line `{}` literal; admission \
             owns only this path, signal, and literal count.",
        ),
        (
            "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/run.json",
            "the exact Run fixture carries one same-line `{}` literal; admission owns only this \
             path, signal, and literal count.",
        ),
    ];
    for (path, note) in named {
        rows.push(ExpectedAdmission {
            path,
            class: "delegated-current-owner-evidence",
            owner,
            note: note.replace("{}", SEARCH_TOKENS[2]),
            source: format!("{{\"{}\":\"{}\"}}\n", SEARCH_TOKENS[2], "e".repeat(64)),
        });
    }
    rows
}

fn row_matches(row: &PostResetAdmitted, expected: &ExpectedAdmission) -> bool {
    let (signals, literals) = observe_admitted(expected.source.as_bytes());
    row.path == expected.path
        && row.class == expected.class
        && row.signals == signals
        && row.identity_literals == literals
        && row.owner == expected.owner
        && row.note == expected.note
}

fn all_identity_free_sources() -> Vec<(&'static str, String)> {
    let mut sources = ADMITTED_AS_RECORDED
        .iter()
        .map(|(path, source)| (*path, (*source).to_owned()))
        .collect::<Vec<_>>();
    sources.extend(
        rfc85_identity_free()
            .into_iter()
            .map(|row| (row.path, row.source)),
    );
    sources
}

fn admitting_all(mut state: Observed, sources: &[(&str, String)]) -> Observed {
    for (path, source) in sources {
        state = state.admitting(path, source);
    }
    state
}

/// Admission adds two permissions and nothing else: the historical record of
/// the reset is what it was, and no admitted path joins a single set inside it.
#[test]
fn later_classified_paths_are_admitted_by_exact_path_and_join_no_frozen_set() {
    let contract = TransitionContract::from_classification();
    let classification = classification();
    let transition = &classification["search"]["transition"];
    let classes = classification["classes"].as_object().unwrap();
    let identity_free = contract
        .post_reset_admitted
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let fixtures = contract
        .post_reset_fixture_admitted
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let expected_identity_free = all_identity_free_sources()
        .into_iter()
        .map(|(path, _)| path.to_owned())
        .collect::<BTreeSet<_>>();
    let expected_fixtures = rfc85_fixtures()
        .into_iter()
        .map(|row| row.path.to_owned())
        .collect::<BTreeSet<_>>();

    assert_eq!(identity_free, expected_identity_free);
    assert_eq!(fixtures, expected_fixtures);
    assert!(identity_free.is_disjoint(&fixtures));
    assert_eq!(
        identity_free.len() as u64,
        transition["post_reset_admitted_path_count"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        fixtures.len() as u64,
        transition["post_reset_fixture_admitted_path_count"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(identity_free.len(), 9);
    assert_eq!(fixtures.len(), 10);

    // Every historical count is still its own. Listing an admitted path in the
    // inventory would claim it existed before the reset; listing one in
    // `required_post_reset` would make a later capability a transition gate.
    assert_eq!(contract.inventory.len(), 338);
    assert_eq!(contract.retired.len(), 44);
    assert_eq!(contract.preserved().len(), 304);
    assert_eq!(contract.required_post_reset.len(), 13);
    assert_eq!(contract.preserved_evidence.len(), 40);
    for path in identity_free.union(&fixtures) {
        assert!(
            !contract.inventory.contains(path)
                && !contract.retired.contains(path)
                && !contract.required_post_reset.contains(path)
                && !contract.preserved_evidence.contains(path)
                && !ORACLE_FILES.contains(&path.as_str())
                && contract
                    .promotion
                    .iter()
                    .all(|row| row.source != *path && row.target != *path),
            "admitted `{path}` joins no historical frozen, promotion, required, or oracle set"
        );
        assert!(
            !path.is_empty()
                && !path.ends_with('/')
                && !path.contains('*')
                && !path.contains('?')
                && !path.contains("..")
                && !path.starts_with('/'),
            "admission `{path}` must name one exact file"
        );
    }

    for entry in contract
        .post_reset_admitted
        .iter()
        .chain(&contract.post_reset_fixture_admitted)
    {
        assert!(classes.contains_key(&entry.class));
        assert!(!entry.owner.is_empty() && !entry.note.is_empty());
        let places = entry
            .signals
            .iter()
            .map(|signal| SEARCH_TOKENS.iter().position(|token| token == signal))
            .collect::<Option<Vec<_>>>();
        assert!(
            !entry.signals.is_empty()
                && places.is_some_and(|at| at.windows(2).all(|pair| pair[0] < pair[1]))
        );
    }
    assert!(
        contract
            .post_reset_admitted
            .iter()
            .all(|entry| entry.identity_literals == 0),
        "the identity-free permission remains zero-identity without exception"
    );
}

/// The fourteen RFC 0085 rows are frozen field by field. Mutating each field
/// demonstrates that omission, substitution, or metadata drift cannot be
/// hidden behind the correct path count.
#[test]
fn rfc85_rows_reject_wrong_path_signal_count_class_owner_or_note() {
    let contract = TransitionContract::from_classification();
    for (rows, expected) in [
        (
            contract.post_reset_admitted.as_slice(),
            rfc85_identity_free(),
        ),
        (
            contract.post_reset_fixture_admitted.as_slice(),
            rfc85_fixtures(),
        ),
    ] {
        for expected in expected {
            let row = rows
                .iter()
                .find(|row| row.path == expected.path)
                .unwrap_or_else(|| panic!("omitted RFC 0085 admission `{}`", expected.path));
            assert!(row_matches(row, &expected));

            let mut mutants = Vec::new();
            let mut mutant = row.clone();
            mutant.path.push_str(".substituted");
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.signals.push(SEARCH_TOKENS[4].to_owned());
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.identity_literals += 1;
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.class.push_str("-wrong");
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.owner.push_str(" (wrong)");
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.note.push_str(" Drift.");
            mutants.push(mutant);
            assert!(
                mutants.iter().all(|row| !row_matches(row, &expected)),
                "every frozen field of `{}` must reject its mutant",
                expected.path
            );
        }
    }
}

/// Both permissions are independently optional. The identity-free permission
/// stays zero-identity; the fixture permission keeps each exact literal count.
#[test]
fn both_admission_permissions_are_optional_exact_and_fail_closed() {
    let contract = TransitionContract::from_classification();
    let classify = |observed: Observed| classify_transition(&contract, &observed);
    let reset = || Observed::maximal_post_reset(&contract);
    let identity_free = all_identity_free_sources();
    let fixtures = rfc85_fixtures();
    let fixture_sources = fixtures
        .iter()
        .map(|row| (row.path, row.source.clone()))
        .collect::<Vec<_>>();

    // Optional independently: every subset, including none and all, remains a
    // complete post-reset state for each permission.
    for sources in [&identity_free, &fixture_sources] {
        for mask in 0..(1usize << sources.len()) {
            let observed =
                sources
                    .iter()
                    .enumerate()
                    .fold(reset(), |state, (index, (path, source))| {
                        if mask & (1 << index) == 0 {
                            state
                        } else {
                            state.admitting(path, source)
                        }
                    });
            assert_eq!(classify(observed), Ok(TransitionState::PostReset));
        }
    }

    // The existing identity-free predicate still rejects no signal, an extra
    // signal, and a pinned identity through the same byte observer.
    let first_identity_free = &identity_free[0];
    let without_signal = "pub fn trajectory(handle: &Handle) { handle.open(); }\n";
    let with_extra_signal = format!(
        "fn trajectory({}: &str, {}: &str) {{}}\n",
        SEARCH_TOKENS[4], SEARCH_TOKENS[2]
    );
    for source in [without_signal, &with_extra_signal] {
        refused(
            classify(reset().admitting(first_identity_free.0, source)),
            "must carry exactly its recorded search signal",
        );
    }
    let pinned = format!(
        "{}const PINNED_MODEL: &str = \"{}\";\n",
        first_identity_free.1,
        "e".repeat(64)
    );
    refused(
        classify(reset().admitting(first_identity_free.0, &pinned)),
        "exact literal counts never relax",
    );

    // The fixture permission is separate: its zero-count Model file and every
    // one-count relational fixture reject both a changed signal list and a
    // changed same-line literal count.
    let model = &fixtures[0];
    let model_with_edge = format!(
        "{}{{\"{}\":\"{}\"}}\n",
        model.source,
        SEARCH_TOKENS[2],
        "e".repeat(64)
    );
    refused(
        classify(reset().admitting(model.path, &model_with_edge)),
        "must carry exactly its recorded search signal",
    );
    let relational = &fixtures[1];
    let no_literal = format!("{{\"{}\":\"not-a-digest\"}}\n", SEARCH_TOKENS[2]);
    let two_literals = format!("{}{}", relational.source, relational.source);
    for source in [&no_literal, &two_literals] {
        refused(
            classify(reset().admitting(relational.path, source)),
            "exact literal counts never relax",
        );
    }

    // Absent before reset is shared, but membership is not inferred between
    // the two permissions.
    for (path, _) in identity_free.iter().chain(&fixture_sources) {
        refused(
            classify(Observed::exact_pre_reset(&contract).with(&[path])),
            "already exists",
        );
    }

    assert_eq!(
        classify(admitting_all(reset(), &identity_free)),
        Ok(TransitionState::PostReset)
    );
    assert_eq!(
        classify(admitting_all(reset(), &fixture_sources)),
        Ok(TransitionState::PostReset)
    );
}

/// Exact means no sibling, descendant, alternate extension, nearby RFC, or
/// unlisted expected artifact inherits either permission.
#[test]
fn no_glob_directory_suffix_or_proximity_admission_exists() {
    let contract = TransitionContract::from_classification();
    let all = all_identity_free_sources();
    let reset = || admitting_all(Observed::maximal_post_reset(&contract), &all);
    for path in [
        "rfcs/0085-standalone-prescribed-dynamic-solid-artifacts.notes.md",
        "rfcs/0086-standalone-prescribed-dynamic-solid-artifacts.md",
        "crates/eqiora-artifact/src/prescribed_dynamic_solid_realization_test.rs",
        "crates/eqiora-artifact/src/prescribed_dynamic_solid_realization/mod.rs",
        "crates/eqiora/tests/prescribed_dynamic_solid_state_run_2d.rs",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/references/derive.py",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/model-copy.json",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/geometry.json",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/checkpoint.json",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/run.json.bak",
        "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/nested/run.json",
    ] {
        refused(
            classify_transition(&contract, &reset().signalling(&[path])),
            "unclassified new signal-bearing",
        );
    }
}
