use eqiora_core::Diagnostic;
use eqiora_lang::{Expr, NamePath, TextRange, VisibilitySyntax};

use super::{Budget, Encoder, encode_expression, encode_name, encode_type_path, encode_visibility};

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
