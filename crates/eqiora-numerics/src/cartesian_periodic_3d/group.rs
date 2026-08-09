use std::collections::BTreeSet;

use eqiora_core::entity::kinds;
use eqiora_core::{Id, RawId};
use eqiora_graph::EdgeKind;
use eqiora_schema::kernel::{BoundarySide, ConnectionSemantics, DomainKind, KernelNode};
use eqiora_sem::{BoundaryJunctionGeometry, KernelProgram};

use super::DIMENSION;

#[derive(Debug, Clone)]
pub(super) struct Generator {
    pub(super) connection: RawId,
    pub(super) lower_port: RawId,
    pub(super) upper_port: RawId,
    pub(super) parent: RawId,
    pub(super) connector: RawId,
    pub(super) axis: usize,
    pub(super) lower_coordinate: f64,
    pub(super) upper_coordinate: f64,
    pub(super) period: f64,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedPort {
    port: RawId,
    parent: RawId,
    connector: RawId,
    axis: usize,
    side: BoundarySide,
}

pub(super) fn derive_generator(
    program: &KernelProgram,
    connection: Id<kinds::Connection>,
) -> Result<Generator, String> {
    let junction = program
        .compose_boundary_physical_junction(connection)
        .map_err(|error| format!("constituent pair is invalid: {error}"))?;
    let BoundaryJunctionGeometry::CartesianPeriodic(identification) = junction.geometry() else {
        return Err("selected Connection is not spatial-periodic".to_owned());
    };
    if identification.ambient_dimension() != DIMENSION {
        return Err("selected pair is not three-dimensional".to_owned());
    }

    let mut resolved = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection.erase())
        .map(|edge| resolve_port(program, edge.to()))
        .collect::<Result<Vec<_>, _>>()?;
    if resolved.len() != 2 || resolved[0].port == resolved[1].port {
        return Err("spatial-periodic Connection must own two distinct Ports".to_owned());
    }
    resolved.sort_by_key(|port| match port.side {
        BoundarySide::Lower => 0,
        BoundarySide::Upper => 1,
    });
    let lower = resolved[0];
    let upper = resolved[1];
    if lower.side != BoundarySide::Lower
        || upper.side != BoundarySide::Upper
        || lower.parent != upper.parent
        || lower.connector != upper.connector
        || lower.axis != upper.axis
        || lower.axis != identification.normal_axis()
    {
        return Err("pair endpoint identity contradicts validated geometry".to_owned());
    }

    Ok(Generator {
        connection: connection.erase(),
        lower_port: lower.port,
        upper_port: upper.port,
        parent: lower.parent,
        connector: lower.connector,
        axis: identification.normal_axis(),
        lower_coordinate: identification.lower_coordinate(),
        upper_coordinate: identification.upper_coordinate(),
        period: identification.period(),
    })
}

fn resolve_port(program: &KernelProgram, port: RawId) -> Result<ResolvedPort, String> {
    let Some(KernelNode::Port(definition)) = program.node(port) else {
        return Err("periodic pair contains a non-Port member".to_owned());
    };
    let (connector, boundary) = definition
        .boundary_physical_contract()
        .ok_or_else(|| "periodic pair member is not boundary-physical".to_owned())?;
    let Some(KernelNode::Domain(boundary_definition)) = program.node(boundary.erase()) else {
        return Err("periodic Port support is not a Domain".to_owned());
    };
    let DomainKind::CartesianBoundary { axis, side } = boundary_definition.kind() else {
        return Err("periodic Port support is not a Cartesian boundary".to_owned());
    };
    let parents = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.from() == boundary.erase())
        .map(|edge| edge.to())
        .collect::<Vec<_>>();
    if parents.len() != 1 {
        return Err("Cartesian boundary must own one exact parent".to_owned());
    }
    Ok(ResolvedPort {
        port,
        parent: parents[0],
        connector: connector.erase(),
        axis: *axis,
        side: *side,
    })
}

pub(super) fn admit_group(
    program: &KernelProgram,
    generators: &mut [Generator],
) -> Result<(RawId, RawId), String> {
    if generators.len() != DIMENSION {
        return Err("three-generator profile requires exactly three pairs".to_owned());
    }
    if generators
        .iter()
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>()
        .len()
        != DIMENSION
    {
        return Err("three-generator profile requires distinct Connections".to_owned());
    }
    let parent = generators[0].parent;
    let connector = generators[0].connector;
    if generators
        .iter()
        .any(|generator| generator.parent != parent)
    {
        return Err("periodic pairs do not share one exact parent".to_owned());
    }
    if generators
        .iter()
        .any(|generator| generator.connector != connector)
    {
        return Err("periodic pairs do not share one exact Connector".to_owned());
    }
    if generators
        .iter()
        .flat_map(|generator| [generator.lower_port, generator.upper_port])
        .collect::<BTreeSet<_>>()
        .len()
        != 2 * DIMENSION
    {
        return Err("periodic group reuses a Port".to_owned());
    }
    generators.sort_by_key(|generator| generator.axis);
    if generators
        .iter()
        .map(|generator| generator.axis)
        .collect::<Vec<_>>()
        != [0, 1, 2]
    {
        return Err("periodic pairs do not cover axes {0,1,2} exactly once".to_owned());
    }

    let family = program
        .nodes()
        .filter_map(|node| match node {
            KernelNode::Connection(definition)
                if definition.semantics() == ConnectionSemantics::SpatialPeriodic =>
            {
                Some(definition.id())
            }
            _ => None,
        })
        .filter_map(|connection| derive_generator(program, connection).ok())
        .filter(|generator| generator.parent == parent && generator.connector == connector)
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>();
    let selected = generators
        .iter()
        .map(|generator| generator.connection)
        .collect::<BTreeSet<_>>();
    if family != selected {
        return Err("selected group does not exhaust its parent/Connector family".to_owned());
    }
    Ok((parent, connector))
}
