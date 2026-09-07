//! Resolve explicit causal Connection drivers without erasing interface endpoints.

use super::*;
use eqiora_schema::kernel::SignalDirection;

pub(crate) fn sources(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    boundary: &BTreeSet<RawId>,
) -> Result<BTreeMap<RawId, RawId>, Vec<Diagnostic>> {
    let mut incoming = BTreeMap::new();
    let mut errors = Vec::new();
    for (&id, node) in nodes {
        let KernelNode::Connection(connection) = node else {
            continue;
        };
        let ConnectionSemantics::Signal { driver } = connection.semantics() else {
            continue;
        };
        let driver = driver.erase();
        let ports = edge_targets(edges, id, EdgeKind::Connects);
        if !ports.contains(&driver)
            || !matches!(nodes.get(&driver), Some(KernelNode::Port(port)) if port.signal_contract().is_some())
        {
            errors.push(kernel_error(
                id,
                "signal driver must be a connected signal Port",
            ));
            continue;
        }
        for port in ports.into_iter().filter(|port| *port != driver) {
            if incoming.insert(port, driver).is_some() {
                errors.push(kernel_error(
                    port,
                    "signal Port has more than one incoming driver",
                ));
            }
            if boundary.contains(&port)
                && matches!(nodes.get(&port), Some(KernelNode::Port(port)) if port.signal_contract().is_some_and(|(direction, _)| direction == SignalDirection::Input))
            {
                errors.push(kernel_error(
                    port,
                    "boundary Input is an external driver and cannot have an incoming connection",
                ));
            }
        }
    }
    for (&port, node) in nodes {
        if !boundary.contains(&port)
            && !incoming.contains_key(&port)
            && matches!(node, KernelNode::Port(definition) if definition.signal_contract().is_some_and(|(direction, _)| direction == SignalDirection::Input))
        {
            errors.push(kernel_error(port, "internal Input has no incoming driver"));
        }
    }
    let mut resolved = BTreeMap::new();
    for (&port, &source) in &incoming {
        let mut terminal = source;
        let mut seen = BTreeSet::from([port]);
        while let Some(&next) = incoming.get(&terminal) {
            if !seen.insert(terminal) {
                errors.push(kernel_error(port, "signal driver cycle"));
                break;
            }
            terminal = next;
        }
        let valid_origin = matches!(nodes.get(&terminal), Some(KernelNode::Port(definition)) if definition.signal_contract().is_some_and(|(direction, _)| direction == SignalDirection::Output || boundary.contains(&terminal)));
        if !valid_origin {
            errors.push(kernel_error(port, "signal relay chain has no effective driver; an internal Input needs an incoming connection"));
        }
        resolved.insert(port, terminal);
    }
    if errors.is_empty() {
        Ok(resolved)
    } else {
        Err(errors)
    }
}

pub(crate) fn program_sources(
    program: &KernelProgram,
) -> Result<BTreeMap<RawId, RawId>, Vec<Diagnostic>> {
    sources(&program.nodes, &program.edges, &program.boundary)
}
