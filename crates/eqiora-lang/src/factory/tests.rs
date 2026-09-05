use crate::{
    BinaryOp, BoundaryPairingSyntax, FrameSyntax, Item, SignalDirectionSyntax, format, parse,
};

use super::*;

use crate::cartesian::CartesianCoordinateSyntax;

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(start, end)
}

fn dimension() -> Expr {
    SourceAstFactory::expression(ExprKind::Number(1.0), range(0, 0)).expect("dimension")
}

fn path(segments: &[&str]) -> NamePath {
    NamePath::from_segments(segments.iter().copied(), range(0, 0)).expect("path")
}

fn private_model(name: &str, items: Vec<Item>) -> crate::ModelDecl {
    SourceAstFactory::model(VisibilitySyntax::Private, name, items, range(0, 0)).expect("model")
}

#[test]
fn owned_flat_model_formats_and_parses_identically() {
    let domain = SourceAstFactory::domain(
        "body",
        DomainSyntax::CartesianBox(vec![(
            CartesianCoordinateSyntax::fixed(0.0, TextRange::new(0, 0)),
            CartesianCoordinateSyntax::fixed(1.0, TextRange::new(0, 0)),
        )]),
        range(0, 0),
    )
    .expect("Domain");
    let representation =
        SourceAstFactory::representation("space", RepresentationSyntax::Continuum, range(0, 0))
            .expect("Representation");
    let field = SourceAstFactory::field(
        "temperature",
        Some("body".to_owned()),
        Some("space".to_owned()),
        dimension(),
        0.0,
        range(0, 0),
    )
    .expect("Field");
    let parameter = SourceAstFactory::parameter(
        "gain",
        dimension(),
        SourceAstFactory::expression(ExprKind::Number(2.0), range(0, 0)).unwrap(),
        range(0, 0),
    )
    .expect("Parameter");
    let output = SourceAstFactory::port(
        "output",
        PortSyntax::Signal {
            direction: SignalDirectionSyntax::Output,
            dimension: dimension(),
        },
        range(0, 0),
    )
    .expect("output Port");
    let input = SourceAstFactory::port(
        "input",
        PortSyntax::Signal {
            direction: SignalDirectionSyntax::Input,
            dimension: dimension(),
        },
        range(0, 0),
    )
    .expect("input Port");
    let clock = SourceAstFactory::clock(
        "sample",
        SourceAstFactory::rational(1, 10),
        SourceAstFactory::rational(0, 1),
        range(0, 0),
    )
    .expect("Clock");
    let residual =
        SourceAstFactory::expression(ExprKind::Name("temperature".to_owned()), range(0, 0))
            .expect("residual");
    let relation = SourceAstFactory::relation(
        "balance",
        ActivationSyntax::Continuous,
        Some("body".to_owned()),
        vec![residual],
        range(0, 0),
    )
    .expect("Relation");
    let connection = SourceAstFactory::connection(
        ConnectionSyntax::Signal,
        vec![path(&["output"]), path(&["input"])],
        range(0, 0),
    )
    .expect("Connection");
    let boundary =
        SourceAstFactory::boundary(vec![path(&["input"])], range(0, 0)).expect("boundary");
    let binding = SourceAstFactory::parameter_binding(
        "gain",
        SourceAstFactory::expression(ExprKind::Number(3.0), range(0, 0)).expect("binding value"),
        range(0, 0),
    )
    .expect("binding");
    let instance =
        SourceAstFactory::instance("nested", path(&["Reusable"]), vec![binding], range(0, 0))
            .expect("instance");
    let model = SourceAstFactory::model(
        VisibilitySyntax::Private,
        "constructed",
        vec![
            Item::Domain(domain),
            Item::Representation(representation),
            Item::Field(field),
            Item::Parameter(parameter),
            Item::Port(output),
            Item::Port(input),
            Item::Clock(clock),
            Item::Relation(relation),
            Item::Connection(connection),
            Item::Boundary(boundary),
            Item::Instance(instance),
        ],
        range(0, 0),
    )
    .expect("model");
    let document = SourceAstFactory::flat_document(vec![model]).expect("document");

    let source = format(&document);
    let reparsed = parse("constructed.eqi", &source)
        .into_document()
        .expect("factory output parses");

    assert_eq!(format(&reparsed), source);
}

#[test]
fn owned_declaration_only_document_preserves_package_visibility() {
    let connector = SourceAstFactory::connector(
        VisibilitySyntax::Public,
        "Pin",
        ConnectorSyntax::ScalarPhysical {
            across_dimension: dimension(),
            through_dimension: dimension(),
        },
        range(0, 0),
    )
    .expect("Connector");
    let resistance = SourceAstFactory::component_parameter(
        VisibilitySyntax::Public,
        "resistance",
        dimension(),
        Some(dimension()),
        range(0, 0),
    )
    .expect("component Parameter");
    let component = SourceAstFactory::component(
        VisibilitySyntax::Public,
        "Resistor",
        vec![ComponentItem::Parameter(resistance)],
        range(0, 0),
    )
    .expect("component");
    let document = SourceAstFactory::document(vec![connector], vec![component], Vec::new())
        .expect("declaration-only document");

    let source = format(&document);
    let reparsed = parse("library.eqi", &source)
        .into_document()
        .expect("factory output parses");
    assert!(reparsed.models().is_empty());
    assert_eq!(
        reparsed.connectors()[0].visibility(),
        VisibilitySyntax::Public
    );
    assert_eq!(
        reparsed.components()[0].visibility(),
        VisibilitySyntax::Public
    );
    assert_eq!(format(&reparsed), source);
    assert!(SourceAstFactory::document(Vec::new(), Vec::new(), Vec::new()).is_err());
}

#[test]
fn factory_constructs_exact_pure_operator_documents_without_weakening_legacy_document_api() {
    let rank = SourceAstFactory::exact_integer("01", range(10, 12)).expect("rank");
    let formal = SourceAstFactory::pure_operator_formal(
        "x",
        PureValueClassSyntax::Spatial { rank },
        range(8, 12),
    )
    .expect("formal");
    let axis = SourceAstFactory::exact_integer("00", range(30, 32)).expect("axis");
    let body = SourceAstFactory::pure_operator_expression(
        PureOperatorExprKind::Component {
            formal: "x".to_owned(),
            formal_range: range(27, 28),
            result_axes: vec![axis],
        },
        range(17, 33),
    )
    .expect("body");
    let operator = SourceAstFactory::pure_operator(
        VisibilitySyntax::Public,
        "identity",
        vec![formal],
        PureValueClassSyntax::Spatial {
            rank: SourceAstFactory::exact_integer("1", range(14, 15)).expect("result rank"),
        },
        body,
        range(0, 34),
    )
    .expect("operator");
    let document = SourceAstFactory::document_with_pure_operators(
        Vec::new(),
        Vec::new(),
        vec![operator],
        Vec::new(),
    )
    .expect("declaration-only document");

    let source = format(&document);
    assert_eq!(
        source,
        "public pure operator identity(x: spatial[1]) -> spatial[1] = component(x, 0);\n"
    );
    assert!(parse("factory-pure.eqi", &source).into_document().is_ok());
    assert!(
        SourceAstFactory::document_with_pure_operators(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new()
        )
        .is_err()
    );
}

#[test]
fn owned_support_slots_and_bindings_format_and_parse_identically() {
    let body = SourceAstFactory::support_slot(
        VisibilitySyntax::Public,
        "body",
        SupportSlotSyntax::Volume {
            ambient_dimension: 2,
        },
        range(0, 0),
    )
    .expect("volume support slot");
    let interface = SourceAstFactory::support_slot(
        VisibilitySyntax::Public,
        "interface",
        SupportSlotSyntax::Boundary {
            parent: "body".to_owned(),
        },
        range(0, 0),
    )
    .expect("boundary support slot");
    let representation =
        SourceAstFactory::representation("space", RepresentationSyntax::Continuum, range(0, 0))
            .expect("Representation");
    let component = SourceAstFactory::component(
        VisibilitySyntax::Private,
        "BoundaryState",
        vec![
            ComponentItem::Support(body),
            ComponentItem::Support(interface),
            ComponentItem::Representation(representation),
        ],
        range(0, 0),
    )
    .expect("component");
    let support =
        SourceAstFactory::support_binding("body", "fluid", range(0, 0)).expect("support binding");
    let instance = SourceAstFactory::instance_with_support_bindings(
        "probe",
        path(&["BoundaryState"]),
        Vec::new(),
        vec![support],
        range(0, 0),
    )
    .expect("support-aware instance");
    let model = private_model("coupled", vec![Item::Instance(instance)]);
    let document =
        SourceAstFactory::document(vec![], vec![component], vec![model]).expect("document");

    let source = format(&document);
    let reparsed = parse("supports.eqi", &source)
        .into_document()
        .expect("factory support source parses");

    assert_eq!(format(&reparsed), source);
    let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
        panic!("model member is an instance");
    };
    assert!(instance.bindings().is_empty());
    assert_eq!(instance.support_bindings()[0].target(), "fluid");
}

#[test]
fn owned_field_slots_and_bindings_format_and_parse_identically() {
    let body = SourceAstFactory::support_slot(
        VisibilitySyntax::Public,
        "body",
        SupportSlotSyntax::Volume {
            ambient_dimension: 2,
        },
        range(0, 0),
    )
    .expect("volume support slot");
    let state = SourceAstFactory::field_slot(
        "state",
        "body",
        dimension(),
        Some(ValueShapeSyntax::SpatialVector),
        range(0, 0),
    )
    .expect("Field slot");
    let component = SourceAstFactory::component(
        VisibilitySyntax::Private,
        "StateLaw",
        vec![
            ComponentItem::Support(body),
            ComponentItem::FieldSlot(state),
        ],
        range(0, 0),
    )
    .expect("component");
    let support =
        SourceAstFactory::support_binding("body", "region", range(0, 0)).expect("support binding");
    let field = SourceAstFactory::field_binding("state", "temperature", range(0, 0))
        .expect("Field binding");
    let instance = SourceAstFactory::instance_with_slot_bindings(
        "law",
        path(&["StateLaw"]),
        Vec::new(),
        vec![support],
        vec![field],
        range(0, 0),
    )
    .expect("slot-aware instance");
    let model = private_model("coupled", vec![Item::Instance(instance)]);
    let document =
        SourceAstFactory::document(vec![], vec![component], vec![model]).expect("document");

    let source = format(&document);
    let reparsed = parse("field-slots.eqi", &source)
        .into_document()
        .expect("factory Field-slot source parses");

    assert_eq!(format(&reparsed), source);
    let ComponentItem::FieldSlot(slot) = &reparsed.components()[0].items()[1] else {
        panic!("second component member is a Field slot");
    };
    assert_eq!(slot.support(), "body");
    let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
        panic!("model member is an instance");
    };
    assert_eq!(instance.field_bindings()[0].target(), "temperature");

    assert!(
        SourceAstFactory::field_slot(
            "state",
            "body",
            dimension(),
            Some(ValueShapeSyntax::Exact(Vec::new())),
            range(0, 0),
        )
        .is_err()
    );
}

#[test]
fn name_rewrite_preserves_expression_tree_and_ranges() {
    let bare = SourceAstFactory::expression(ExprKind::Name("x".to_owned()), range(10, 11))
        .expect("bare name");
    let qualified_path =
        NamePath::from_segments(["plant", "terminal"], range(20, 34)).expect("qualified");
    let qualified = SourceAstFactory::expression(ExprKind::Path(qualified_path), range(20, 34))
        .expect("qualified name");
    let call = SourceAstFactory::expression(
        ExprKind::Call {
            callee: path(&["across"]),
            arguments: vec![qualified],
        },
        range(13, 35),
    )
    .expect("call");
    let expression = SourceAstFactory::expression(
        ExprKind::Binary {
            op: BinaryOp::Add,
            left: Box::new(bare),
            right: Box::new(call),
        },
        range(10, 35),
    )
    .expect("expression");

    let rewritten = expression.rewrite_name_paths(|name| match name.as_str() {
        "x" => Some(NamePath::from_segments(["scope", "x"], range(100, 200)).expect("replacement")),
        "plant.terminal" => {
            Some(NamePath::from_segments(["terminal"], range(300, 400)).expect("replacement"))
        }
        "across" => Some(
            NamePath::from_segments(["operators", "across"], range(500, 600)).expect("replacement"),
        ),
        _ => None,
    });

    assert_eq!(rewritten.range(), expression.range());
    let ExprKind::Binary { left, right, .. } = rewritten.kind() else {
        panic!("binary topology is preserved");
    };
    assert_eq!(left.range(), range(10, 11));
    let ExprKind::Path(left_path) = left.kind() else {
        panic!("bare name was rewritten to a qualified path");
    };
    assert_eq!(left_path.as_str(), "scope.x");
    assert_eq!(left_path.range(), range(10, 11));
    assert_eq!(right.range(), range(13, 35));
    let ExprKind::Call { callee, arguments } = right.kind() else {
        panic!("Call topology is preserved");
    };
    assert_eq!(callee.as_str(), "operators.across");
    assert_eq!(callee.range(), range(0, 0));
    assert_eq!(arguments[0].range(), range(20, 34));
    assert!(matches!(
        arguments[0].kind(),
        ExprKind::Name(name) if name == "terminal"
    ));
}

#[test]
fn factory_constructs_closed_field_physical_source_shapes() {
    let trace =
        SourceAstFactory::connector_quantity("velocity", dimension()).expect("trace quantity");
    let flux =
        SourceAstFactory::connector_quantity("traction", dimension()).expect("flux quantity");
    let connector = SourceAstFactory::connector(
        VisibilitySyntax::Public,
        "MechanicalBoundary",
        ConnectorSyntax::FieldPhysical {
            trace,
            flux,
            shape: ValueShapeSyntax::Exact(vec![2]),
            frame: FrameSyntax::Spatial,
            pairing: BoundaryPairingSyntax::EuclideanBoundaryDuality,
        },
        range(0, 0),
    )
    .expect("field-physical Connector");
    let field = SourceAstFactory::field_with_shape(
        "velocity",
        None,
        None,
        Some(ValueShapeSyntax::Exact(vec![2])),
        dimension(),
        None,
        range(0, 0),
    )
    .expect("shaped Field");
    let port = SourceAstFactory::port(
        "interface",
        PortSyntax::FieldPhysical {
            connector: path(&["MechanicalBoundary"]),
            support: "wall".to_owned(),
        },
        range(0, 0),
    )
    .expect("field-physical Port");
    let model = SourceAstFactory::model(
        VisibilitySyntax::Private,
        "coupled",
        vec![Item::Field(field), Item::Port(port)],
        range(0, 0),
    )
    .expect("model");
    let document =
        SourceAstFactory::document(vec![connector], Vec::new(), vec![model]).expect("document");
    let source = format(&document);

    assert_eq!(
        format(
            &parse("factory-boundary.eqi", &source)
                .into_document()
                .expect("factory source parses")
        ),
        source
    );
    assert!(SourceAstFactory::connector_quantity("not-valid", dimension()).is_err());
    assert!(
        SourceAstFactory::field_with_shape(
            "bad",
            None,
            None,
            Some(ValueShapeSyntax::Exact(vec![0])),
            dimension(),
            None,
            range(0, 0),
        )
        .is_err()
    );
}

#[test]
fn factory_constructs_complete_exterior_families_and_roundtrips() {
    let body = SourceAstFactory::support_slot(
        VisibilitySyntax::Public,
        "body",
        SupportSlotSyntax::Volume {
            ambient_dimension: 2,
        },
        range(0, 0),
    )
    .expect("body support");
    let exterior = SourceAstFactory::support_slot(
        VisibilitySyntax::Public,
        "exterior",
        SupportSlotSyntax::CompleteExterior {
            parent: "body".to_owned(),
        },
        range(0, 0),
    )
    .expect("complete exterior support");
    let binder = SourceAstFactory::boundary_family_binder("boundary", "exterior", range(0, 0))
        .expect("family binder");
    let port = SourceAstFactory::component_port(
        VisibilitySyntax::Public,
        "mechanical",
        PortSyntax::FieldPhysical {
            connector: path(&["MechanicalBoundary"]),
            support: "boundary".to_owned(),
        },
        range(0, 0),
    )
    .expect("component Port");
    let port_family =
        SourceAstFactory::component_port_family(port, binder.clone()).expect("Port family");
    let selector = SourceAstFactory::boundary_port_selector("boundary", "boundary", range(0, 0))
        .expect("selector");
    let selected_port = SourceAstFactory::expression(
        ExprKind::BoundaryPortSelection {
            port: Box::new(path(&["mechanical"])),
            selector: Box::new(selector.clone()),
        },
        range(0, 0),
    )
    .expect("selected Port expression");
    let residual = SourceAstFactory::expression(
        ExprKind::Call {
            callee: path(&["flux"]),
            arguments: vec![selected_port],
        },
        range(0, 0),
    )
    .expect("family residual");
    let relation = SourceAstFactory::relation(
        "natural",
        ActivationSyntax::Continuous,
        Some("boundary".to_owned()),
        vec![residual],
        range(0, 0),
    )
    .expect("Relation");
    let relation_family =
        SourceAstFactory::relation_family(relation, binder.clone()).expect("Relation family");
    let left = SourceAstFactory::boundary_port_reference(
        path(&["child", "mechanical"]),
        Some(selector.clone()),
    )
    .expect("left family Port");
    let right = SourceAstFactory::boundary_port_reference(path(&["mechanical"]), Some(selector))
        .expect("right family Port");
    let connection =
        SourceAstFactory::boundary_connection(Some(binder), vec![left, right], range(0, 0))
            .expect("pointwise Connection");
    let component = SourceAstFactory::component(
        VisibilitySyntax::Private,
        "BoundaryLaw",
        vec![
            ComponentItem::Support(body),
            ComponentItem::Support(exterior),
            ComponentItem::PortFamily(port_family),
            ComponentItem::RelationFamily(relation_family),
            ComponentItem::BoundaryConnection(connection),
        ],
        range(0, 0),
    )
    .expect("component");

    let members = ["x_lower", "x_upper", "y_lower", "y_upper"]
        .into_iter()
        .map(|member| {
            SourceAstFactory::boundary_set_member(member, range(0, 0)).expect("boundary member")
        })
        .collect();
    let exterior_binding = SourceAstFactory::boundary_set_binding("exterior", members, range(0, 0))
        .expect("boundary-set binding");
    let instance = SourceAstFactory::instance_with_boundary_set_bindings(
        "law",
        path(&["BoundaryLaw"]),
        Vec::new(),
        vec![
            SourceAstFactory::support_binding("body", "fluid", range(0, 0)).expect("body binding"),
        ],
        vec![exterior_binding],
        Vec::new(),
        range(0, 0),
    )
    .expect("family-aware instance");
    let model = private_model("coupled", vec![Item::Instance(instance)]);
    let document =
        SourceAstFactory::document(Vec::new(), vec![component], vec![model]).expect("document");

    let source = format(&document);
    let reparsed = parse("complete-exterior-factory.eqi", &source)
        .into_document()
        .expect("factory boundary-family source parses");
    assert_eq!(format(&reparsed), source);
    let Item::Instance(instance) = &reparsed.models()[0].items()[0] else {
        panic!("model member is an instance");
    };
    assert_eq!(instance.boundary_set_bindings()[0].members().len(), 4);

    let signal_port = SourceAstFactory::component_port(
        VisibilitySyntax::Public,
        "signal",
        PortSyntax::Signal {
            direction: SignalDirectionSyntax::Input,
            dimension: dimension(),
        },
        range(0, 0),
    )
    .expect("signal Port");
    let binder =
        SourceAstFactory::boundary_family_binder("b", "exterior", range(0, 0)).expect("binder");
    assert!(SourceAstFactory::component_port_family(signal_port, binder).is_err());
}

#[test]
fn construction_rejects_unrepresentable_source_shapes() {
    assert!(NamePath::from_segments(Vec::<String>::new(), range(0, 0)).is_err());
    assert!(NamePath::from_segments(["not-valid"], range(0, 0)).is_err());
    assert!(
        SourceAstFactory::connection(
            ConnectionSyntax::Conserving,
            vec![path(&["only_one"])],
            range(0, 0),
        )
        .is_err()
    );
    assert!(SourceAstFactory::expression(ExprKind::Number(1.0), range(2, 1)).is_err());
    assert!(
        SourceAstFactory::support_slot(
            VisibilitySyntax::Public,
            "boundary",
            SupportSlotSyntax::Boundary {
                parent: "not-valid".to_owned(),
            },
            range(0, 0),
        )
        .is_err()
    );
    assert!(SourceAstFactory::support_binding("body", "not-valid", range(0, 0)).is_err());
}

#[test]
fn spatial_periodic_connection_is_closed_model_only() {
    let ports = ["lower", "upper"]
        .into_iter()
        .map(|name| {
            SourceAstFactory::boundary_port_reference(path(&[name]), None)
                .expect("periodic Port reference")
        })
        .collect();
    let connection = SourceAstFactory::spatial_periodic_boundary_connection(ports, range(0, 0))
        .expect("closed-model periodic Connection shape");

    let error = SourceAstFactory::component(
        VisibilitySyntax::Private,
        "InvalidPeriodicComponent",
        vec![ComponentItem::BoundaryConnection(connection)],
        range(0, 0),
    )
    .expect_err("periodic Connection must not enter reusable Component syntax");

    assert_eq!(
        error.to_string(),
        "a spatial-periodic Connection belongs only to a closed Model"
    );
}
