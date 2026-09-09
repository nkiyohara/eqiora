use super::*;
use std::collections::BTreeSet;

pub(super) fn resolve(specs: Vec<NotationSpec>) -> ModelNotation {
    // One entry per exact identity, never per selector or reference occurrence.
    let specs = specs
        .into_iter()
        .map(|value| (value.identity, value))
        .collect::<BTreeMap<_, _>>();
    let bases = specs
        .iter()
        .map(|(key, value)| {
            let base = value
                .declared
                .as_ref()
                .map(NotationLabel::from_notation)
                .or_else(|| NotationLabel::identifier(&value.declaration))
                .unwrap_or_else(|| fallback(*key));
            let qualified = base
                .qualify(&value.qualifiers.iter().collect::<Vec<_>>(), &[])
                .unwrap_or_else(|| fallback(*key));
            (*key, qualified)
        })
        .collect::<BTreeMap<_, _>>();
    let mut labels = bases.clone();
    let depth = specs
        .values()
        .map(|spec| spec.instance_path.len())
        .max()
        .unwrap_or(0);
    for length in 1..=depth {
        let collisions = collisions(&labels);
        if collisions.is_empty() {
            break;
        }
        for key in collisions {
            let spec = &specs[&key];
            let start = spec.instance_path.len().saturating_sub(length);
            let suffix = spec.instance_path[start..]
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            labels.insert(
                key,
                bases[&key]
                    .qualify(&[], &suffix)
                    .unwrap_or_else(|| fallback(key)),
            );
        }
    }
    for key in collisions(&labels) {
        let spec = &specs[&key];
        let mut suffix = spec
            .instance_path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        suffix.push(&spec.declaration);
        suffix.push(&spec.role_name);
        let member = key.member.map(|member| member.to_string());
        if let Some(member) = &member {
            suffix.push(member);
        }
        labels.insert(
            key,
            bases[&key]
                .qualify(&[], &suffix)
                .unwrap_or_else(|| fallback(key)),
        );
    }
    for key in collisions(&labels) {
        labels.insert(key, fallback(key));
    }
    // An authored/generated identifier can deliberately imitate a fallback. One
    // final global transition closes that class without an order-sensitive loop.
    if !collisions(&labels).is_empty() {
        for (key, label) in &mut labels {
            *label = fallback(*key);
        }
    }
    ModelNotation {
        entries: specs
            .into_iter()
            .map(|(identity, spec)| {
                let label = labels
                    .remove(&identity)
                    .expect("complete full-scope label inventory");
                (
                    identity,
                    ResolvedNotation {
                        identity,
                        selector: spec.selector,
                        graph_id: spec.graph_id,
                        definition: spec.definition,
                        instance: spec.instance,
                        label,
                    },
                )
            })
            .collect(),
    }
}

fn collisions(labels: &BTreeMap<QuantityIdentity, NotationLabel>) -> BTreeSet<QuantityIdentity> {
    let mut collisions = BTreeSet::new();
    for profile in [
        NotationProfile::Latex,
        NotationProfile::MathMl,
        NotationProfile::Unicode,
        NotationProfile::Plain,
        NotationProfile::Speech,
    ] {
        let mut seen = BTreeMap::new();
        for (key, label) in labels {
            if let Some(previous) = seen.insert(label.render(profile), *key) {
                collisions.insert(previous);
                collisions.insert(*key);
            }
        }
    }
    collisions
}

fn fallback(key: QuantityIdentity) -> NotationLabel {
    // Fixed-width identities plus a closed role are an injective encoding, not
    // a truncated hash or encounter-order counter. At most 204 ASCII bytes;
    // punctuation escaping stays well below the 512-node generated budget.
    NotationLabel::identifier(&key.to_string()).expect("bounded exact quantity identity")
}
