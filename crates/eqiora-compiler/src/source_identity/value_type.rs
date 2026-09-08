use eqiora_core::ScalarDomain;
use eqiora_lang::{ValueTypeSyntax, ValueTypeSyntaxKind};

use super::*;

pub(super) fn encode_value_type(
    encoder: &mut Encoder,
    value: &ValueTypeSyntax,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.account_expression(depth)?;
    match value.kind() {
        ValueTypeSyntaxKind::Coordinates(name)
        | ValueTypeSyntaxKind::Counts(name)
        | ValueTypeSyntaxKind::Index(name) => {
            encoder.u8(match value.kind() {
                ValueTypeSyntaxKind::Coordinates(_) => 4,
                ValueTypeSyntaxKind::Counts(_) => 5,
                _ => 6,
            })?;
            encode_path(encoder, name, budget)
        }
        ValueTypeSyntaxKind::Scalar { domain, dimension } => {
            encoder.u8(0)?;
            encoder.u8(match domain {
                ScalarDomain::Real => 0,
                ScalarDomain::Complex => 1,
                ScalarDomain::Integer => 2,
                ScalarDomain::Boolean => 3,
            })?;
            encode_expression(encoder, dimension, budget, next_depth(depth)?)
        }
        ValueTypeSyntaxKind::Vector { scalar, extent } => {
            encoder.u8(1)?;
            encoder.u32(*extent)?;
            encode_value_type(encoder, scalar, budget, next_depth(depth)?)
        }
        ValueTypeSyntaxKind::Tensor { scalar, extents } => {
            encoder.u8(2)?;
            encoder.u32(as_u32(extents.len(), "tensor rank")?)?;
            for extent in extents {
                encoder.u32(*extent)?;
            }
            encode_value_type(encoder, scalar, budget, next_depth(depth)?)
        }
        ValueTypeSyntaxKind::Array { element, extent } => {
            encoder.u8(3)?;
            encoder.u32(*extent)?;
            encode_value_type(encoder, element, budget, next_depth(depth)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identity_retains_scalar_domain_array_nesting_and_spatial_roles() {
        let mut identities = std::collections::BTreeSet::new();
        for value_type in [
            "V",
            "complex<V>",
            "array<V, 2>",
            "vector<V, 2>",
            "array<array<V, 2>, 2>",
            "array<vector<V, 2>, 2>",
            "tensor<V, 2, 2>",
        ] {
            for source in [
                format!("model M() {{ parameter value: {value_type} = 0; }}"),
                format!("component C(parameter value: {value_type}) {{  }}"),
            ] {
                let document = eqiora_lang::parse("types.eqi", &source)
                    .into_document()
                    .unwrap();
                let identity = LocalSourceIdentity::from_document(&document).unwrap();
                assert!(identities.insert(identity), "{value_type}");
                let formatted = eqiora_lang::format(&document);
                let reparsed = eqiora_lang::parse("moved.eqi", &formatted)
                    .into_document()
                    .unwrap();
                assert_eq!(
                    LocalSourceIdentity::from_document(&reparsed).unwrap(),
                    identity
                );
            }
        }
    }
}
