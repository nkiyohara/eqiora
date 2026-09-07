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
    let mut failed = BTreeSet::new();
    for &port in incoming.keys() {
        if resolved.contains_key(&port) || failed.contains(&port) {
            continue;
        }
        let mut current = port;
        let mut path = Vec::new();
        let mut seen = BTreeSet::new();
        let origin = loop {
            if let Some(&origin) = resolved.get(&current) {
                break Some(origin);
            }
            if failed.contains(&current) {
                break None;
            }
            if !seen.insert(current) {
                errors.push(kernel_error(current, "signal driver cycle"));
                break None;
            }
            let Some(&next) = incoming.get(&current) else {
                break Some(current);
            };
            path.push(current);
            current = next;
        };
        if let Some(origin) = origin {
            let valid_origin = matches!(nodes.get(&origin), Some(KernelNode::Port(definition)) if definition.signal_contract().is_some_and(|(direction, _)| direction == SignalDirection::Output || boundary.contains(&origin)));
            if valid_origin {
                for port in path {
                    resolved.insert(port, origin);
                }
                continue;
            }
            errors.push(kernel_error(origin, "signal relay chain has no effective driver; an internal Input needs an incoming connection"));
        }
        failed.extend(path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{ScalarDomain, ValueType};
    use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora_schema::kernel::{ConnectionDef, PortDef};

    fn resolve(
        links: &[(usize, usize)],
        boundary: &[usize],
        foreign_driver: bool,
    ) -> Result<BTreeMap<RawId, RawId>, Vec<Diagnostic>> {
        let count = links
            .iter()
            .flat_map(|&(source, sink)| [source, sink])
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(3);
        let ports = (0..count)
            .map(|_| Id::<kinds::Port>::new())
            .collect::<Vec<_>>();
        let mut nodes = BTreeMap::new();
        let mut transaction = Transaction::new("directed signal relay");
        for (index, &port) in ports.iter().enumerate() {
            let direction = if index + 1 == count {
                SignalDirection::Output
            } else {
                SignalDirection::Input
            };
            let node = KernelNode::from(PortDef::signal(
                port,
                direction,
                ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            ));
            nodes.insert(node.id(), node.clone());
            transaction.push(Op::DefineKernelNode { node });
        }
        for &(source, sink) in links {
            let id = Id::<kinds::Connection>::new();
            let driver = if foreign_driver {
                Id::new()
            } else {
                ports[source]
            };
            let node = KernelNode::from(ConnectionDef::new(
                id,
                ConnectionSemantics::Signal { driver },
            ));
            nodes.insert(node.id(), node.clone());
            transaction.push(Op::DefineKernelNode { node });
            for port in [ports[source], ports[sink]] {
                transaction.push(Op::Connect {
                    from: id.erase(),
                    to: port.erase(),
                    edge: EdgeKind::Connects,
                });
            }
        }
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let edges = store.snapshot().edges().copied().collect::<Vec<_>>();
        sources(
            &nodes,
            &edges,
            &boundary.iter().map(|&index| ports[index].erase()).collect(),
        )
    }

    #[test]
    fn long_relay_chain_shares_one_origin_and_reports_one_cycle() {
        let mut links = (0..511).map(|index| (index, index + 1)).collect::<Vec<_>>();
        let resolved = resolve(&links, &[0], false).unwrap();
        assert_eq!(resolved.len(), 511);
        assert_eq!(resolved.values().collect::<BTreeSet<_>>().len(), 1);
        links.push((511, 1));
        let errors = resolve(&links, &[0], false).unwrap_err();
        assert_eq!(
            errors
                .iter()
                .filter(|error| error.message().contains("cycle"))
                .count(),
            1
        );
    }

    #[test]
    fn explicit_relays_require_member_driver_and_one_acyclic_external_origin() {
        let resolved = resolve(&[(0, 1), (1, 2)], &[0], false).unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved.values().collect::<BTreeSet<_>>().len(), 1);
        for (links, boundary, foreign, expected) in [
            (vec![(0, 1), (1, 2)], vec![0], true, "connected signal Port"),
            (vec![(0, 1), (1, 2)], vec![], false, "no effective driver"),
            (vec![(0, 1), (1, 0)], vec![], false, "cycle"),
            (
                vec![(0, 1), (2, 1)],
                vec![0],
                false,
                "more than one incoming driver",
            ),
            (vec![(2, 0), (0, 1)], vec![0], false, "boundary Input"),
        ] {
            let errors = resolve(&links, &boundary, foreign).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains(expected)),
                "{errors:?}"
            );
        }
    }
}
