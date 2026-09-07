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
        Ok(())
    }
}
