use super::*;
use std::collections::BTreeSet;

fn spec(byte: u8, path: &[&str], declared: &str) -> NotationSpec {
    NotationSpec {
        identity: QuantityIdentity {
            scope: FullElaborationIdentity::from_sha256([0xaa; 32]),
            occurrence: FullElaborationIdentity::from_sha256([byte; 32]),
            role: QuantityRole::Value,
            member: None,
        },
        graph_identity: None,
        graph_id: None,
        selector: format!("quantity{byte}"),
        definition: None,
        instance: None,
        declared: Some(Notation::parse(declared).unwrap()),
        qualifiers: vec![],
        instance_path: path.iter().map(|s| (*s).into()).collect(),
        declaration: "value".into(),
        role_name: "value".into(),
    }
}

fn distinct(catalog: &ModelNotation) {
    for profile in [
        NotationProfile::Rich,
        NotationProfile::Plain,
        NotationProfile::Speech,
    ] {
        assert_eq!(
            catalog
                .iter()
                .map(|entry| entry.render(profile))
                .collect::<BTreeSet<_>>()
                .len(),
            catalog.iter().len()
        );
    }
}

#[test]
fn shortest_suffix_and_repeated_identity_are_discovery_order_independent() {
    let first = spec(1, &["plant", "left", "leaf"], "@{x}");
    let second = spec(2, &["plant", "right", "leaf"], "@{x}");
    let a = ModelNotation::resolve(vec![first.clone(), second.clone(), first.clone()]);
    let b = ModelNotation::resolve(vec![second, first]);
    assert_eq!(a, b);
    let labels = a
        .iter()
        .map(|entry| entry.render(NotationProfile::Rich))
        .collect::<Vec<_>>();
    assert_eq!(labels, ["x_{l e f t l e a f}", "x_{r i g h t l e a f}"]);
    distinct(&a);
}

#[test]
fn total_fallback_closes_path_exhaustion_and_deliberate_fallback_capture() {
    let first = spec(1, &[], "@{x}");
    let second = spec(2, &[], "@{x}");
    let pair = ModelNotation::resolve(vec![first.clone(), second.clone()]);
    distinct(&pair);
    let mut imitator = spec(3, &[], "@{y}");
    imitator.declared = None;
    imitator.declaration = first.identity.to_string().replace(':', "x3a");
    assert!(
        imitator
            .declaration
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    );
    // A generated identifier can imitate a fallback: exact identity must still
    // terminate injectively without discovery-order numbering.
    let a = ModelNotation::resolve(vec![first.clone(), second.clone(), imitator.clone()]);
    let b = ModelNotation::resolve(vec![imitator, second, first]);
    assert_eq!(a, b);
    distinct(&a);
    assert!(
        a.iter()
            .all(|entry| entry.render(NotationProfile::Plain).len() < 16384)
    );
}

#[test]
fn explicit_versus_derived_qualification_collisions_are_rechecked_globally() {
    let a = spec(1, &["r"], "@{x}");
    let b = spec(2, &["s"], "@{x}");
    let mut explicit = spec(3, &[], "@{x}");
    explicit.qualifiers.push(Notation::parse("@{r}").unwrap());
    let result = ModelNotation::resolve(vec![a, b, explicit]);
    distinct(&result);
    assert_eq!(
        result
            .iter()
            .find(|entry| entry.selector() == "quantity2")
            .unwrap()
            .render(NotationProfile::Rich),
        "x_{s}"
    );
}
