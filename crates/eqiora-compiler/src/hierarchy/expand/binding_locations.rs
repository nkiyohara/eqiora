use super::*;

pub(super) fn instance_binding_locations(
    file: &str,
    instance: &InstanceDecl,
) -> Vec<SourceLocation> {
    selected_locations(file, instance, |_| true)
}
fn selected_locations(
    file: &str,
    instance: &InstanceDecl,
    accepts: impl Fn(&str) -> bool,
) -> Vec<SourceLocation> {
    let mut locations = instance
        .bindings()
        .iter()
        .filter(|binding| accepts(binding.name()))
        .map(|binding| SourceLocation::new(file, binding.range()))
        .collect::<Vec<_>>();
    normalize_binding_locations(&mut locations);
    locations
}
pub(super) fn field_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
    component: &eqiora_lang::ComponentDecl,
) -> Vec<SourceLocation> {
    use eqiora_lang::SignatureItem;
    if !component
        .signature()
        .iter()
        .any(|item| matches!(item, SignatureItem::Field(_)))
    {
        return Vec::new();
    }
    selected_locations(file, instance, |name| {
        component.signature().iter().any(|item| {
            item.name() == name
                && matches!(
                    item,
                    SignatureItem::Field(_) | SignatureItem::Clock(_) | SignatureItem::Support(_)
                )
        })
    })
}
pub(super) fn parameter_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
    component: &eqiora_lang::ComponentDecl,
) -> Vec<SourceLocation> {
    selected_locations(file, instance, |name| {
        component.signature().iter().any(|item| {
            item.name() == name && matches!(item, eqiora_lang::SignatureItem::Parameter(_))
        })
    })
}
pub(super) fn boundary_set_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
    support_bindings: &ResolvedSupportBindings<FullElaborationIdentity>,
) -> Vec<SourceLocation> {
    if support_bindings.boundary_sets().next().is_none() {
        return Vec::new();
    }
    let mut locations = selected_locations(file, instance, |name| {
        support_bindings.singular_targets().contains_key(name)
    });
    locations.extend(
        support_bindings
            .boundary_sets()
            .map(|(_, set)| SourceLocation::new(file, set.source_range())),
    );
    normalize_binding_locations(&mut locations);
    locations
}

pub(super) fn normalize_binding_locations(bindings: &mut Vec<SourceLocation>) {
    bindings.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.range.start().cmp(&right.range.start()))
            .then_with(|| left.range.end().cmp(&right.range.end()))
    });
    bindings.dedup_by(|left, right| left.file == right.file && left.range == right.range);
}

pub(super) fn compare_physical_connection_origins(
    left: &PhysicalConnectionOrigin,
    right: &PhysicalConnectionOrigin,
) -> core::cmp::Ordering {
    left.declaration_path
        .cmp(&right.declaration_path)
        .then_with(|| {
            left.source
                .definition
                .file
                .cmp(&right.source.definition.file)
        })
        .then_with(|| {
            left.source
                .definition
                .range
                .start()
                .cmp(&right.source.definition.range.start())
        })
        .then_with(|| {
            left.source
                .definition
                .range
                .end()
                .cmp(&right.source.definition.range.end())
        })
}

pub(super) fn boundary_family_bindings(
    base: &[SourceLocation],
    file: &str,
    member_range: eqiora_lang::TextRange,
) -> Vec<SourceLocation> {
    let mut bindings = base.to_vec();
    bindings.push(SourceLocation::new(file, member_range));
    normalize_binding_locations(&mut bindings);
    bindings
}
