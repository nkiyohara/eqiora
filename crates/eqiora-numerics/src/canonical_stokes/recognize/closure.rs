use super::*;

pub(super) fn require_closed_model(
    program: &KernelProgram,
    model: &SteadyIncompressibleStokesModel2d,
    representation: RawId,
    boundary: &LoweredBoundaryProjection2d,
    coefficient_fields: &BTreeSet<RawId>,
    coefficient_definitions: &BTreeSet<RawId>,
    additional_parameters: &BTreeSet<RawId>,
) -> Result<(), Diagnostic> {
    let mut domains = BTreeSet::from([model.domain()]);
    let mut relations = BTreeSet::from([
        model.force_potential_definition(),
        model.momentum_relation(),
        model.incompressibility_relation(),
    ]);
    domains.extend(
        model
            .boundary_relations()
            .iter()
            .map(|binding| binding.boundary()),
    );
    domains.extend(boundary.connector_domains.iter().copied());
    relations.extend(
        model
            .boundary_relations()
            .iter()
            .map(|binding| binding.relation()),
    );
    relations.extend(coefficient_definitions.iter().copied());
    relations.extend(boundary.prescribed_velocity_definitions.iter().copied());
    debug_assert!(boundary.uninterpreted_live_relations.is_subset(&relations));
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && relations.contains(&edge.to()))
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let parameters = relations
        .iter()
        .copied()
        .flat_map(|relation| {
            relation_expression(program, relation)
                .expect("admitted Relations were already inspected")
                .nodes()
                .iter()
        })
        .filter_map(|node| match node {
            ExprNode::Symbol(SymbolRef::Parameter(parameter)) => Some(parameter.erase()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let parameters = parameters
        .union(additional_parameters)
        .copied()
        .collect::<BTreeSet<_>>();
    let mut fields = BTreeSet::from([model.velocity(), model.pressure(), model.force_potential()]);
    fields.extend(coefficient_fields.iter().copied());
    fields.extend(boundary.prescribed_velocity_fields.iter().copied());
    for node in program.nodes() {
        let admitted = match node {
            KernelNode::Domain(value) => domains.contains(&value.id().erase()),
            KernelNode::Representation(value) => value.id().erase() == representation,
            KernelNode::Field(value) => fields.contains(&value.id().erase()),
            KernelNode::Parameter(value) => parameters.contains(&value.id().erase()),
            KernelNode::Relation(value) => relations.contains(&value.id().erase()),
            KernelNode::Activation(value) => activations.contains(&value.id().erase()),
            KernelNode::Port(value) => boundary.ports.contains(&value.id().erase()),
            KernelNode::Connection(value) => boundary.connections.contains(&value.id().erase()),
            _ => false,
        };
        if !admitted {
            return Err(model_lowering_error(
                program,
                format!(
                    "closed 2D steady Stokes lowering would ignore unexpected {:?} node {}",
                    node.kind(),
                    node.id()
                ),
            ));
        }
    }
    Ok(())
}
