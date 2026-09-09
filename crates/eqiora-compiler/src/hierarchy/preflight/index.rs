use super::*;

pub(super) fn index_unit<'a>(
    namespace: DefinitionNamespace,
    file: &'a str,
    document: &'a Document,
    limits: HierarchyLimits,
    connectors: &mut BTreeMap<DefinitionKey, ConnectorDefinition<'a>>,
    pure_operators: &mut BTreeMap<DefinitionKey, PureOperatorSourceDefinition<'a>>,
    components: &mut BTreeMap<DefinitionKey, ComponentDefinition<'a>>,
    models: &mut BTreeMap<DefinitionKey, ModelDefinition<'a>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let compiled_operators = match compile_definitions(file, document) {
        Ok(definitions) => definitions,
        Err(error) => {
            diagnostics.push(error);
            BTreeMap::new()
        }
    };
    for declaration in document.pure_operators() {
        validate_identifier(
            file,
            declaration.name(),
            declaration.range(),
            limits,
            diagnostics,
        );
        let key = DefinitionKey {
            namespace: namespace.clone(),
            name: declaration.name().to_owned(),
        };
        let Some(definition) = compiled_operators.get(declaration.name()).cloned() else {
            continue;
        };
        if pure_operators
            .insert(
                key,
                PureOperatorSourceDefinition {
                    declaration,
                    definition,
                },
            )
            .is_some()
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!(
                    "duplicate pure operator declaration `{}`",
                    declaration.name()
                ),
            ));
        }
    }
    for declaration in document.connectors() {
        validate_identifier(
            file,
            declaration.name(),
            declaration.range(),
            limits,
            diagnostics,
        );
        let key = DefinitionKey {
            namespace: namespace.clone(),
            name: declaration.name().to_owned(),
        };
        if connectors
            .insert(
                key,
                ConnectorDefinition {
                    namespace: namespace.clone(),
                    file,
                    declaration,
                },
            )
            .is_some()
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!("duplicate Connector declaration `{}`", declaration.name()),
            ));
        }
    }
    for declaration in document.components() {
        validate_identifier(
            file,
            declaration.name(),
            declaration.range(),
            limits,
            diagnostics,
        );
        let key = DefinitionKey {
            namespace: namespace.clone(),
            name: declaration.name().to_owned(),
        };
        if components
            .insert(
                key,
                ComponentDefinition {
                    owned_interfaces: interfaces::component_items(declaration),
                    namespace: namespace.clone(),
                    file,
                    declaration,
                },
            )
            .is_some()
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!("duplicate component declaration `{}`", declaration.name()),
            ));
        }
    }
    for declaration in document.models() {
        validate_identifier(
            file,
            declaration.name(),
            declaration.range(),
            limits,
            diagnostics,
        );
        let key = DefinitionKey {
            namespace: namespace.clone(),
            name: declaration.name().to_owned(),
        };
        if models
            .insert(
                key,
                ModelDefinition {
                    owned_interfaces: interfaces::model_items(declaration),
                    namespace: namespace.clone(),
                    file,
                    declaration,
                },
            )
            .is_some()
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                declaration.range(),
                format!("duplicate model declaration `{}`", declaration.name()),
            ));
        }
    }
}

pub(super) fn validate_identifier(
    file: &str,
    name: &str,
    range: TextRange,
    limits: HierarchyLimits,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if name == crate::math::ROOT {
        diagnostics.push(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "identifier `math` is reserved for compiler-owned scalar mathematics",
        ));
    }
    if name.len() > limits.max_identifier_bytes {
        diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            format!(
                "identifier `{name}` requires {} bytes, exceeding the {} byte limit",
                name.len(),
                limits.max_identifier_bytes
            ),
        ));
    }
}

pub(super) fn validate_binding_names(
    file: &str,
    instance: &InstanceDecl,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();
    for binding in instance.bindings() {
        if !names.insert(binding.name()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                binding.range(),
                format!(
                    "duplicate named binding `{}` in instance `{}`",
                    binding.name(),
                    instance.name()
                ),
            ));
        }
    }
}
