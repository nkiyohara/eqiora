//! Concrete reduction extents are checked in every existing occurrence context.
use super::*;
use crate::hierarchy::body_check::{DefinitionBodyProof, validate_component_body};
use crate::hierarchy::preflight::ComponentDefinition;

pub(super) fn needs_context(
    definition: &ComponentDefinition<'_>,
    values: &SymbolicParameterMap,
) -> bool {
    let unresolved = definition.owned_items().any(|item| {
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
        matches!(
            super::super::parameters::structural_extent(definition.file, extent, values),
            Ok(None)
        )
    });
    unresolved
        && definition.declaration.items().iter().any(|item| {
            let equations = match item {
                ComponentItem::Relation(relation) => relation.equations(),
                ComponentItem::Initial(initial) => initial.equations(),
                ComponentItem::RelationFamily(family) => family.relation().equations(),
                ComponentItem::Let(value) => {
                    return super::super::reductions::contains(value.value());
                }
                _ => return false,
            };
            equations.iter().any(|equation| {
                super::super::reductions::contains(equation.left())
                    || super::super::reductions::contains(equation.right())
            })
        })
}

pub(super) fn validate(
    elaborator: &Elaborator<'_>,
    definition: &ComponentDefinition<'_>,
    values: &SymbolicParameterMap,
    contexts: Option<&[SymbolicParameterMap]>,
    supports: &SupportInterface,
    fields: &FieldInterface,
) -> Result<DefinitionBodyProof, Vec<Diagnostic>> {
    let Some(contexts) = contexts
        .filter(|contexts| !contexts.is_empty())
        .filter(|_| needs_context(definition, values))
    else {
        // Uninstantiated and ordinary definitions keep their complete symbolic check.
        return validate_component_body(elaborator, definition, values, supports, fields);
    };
    let mut retained: Option<DefinitionBodyProof> = None;
    for values in contexts {
        let proof = validate_component_body(elaborator, definition, values, supports, fields)?;
        if let Some(previous) = &retained {
            if !previous.same_physical_contract(&proof) {
                return Err(vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    definition.file,
                    definition.range(),
                    "reduction extent specialization changes the physical definition contract; context-dependent topology is outside this profile",
                )]);
            }
        } else {
            retained = Some(proof);
        }
    }
    Ok(retained.expect("nonempty checked contexts"))
}
