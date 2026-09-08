//! Resolve occurrence Parameters only after exact support bindings are validated.
use super::*;

pub(super) fn resolve(
    component: &ComponentDefinition<'_>,
    instance: &InstanceDecl,
    instance_file: &str,
    instance_path: &InstancePath,
    parent_scope: &Scope,
) -> Result<BTreeMap<String, super::super::parameters::ResolvedParameter>, Vec<Diagnostic>> {
    ParameterResolver::new(
        component.file,
        instance_file,
        component,
        instance,
        |name| parent_scope.parameter(name).cloned(),
        |name| super::super::clocks::occurrence(parent_scope, name),
        |name| {
            parent_scope
                .spatial_support(name)
                .map(super::super::parameters::frames::occurrence)
        },
    )
    .and_then(|resolver| {
        resolver.resolve_all(|name| {
            super::super::clocks::component_occurrence(
                component.file,
                component.declaration,
                instance,
                parent_scope,
                name,
            )
        })
    })
    .map_err(|errors| contextualize_diagnostics(errors, instance_path))
}
