use super::*;

pub(super) fn lower_port(
    file: &str,
    range: TextRange,
    id: Id<kinds::Port>,
    contract: &PortContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<PortDef, Diagnostic> {
    match resolve_port_contract(file, range, contract, bindings)? {
        ResolvedPortContract::Signal {
            direction: SignalDirectionSyntax::Input,
            value_type,
        } => Ok(PortDef::signal(id, SignalDirection::Input, value_type)),
        ResolvedPortContract::Signal {
            direction: SignalDirectionSyntax::Output,
            value_type,
        } => Ok(PortDef::signal(id, SignalDirection::Output, value_type)),
        ResolvedPortContract::ScalarPhysical { domain, .. } => {
            Ok(PortDef::scalar_physical(id, domain))
        }
        ResolvedPortContract::BoundaryPhysical {
            connector,
            boundary,
            ..
        } => Ok(PortDef::boundary_physical(id, connector, boundary)),
    }
}

pub(super) fn lower_clock(
    period: eqiora_lang::RationalSyntax,
    phase: eqiora_lang::RationalSyntax,
) -> Result<(RationalTime, RationalTime), Diagnostic> {
    Ok((
        RationalTime::new(period.numerator(), period.denominator())?,
        RationalTime::new(phase.numerator(), phase.denominator())?,
    ))
}
