use eqiora_core::Diagnostic;
use eqiora_lang::{BoundarySetBindingDecl, InstanceDecl};

use super::{
    Budget, Encoder, encode_expression, encode_name, encode_sorted_records, encode_type_path,
    source_identity_error,
};

pub(super) fn encode_instance(
    encoder: &mut Encoder,
    declaration: &InstanceDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let binding_count = declaration
        .bindings()
        .len()
        .checked_add(declaration.support_bindings().len())
        .and_then(|count| count.checked_add(declaration.boundary_set_bindings().len()))
        .and_then(|count| count.checked_add(declaration.field_bindings().len()))
        .and_then(|count| count.checked_add(declaration.property_binding_syntax().len()))
        .ok_or_else(|| source_identity_error("instance binding count overflows usize"))?;
    if binding_count > budget.limits.max_bindings_per_instance {
        return Err(source_identity_error(format!(
            "instance `{}` has {} bindings, exceeding the {} binding limit",
            declaration.name(),
            binding_count,
            budget.limits.max_bindings_per_instance
        )));
    }
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_type_path(encoder, declaration.definition(), budget)
    })?;
    let bindings = encode_sorted_records(declaration.bindings(), budget, |binding, budget| {
        let mut binding_encoder = Encoder::new(budget.limits.max_canonical_bytes);
        binding_encoder.field(1, |encoder| {
            encode_name(encoder, binding.parameter(), budget)
        })?;
        binding_encoder.field(2, |encoder| {
            encode_expression(encoder, binding.value(), budget, 1)
        })?;
        binding_encoder.finish()
    })?;
    encoder.field(3, |encoder| encoder.records(&bindings))?;
    if !declaration.support_bindings().is_empty() {
        let support_bindings =
            encode_sorted_records(declaration.support_bindings(), budget, |binding, budget| {
                let mut binding_encoder = Encoder::new(budget.limits.max_canonical_bytes);
                binding_encoder.field(1, |encoder| encode_name(encoder, binding.slot(), budget))?;
                binding_encoder
                    .field(2, |encoder| encode_name(encoder, binding.target(), budget))?;
                binding_encoder.finish()
            })?;
        encoder.field(4, |encoder| encoder.records(&support_bindings))?;
    }
    if !declaration.field_bindings().is_empty() {
        let field_bindings =
            encode_sorted_records(declaration.field_bindings(), budget, |binding, budget| {
                let mut binding_encoder = Encoder::new(budget.limits.max_canonical_bytes);
                binding_encoder.field(1, |encoder| encode_name(encoder, binding.slot(), budget))?;
                binding_encoder
                    .field(2, |encoder| encode_name(encoder, binding.target(), budget))?;
                binding_encoder.finish()
            })?;
        encoder.field(5, |encoder| encoder.records(&field_bindings))?;
    }
    if !declaration.boundary_set_bindings().is_empty() {
        let boundary_set_bindings = encode_sorted_records(
            declaration.boundary_set_bindings(),
            budget,
            encode_boundary_set_binding,
        )?;
        // Append-only optional field: legacy instances retain their exact v1
        // records when no complete-exterior binding is present.
        encoder.field(6, |encoder| encoder.records(&boundary_set_bindings))?;
    }
    let property_syntax = declaration.property_binding_syntax().collect::<Vec<_>>();
    if !property_syntax.is_empty() {
        let property_bindings =
            encode_sorted_records(&property_syntax, budget, |binding, budget| {
                let (property, release, _) = *binding;
                let mut value = Encoder::new(budget.limits.max_canonical_bytes);
                value.field(1, |encoder| encode_name(encoder, property, budget))?;
                value.field(2, |encoder| encode_type_path(encoder, release, budget))?;
                value.finish()
            })?;
        encoder.field(7, |encoder| encoder.records(&property_bindings))?;
    }
    Ok(())
}

fn encode_boundary_set_binding(
    binding: &BoundarySetBindingDecl,
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    budget.account_boundary_set_members(binding.members().len())?;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| encode_name(encoder, binding.slot(), budget))?;
    let members = encode_sorted_records(binding.members(), budget, |member, budget| {
        let mut member_encoder = Encoder::new(budget.limits.max_canonical_bytes);
        encode_name(&mut member_encoder, member.target(), budget)?;
        member_encoder.finish()
    })?;
    encoder.field(2, |encoder| encoder.records(&members))?;
    encoder.finish()
}
