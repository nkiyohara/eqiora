use eqiora_core::Diagnostic;
use eqiora_lang::{Expr, NamePath, TextRange, ValueTypeSyntax, VisibilitySyntax};

use super::{
    Budget, Encoder, encode_expression, encode_name, encode_sorted_records, encode_type_path,
    encode_visibility, source_identity_error,
};

pub(super) fn encode_property_contract(
    declaration: &(VisibilitySyntax, &str, &ValueTypeSyntax, TextRange),
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    let (visibility, name, value_type, _) = *declaration;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, name, budget))?;
    encoder.field(2, |encoder| {
        super::value_type::encode_value_type(encoder, value_type, budget, 1)
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

#[cfg(test)]
mod tests {
    use crate::source_identity::LocalSourceIdentity;

    #[test]
    fn property_contract_identity_retains_domain_and_array_roles() {
        let identity = |kind: &str| {
            let source = format!("property contract Value(): {kind} {{ derivatives value_only; }}");
            let document = eqiora_lang::parse("property.eqi", &source)
                .into_document()
                .unwrap();
            let identity = LocalSourceIdentity::from_document(&document).unwrap();
            let formatted = eqiora_lang::format(&document);
            let reparsed = eqiora_lang::parse("again.eqi", &formatted)
                .into_document()
                .unwrap();
            assert_eq!(
                identity,
                LocalSourceIdentity::from_document(&reparsed).unwrap()
            );
            identity
        };
        assert_ne!(identity("V"), identity("complex<V>"));
        assert_ne!(identity("array<V, 2>"), identity("array<complex<V>, 2>"));
        assert_ne!(identity("array<V, 2>"), identity("vector<V, 2>"));
        assert_ne!(identity("array<V, 2>"), identity("array<V, 3>"));
    }
}
