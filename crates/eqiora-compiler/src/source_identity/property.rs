use eqiora_core::Diagnostic;
use eqiora_lang::{Expr, NamePath, TextRange, VisibilitySyntax};

use super::{
    Budget, Encoder, encode_expression, encode_name, encode_sorted_records, encode_type_path,
    encode_visibility, source_identity_error,
};

pub(super) fn encode_property_contract(
    declaration: &(VisibilitySyntax, &str, &Expr, TextRange),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, dimension, _) = *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, dimension, budget, 1)
    })?;
    if visibility == VisibilitySyntax::Public {
        encoder.field(3, |encoder| encode_visibility(encoder, visibility))?;
    }
    encoder.finish()
}

pub(super) fn encode_property_release(
    declaration: &(
        VisibilitySyntax,
        &str,
        &NamePath,
        &Expr,
        &Expr,
        &Expr,
        &NamePath,
        &NamePath,
        TextRange,
    ),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, contract, source_value, source_dimension, scale, citation, license, _) =
        *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| encode_type_path(encoder, contract, budget))?;
    encoder.field(3, |encoder| {
        encode_expression(encoder, source_value, budget, 1)
    })?;
    encoder.field(4, |encoder| {
        encode_expression(encoder, source_dimension, budget, 1)
    })?;
    encoder.field(5, |encoder| encode_expression(encoder, scale, budget, 1))?;
    encoder.field(6, |encoder| encode_type_path(encoder, citation, budget))?;
    encoder.field(7, |encoder| encode_type_path(encoder, license, budget))?;
    if visibility == VisibilitySyntax::Public {
        encoder.field(8, |encoder| encode_visibility(encoder, visibility))?;
    }
    encoder.finish()
}

pub(super) fn encode_material_composition(
    declaration: &(
        VisibilitySyntax,
        &str,
        Vec<(&str, &NamePath, TextRange)>,
        TextRange,
    ),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, properties, _) = declaration;
    if properties.len() > budget.limits.max_bindings_per_instance {
        return Err(source_identity_error(format!(
            "material composition `{name}` has {} properties, exceeding the {} binding limit",
            properties.len(),
            budget.limits.max_bindings_per_instance
        )));
    }
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    let properties = encode_sorted_records(properties, budget, |binding, budget| {
        let (property, release, _) = *binding;
        let mut value = Encoder::new(budget.limits.max_canonical_bytes);
        value.field(1, |encoder| encode_name(encoder, property, budget))?;
        value.field(2, |encoder| encode_type_path(encoder, release, budget))?;
        value.finish()
    })?;
    encoder.field(2, |encoder| encoder.records(&properties))?;
    if *visibility == VisibilitySyntax::Public {
        encoder.field(3, |encoder| encode_visibility(encoder, *visibility))?;
    }
    encoder.finish()
}
