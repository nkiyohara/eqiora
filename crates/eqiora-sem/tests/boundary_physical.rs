use eqiora_core::ValueFrame;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId, ValueShape};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, AxisBounds, BoundaryPairing, BoundaryPhysicalConnector, BoundarySide,
    ConnectionDef, ConnectionSemantics, DomainDef, ExprDagBuilder, KernelNode, PortDef,
    RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{BoundaryJunctionGeometry, KernelProgram};

fn length(value: f64) -> DynQuantity {
    DynQuantity::new(
        value,
        DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension"),
    )
}

#[derive(Debug)]
struct InterfaceFixture {
    program: KernelProgram,
    connection: Id<kinds::Connection>,
}

fn interface_program(
    right_start: f64,
    semantics: ConnectionSemantics,
    share_parent: bool,
    channels: Option<u32>,
) -> Result<InterfaceFixture, Vec<eqiora_core::Diagnostic>> {
    let connector = Id::<kinds::Domain>::new();
    let left_volume = Id::<kinds::Domain>::new();
    let right_volume = Id::<kinds::Domain>::new();
    let left_boundary = Id::<kinds::Domain>::new();
    let right_boundary = Id::<kinds::Domain>::new();
    let left_port = Id::<kinds::Port>::new();
    let right_port = Id::<kinds::Port>::new();
    let left_relation = Id::<kinds::Relation>::new();
    let right_relation = Id::<kinds::Relation>::new();
    let left_activation = Id::<kinds::Activation>::new();
    let right_activation = Id::<kinds::Activation>::new();
    let connection = Id::<kinds::Connection>::new();
    let model = OntologyId::<Model>::new();

    let velocity = DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).expect("bounded dimension");
    let traction = DimExponents::from_integers([1, -1, -2, 0, 0, 0, 0]).expect("bounded dimension");
    let trace_type = eqiora_core::ValueType::shaped(
        eqiora_core::ScalarDomain::Real,
        velocity,
        ValueShape::new([2]).unwrap(),
        ValueFrame::SpatialCartesian,
    )
    .unwrap();
    let trace_type = match channels {
        Some(extent) => trace_type.array(extent).unwrap(),
        None => trace_type,
    };
    let connector_contract = BoundaryPhysicalConnector::new(
        trace_type.clone(),
        trace_type
            .clone()
            .with_dimension(traction)
            .expect("valid real tensor dimension"),
        BoundaryPairing::EuclideanBoundaryDuality,
    )
    .unwrap();

    let residuals = |port: Id<kinds::Port>| {
        let mut expression = ExprDagBuilder::new();
        let trace = expression.symbol(SymbolRef::PortTrace(port)).unwrap();
        let flux = expression.symbol(SymbolRef::PortFlux(port)).unwrap();
        {
            let trace_zero = expression
                .constant(eqiora_core::ValueLiteral::from_real(trace_type.clone(), 0.0).unwrap())
                .unwrap();
            let flux_zero = expression
                .constant(
                    eqiora_core::ValueLiteral::from_real(
                        trace_type
                            .clone()
                            .with_dimension(traction)
                            .expect("valid real tensor dimension"),
                        0.0,
                    )
                    .unwrap(),
                )
                .unwrap();
            expression.finish([trace, trace_zero, flux, flux_zero])
        }
        .unwrap()
    };

    let mut nodes = vec![
        KernelNode::from(DomainDef::boundary_physical(connector, connector_contract)),
        KernelNode::from(
            DomainDef::cartesian_box(
                left_volume,
                vec![
                    AxisBounds::new(length(0.0), length(1.0)).unwrap(),
                    AxisBounds::new(length(0.0), length(1.0)).unwrap(),
                ],
            )
            .unwrap(),
        ),
    ];
    if !share_parent {
        nodes.push(KernelNode::from(
            DomainDef::cartesian_box(
                right_volume,
                vec![
                    AxisBounds::new(length(right_start), length(2.0)).unwrap(),
                    AxisBounds::new(length(0.0), length(1.0)).unwrap(),
                ],
            )
            .unwrap(),
        ));
    }
    nodes.extend([
        KernelNode::from(DomainDef::cartesian_boundary(
            left_boundary,
            0,
            BoundarySide::Upper,
        )),
        KernelNode::from(DomainDef::cartesian_boundary(
            right_boundary,
            0,
            BoundarySide::Lower,
        )),
        KernelNode::from(PortDef::boundary_physical(
            left_port,
            connector,
            left_boundary,
        )),
        KernelNode::from(PortDef::boundary_physical(
            right_port,
            connector,
            right_boundary,
        )),
        KernelNode::from(RelationDef::new(left_relation, residuals(left_port)).unwrap()),
        KernelNode::from(RelationDef::new(right_relation, residuals(right_port)).unwrap()),
        KernelNode::from(ActivationDef::continuous(left_activation)),
        KernelNode::from(ActivationDef::continuous(right_activation)),
        KernelNode::from(ConnectionDef::new(connection, semantics)),
    ]);
    let mut transaction = Transaction::new("two field-valued physical boundaries");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in [
        (
            left_boundary.erase(),
            left_volume.erase(),
            EdgeKind::BoundaryOf,
        ),
        (
            right_boundary.erase(),
            if share_parent {
                left_volume.erase()
            } else {
                right_volume.erase()
            },
            EdgeKind::BoundaryOf,
        ),
        (left_relation.erase(), left_port.erase(), EdgeKind::HasPort),
        (
            right_relation.erase(),
            right_port.erase(),
            EdgeKind::HasPort,
        ),
        (
            left_relation.erase(),
            left_port.erase(),
            EdgeKind::DependsOn,
        ),
        (
            right_relation.erase(),
            right_port.erase(),
            EdgeKind::DependsOn,
        ),
        (
            left_relation.erase(),
            left_boundary.erase(),
            EdgeKind::AppliesOn,
        ),
        (
            right_relation.erase(),
            right_boundary.erase(),
            EdgeKind::AppliesOn,
        ),
        (
            left_activation.erase(),
            left_relation.erase(),
            EdgeKind::Activates,
        ),
        (
            right_activation.erase(),
            right_relation.erase(),
            EdgeKind::Activates,
        ),
        (connection.erase(), left_port.erase(), EdgeKind::Connects),
        (connection.erase(), right_port.erase(), EdgeKind::Connects),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    let members = transaction
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode { node } => Some(node.id()),
            _ => None,
        })
        .collect::<Vec<_>>();
    transaction.push(Op::DefineOntologyView {
        view: ModelView::new(model, members, []).unwrap().into(),
    });

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).map(|program| InterfaceFixture {
        program,
        connection,
    })
}

#[test]
fn coincident_2d_vector_interface_is_admitted_componentwise() {
    let fixture = interface_program(1.0, ConnectionSemantics::Conserving, false, None)
        .expect("coincident interface must validate");
    assert_eq!(fixture.program.nodes().count(), 12);

    let junction = fixture
        .program
        .compose_boundary_physical_junction(fixture.connection)
        .expect("validated interface must compose");
    let typed = junction.typed();
    assert_eq!(typed.expression().roots().len(), 2);
    for root in typed.expression().roots() {
        let root_type = typed.node_type(*root).expect("every root is typed");
        assert_eq!(root_type.shape().extents()[0].get(), 2);
        assert_eq!(root_type.frame(), ValueFrame::SpatialCartesian);
    }
}

#[test]
fn noncoincident_cartesian_interface_fails_closed() {
    let diagnostics = interface_program(1.25, ConnectionSemantics::Conserving, false, None)
        .expect_err("separated boundaries must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("one coincident Cartesian boundary")
    }));
}

#[test]
fn boundary_junction_preserves_arrays_of_spatial_vectors() {
    let fixture = interface_program(1.0, ConnectionSemantics::Conserving, false, Some(3)).unwrap();
    let junction = fixture
        .program
        .compose_boundary_physical_junction(fixture.connection)
        .unwrap();
    for root in junction.typed().expression().roots() {
        let root_type = junction.typed().node_type(*root).unwrap();
        assert_eq!(root_type.value_type.array_rank(), 1);
        assert_eq!(
            root_type
                .shape()
                .extents()
                .iter()
                .map(|n| n.get())
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(root_type.frame(), ValueFrame::SpatialCartesian);
    }
}

#[test]
fn opposite_sides_of_one_parent_form_a_spatial_periodic_junction() {
    let fixture = interface_program(0.0, ConnectionSemantics::SpatialPeriodic, true, None)
        .expect("opposite sides of one parent must validate");
    assert_eq!(fixture.program.nodes().count(), 11);

    let junction = fixture
        .program
        .compose_boundary_physical_junction(fixture.connection)
        .expect("validated periodic pair must compose");
    let BoundaryJunctionGeometry::CartesianPeriodic(identification) = junction.geometry() else {
        panic!("spatial-periodic junction must retain its derived chart map");
    };
    assert_eq!(identification.ambient_dimension(), 2);
    assert_eq!(identification.normal_axis(), 0);
    assert_eq!(identification.period(), 1.0);
    assert_eq!(identification.tangential_intervals(), &[(0.0, 1.0)]);
    assert_eq!(junction.typed().expression().roots().len(), 2);
}

#[test]
fn spatial_periodic_connection_rejects_distinct_parents() {
    let diagnostics = interface_program(1.0, ConnectionSemantics::SpatialPeriodic, false, None)
        .expect_err("periodic pair across distinct parents must fail");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message().contains("one exact parent Domain") })
    );
}
