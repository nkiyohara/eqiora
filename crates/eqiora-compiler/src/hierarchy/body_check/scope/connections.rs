//! Exact endpoint contracts and directed connection validation.
use super::*;

pub(in crate::hierarchy::body_check) fn validate_connection(
    scope: &DefinitionScope<'_, '_>,
    declaration: &ConnectionDecl,
    connected_ports: &mut BTreeSet<Vec<String>>,
    connection_limits: ConnectionSetLimits,
) -> Result<Option<PhysicalConnectionFragment>, Diagnostic> {
    let paths = declaration
        .port_expressions()
        .iter()
        .map(|expression| {
            if matches!(expression.kind(), eqiora_lang::ExprKind::Member { .. }) {
                scope.indexed_member(expression).map(|(path, _)| path)
            } else {
                crate::source_endpoints::path(scope.file, expression)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut keys = Vec::with_capacity(paths.len());
    let mut contracts = Vec::with_capacity(paths.len());
    for (path, expression) in paths.iter().zip(declaration.port_expressions()) {
        keys.push(
            if matches!(expression.kind(), eqiora_lang::ExprKind::Member { .. }) {
                scope.indexed_member(expression)?.1
            } else {
                path.segments().map(str::to_owned).collect::<Vec<_>>()
            },
        );
        let mut contract = scope.resolve_port(path)?;
        if declaration.syntax() == ConnectionSyntax::Signal
            && scope.exposed_signals.contains(path.as_str())
            && let PortContract::Signal { direction, .. } = &mut contract
        {
            *direction = match direction {
                SignalDirectionSyntax::Input => SignalDirectionSyntax::Output,
                SignalDirectionSyntax::Output => SignalDirectionSyntax::Input,
            };
        }
        contracts.push(contract);
    }
    if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            "Connection repeats the same Port",
        ));
    }
    let scalar_physical = matches!(contracts.first(), Some(PortContract::Physical { .. }))
        && contracts
            .iter()
            .all(|contract| matches!(contract, PortContract::Physical { .. }));
    if scalar_physical {
        validate_connection_contract(declaration, &contracts, scope.file)?;
        let endpoints = keys.iter().map(|key| match key.as_slice() {
            [port] => ResolvedPhysicalEndpoint::Local(port.clone()),
            [instance, port] => ResolvedPhysicalEndpoint::Child {
                instance: instance.clone(),
                port: port.clone(),
            },
            _ => unreachable!("resolved visible Port keys have one or two segments"),
        });
        return ConnectionFragment::try_new(endpoints, connection_limits)
            .map(Some)
            .map_err(|error| connection_fragment_error(scope.file, declaration.range(), error));
    }
    let boundary_physical = matches!(
        contracts.first(),
        Some(PortContract::BoundaryPhysical { .. })
    ) && contracts
        .iter()
        .all(|contract| matches!(contract, PortContract::BoundaryPhysical { .. }));
    if boundary_physical {
        if declaration.syntax() != ConnectionSyntax::Conserving {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "field-physical Ports require a conserving Connection",
            ));
        }
        let Some(PortContract::BoundaryPhysical { nominal, .. }) = contracts.first() else {
            unreachable!("boundary-physical family was established");
        };
        if contracts.iter().skip(1).any(|contract| {
            !matches!(contract, PortContract::BoundaryPhysical { nominal: candidate, .. } if candidate == nominal)
        }) {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                scope.file,
                declaration.range(),
                "field-physical Connection requires the exact same specialized Connector",
            ));
        }
        let endpoints = keys.iter().map(|key| match key.as_slice() {
            [port] => ResolvedPhysicalEndpoint::Local(port.clone()),
            [instance, port] => ResolvedPhysicalEndpoint::Child {
                instance: instance.clone(),
                port: port.clone(),
            },
            _ => unreachable!("resolved visible Port keys have one or two segments"),
        });
        return ConnectionFragment::try_new(endpoints, connection_limits)
            .map(Some)
            .map_err(|error| connection_fragment_error(scope.file, declaration.range(), error));
    }
    let members = if declaration.syntax() == ConnectionSyntax::Signal {
        &keys[1..]
    } else {
        &keys[..]
    };
    if let Some(key) = members
        .iter()
        .find(|key| connected_ports.contains(key.as_slice()))
    {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            scope.file,
            declaration.range(),
            format!(
                "Port `{}` already belongs to another Connection",
                key.join(".")
            ),
        ));
    }
    validate_connection_contract(declaration, &contracts, scope.file)?;
    connected_ports.extend(members.iter().cloned());
    Ok(None)
}
