//! Materialize named Input arguments as ordinary directed Connections.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn add_input_bindings(
        &mut self,
        instance: &eqiora_lang::InstanceDecl,
        scope: &Scope,
        instance_path: &InstancePath,
        mut declaration_path: Vec<String>,
        namespace: &DefinitionNamespace,
        origin: ConnectionOrigin,
    ) -> Result<(), Diagnostic> {
        let child = self.elaborator.resolve_component(
            namespace,
            instance.definition(),
            &origin.definition_file,
            instance.range(),
        )?;
        declaration_path.push(instance.name().to_owned());
        for connection in super::super::named_bindings::input_connections(
            &origin.definition_file,
            child.declaration,
            instance,
        )? {
            if let Some(family) = instance.family() {
                let set = scope
                    .index_set(family.set().as_str())
                    .ok_or_else(|| hierarchy_error("unresolved family IndexSet"))?;
                let endpoints = connection.port_expressions();
                let eqiora_lang::ExprKind::Path(target) = endpoints[1].kind() else {
                    unreachable!("generated Input target")
                };
                let member = target.segments().last().expect("Input target member");
                for ordinal in 0..set.extent() {
                    let source = scope.endpoint(&origin.definition_file, &endpoints[0])?;
                    let target = scope
                        .indexed_input(instance.name(), ordinal, member)
                        .ok_or_else(|| {
                            hierarchy_error("indexed occurrence has no named Input endpoint")
                        })?;
                    let mut path = declaration_path.clone();
                    path.push(ordinal.to_string());
                    self.add_resolved_connection(
                        connection.syntax(),
                        vec![source, target],
                        connection.range(),
                        instance_path,
                        path,
                        ConnectionOrigin {
                            instance: origin.instance.clone(),
                            bindings: origin.bindings.clone(),
                            definition_file: origin.definition_file.clone(),
                        },
                    )?;
                }
            } else {
                self.add_connection(
                    &connection,
                    scope,
                    instance_path,
                    declaration_path.clone(),
                    ConnectionOrigin {
                        instance: origin.instance.clone(),
                        bindings: origin.bindings.clone(),
                        definition_file: origin.definition_file.clone(),
                    },
                )?;
            }
        }
        Ok(())
    }
}
