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

const IDENTITY_FREE_PATH_ORDER: [&str; 17] = [
    "crates/eqiora-python/src/trajectory.rs",
    "bindings/python/python/eqiora/trajectory.pyi",
    "crates/eqiora-python/src/result.rs",
    "crates/eqiora-artifact/src/cartesian_q1_field_snapshot.rs",
    "crates/eqiora/tests/generated_cartesian_q1_spatial_output.rs",
    "rfcs/0085-standalone-prescribed-dynamic-solid-artifacts.md",
    "crates/eqiora-artifact/src/prescribed_dynamic_solid_realization.rs",
    "crates/eqiora/tests/prescribed_dynamic_solid_state_run_3d.rs",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/references/\
     derive_prescribed_dynamic_solid_state_run_3d.py",
    "crates/eqiora-artifact/src/prescribed_dynamic_solid_provider_occurrence.rs",
    "crates/eqiora-api/src/prescribed_dynamic_solid/provider/protocol/control.rs",
    "crates/eqiora/tests/prescribed_dynamic_solid_subprocess_provider_3d.rs",
    "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/references/\
     derive_provider_occurrence.py",
    "examples/python/prescribed_dynamic_solid_provider.py",
    "docs/external-boundary-provider.md",
    "crates/eqiora-numerics/src/canonical_stokes/\
     navier_stokes_geometry_realization/tests.rs",
    "crates/eqiora/src/bin/eqiora-mcp/tool.rs",
];

const FIXTURE_PATH_ORDER: [&str; 14] = [
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/model.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
     geometry-identity.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/realization.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
     prior-displacement-snapshot.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
     prior-velocity-snapshot.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
     accepted-displacement-snapshot.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/\
     accepted-velocity-snapshot.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/prior-state.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/accepted-state.json",
    "verify/artifacts/prescribed-dynamic-solid-state-run-3d/expected/run.json",
    "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/\
     provider-occurrence.json",
    "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/run.json",
    "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/\
     d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.json",
    "verify/interfaces/mcp-stdio-compile-check/expected/tool-definition.json",
];

fn rows_have_exact_unique_path_order(rows: &[PostResetAdmitted], expected: &[&str]) -> bool {
    let paths = rows.iter().map(|row| row.path.as_str()).collect::<Vec<_>>();
    rows.len() == expected.len()
        && paths.iter().copied().eq(expected.iter().copied())
        && paths.iter().copied().collect::<BTreeSet<_>>().len() == rows.len()
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

fn issue118_identity_free() -> Vec<ExpectedAdmission> {
    vec![
        ExpectedAdmission {
            path: "crates/eqiora-artifact/src/prescribed_dynamic_solid_provider_occurrence.rs",
            class: "non-fixture-search-hit",
            owner: "the prescribed dynamic-solid provider-occurrence artifact owner",
            note: format!(
                "the role-preserving provider-occurrence artifact names `{}` as its exact \
                 current-Model edge but freezes no Model-derived identity literal; this \
                 admission owns only the future path and signal shape.",
                SEARCH_TOKENS[2]
            ),
            source: format!("struct Wire {{ {}: String }}\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "crates/eqiora-api/src/prescribed_dynamic_solid/provider/protocol/control.rs",
            class: "non-fixture-search-hit",
            owner: "the connected-subprocess provider protocol control owner",
            note: format!(
                "the closed protocol control DTOs name `{}` as a caller-owned binding field but \
                 freeze no Model-derived identity literal; this admission owns only the future \
                 path and signal shape.",
                SEARCH_TOKENS[2]
            ),
            source: format!("struct Bind {{ {}: String }}\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "crates/eqiora/tests/prescribed_dynamic_solid_subprocess_provider_3d.rs",
            class: "non-fixture-search-hit",
            owner: "interfaces.prescribed-dynamic-solid-subprocess-provider-3d independent oracle",
            note: format!(
                "the independent protocol and exact-artifact oracle names `{}` as a canonical \
                 field and mutant but derives expected identities instead of pinning a \
                 Model-derived literal.",
                SEARCH_TOKENS[2]
            ),
            source: format!("let key = \"{}\";\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/references/\
                   derive_provider_occurrence.py",
            class: "non-fixture-search-hit",
            owner: "interfaces.prescribed-dynamic-solid-subprocess-provider-3d independent oracle",
            note: format!(
                "the independent standard-library derivation names `{}` while deriving every \
                 identity from accepted inputs and exact protocol bytes; it freezes no \
                 Model-derived identity literal.",
                SEARCH_TOKENS[2]
            ),
            source: format!("EDGE = \"{}\"\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "examples/python/prescribed_dynamic_solid_provider.py",
            class: "non-fixture-search-hit",
            owner: "the prescribed dynamic-solid affine Python provider",
            note: format!(
                "the positive provider reads `{}` from the exact bind payload but freezes no \
                 Model-derived identity literal; this permission grants no Python path family \
                 or sibling admission.",
                SEARCH_TOKENS[2]
            ),
            source: format!("edge = request[\"{}\"]\n", SEARCH_TOKENS[2]),
        },
        ExpectedAdmission {
            path: "docs/external-boundary-provider.md",
            class: "current-owner-assertion",
            owner: "interfaces.prescribed-dynamic-solid-subprocess-provider-3d current-owner documentation",
            note: format!(
                "the current-owner documentation names `{}` as the occurrence's current-Model \
                 lineage edge without freezing any Model-derived identity literal; \
                 documentation proximity grants no admission.",
                SEARCH_TOKENS[2]
            ),
            source: format!("The lineage edge is `{}`.\n", SEARCH_TOKENS[2]),
        },
    ]
}

fn non_box_transient_identity_free() -> Vec<ExpectedAdmission> {
    vec![ExpectedAdmission {
        path: "crates/eqiora-numerics/src/canonical_stokes/\
               navier_stokes_geometry_realization/tests.rs",
        class: "non-fixture-search-hit",
        owner: "fluid.non-box-transient-navier-stokes-2d-private independent oracle",
        note: format!(
            "the private non-box transient oracle names `{}` while proving that the compiled \
             program remains bound to the accepted source digest; it derives that digest from \
             live source bytes, freezes no Model-derived identity literal, and this permission \
             grants no sibling module or directory admission.",
            SEARCH_TOKENS[4]
        ),
        source: format!(
            "fn assert_source_binding({}: [u8; 32]) {{ assert_ne!({}, [0; 32]); }}\n",
            SEARCH_TOKENS[4], SEARCH_TOKENS[4]
        ),
    }]
}

fn issue118_fixture_source(count: usize) -> String {
    let identities = (1..count)
        .map(|_| format!("\"{}\"", "e".repeat(64)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"{}\":\"{}\",\"other_model_identities\":[{}]}}\n",
        SEARCH_TOKENS[2],
        "e".repeat(64),
        identities
    )
}

fn issue118_fixtures() -> Vec<ExpectedAdmission> {
    let owner = "interfaces.prescribed-dynamic-solid-subprocess-provider-3d independent oracle";
    vec![
        ExpectedAdmission {
            path: "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/\
                   provider-occurrence.json",
            class: "delegated-current-owner-evidence",
            owner,
            note: format!(
                "the exact provider-occurrence fixture carries `{}` and thirteen same-line \
                 Model-derived lower-hex-64 literal occurrences; admission owns only this path, \
                 signal, and literal count.",
                SEARCH_TOKENS[2]
            ),
            source: issue118_fixture_source(13),
        },
        ExpectedAdmission {
            path: "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/\
                   run.json",
            class: "delegated-current-owner-evidence",
            owner,
            note: format!(
                "the exact two-output Run fixture carries `{}` and four same-line Model-derived \
                 lower-hex-64 literal occurrences; admission owns only this path, signal, and \
                 literal count.",
                SEARCH_TOKENS[2]
            ),
            source: issue118_fixture_source(4),
        },
    ]
}

fn issue79_identity_free() -> ExpectedAdmission {
    ExpectedAdmission {
        path: "crates/eqiora/src/bin/eqiora-mcp/tool.rs",
        class: "non-fixture-search-hit",
        owner: "eqiora-mcp private tool adapter",
        note: "the private MCP tool names the current Model and Transaction schemas without \
               freezing a Model-derived identity; admission owns only this exact production \
               path, ordered signal list, and zero-literal count."
            .to_owned(),
        source: format!(
            "const MODEL_SCHEMA: &str = \"{}8\";\nconst TRANSACTION_SCHEMA: &str = \"{}8\";\n",
            SEARCH_TOKENS[0], SEARCH_TOKENS[1]
        ),
    }
}

fn issue79_fixture() -> ExpectedAdmission {
    ExpectedAdmission {
        path: "verify/interfaces/mcp-stdio-compile-check/expected/tool-definition.json",
        class: "delegated-current-owner-evidence",
        owner: "interfaces.mcp-stdio-compile-check independent oracle",
        note: "the exact tool-definition snapshot delegates the current Model and Transaction \
               schema names to their existing owner and freezes no Model-derived identity; \
               admission owns only this exact fixture path, ordered signal list, and \
               zero-literal count."
            .to_owned(),
        source: format!(
            "{{\"modelSchema\":\"{}8\",\"transactionSchema\":\"{}8\"}}\n",
            SEARCH_TOKENS[0], SEARCH_TOKENS[1]
        ),
    }
}

fn package_conformance_fixture_source(count: usize) -> String {
    let identities = (0..count)
        .map(|_| format!("\"{}\"", "e".repeat(64)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"model_source_identities\":[{identities}]}}\n")
}

fn package_conformance_fixture() -> ExpectedAdmission {
    ExpectedAdmission {
        path: "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/\
               d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.json",
        class: "source-or-package-identity",
        owner: "interfaces.python-package-conformance independent oracle",
        note: "admission owns only the exact path/search shape/count; release bytes and \
               source/package identities remain owned by that oracle; it creates no raw \
               release-wire predicate"
            .to_owned(),
        source: package_conformance_fixture_source(2),
    }
}

fn all_fixtures() -> Vec<ExpectedAdmission> {
    let mut rows = rfc85_fixtures();
    rows.extend(issue118_fixtures());
    rows.push(package_conformance_fixture());
    rows.push(issue79_fixture());
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
    sources.extend(
        issue118_identity_free()
            .into_iter()
            .map(|row| (row.path, row.source)),
    );
    sources.extend(
        non_box_transient_identity_free()
            .into_iter()
            .map(|row| (row.path, row.source)),
    );
    let issue79 = issue79_identity_free();
    sources.push((issue79.path, issue79.source));
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
    let classified = contract
        .post_reset_classified
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let expected_identity_free = all_identity_free_sources()
        .into_iter()
        .map(|(path, _)| path.to_owned())
        .collect::<BTreeSet<_>>();
    let expected_fixtures = all_fixtures()
        .into_iter()
        .map(|row| row.path.to_owned())
        .collect::<BTreeSet<_>>();

    assert_eq!(identity_free, expected_identity_free);
    assert_eq!(fixtures, expected_fixtures);
    assert!(identity_free.is_disjoint(&fixtures));
    assert!(identity_free.is_disjoint(&classified));
    assert!(fixtures.is_disjoint(&classified));
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
    assert_eq!(identity_free.len(), 17);
    assert_eq!(fixtures.len(), 14);
    assert_eq!(
        contract
            .post_reset_fixture_admitted
            .iter()
            .map(|entry| entry.identity_literals)
            .sum::<usize>(),
        28
    );

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
                && !classified.contains(path)
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
            (!entry.signals.is_empty() || entry.identity_literals > 0)
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

/// Classification arrays are exact ordered artifacts, independently of the
/// optional repository-path subsets they permit.
#[test]
fn admission_arrays_reject_row_reorder_and_duplicate_paths() {
    let contract = TransitionContract::from_classification();
    for (name, rows, expected, predecessor, appended) in [
        (
            "identity-free",
            contract.post_reset_admitted.as_slice(),
            IDENTITY_FREE_PATH_ORDER.as_slice(),
            15,
            16,
        ),
        (
            "fixture",
            contract.post_reset_fixture_admitted.as_slice(),
            FIXTURE_PATH_ORDER.as_slice(),
            12,
            13,
        ),
    ] {
        assert!(
            rows_have_exact_unique_path_order(rows, expected),
            "the {name} classification array must retain its exact unique path order"
        );

        let mut reordered = rows.to_vec();
        reordered.swap(predecessor, appended);
        assert!(
            !rows_have_exact_unique_path_order(&reordered, expected),
            "the {name} appended row must not move before its accepted predecessor"
        );

        let zero_literal_row = rows
            .last()
            .expect("each permission has one appended row")
            .clone();
        assert_eq!(zero_literal_row.identity_literals, 0);

        let mut extra_duplicate = rows.to_vec();
        extra_duplicate.push(zero_literal_row.clone());
        assert!(
            !rows_have_exact_unique_path_order(&extra_duplicate, expected),
            "an extra duplicate zero-literal {name} row must be refused"
        );

        let mut same_length_collision = rows.to_vec();
        same_length_collision[0] = zero_literal_row;
        assert!(
            !rows_have_exact_unique_path_order(&same_length_collision, expected),
            "a duplicate {name} path replacing another row must be refused"
        );
    }
}

/// Every precommitted successor row is frozen field by field. Mutating each field
/// demonstrates that omission, substitution, or metadata drift cannot be
/// hidden behind the correct path count.
#[test]
fn successor_rows_reject_wrong_path_signal_count_class_owner_or_note() {
    let contract = TransitionContract::from_classification();
    for (rows, expected) in [
        (
            contract.post_reset_admitted.as_slice(),
            [
                rfc85_identity_free(),
                issue118_identity_free(),
                non_box_transient_identity_free(),
                vec![issue79_identity_free()],
            ]
            .concat(),
        ),
        (
            contract.post_reset_fixture_admitted.as_slice(),
            all_fixtures(),
        ),
    ] {
        for expected in expected {
            let row = rows
                .iter()
                .find(|row| row.path == expected.path)
                .unwrap_or_else(|| panic!("omitted successor admission `{}`", expected.path));
            assert!(row_matches(row, &expected));

            let mut mutants = Vec::new();
            let mut mutant = row.clone();
            mutant.path.push_str(".substituted");
            mutants.push(mutant);
            let mut mutant = row.clone();
            mutant.signals.push(SEARCH_TOKENS[4].to_owned());
            mutants.push(mutant);
            if !row.signals.is_empty() {
                let mut mutant = row.clone();
                mutant.signals.pop();
                mutants.push(mutant);
                let mut mutant = row.clone();
                let replacement = SEARCH_TOKENS
                    .iter()
                    .find(|token| !row.signals.iter().any(|signal| signal == **token))
                    .expect("the frozen search vocabulary has a substitution token");
                mutant.signals[0] = (*replacement).to_owned();
                mutants.push(mutant);
            }
            if row.signals.len() > 1 {
                let mut mutant = row.clone();
                mutant.signals.swap(0, 1);
                mutants.push(mutant);
            }
            let mut mutant = row.clone();
            mutant.identity_literals += 1;
            mutants.push(mutant);
            if row.identity_literals > 0 {
                let mut mutant = row.clone();
                mutant.identity_literals -= 1;
                mutants.push(mutant);
            }
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

    // The new release row cannot be made into a second description of either
    // a historical member or one of the complete later classifications.
    let expected = package_conformance_fixture();
    let row = contract
        .post_reset_fixture_admitted
        .iter()
        .find(|row| row.path == expected.path)
        .expect("the package-conformance release admission must exist");
    for overlap in [
        contract.inventory.iter().next().unwrap().as_str(),
        contract.post_reset_classified[0].path.as_str(),
    ] {
        let mut mutant = row.clone();
        mutant.path = overlap.to_owned();
        assert!(
            !row_matches(&mutant, &expected),
            "an admission overlapping `{overlap}` must be refused"
        );
    }

    // The MCP production and independent-fixture rows cannot exchange their
    // containment classes even when every other frozen field remains intact.
    let production = issue79_identity_free();
    let fixture = issue79_fixture();
    let production_row = contract
        .post_reset_admitted
        .iter()
        .find(|row| row.path == production.path)
        .expect("the MCP production admission must exist");
    let fixture_row = contract
        .post_reset_fixture_admitted
        .iter()
        .find(|row| row.path == fixture.path)
        .expect("the MCP tool-definition admission must exist");
    let mut swapped_production = production_row.clone();
    swapped_production.class = fixture.class.to_owned();
    let mut swapped_fixture = fixture_row.clone();
    swapped_fixture.class = production.class.to_owned();
    assert!(!row_matches(&swapped_production, &production));
    assert!(!row_matches(&swapped_fixture, &fixture));
}

/// Both permissions are independently optional. The identity-free permission
/// stays zero-identity; the fixture permission keeps each exact literal
/// occurrence count.
#[test]
fn both_admission_permissions_are_optional_exact_and_fail_closed() {
    let contract = TransitionContract::from_classification();
    let classify = |observed: Observed| classify_transition(&contract, &observed);
    let reset = || Observed::maximal_post_reset(&contract);
    let identity_free = all_identity_free_sources();
    let fixtures = all_fixtures();
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
        "exact literal occurrence counts never relax",
    );

    // The fixture permission is separate: its zero-count Model file and every
    // one-count relational fixture reject both a changed signal list and a
    // changed same-line literal occurrence count.
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
    let two_literals_same_line = format!(
        "{{\"{}\":\"{}\",\"other_model\":\"{}\"}}\n",
        SEARCH_TOKENS[2],
        "e".repeat(64),
        "f".repeat(64)
    );
    assert_eq!(
        observe_admitted(two_literals_same_line.as_bytes()),
        (vec![SEARCH_TOKENS[2].to_owned()], 2),
        "the observer counts both lower-hex-64 occurrences on one Model line"
    );
    for source in [&no_literal, &two_literals_same_line] {
        refused(
            classify(reset().admitting(relational.path, source)),
            "exact literal occurrence counts never relax",
        );
    }

    // The package-release fixture is discovered by its two same-line
    // source/package identities alone. It carries no Model search token, and
    // neither changing the count nor adding a token is an admissible rewrite.
    let package = package_conformance_fixture();
    assert_eq!(
        observe_admitted(package.source.as_bytes()),
        (Vec::new(), 2),
        "the independent oracle's release carries no search token and two identity occurrences"
    );
    for count in [1, 3] {
        refused(
            classify(reset().admitting(package.path, &package_conformance_fixture_source(count))),
            "exact literal occurrence counts never relax",
        );
    }
    let package_with_signal = format!("{}{}\n", package.source, SEARCH_TOKENS[4]);
    refused(
        classify(reset().admitting(package.path, &package_with_signal)),
        "must carry exactly its recorded search signal",
    );

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
        "crates/eqiora-artifact/src/prescribed_dynamic_solid_provider_occurrence/mod.rs",
        "crates/eqiora-api/src/prescribed_dynamic_solid/provider/protocol/control_test.rs",
        "crates/eqiora/tests/prescribed_dynamic_solid_subprocess_provider_2d.rs",
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/references/derive.py",
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/\
         provider-occurrence-copy.json",
        "verify/interfaces/prescribed-dynamic-solid-subprocess-provider-3d/expected/run.json.bak",
        "examples/python/prescribed_dynamic_solid_provider_test.py",
        "docs/external-boundary-provider-notes.md",
        "crates/eqiora-numerics/src/canonical_stokes/\
         navier_stokes_geometry_realization/test.rs",
        "crates/eqiora-numerics/src/canonical_stokes/\
         navier_stokes_geometry_realization/tests_copy.rs",
        "crates/eqiora-numerics/src/canonical_stokes/\
         navier_stokes_geometry_realization/tests/helpers.rs",
        "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/\
         ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff.json",
        "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/\
         d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.json.bak",
        "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/\
         d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.toml",
        "verify/interfaces/python-package-conformance/models/false-scientific-claim/store/nested/\
         d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.json",
        "verify/interfaces/python-package-conformance/models/false-scientific-claim/sibling-store/\
         d1e08b039c49c53cb963f314d424277a49e959b7d14a64208a86be972d06caf7.json",
        "verify/interfaces/python-package-conformance/models/another-claim/fixture.json",
        "crates/eqiora/src/bin/eqiora-mcp/tool.rs.bak",
        "crates/eqiora/src/bin/eqiora-mcp/tool_test.rs",
        "crates/eqiora/src/bin/eqiora-mcp/tool/mod.rs",
        "verify/interfaces/mcp-stdio-compile-check/expected/tool-definition.json.bak",
        "verify/interfaces/mcp-stdio-compile-check/expected/tool-definition-copy.json",
        "verify/interfaces/mcp-stdio-compile-check/expected/nested/tool-definition.json",
        "verify/interfaces/mcp-stdio-compile-check/expected/contract.json",
    ] {
        refused(
            classify_transition(&contract, &reset().signalling(&[path])),
            "unclassified new signal-bearing",
        );
    }
}
