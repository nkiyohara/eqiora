//! Signature ownership and category are part of source identity.
use super::*;
use eqiora_lang::SignatureItem;

pub(super) fn encode_signature(
    items: &[SignatureItem],
    budget: &mut Budget,
) -> Result<Vec<Vec<u8>>, Diagnostic> {
    encode_sorted_records(items, budget, |item, budget| {
        let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
        match item {
            SignatureItem::Parameter(value) => {
                encoder.u16(1)?;
                encode_component_parameter(&mut encoder, value, budget)?;
            }
            SignatureItem::Support(value) => {
                encoder.u16(2)?;
                encode_support_slot(&mut encoder, value, budget)?;
            }
            SignatureItem::Field(value) => {
                encoder.u16(3)?;
                encode_field(&mut encoder, value, budget)?;
            }
            SignatureItem::Clock(value) => {
                encoder.u16(4)?;
                encode_name(&mut encoder, value.name(), budget)?;
            }
            SignatureItem::Property(value) => {
                encoder.u16(5)?;
                encoder.field(1, |encoder| encode_name(encoder, value.name(), budget))?;
                encoder.field(2, |encoder| {
                    encode_type_path(encoder, value.contract(), budget)
                })?;
            }
            SignatureItem::Input(value) => {
                encoder.u16(6)?;
                encode_field(&mut encoder, value, budget)?;
            }
            SignatureItem::Output(value) => {
                encoder.u16(7)?;
                encode_field(&mut encoder, value, budget)?;
            }
            SignatureItem::Port(value) => {
                encoder.u16(8)?;
                encode_component_port(&mut encoder, value, budget)?;
            }
            SignatureItem::PortFamily(value) => {
                encoder.u16(9)?;
                encode_component_port_family(&mut encoder, value, budget)?;
            }
            _ => return Err(source_identity_error("unsupported signature item")),
        }
        encoder.finish()
    })
}
