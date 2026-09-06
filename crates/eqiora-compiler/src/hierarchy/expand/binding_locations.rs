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
