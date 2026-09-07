use super::*;

pub(super) fn instance_binding_locations(
    file: &str,
    instance: &InstanceDecl,
) -> Vec<SourceLocation> {
    let mut ranges = instance
        .bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            instance
                .support_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .boundary_set_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .field_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .clock_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

pub(super) fn field_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
) -> Vec<SourceLocation> {
    if instance.field_bindings().is_empty() {
        return Vec::new();
    }
    let mut ranges = instance
        .support_bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            instance
                .field_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .chain(
            instance
                .clock_bindings()
                .iter()
                .map(|binding| binding.range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

pub(super) fn parameter_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
) -> Vec<SourceLocation> {
    let mut ranges = instance
        .bindings()
        .iter()
        .map(|binding| binding.range())
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
}

pub(super) fn boundary_set_forwarding_locations(
    file: &str,
    instance: &InstanceDecl,
    support_bindings: &ResolvedSupportBindings<FullElaborationIdentity>,
) -> Vec<SourceLocation> {
    if support_bindings.boundary_sets().next().is_none() {
        return Vec::new();
    }
    let mut ranges = instance
        .support_bindings()
        .iter()
        .map(|binding| binding.range())
        .chain(
            support_bindings
                .boundary_sets()
                .map(|(_, set)| set.source_range()),
        )
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start(), range.end()));
    ranges
        .into_iter()
        .map(|range| SourceLocation::new(file, range))
        .collect()
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
