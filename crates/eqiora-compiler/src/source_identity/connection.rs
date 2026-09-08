//! Canonical explicit connection syntax, including additive indexed binders.
use super::*;

pub(super) fn encode_connection(
    encoder: &mut Encoder,
    declaration: &ConnectionDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    budget.check_connection_members(declaration.port_expressions().len(), "Connection")?;
    encoder.field(1, |encoder| {
        encoder.u8(match declaration.syntax() {
            ConnectionSyntax::Signal => 1,
            ConnectionSyntax::Conserving => 2,
            ConnectionSyntax::SpatialPeriodic => 3,
        })
    })?;
    encoder.field(2, |encoder| match declaration.syntax() {
        ConnectionSyntax::Conserving => {
            let paths = encode_sorted_endpoints(declaration.port_expressions(), budget)?;
            encoder.records(&paths)
        }
        ConnectionSyntax::Signal => {
            let Some((output, inputs)) = declaration.port_expressions().split_first() else {
                return Err(source_identity_error(
                    "signal Connection has no output member",
                ));
            };
            encoder.field(1, |encoder| encode_expression(encoder, output, budget, 0))?;
            let inputs = encode_sorted_endpoints(inputs, budget)?;
            encoder.field(2, |encoder| encoder.records(&inputs))
        }
        ConnectionSyntax::SpatialPeriodic => {
            let paths = encode_sorted_endpoints(declaration.port_expressions(), budget)?;
            encoder.records(&paths)
        }
    })?;
    if let Some(binder) = declaration.binder() {
        encoder.field(3, |encoder| {
            encode_boundary_family_binder(encoder, binder, budget)
        })?;
    }
    Ok(())
}
