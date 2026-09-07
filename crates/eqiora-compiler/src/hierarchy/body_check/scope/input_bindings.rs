//! Validate named Input arguments through the ordinary directed connection owner.
use super::*;

pub(in crate::hierarchy::body_check) fn validate_input_bindings(
    scope: &DefinitionScope<'_, '_>,
    instance: &InstanceDecl,
    connected_ports: &mut BTreeSet<Vec<String>>,
    limits: ConnectionSetLimits,
) -> Result<(), Diagnostic> {
    let Some(child) = scope.children.get(instance.name()) else {
        return Ok(());
    };
    for connection in crate::hierarchy::named_bindings::input_connections(
        scope.file,
        child.declaration,
        instance,
    )? {
        validate_connection(scope, &connection, connected_ports, limits)?;
    }
    Ok(())
}
