use super::*;

pub(super) fn encode_parameter(
    encoder: &mut Encoder,
    declaration: &ParameterDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_expression(encoder, declaration.dimension(), budget, 1)
    })?;
    match declaration.value().kind() {
        ExprKind::Number(value) => encoder.field(3, |encoder| encoder.f64(*value)),
        ExprKind::Quantity { value, unit } => {
            encoder.field(3, |encoder| encoder.f64(*value))?;
            encoder.field(4, |encoder| encode_expression(encoder, unit, budget, 1))
        }
        _ => Err(source_identity_error(
            "parameter value must be a numeric or quantity literal",
        )),
    }
}

pub(super) fn encode_let(
    encoder: &mut Encoder,
    item: &Item,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let Item::Let(declaration) = item else {
        return Err(source_identity_error(
            "let encoder received a non-let model item",
        ));
    };
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    if let Some(dimension) = declaration.dimension() {
        encoder.field(2, |encoder| {
            encode_expression(encoder, dimension, budget, 1)
        })?;
    }
    encoder.field(3, |encoder| {
        encode_expression(encoder, declaration.value(), budget, 1)
    })
}
