//! Target-directed named binding syntax has one canonical record family.
use super::*;
use eqiora_lang::InstanceDecl;

pub(super) fn encode_instance(
    encoder: &mut Encoder,
    declaration: &InstanceDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    if declaration.bindings().len() > budget.limits.max_bindings_per_instance {
        return Err(source_identity_error(
            "instance named binding count exceeds resource limit",
        ));
    }
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_type_path(encoder, declaration.definition(), budget)
    })?;
    let bindings = encode_sorted_records(declaration.bindings(), budget, |binding, budget| {
        if let eqiora_lang::ExprKind::Call {
            callee,
            arguments: eqiora_lang::CallArguments::Positional(arguments),
        } = binding.value().kind()
            && callee.as_str() == "boundaries"
        {
            budget.account_boundary_set_members(arguments.len())?;
        }
        let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
        encoder.field(1, |encoder| encode_name(encoder, binding.name(), budget))?;
        encoder.field(2, |encoder| {
            encode_expression(encoder, binding.value(), budget, 1)
        })?;
        encoder.finish()
    })?;
    encoder.field(3, |encoder| encoder.records(&bindings))?;
    if let Some(family) = declaration.family() {
        encoder.field(4, |encoder| {
            encode_name(encoder, family.member(), budget)?;
            encode_path(encoder, family.set(), budget)
        })?;
    }
    Ok(())
}
