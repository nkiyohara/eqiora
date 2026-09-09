//! Concrete reduction extents are checked in every existing occurrence context.
use super::*;
use crate::hierarchy::body_check::{DefinitionBodyProof, validate_component_body};
use crate::hierarchy::preflight::ComponentDefinition;

pub(super) fn needs_context(
    definition: &ComponentDefinition<'_>,
    _values: &SymbolicParameterMap,
) -> bool {
    if definition.signature().iter().any(|item| match item {
        eqiora_lang::SignatureItem::Field(value)
        | eqiora_lang::SignatureItem::Input(value)
        | eqiora_lang::SignatureItem::Output(value) => {
            super::super::parameters::extent_expressions(value.value_type())
                .iter()
                .any(|extent| crate::hierarchy::closed_index(extent).is_err())
        }
        _ => false,
    }) {
        return true;
    }
    definition.owned_items().any(|item| {
        if matches!(item, ComponentItem::Instance(_)) {
            return true;
        }
        let syntax = match item {
            ComponentItem::Observable(value) => Some(value.value_type()),
            ComponentItem::Field(field) => Some(field.value_type()),
            ComponentItem::Parameter(parameter) => Some(parameter.value_type()),
            ComponentItem::Port(port) => match port.syntax() {
                eqiora_lang::PortSyntax::Signal { value_type, .. } => Some(value_type),
                _ => None,
            },
            _ => None,
        };
        if syntax.is_some_and(|syntax| {
            super::super::parameters::extent_expressions(syntax)
                .into_iter()
                .any(|extent| crate::hierarchy::closed_index(extent).is_err())
        }) {
            return true;
        }
        let ComponentItem::IndexSet(set) = item else {
            return false;
        };
        let ExprKind::Call {
            arguments: eqiora_lang::CallArguments::Positional(arguments),
            ..
        } = set.value().kind()
        else {
            return false;
        };
        let [extent] = arguments.as_slice() else {
            return false;
        };
        crate::hierarchy::closed_index(extent).is_err()
    })
}

pub(super) fn validate(
    elaborator: &Elaborator<'_>,
    definition: &ComponentDefinition<'_>,
    values: &SymbolicParameterMap,
    contexts: Option<&[SymbolicParameterMap]>,
    supports: &SupportInterface,
) -> Result<Vec<DefinitionBodyProof>, Vec<Diagnostic>> {
    let Some(contexts) = contexts
        .filter(|contexts| !contexts.is_empty())
        .filter(|_| needs_context(definition, values))
    else {
        // Uninstantiated and ordinary definitions keep their complete symbolic check.
        let fields =
            component_field_interface(definition.file, definition.declaration, supports, values)?;
        return validate_component_body(elaborator, definition, values, supports, &fields)
            .map(|proof| vec![proof]);
    };
    let mut retained = Vec::new();
    for values in contexts {
        let fields =
            component_field_interface(definition.file, definition.declaration, supports, values)?;
        let proof = validate_component_body(elaborator, definition, values, supports, &fields)?;
        retained.push(proof);
    }
    Ok(retained)
}
pub(super) fn instance_extent<'a>(
    file: &str,
    instance: &eqiora_lang::InstanceDecl,
    mut sets: impl Iterator<Item = &'a eqiora_lang::NamedDefinitionDecl>,
    values: &super::SymbolicParameterMap,
    limit: usize,
) -> Result<Option<u32>, eqiora_core::Diagnostic> {
    let Some(family) = instance.family() else {
        return Ok(Some(1));
    };
    let invalid = |message| {
        crate::diagnostics::source_error(
            eqiora_core::diagnostic::codes::LANGUAGE_TYPE_ERROR,
            file,
            family.range(),
            message,
        )
    };
    let set = sets
        .find(|set| set.name() == family.set().as_str())
        .ok_or_else(|| invalid("indexed instance requires an enclosing IndexSet"))?;
    let Some((extent, _)) = super::super::parameters::index_set_extent(file, set, values)? else {
        // An unselected generic definition is checked symbolically, without
        // inventing a member. Selected expansion independently requires exact
        // extents in definition_graph::selected before allocating occurrences.
        return Ok(None);
    };
    if extent as usize > limit {
        return Err(invalid(
            "indexed instance exceeds the Component instances limit",
        ));
    }
    Ok(Some(extent))
}
