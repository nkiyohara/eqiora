//! Canonical scalar conserving-network composition (RFC 0024 Phase 1).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, GraphPath, Id, RawId};
use eqiora_graph::{Edge, EdgeKind};
use eqiora_schema::kernel::physical_closure::{
    PhysicalClosureViolation, PhysicalEndpointSlots, PhysicalSlot,
};
use eqiora_schema::kernel::{
    ActivationKind, ConnectionSemantics, DomainKind, ExprDag, ExprDagBuilder, ExprNode, KernelNode,
    SymbolRef,
};

use crate::KernelProgram;
use crate::evaluate::evaluate_expression;

/// Canonical identity of one closed scalar physical subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScalarPhysicalSubsystemId(Id<kinds::Connection>);

impl ScalarPhysicalSubsystemId {
    /// Lowest canonical Connection ID in the subsystem.
    #[must_use]
    pub const fn connection(self) -> Id<kinds::Connection> {
        self.0
    }
}

/// One canonical unknown slot in a scalar physical subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalUnknown {
    /// Across variable of a Port.
    Across(Id<kinds::Port>),
    /// Through variable, positive from the junction into the owning Relation.
    Through(Id<kinds::Port>),
}

impl PhysicalUnknown {
    /// Port whose canonical across or through slot this value names.
    #[must_use]
    pub const fn port(self) -> Id<kinds::Port> {
        match self {
            Self::Across(port) | Self::Through(port) => port,
        }
    }

    const fn role(self) -> u8 {
        match self {
            Self::Across(_) => 0,
            Self::Through(_) => 1,
        }
    }
}

impl PartialOrd for PhysicalUnknown {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PhysicalUnknown {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.port()
            .erase()
            .cmp(&other.port().erase())
            .then_with(|| self.role().cmp(&other.role()))
    }
}

/// One participating constitutive Relation and its original root ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationResidual {
    relation: Id<kinds::Relation>,
    dag: ExprDag,
}

impl RelationResidual {
    /// Relation owning this residual group.
    #[must_use]
    pub const fn relation(&self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Original validated Relation DAG.
    #[must_use]
    pub const fn dag(&self) -> &ExprDag {
        &self.dag
    }
}

/// Deterministically generated equality and conservation residuals.
#[derive(Debug, Clone, PartialEq)]
pub struct JunctionResidual {
    connection: Id<kinds::Connection>,
    dag: ExprDag,
}

impl JunctionResidual {
    /// Connection owning this generated group.
    #[must_use]
    pub const fn connection(&self) -> Id<kinds::Connection> {
        self.connection
    }

    /// Across-equality roots followed by one left-associated through sum.
    #[must_use]
    pub const fn dag(&self) -> &ExprDag {
        &self.dag
    }
}

/// Immutable canonical residual system for one closed physical subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedResidualSystem {
    subsystem: ScalarPhysicalSubsystemId,
    unknowns: Vec<PhysicalUnknown>,
    parameters: Vec<Id<kinds::Parameter>>,
    uses_time: bool,
    relations: Vec<RelationResidual>,
    junctions: Vec<JunctionResidual>,
}

impl ComposedResidualSystem {
    /// Canonical subsystem identity.
    #[must_use]
    pub const fn subsystem(&self) -> ScalarPhysicalSubsystemId {
        self.subsystem
    }

    /// Across/Through pairs in canonical Port order.
    #[must_use]
    pub fn unknowns(&self) -> &[PhysicalUnknown] {
        &self.unknowns
    }

    /// Sorted unique known Parameters read by participating Relations.
    #[must_use]
    pub fn parameters(&self) -> &[Id<kinds::Parameter>] {
        &self.parameters
    }

    /// Whether any participating Relation reads model time.
    #[must_use]
    pub const fn uses_time(&self) -> bool {
        self.uses_time
    }

    /// Participating Relations in canonical ID order.
    #[must_use]
    pub fn relations(&self) -> &[RelationResidual] {
        &self.relations
    }

    /// Junction residual groups in canonical Connection order.
    #[must_use]
    pub fn junctions(&self) -> &[JunctionResidual] {
        &self.junctions
    }

    /// Evaluate every canonical residual root through the reference DAG
    /// evaluator at one explicit scalar point.
    ///
    /// Unknown values follow [`Self::unknowns`], parameter values follow
    /// [`Self::parameters`], and roots follow the relation-then-junction order
    /// owned by this composed system. Time is required exactly when
    /// [`Self::uses_time`] is true. This path does not lower, regroup, or
    /// regenerate any residual DAG.
    ///
    /// # Errors
    /// Returns `EQ0502` for an input-shape or time-presence mismatch and
    /// `EQ0505` for a non-finite input or intermediate value.
    pub fn evaluate_reference(
        &self,
        unknown_values: &[f64],
        parameter_values: &[f64],
        time: Option<f64>,
    ) -> Result<Vec<f64>, Diagnostic> {
        if unknown_values.len() != self.unknowns.len()
            || parameter_values.len() != self.parameters.len()
        {
            return Err(physical_input_error(format!(
                "scalar physical residual evaluation expects {}/{} unknown/parameter values, received {}/{}",
                self.unknowns.len(),
                self.parameters.len(),
                unknown_values.len(),
                parameter_values.len(),
            )));
        }
        match (self.uses_time, time) {
            (true, None) => {
                return Err(physical_input_error(
                    "time-dependent scalar physical residual evaluation requires model time",
                ));
            }
            (false, Some(_)) => {
                return Err(physical_input_error(
                    "time-independent scalar physical residual evaluation does not accept model time",
                ));
            }
            (true, Some(value)) if !value.is_finite() => {
                return Err(physical_nonfinite_error("model time must be finite"));
            }
            (true, Some(_)) | (false, None) => {}
        }
        if unknown_values
            .iter()
            .chain(parameter_values)
            .any(|value| !value.is_finite())
        {
            return Err(physical_nonfinite_error(
                "scalar physical residual inputs must be finite",
            ));
        }

        let input_count = self
            .unknowns
            .len()
            .checked_add(self.parameters.len())
            .and_then(|count| count.checked_add(usize::from(self.uses_time)))
            .ok_or_else(|| physical_input_error("scalar physical input count overflowed"))?;
        let mut inputs = HashMap::new();
        inputs
            .try_reserve(input_count)
            .map_err(|_| physical_input_error("could not reserve scalar physical input lookup"))?;
        for (unknown, value) in self.unknowns.iter().zip(unknown_values) {
            let symbol = match unknown {
                PhysicalUnknown::Across(port) => SymbolRef::Across(*port),
                PhysicalUnknown::Through(port) => SymbolRef::Through(*port),
            };
            inputs.insert(symbol, *value);
        }
        for (parameter, value) in self.parameters.iter().zip(parameter_values) {
            inputs.insert(SymbolRef::Parameter(*parameter), *value);
        }
        if let Some(value) = time {
            inputs.insert(SymbolRef::Time, value);
        }

        let residual_count = self
            .relations
            .iter()
            .map(|residual| residual.dag().roots().len())
            .chain(
                self.junctions
                    .iter()
                    .map(|residual| residual.dag().roots().len()),
            )
            .try_fold(0usize, usize::checked_add)
            .ok_or_else(|| physical_input_error("scalar physical residual count overflowed"))?;
        let mut residuals = Vec::new();
        residuals.try_reserve_exact(residual_count).map_err(|_| {
            physical_input_error("could not reserve scalar physical residual output")
        })?;
        for relation in &self.relations {
            residuals.extend(evaluate_expression(
                relation.relation().erase(),
                relation.dag(),
                &mut |symbol| inputs.get(&symbol).copied(),
            )?);
        }
        for junction in &self.junctions {
            residuals.extend(evaluate_expression(
                junction.connection().erase(),
                junction.dag(),
                &mut |symbol| inputs.get(&symbol).copied(),
            )?);
        }
        Ok(residuals)
    }
}

fn physical_input_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::MISSING_EXECUTION_INPUT, message).with_graph_path(GraphPath::new([
        "semantic",
        "scalar-physical",
        "inputs",
    ]))
}

fn physical_nonfinite_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NONFINITE_EVALUATION, message).with_graph_path(GraphPath::new([
        "semantic",
        "scalar-physical",
        "inputs",
    ]))
}

impl KernelProgram {
    /// Compose the closed scalar physical subsystem containing `connection`.
    ///
    /// The immutable result is canonical and is the only Phase 1 handoff to
    /// later interpreter or Operator-IR execution work.
    ///
    /// # Errors
    /// Returns `EQ0302` when the selected Connection is not a validated scalar
    /// physical junction in this program.
    pub fn compose_scalar_physical_subsystem(
        &self,
        connection: Id<kinds::Connection>,
    ) -> Result<ComposedResidualSystem, Diagnostic> {
        let system = self.compose_scalar_physical_execution_subsystem(connection)?;
        for relation in system.relations() {
            if relation
                .dag()
                .nodes()
                .iter()
                .any(|node| !is_static_physical_node(node))
            {
                return Err(kernel_error(
                    relation.relation().erase(),
                    "static scalar physical composition does not admit state, derivative, signal, hybrid, or spatial expressions",
                ));
            }
        }
        Ok(system)
    }

    /// Compose one scalar physical closure for the joint reference-time
    /// execution contract. Original Relation DAGs are retained unchanged;
    /// only the deterministic junction residuals are generated here.
    pub(crate) fn compose_scalar_physical_execution_subsystem(
        &self,
        connection: Id<kinds::Connection>,
    ) -> Result<ComposedResidualSystem, Diagnostic> {
        let seed = connection.erase();
        if !is_physical_connection(self, seed) {
            return Err(kernel_error(
                seed,
                "selected Connection is not a scalar physical junction",
            ));
        }

        let mut connections = BTreeSet::from([seed]);
        let mut ports = BTreeSet::new();
        let mut relations = BTreeSet::new();
        loop {
            let before = (connections.len(), ports.len(), relations.len());

            for edge in self.edges() {
                if edge.kind() == EdgeKind::Connects && connections.contains(&edge.from()) {
                    ports.insert(edge.to());
                }
            }
            for edge in self.edges() {
                if edge.kind() == EdgeKind::HasPort && ports.contains(&edge.to()) {
                    relations.insert(edge.from());
                }
            }
            for node in self.nodes() {
                let KernelNode::Relation(relation) = node else {
                    continue;
                };
                if expression_physical_ports(relation.residuals())
                    .iter()
                    .any(|port| ports.contains(port))
                {
                    relations.insert(relation.id().erase());
                }
            }
            for &relation in &relations {
                for edge in self.edges() {
                    if edge.from() == relation
                        && edge.kind() == EdgeKind::HasPort
                        && is_physical_port(self, edge.to())
                    {
                        ports.insert(edge.to());
                    }
                }
                let Some(KernelNode::Relation(definition)) = self.node(relation) else {
                    return Err(kernel_error(
                        relation,
                        "validated physical Relation definition is missing",
                    ));
                };
                ports.extend(expression_physical_ports(definition.residuals()));
            }
            for edge in self.edges() {
                if edge.kind() == EdgeKind::Connects && ports.contains(&edge.to()) {
                    connections.insert(edge.from());
                }
            }

            if before == (connections.len(), ports.len(), relations.len()) {
                break;
            }
        }

        let subsystem = connections
            .first()
            .copied()
            .and_then(RawId::downcast::<kinds::Connection>)
            .map(ScalarPhysicalSubsystemId)
            .ok_or_else(|| kernel_error(seed, "physical subsystem has no Connection identity"))?;

        let unknown_capacity = ports.len().checked_mul(2).ok_or_else(|| {
            kernel_error(seed, "physical unknown count overflows the platform size")
        })?;
        let mut unknowns = Vec::with_capacity(unknown_capacity);
        for port in &ports {
            let port = port.downcast::<kinds::Port>().ok_or_else(|| {
                kernel_error(*port, "physical subsystem contains a non-Port member")
            })?;
            unknowns.push(PhysicalUnknown::Across(port));
            unknowns.push(PhysicalUnknown::Through(port));
        }

        let mut parameters = BTreeSet::new();
        let mut uses_time = false;
        let mut relation_residuals = Vec::with_capacity(relations.len());
        for relation in relations {
            let Some(KernelNode::Relation(definition)) = self.node(relation) else {
                return Err(kernel_error(
                    relation,
                    "physical Relation definition is missing",
                ));
            };
            for node in definition.residuals().nodes() {
                if let ExprNode::Symbol(symbol) = node {
                    match symbol {
                        SymbolRef::Parameter(parameter) => {
                            parameters.insert(parameter.erase());
                        }
                        SymbolRef::Time => uses_time = true,
                        _ => {}
                    }
                }
            }
            relation_residuals.push(RelationResidual {
                relation: relation.downcast::<kinds::Relation>().ok_or_else(|| {
                    kernel_error(
                        relation,
                        "physical subsystem contains a non-Relation member",
                    )
                })?,
                dag: definition.residuals().clone(),
            });
        }

        let parameters = parameters
            .into_iter()
            .map(|parameter| {
                parameter.downcast::<kinds::Parameter>().ok_or_else(|| {
                    kernel_error(
                        parameter,
                        "physical subsystem contains a non-Parameter symbol",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let junctions = connections
            .into_iter()
            .map(|connection| compose_junction(self, connection))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ComposedResidualSystem {
            subsystem,
            unknowns,
            parameters,
            uses_time,
            relations: relation_residuals,
            junctions,
        })
    }
}

fn is_static_physical_node(node: &ExprNode) -> bool {
    matches!(
        node,
        ExprNode::Constant(_)
            | ExprNode::Neg(_)
            | ExprNode::Add(_, _)
            | ExprNode::Sub(_, _)
            | ExprNode::Mul(_, _)
            | ExprNode::Div(_, _)
            | ExprNode::PowI(_, _)
            | ExprNode::UnaryMath(_, _)
            | ExprNode::Symbol(
                SymbolRef::Parameter(_)
                    | SymbolRef::Time
                    | SymbolRef::Across(_)
                    | SymbolRef::Through(_),
            )
    )
}

pub(crate) fn validate_scalar_physical_networks(
    nodes: &BTreeMap<RawId, KernelNode>,
    edges: &[Edge],
    boundary: &BTreeSet<RawId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (&id, node) in nodes {
        let is_scalar_physical_domain = matches!(
            node,
            KernelNode::Domain(domain)
                if matches!(domain.kind(), DomainKind::ScalarPhysical { .. })
        );
        if is_scalar_physical_domain
            && edges.iter().any(|edge| {
                (edge.to() == id
                    && matches!(
                        edge.kind(),
                        EdgeKind::DefinedOn | EdgeKind::AppliesOn | EdgeKind::BoundaryOf
                    ))
                    || (edge.from() == id && edge.kind() == EdgeKind::BoundaryOf)
            })
        {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Domain cannot be spatial support or a boundary",
            ));
        }

        let KernelNode::Port(port) = node else {
            continue;
        };
        let Some(domain) = port.physical_domain() else {
            continue;
        };
        if !matches!(
            nodes.get(&domain.erase()),
            Some(KernelNode::Domain(domain))
                if matches!(domain.kind(), DomainKind::ScalarPhysical { .. })
        ) {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Port must name a scalar physical Domain",
            ));
        }
        let owners = edge_sources(edges, id, EdgeKind::HasPort);
        let memberships = edge_sources(edges, id, EdgeKind::Connects);
        let mut closure = PhysicalEndpointSlots::open();
        let owner_overfill = owners
            .iter()
            .any(|_| closure.fill_owner() == Err(PhysicalClosureViolation::MultipleOwners));
        let membership_overfill = memberships.iter().any(|_| {
            closure.fill_membership() == Err(PhysicalClosureViolation::MultipleMemberships)
        });
        if owner_overfill || closure.owner() == PhysicalSlot::Open {
            diagnostics.push(kernel_error(
                id,
                format!(
                    "scalar physical Port requires exactly one owning Relation, found {}",
                    owners.len()
                ),
            ));
        }
        if membership_overfill || closure.membership() == PhysicalSlot::Open {
            diagnostics.push(kernel_error(
                id,
                format!(
                    "scalar physical Port requires exactly one Connection membership, found {}",
                    memberships.len()
                ),
            ));
        }
        if boundary.contains(&id) {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Port cannot be a Phase 1 model-root boundary",
            ));
        }
    }

    for (&id, node) in nodes {
        let KernelNode::Relation(relation) = node else {
            continue;
        };
        let owned_ports = edge_targets(edges, id, EdgeKind::HasPort);
        let physical_symbols = expression_physical_ports(relation.residuals());
        let participates = !physical_symbols.is_empty()
            || owned_ports.iter().any(|port| {
                matches!(
                    nodes.get(port),
                    Some(KernelNode::Port(port)) if port.physical_domain().is_some()
                )
            });
        if !participates {
            continue;
        }

        if !edge_targets(edges, id, EdgeKind::AppliesOn).is_empty() {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Relation cannot have spatial AppliesOn scope",
            ));
        }
        if owned_ports.iter().any(|port| {
            !matches!(
                nodes.get(port),
                Some(KernelNode::Port(port))
                    if port.physical_domain().is_some() || port.signal_contract().is_some()
            )
        }) {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Relation may own only scalar physical or causal signal Ports",
            ));
        }
        let activations = edge_sources(edges, id, EdgeKind::Activates);
        if activations.len() != 1
            || !matches!(
                activations.first().and_then(|activation| nodes.get(activation)),
                Some(KernelNode::Activation(activation))
                    if matches!(activation.kind(), ActivationKind::Continuous)
            )
        {
            diagnostics.push(kernel_error(
                id,
                "scalar physical Relation requires exactly one continuous Activation",
            ));
        }

        for (index, node) in relation.residuals().nodes().iter().enumerate() {
            let invalid = !matches!(
                node,
                ExprNode::Constant(_)
                    | ExprNode::Neg(_)
                    | ExprNode::Add(_, _)
                    | ExprNode::Sub(_, _)
                    | ExprNode::Mul(_, _)
                    | ExprNode::Div(_, _)
                    | ExprNode::PowI(_, _)
                    | ExprNode::UnaryMath(_, _)
                    | ExprNode::Symbol(
                        SymbolRef::Parameter(_)
                            | SymbolRef::Time
                            | SymbolRef::Field(_)
                            | SymbolRef::Derivative(_)
                            | SymbolRef::Port(_)
                            | SymbolRef::Across(_)
                            | SymbolRef::Through(_),
                    )
            );
            if invalid {
                diagnostics.push(
                    kernel_error(
                        id,
                        "scalar physical Relation contains an unsupported hybrid or spatial expression",
                    )
                    .with_graph_path(expression_path(id, index)),
                );
            }
        }
    }
}

fn compose_junction(
    program: &KernelProgram,
    connection: RawId,
) -> Result<JunctionResidual, Diagnostic> {
    let ports = edge_targets(program.edges(), connection, EdgeKind::Connects)
        .into_iter()
        .map(|port| {
            port.downcast::<kinds::Port>()
                .ok_or_else(|| kernel_error(port, "physical junction contains a non-Port member"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let anchor = ports
        .first()
        .copied()
        .ok_or_else(|| kernel_error(connection, "physical junction has no anchor Port"))?;
    let mut builder = ExprDagBuilder::new();
    let anchor_across = builder.symbol(SymbolRef::Across(anchor))?;
    let mut roots = Vec::with_capacity(ports.len());
    for port in ports.iter().skip(1) {
        let across = builder.symbol(SymbolRef::Across(*port))?;
        roots.push(builder.sub(across, anchor_across)?);
    }
    let mut through = builder.symbol(SymbolRef::Through(anchor))?;
    for port in ports.iter().skip(1) {
        let next = builder.symbol(SymbolRef::Through(*port))?;
        through = builder.add(through, next)?;
    }
    roots.push(through);
    Ok(JunctionResidual {
        connection: connection.downcast::<kinds::Connection>().ok_or_else(|| {
            kernel_error(
                connection,
                "physical junction has a non-Connection identity",
            )
        })?,
        dag: builder.finish(roots)?,
    })
}

fn is_physical_connection(program: &KernelProgram, connection: RawId) -> bool {
    matches!(
        program.node(connection),
        Some(KernelNode::Connection(definition))
            if definition.semantics() == ConnectionSemantics::Conserving
    ) && edge_targets(program.edges(), connection, EdgeKind::Connects)
        .iter()
        .all(|port| {
            matches!(
                program.node(*port),
                Some(KernelNode::Port(port)) if port.physical_domain().is_some()
            )
        })
}

fn is_physical_port(program: &KernelProgram, port: RawId) -> bool {
    matches!(
        program.node(port),
        Some(KernelNode::Port(definition)) if definition.physical_domain().is_some()
    )
}

fn expression_physical_ports(expression: &ExprDag) -> BTreeSet<RawId> {
    expression
        .nodes()
        .iter()
        .filter_map(|node| match node {
            ExprNode::Symbol(SymbolRef::Across(port) | SymbolRef::Through(port)) => {
                Some(port.erase())
            }
            _ => None,
        })
        .collect()
}

fn edge_targets(edges: &[Edge], from: RawId, kind: EdgeKind) -> BTreeSet<RawId> {
    edges
        .iter()
        .filter(|edge| edge.from() == from && edge.kind() == kind)
        .map(Edge::to)
        .collect()
}

fn edge_sources(edges: &[Edge], to: RawId, kind: EdgeKind) -> BTreeSet<RawId> {
    edges
        .iter()
        .filter(|edge| edge.to() == to && edge.kind() == kind)
        .map(Edge::from)
        .collect()
}

fn kernel_error(id: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_KERNEL_DEFINITION, message).with_graph_path(GraphPath::new([
        "semantic",
        &format!("{:?}", id.kind()),
        &id.to_string(),
    ]))
}

fn expression_path(owner: RawId, index: usize) -> GraphPath {
    GraphPath::new([
        "semantic".to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
        "expression".to_owned(),
        index.to_string(),
    ])
}
