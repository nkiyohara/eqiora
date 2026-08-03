//! Exact-path post-reset admission regressions for the transition contract.

use super::*;

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

    // Signal. The first mutant carries none; the second constructs one extra
    // searched spelling from the frozen vocabulary rather than duplicating it
    // in this sibling file.
    let without_signal =
        "pub fn trajectory(handle: &Handle) -> PyResult<Trajectory> {\n    handle.open()\n}\n";
    let with_extra_signal = format!(
        "pub fn trajectory({}: &str, {}: &str) -> PyResult<Trajectory> {{\n    todo!()\n}}\n",
        SEARCH_TOKENS[4], SEARCH_TOKENS[2],
    );
    for source in [without_signal, &with_extra_signal] {
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

    // Exact. Admission reaches only those exact paths: not a sibling in the
    // same directory, a file below one, another spelling of the same name, or
    // the other extension of the same module.
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
