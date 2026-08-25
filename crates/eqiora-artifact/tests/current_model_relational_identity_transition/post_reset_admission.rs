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

const IDENTITY_FREE_PATH_ORDER: [&str; 29] = [
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
    "docs/site/src/content/docs/gallery/exact-cylinder-steady-stokes.mdx",
    "docs/site/src/content/docs/reference/control-v2/index.mdx",
    "docs/site/src/content/docs/reference/mcp/index.mdx",
    "docs/site/src/content/docs/reference/python/eqiora.mdx",
    "docs/site/src/content/docs/reference/python/fluid.mdx",
    "docs/site/src/content/docs/reference/python/fsi.mdx",
    "docs/site/src/content/docs/reference/python/solid.mdx",
    "docs/site/src/content/docs/reference/python/trajectory.mdx",
    "tools/site/check_gallery_publication.py",
    "tools/site/produce_exact_cylinder_pressure.py",
    "tools/site/tests/gallery_publication/fixtures.py",
    "tools/site/tests/gallery_publication/test_predicate.py",
];

const FIXTURE_PATH_ORDER: [&str; 27] = [
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
    "crates/eqiora-numerics/src/canonical_stokes/dissipation_profile/\
     e1-sealed-inputs-v1.json",
    "crates/eqiora-numerics/src/cartesian_periodic_3d/collocated_tests.rs",
    "verify/fluid/cartesian-periodic-collocated-view-3d/case.toml",
    "verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/p-head-nontest.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/p-head-test.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/r-base-nontest.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/r-base-test.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/r-head-nontest.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/r-head-test.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/t-all-head-test.jsonl",
    "verify/quality/semantic-impact-cargo-authority/fixtures/t-auto-head-test.jsonl",
    "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json",
];

type SiteAdmission = (
    &'static str,
    &'static str,
    (&'static str, &'static str),
    &'static str,
    &'static str,
);

#[rustfmt::skip]
const SITE_ADMISSIONS: [SiteAdmission; 13] = [
    ("docs/site/src/content/docs/gallery/exact-cylinder-steady-stokes.mdx", "current-owner-assertion", ("9f6b9bb4d1f7b6dfa2631fbfa97bef2d", "0849445c15a2d5ab4379eb4c64b3f60d"), "gallery.exact-cylinder-steady-stokes current-owner documentation", "the accepted Cylinder gallery projection names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; gallery science and prose remain owned by their accepted authorities."),
    ("docs/site/src/content/docs/reference/control-v2/index.mdx", "current-owner-assertion", ("71265ea8c47bcbf73c5d3d606311bec9", "05b3c4da3b39948eb744d3e3fa57e0f9"), "interfaces.control-plane-compile-check current-owner reference projection", "the accepted control-v2 reference projection names the current Model and Transaction schema families without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes."),
    ("docs/site/src/content/docs/reference/mcp/index.mdx", "current-owner-assertion", ("8aa18b3a0c67d23ebe31d380042551fb", "f9d61f9c5b9e78203aaabf2e44265aef"), "interfaces.mcp-stdio-compile-check current-owner reference projection", "the accepted MCP reference projection names the current Model and Transaction schema families without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes."),
    ("docs/site/src/content/docs/reference/python/eqiora.mdx", "current-owner-assertion", ("962e184c67359ff1e3753cff704ae3a4", "b8093829ceb1ee446c93f12b4632e499"), "the accepted Python API reference projection", "the generated top-level Python reference names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; API meaning remains owned by the accepted projection."),
    ("docs/site/src/content/docs/reference/python/fluid.mdx", "current-owner-assertion", ("97874be5371df59158beec442545e29b0", "2ef3956b028b7f515fe8354796ce2e6"), "the accepted Python API reference projection", "the generated fluid Python reference names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; API meaning remains owned by the accepted projection."),
    ("docs/site/src/content/docs/reference/python/fsi.mdx", "current-owner-assertion", ("cc3cb3f8119883005f3dcbe63e5b9cc", "f01d2c2c1ab120669ad8c07f92b16e3e5"), "the accepted Python API reference projection", "the generated FSI Python reference names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; API meaning remains owned by the accepted projection."),
    ("docs/site/src/content/docs/reference/python/solid.mdx", "current-owner-assertion", ("8ada7c9fe856095da5334bdcde07a017", "48aee2d82dc201098695f03fd2c1a841"), "the accepted Python API reference projection", "the generated solid Python reference names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; API meaning remains owned by the accepted projection."),
    ("docs/site/src/content/docs/reference/python/trajectory.mdx", "current-owner-assertion", ("faabff5c1b7c1b58d9a6da1f71ce4ce", "725c362c37bbcc351b3f44201bf13cddf"), "the accepted Python API reference projection", "the generated trajectory Python reference names `{edge}` without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; API meaning remains owned by the accepted projection."),
    ("tools/site/check_gallery_publication.py", "non-fixture-search-hit", ("e9a27aeae4c77c951be9017faef0343", "a2e712aeedad92238249993b87967a150"), "the accepted Gallery publication checker", "the accepted checker names `{edge}` as a publication predicate field without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; predicate semantics remain owned by its accepted evidence."),
    ("tools/site/produce_exact_cylinder_pressure.py", "non-fixture-search-hit", ("b999486b9333cfea52ad7bcc33184244", "4954d717bbd65230673b4a7536e18120"), "the accepted exact-cylinder pressure producer", "the accepted producer names `{edge}` as an input lineage field without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; scientific and publication semantics remain with their accepted authorities."),
    ("tools/site/tests/gallery_publication/fixtures.py", "non-fixture-search-hit", ("7b88383809422f73dec1a6cf21d4e100", "6610b4b81917379abdd97a1134261a7b"), "the accepted Gallery publication fixture helper", "the accepted fixture helper names `{edge}` while constructing causal publication mutants and freezes no Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes."),
    ("tools/site/tests/gallery_publication/test_predicate.py", "non-fixture-search-hit", ("9db7b122386c52998fd4dd35714f5ff9", "f7b5b39ae06e4e8078237778eaad25f0"), "the accepted Gallery publication predicate oracle", "the accepted predicate test names `{edge}` as an asserted and mutated publication field without freezing a Model-derived identity literal. Admission owns only this exact path, observed shape, category, and accepted bytes; expected publication semantics remain owned by that independent oracle."),
    ("docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json", "delegated-current-owner-evidence", ("f5df7f5dd74abcac60776786e5863a9c", "fb81ab88855b9f77ee90c38decb61813"), "gallery.exact-cylinder-steady-stokes accepted publication evidence", "the accepted minified publication projection names `{edge}` and contains 75 same-line lower-hex-64 occurrences. Admission owns only this exact path, observed shape, category, and accepted bytes; scientific, source, package, renderer, receipt, and publication identities remain owned by the accepted Gallery authorities."),
];

const SEMANTIC_IMPACT_FIXTURES: [(&str, usize); 9] = [
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test.jsonl",
        17,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-head-nontest.jsonl",
        17,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-head-test.jsonl",
        17,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/r-base-nontest.jsonl",
        0,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/r-base-test.jsonl",
        0,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/r-head-nontest.jsonl",
        0,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/r-head-test.jsonl",
        0,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/t-all-head-test.jsonl",
        17,
    ),
    (
        "verify/quality/semantic-impact-cargo-authority/fixtures/t-auto-head-test.jsonl",
        17,
    ),
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

fn stokes_e1_sealed_input_fixture() -> ExpectedAdmission {
    ExpectedAdmission {
        path: "crates/eqiora-numerics/src/canonical_stokes/dissipation_profile/\
               e1-sealed-inputs-v1.json",
        class: "source-or-package-identity",
        owner: "geometry.stokes-dissipation-profile-2d independent evidence authority",
        note: "the exact minified E1 sealed input has no recorded search token and carries five \
               source, contract, and review SHA-256 identities on its single \
               `scientific_model`-bearing line. Admission owns only this path, empty search \
               shape, and occurrence count; the sealed-input authority retains ownership of \
               those bytes and identities, and this creates no Model artifact identity claim."
            .to_owned(),
        source: format!(
            "{{\"scientific_model\":{{}},\"authority_sha256\":[{}]}}\n",
            (0..5)
                .map(|_| format!("\"{}\"", "e".repeat(64)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn taylor_collocated_fixtures() -> Vec<ExpectedAdmission> {
    let identity = "e".repeat(64);
    vec![
        ExpectedAdmission {
            path: "crates/eqiora-numerics/src/cartesian_periodic_3d/collocated_tests.rs",
            class: "delegated-current-owner-evidence",
            owner: "fluid.cartesian-periodic-collocated-view-3d independent structural oracle",
            note: "the independent structural oracle pins the accepted current Model artifact \
                   identity once on its `MODEL_SHA256` line and carries no recorded search \
                   token. Admission owns only this exact path, empty search shape, and one \
                   Model-derived occurrence; the evidence authority owns the value and \
                   assertions."
                .to_owned(),
            source: format!("const MODEL_SHA256: &str = \"{identity}\";\n"),
        },
        ExpectedAdmission {
            path: "verify/fluid/cartesian-periodic-collocated-view-3d/case.toml",
            class: "delegated-current-owner-evidence",
            owner: "fluid.cartesian-periodic-collocated-view-3d registered evidence manifest",
            note: "the registered evidence manifest pins the same accepted current Model \
                   artifact identity once on `model_artifact_sha256` and carries no recorded \
                   search token. Admission owns only this exact path, empty search shape, and \
                   one Model-derived occurrence; the registered case owns the claim and value."
                .to_owned(),
            source: format!("model_artifact_sha256 = \"{identity}\"\n"),
        },
    ]
}

fn semantic_impact_cargo_authority_fixtures() -> Vec<ExpectedAdmission> {
    SEMANTIC_IMPACT_FIXTURES
        .iter()
        .copied()
        .map(|(path, count)| {
            let identities = (0..count)
                .map(|_| format!("\"{}\"", "e".repeat(64)))
                .collect::<Vec<_>>()
                .join(",");
            let name = path.rsplit('/').next().unwrap();
            ExpectedAdmission {
                path,
                class: "source-or-package-identity",
                owner: "quality.semantic-impact-cargo-authority independent exact-record oracle",
                note: format!(
                    "the exact external `{name}` fixture records Cargo manifest, source, \
                     package, dependency, target, commit, tree, overlay, blob, and content \
                     identities; its {count} same-line lower-hex-64 occurrences are Cargo, \
                     source, package, and revision records on Model/Transaction-bearing \
                     serialized authority lines, not current Model artifact identities. \
                     Admission owns only this exact path, empty search shape, and occurrence \
                     count; the Cargo authority oracle retains ownership of its bytes and \
                     values. Externalization moved the serialized authority text without \
                     changing the JSONL bytes consumed at runtime or any scientific meaning."
                ),
                source: format!(
                    "CARGO MODEL TRANSACTION SOURCE PACKAGE REVISION RECORDS [{identities}]\n"
                ),
            }
        })
        .collect()
}

fn site_expected((path, class, _, owner, note): SiteAdmission) -> ExpectedAdmission {
    ExpectedAdmission {
        path,
        class,
        owner,
        note: note.replace("{edge}", SEARCH_TOKENS[4]),
        source: fs::read_to_string(repository_root().join(path))
            .unwrap_or_else(|error| panic!("accepted site input `{path}` must be UTF-8: {error}")),
    }
}

fn site_digest(admission: SiteAdmission) -> String {
    format!("{}{}", admission.2.0, admission.2.1)
}

fn site_identity_free() -> Vec<ExpectedAdmission> {
    SITE_ADMISSIONS[..12]
        .iter()
        .copied()
        .map(site_expected)
        .collect()
}

fn site_fixture() -> ExpectedAdmission {
    site_expected(SITE_ADMISSIONS[12])
}

fn has_exact_accepted_digests(contract: &TransitionContract) -> bool {
    let expected = SITE_ADMISSIONS
        .iter()
        .copied()
        .map(|admission| (admission.0, site_digest(admission)))
        .collect::<BTreeMap<_, _>>();
    let actual = contract
        .post_reset_admitted
        .iter()
        .chain(&contract.post_reset_fixture_admitted)
        .filter_map(|row| {
            row.accepted_sha256
                .as_deref()
                .map(|digest| (row.path.as_str(), digest.to_owned()))
        })
        .collect::<BTreeMap<_, _>>();
    actual == expected
}

fn all_fixtures() -> Vec<ExpectedAdmission> {
    let mut rows = rfc85_fixtures();
    rows.extend(issue118_fixtures());
    rows.push(package_conformance_fixture());
    rows.push(issue79_fixture());
    rows.push(stokes_e1_sealed_input_fixture());
    rows.extend(taylor_collocated_fixtures());
    rows.extend(semantic_impact_cargo_authority_fixtures());
    rows.push(site_fixture());
    rows
}

fn row_matches(row: &PostResetAdmitted, expected: &ExpectedAdmission) -> bool {
    let (signals, literals, _) = observe_admitted(expected.source.as_bytes());
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
    sources.extend(
        site_identity_free()
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

/// Accepted alpha.3 site bytes are optional exact-path permissions. The real
/// tree proves the ordinary positive before any mutation receives credit.
#[test]
fn accepted_site_paths_are_exact_optional_and_byte_bound() {
    let contract = TransitionContract::from_classification();
    let observed = Observed::from_repository(&contract, &repository_root());
    assert_eq!(
        classify_transition(&contract, &observed),
        Ok(TransitionState::PostReset)
    );
    assert!(has_exact_accepted_digests(&contract));

    for (index, admission) in SITE_ADMISSIONS.iter().copied().enumerate() {
        let (path, class, _, _, _) = admission;
        let digest = site_digest(admission);
        let rows = if index < 12 {
            &contract.post_reset_admitted
        } else {
            &contract.post_reset_fixture_admitted
        };
        let row = rows.iter().find(|row| row.path == path).unwrap();
        assert!(row_matches(row, &site_expected(admission)));
        assert_eq!(row.class, class);
        assert_eq!(row.accepted_sha256.as_deref(), Some(digest.as_str()));
        assert_eq!(observed.admitted[path].2, digest);
    }

    for (index, (path, _, _, _, _)) in SITE_ADMISSIONS.iter().copied().enumerate() {
        for wrong_path in [false, true] {
            let mut mutant = TransitionContract::from_classification();
            let rows = if index < 12 {
                &mut mutant.post_reset_admitted
            } else {
                &mut mutant.post_reset_fixture_admitted
            };
            let at = rows.iter().position(|row| row.path == path).unwrap();
            if wrong_path {
                rows[at].path.push_str(".wrong");
            } else {
                rows.remove(at);
            }
            refused(
                classify_transition(&mutant, &observed),
                &format!("unclassified new signal-bearing paths [\"{path}\"]"),
            );
        }
    }

    for path in [
        "docs/site/src/content/docs/gallery/exact-cylinder-steady-stokes-copy.mdx",
        "docs/site/src/content/docs/reference/python/eqiora-copy.mdx",
        "docs/site/src/content/docs/reference/control-v2/copy.mdx",
        "docs/site/src/data/gallery/exact-cylinder-steady-stokes.publication.json.bak",
        "tools/site/check_gallery_publication_copy.py",
        "tools/site/tests/gallery_publication/nested/test_predicate.py",
    ] {
        refused(
            classify_transition(&contract, &observed.clone().signalling(&[path])),
            "unclassified new signal-bearing",
        );
    }

    for admission in SITE_ADMISSIONS.iter().copied() {
        let (path, _, _, _, _) = admission;
        let expected_digest = site_digest(admission);
        let before = observed.admitted[path].clone();
        let mut changed = site_expected(admission).source;
        changed.push('\n');
        let drifted = observed.clone().admitting(path, &changed);
        assert_eq!(drifted.admitted[path].0, before.0);
        assert_eq!(drifted.admitted[path].1, before.1);
        let reason = classify_transition(&contract, &drifted).unwrap_err();
        assert!(reason.contains(path));
        assert!(reason.contains(&expected_digest));
        assert!(reason.contains(&drifted.admitted[path].2));
    }

    let first = SITE_ADMISSIONS[0];
    let first_digest = site_digest(first);
    for invalid in [
        None,
        Some(site_digest(SITE_ADMISSIONS[1])),
        Some(first_digest.to_ascii_uppercase()),
        Some("g".repeat(64)),
        Some(first_digest[..63].to_owned()),
    ] {
        let mut mutant = TransitionContract::from_classification();
        mutant.post_reset_admitted[17].accepted_sha256 = invalid;
        assert!(!has_exact_accepted_digests(&mutant));
    }
    let mut preexisting_digest = TransitionContract::from_classification();
    preexisting_digest.post_reset_admitted[0].accepted_sha256 = Some(first_digest);
    assert!(!has_exact_accepted_digests(&preexisting_digest));

    for (index, admission) in SITE_ADMISSIONS.iter().copied().take(12).enumerate() {
        let mut category = TransitionContract::from_classification();
        category.post_reset_admitted[index + 17].class = if index < 8 {
            "non-fixture-search-hit"
        } else {
            "current-owner-assertion"
        }
        .to_owned();
        assert!(!row_matches(
            &category.post_reset_admitted[index + 17],
            &site_expected(admission)
        ));
        let mut moved = TransitionContract::from_classification();
        let row = moved.post_reset_admitted.remove(index + 17);
        moved.post_reset_fixture_admitted.push(row);
        assert!(!rows_have_exact_unique_path_order(
            &moved.post_reset_admitted,
            &IDENTITY_FREE_PATH_ORDER
        ));
        assert!(!rows_have_exact_unique_path_order(
            &moved.post_reset_fixture_admitted,
            &FIXTURE_PATH_ORDER
        ));
    }
    let mut moved = TransitionContract::from_classification();
    moved
        .post_reset_admitted
        .push(moved.post_reset_fixture_admitted.pop().unwrap());
    assert!(!rows_have_exact_unique_path_order(
        &moved.post_reset_admitted,
        &IDENTITY_FREE_PATH_ORDER
    ));
    assert!(!rows_have_exact_unique_path_order(
        &moved.post_reset_fixture_admitted,
        &FIXTURE_PATH_ORDER
    ));
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
    assert_eq!(identity_free.len(), 29);
    assert_eq!(fixtures.len(), 27);
    assert_eq!(
        contract
            .post_reset_fixture_admitted
            .iter()
            .map(|entry| entry.identity_literals)
            .sum::<usize>(),
        195
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
            (!entry.signals.is_empty()
                || entry.identity_literals > 0
                || SEMANTIC_IMPACT_FIXTURES
                    .iter()
                    .any(|(path, _)| *path == entry.path))
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
    for (name, rows, expected, predecessor, appended, zero_literal) in [
        (
            "identity-free",
            contract.post_reset_admitted.as_slice(),
            IDENTITY_FREE_PATH_ORDER.as_slice(),
            27,
            28,
            28,
        ),
        (
            "fixture",
            contract.post_reset_fixture_admitted.as_slice(),
            FIXTURE_PATH_ORDER.as_slice(),
            25,
            26,
            20,
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

        let mut missing = rows.to_vec();
        missing.pop();
        assert!(
            !rows_have_exact_unique_path_order(&missing, expected),
            "a missing {name} row must be refused"
        );

        let mut extra = rows.to_vec();
        let mut extra_row = rows.last().unwrap().clone();
        extra_row.path.push_str(".extra");
        extra.push(extra_row);
        assert!(
            !rows_have_exact_unique_path_order(&extra, expected),
            "an extra {name} row must be refused"
        );

        let mut wrong_path = rows.to_vec();
        wrong_path[0].path.push_str(".wrong");
        assert!(
            !rows_have_exact_unique_path_order(&wrong_path, expected),
            "a wrong {name} path must be refused"
        );

        let zero_literal_row = rows[zero_literal].clone();
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

    let semantic = &contract.post_reset_fixture_admitted[17..];
    assert_eq!(semantic[0].identity_literals, 17);
    assert_eq!(semantic[3].identity_literals, 0);
    for (index, expected) in [(0, 16), (0, 18), (3, 1)] {
        let mut wrong_count = semantic[index].clone();
        wrong_count.identity_literals = expected;
        assert!(
            !row_matches(
                &wrong_count,
                &semantic_impact_cargo_authority_fixtures()[index]
            ),
            "a semantic-impact zero- or 17-count row must reject count drift"
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
                site_identity_free(),
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

    // Optional independently: the empty, full, singleton, and one-omitted
    // boundaries remain complete post-reset states for each permission. The
    // predicate checks rows independently, so these linear boundaries expose
    // either possible coupling without enumerating every fixture subset.
    for sources in [&identity_free, &fixture_sources] {
        assert_eq!(classify(reset()), Ok(TransitionState::PostReset));
        assert_eq!(
            classify(admitting_all(reset(), sources)),
            Ok(TransitionState::PostReset)
        );
        for omitted in 0..sources.len() {
            let singleton = &sources[omitted];
            assert_eq!(
                classify(reset().admitting(singleton.0, &singleton.1)),
                Ok(TransitionState::PostReset)
            );
            let all_but_one =
                sources
                    .iter()
                    .enumerate()
                    .fold(reset(), |state, (index, (path, source))| {
                        if index == omitted {
                            state
                        } else {
                            state.admitting(path, source)
                        }
                    });
            assert_eq!(classify(all_but_one), Ok(TransitionState::PostReset));
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
        (
            vec![SEARCH_TOKENS[2].to_owned()],
            2,
            raw_sha256(two_literals_same_line.as_bytes())
        ),
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
        (Vec::new(), 2, raw_sha256(package.source.as_bytes())),
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

    let old_host = "tools/xtask/tests/semantic_impact_cargo_authority.rs";
    assert!(
        !contract
            .post_reset_fixture_admitted
            .iter()
            .any(|entry| entry.path == old_host),
        "the identity-free old Rust host retains no fixture permission"
    );
    let old_host_bytes = fs::read(repository_root().join(old_host)).unwrap();
    assert_eq!(
        observe_admitted(&old_host_bytes),
        (Vec::new(), 0, raw_sha256(&old_host_bytes))
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
        "crates/eqiora-numerics/src/canonical_stokes/dissipation_profile/\
         e1-sealed-inputs-v2.json",
        "crates/eqiora-numerics/src/canonical_stokes/dissipation_profile/\
         e1-sealed-inputs-v1.json.bak",
        "crates/eqiora-numerics/src/canonical_stokes/dissipation_profile/expected/\
         e1-sealed-inputs-v1.json",
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
        "crates/eqiora-numerics/src/cartesian_periodic_3d/collocated_tests.rs.bak",
        "crates/eqiora-numerics/src/cartesian_periodic_3d/collocated_tests_copy.rs",
        "crates/eqiora-numerics/src/cartesian_periodic_3d/collocated_tests/helper.rs",
        "verify/fluid/cartesian-periodic-collocated-view-3d/case.toml.bak",
        "verify/fluid/cartesian-periodic-collocated-view-3d/case-copy.toml",
        "verify/fluid/cartesian-periodic-collocated-view-3d/nested/case.toml",
        "verify/fluid/cartesian-periodic-collocated-view-2d/case.toml",
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test.jsonl.bak",
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test-copy.jsonl",
        "verify/quality/semantic-impact-cargo-authority/fixtures/nested/p-bin-head-test.jsonl",
        "verify/quality/semantic-impact-cargo-authority/fixtures/p-bin-head-test.toml",
        "verify/quality/semantic-impact-cargo-authority/fixtures/sibling.jsonl",
        "tools/xtask/tests/semantic_impact_cargo_authority.rs.bak",
        "tools/xtask/tests/semantic_impact_cargo_authority_copy.rs",
        "tools/xtask/tests/semantic_impact_cargo_authority/helper.rs",
        "tools/xtask/tests/semantic_impact_package_authority.rs",
        "tools/xtask/tests/semantic_impact.rs",
    ] {
        refused(
            classify_transition(&contract, &reset().signalling(&[path])),
            "unclassified new signal-bearing",
        );
    }
}
