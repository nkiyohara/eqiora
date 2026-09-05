use super::*;

#[test]
fn parser_retains_exact_pure_operator_syntax_and_qualified_applications() {
    let source = r#"public pure operator dyadic(left: spatial[01], right: spatial[1]) -> spatial[2] =
  component(left, 0) * component(right, 1) + rational(03, 4) * delta(0, 1);

model coupled {
  field u: 1 = 0;
  field v: 1 = 0;
  relation law continuous { ops.dyadic(u, v) = 0; }
}"#;
    let document = parse("pure-operator.eqi", source)
        .into_document()
        .expect("bounded pure operator source");

    let operator = &document.pure_operators()[0];
    assert_eq!(operator.visibility(), VisibilitySyntax::Public);
    assert_eq!(operator.name(), "dyadic");
    assert_eq!(operator.formals().len(), 2);
    assert_eq!(operator.formals()[0].name(), "left");
    let PureValueClassSyntax::Spatial { rank } = operator.formals()[0].value_class() else {
        panic!("first formal is spatial");
    };
    assert_eq!(rank.value(), 1);
    assert_eq!(rank.spelling(), "01");
    assert_eq!(
        &source[rank.range().start() as usize..rank.range().end() as usize],
        "01"
    );
    assert_eq!(
        &source[operator.range().start() as usize..operator.range().end() as usize],
        &source[..source.find("\n\nmodel").expect("model separator")]
    );

    let PureOperatorExprKind::Binary { left, right, .. } = operator.body().kind() else {
        panic!("addition is the pure body root");
    };
    assert!(matches!(
        left.kind(),
        PureOperatorExprKind::Binary {
            op: PureOperatorBinaryOp::Mul,
            ..
        }
    ));
    let PureOperatorExprKind::Binary {
        op: PureOperatorBinaryOp::Mul,
        left: rational,
        right: delta,
    } = right.kind()
    else {
        panic!("right term retains exact rational and delta nodes");
    };
    let PureOperatorExprKind::Rational { numerator, .. } = rational.kind() else {
        panic!("left factor is rational");
    };
    assert_eq!(numerator.spelling(), "03");
    assert!(matches!(delta.kind(), PureOperatorExprKind::Delta { .. }));

    let Item::Relation(relation) = &document.models()[0].items()[2] else {
        panic!("model relation retained");
    };
    let ExprKind::Call { callee, arguments } = relation.residuals()[0].kind() else {
        panic!("residual is a qualified application");
    };
    assert_eq!(callee.segments().collect::<Vec<_>>(), ["ops", "dyadic"]);
    assert_eq!(arguments.len(), 2);
    assert!(matches!(arguments[0].kind(), ExprKind::Name(name) if name == "u"));
    assert!(matches!(arguments[1].kind(), ExprKind::Name(name) if name == "v"));
}

#[test]
fn parser_rejects_operators_outside_the_exact_body_vocabulary() {
    for source in [
        "pure operator bad(x: scalar) -> scalar = rational(1.0, 2);",
        "pure operator bad(x: scalar) -> scalar = rational(1, 0);",
        "pure operator bad(x: scalar) -> scalar = rational(1, 2) / rational(3, 4);",
        "pure operator bad(x: scalar) -> scalar = x;",
        "pure operator bad() -> scalar = rational(1, 2);",
    ] {
        assert!(
            parse("invalid-pure-operator.eqi", source)
                .into_document()
                .is_err(),
            "source must fail closed: {source}"
        );
    }
}

#[test]
fn parser_represents_scalar_component_selection_with_zero_axes() {
    let source = "pure operator negate(s: scalar) -> scalar = -component(s);";
    let document = parse("scalar-component.eqi", source)
        .into_document()
        .expect("scalar component selection");
    let PureOperatorExprKind::Neg(value) = document.pure_operators()[0].body().kind() else {
        panic!("negation retained");
    };
    assert!(matches!(
        value.kind(),
        PureOperatorExprKind::Component { result_axes, .. } if result_axes.is_empty()
    ));
}

#[test]
fn parser_requires_pure_operators_before_models_and_nonempty_calls() {
    let late = parse(
        "late-operator.eqi",
        "model M {} pure operator identity(x: scalar) -> scalar = component(x, 0);",
    );
    assert!(late.into_document().is_err());

    let empty_call = parse(
        "empty-call.eqi",
        "model M { relation r continuous { ops.identity() = 0; } }",
    );
    assert!(empty_call.into_document().is_err());
}

#[test]
fn parser_builds_continuous_and_periodic_relations() {
    let source = r#"
model thermal {
  field temperature: K = 293;
  field command: 1 = 0;
  clock control = periodic(period = 1 / 10, phase = 0 / 1);
  relation plant continuous {
derivative(temperature) - command = 0;
  }
  relation controller periodic(control) {
next(command) - pre(command) = 0;
  }
}
"#;
    let result = parse("thermal.eqi", source);
    let document = result.into_document().expect("valid source");

    assert_eq!(document.models().len(), 1);
    assert_eq!(document.models()[0].items().len(), 5);
}

#[test]
fn parser_recovers_after_an_invalid_item() {
    let source = "model m { nonsense; field x: 1 = 0; }";
    let result = parse("recovery.eqi", source);

    assert!(!result.diagnostics().is_empty());
    assert_eq!(
        result.document().expect("recovered").models()[0]
            .items()
            .len(),
        1
    );
}

#[test]
fn parser_retains_scalar_physical_contracts_and_source_ranges() {
    let source = r#"model circuit {
  domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port terminal: conserving on electrical;
  relation component continuous { across(terminal) = 0; }
}"#;
    let document = parse("circuit.eqi", source)
        .into_document()
        .expect("valid physical source");
    let items = document.models()[0].items();

    let Item::Domain(domain) = &items[0] else {
        panic!("first item is the physical Domain");
    };
    let DomainSyntax::ScalarPhysical {
        across_dimension,
        through_dimension,
    } = domain.syntax()
    else {
        panic!("Domain retains the scalar physical contract");
    };
    assert_eq!(
        &source[domain.range().start() as usize..domain.range().end() as usize],
        "domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);"
    );
    assert_eq!(
        &source[across_dimension.range().start() as usize..across_dimension.range().end() as usize],
        "kg * m ^ 2 / (s ^ 3 * A)"
    );
    assert_eq!(
        &source
            [through_dimension.range().start() as usize..through_dimension.range().end() as usize],
        "A"
    );

    let Item::Port(port) = &items[1] else {
        panic!("second item is the physical Port");
    };
    assert!(matches!(
        port.syntax(),
        PortSyntax::ScalarPhysical { domain } if domain == "electrical"
    ));
    assert_eq!(
        &source[port.range().start() as usize..port.range().end() as usize],
        "port terminal: conserving on electrical;"
    );
}

#[test]
fn parser_builds_typed_component_interfaces_instances_and_paths() {
    let source = r#"
connector Pin = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);

component Pair {
  public parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2);
  parameter scale: 1 = 2;
  public port positive: conserving on Pin;
  port command: signal input 1;
  instance inner: Catalog.Resistor(resistance = resistance * scale);
  relation law continuous { across(inner.positive) - resistance = 0; }
  connect conserving inner.positive, positive;
}

model parallel {
  instance r2: Pair(resistance = 2);
  instance r4: Pair(resistance = 4);
  connect conserving r2.positive, r4.positive;
  boundary r2.positive;
}
"#;
    let document = parse("components.eqi", source)
        .into_document()
        .expect("component syntax is valid");

    assert_eq!(document.connectors().len(), 1);
    assert_eq!(document.connectors()[0].name(), "Pin");
    assert!(matches!(
        document.connectors()[0].syntax(),
        ConnectorSyntax::ScalarPhysical { .. }
    ));

    let component = &document.components()[0];
    assert_eq!(component.name(), "Pair");
    let ComponentItem::Parameter(resistance) = &component.items()[0] else {
        panic!("first member is the public Parameter");
    };
    assert_eq!(resistance.visibility(), VisibilitySyntax::Public);
    assert!(resistance.default().is_none());
    let ComponentItem::Parameter(scale) = &component.items()[1] else {
        panic!("second member is the private Parameter");
    };
    assert_eq!(scale.visibility(), VisibilitySyntax::Private);
    assert!(scale.default().is_some());
    let ComponentItem::Port(positive) = &component.items()[2] else {
        panic!("third member is the public Port");
    };
    assert_eq!(positive.visibility(), VisibilitySyntax::Public);
    let PortSyntax::ScalarPhysicalConnector { connector } = positive.syntax() else {
        panic!("physical component Ports retain nominal Connector syntax");
    };
    assert_eq!(connector.segments().collect::<Vec<_>>(), ["Pin"]);

    let ComponentItem::Instance(inner) = &component.items()[4] else {
        panic!("fifth member is the nested instance");
    };
    assert_eq!(
        inner.definition().segments().collect::<Vec<_>>(),
        ["Catalog", "Resistor"]
    );
    assert_eq!(inner.bindings()[0].parameter(), "resistance");

    let ComponentItem::Relation(relation) = &component.items()[5] else {
        panic!("sixth member is the Relation");
    };
    let ExprKind::Binary { left, .. } = relation.residuals()[0].kind() else {
        panic!("Relation retains its subtraction");
    };
    let ExprKind::Call { arguments, .. } = left.kind() else {
        panic!("left side is across(...)");
    };
    let ExprKind::Path(path) = arguments[0].kind() else {
        panic!("instance Port selection is a structured path");
    };
    assert_eq!(path.segments().collect::<Vec<_>>(), ["inner", "positive"]);
    assert_eq!(
        &source[path.range().start() as usize..path.range().end() as usize],
        "inner.positive"
    );

    let Item::Connection(connection) = &document.models()[0].items()[2] else {
        panic!("third model member is a Connection");
    };
    assert_eq!(
        connection.port_paths()[0].segments().collect::<Vec<_>>(),
        ["r2", "positive"]
    );
    let Item::Boundary(boundary) = &document.models()[0].items()[3] else {
        panic!("fourth model member is the boundary");
    };
    assert!(boundary.port_paths()[0].is_qualified());
}

#[test]
fn parser_retains_component_support_slots_representations_and_mixed_bindings() {
    let source = r#"component BoundaryState {
  public support body: volume(ambient_dimension = 2);
  public support interface: boundary(parent = body);
  representation state_space = continuum;
  field state on body as state_space: 1 = 0;
}

model coupled {
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance probe: BoundaryState(gain = 2, support body = fluid, support interface = wall);
}"#;
    let document = parse("supports.eqi", source)
        .into_document()
        .expect("support-slot syntax is valid");
    let component = &document.components()[0];

    let ComponentItem::Support(body) = &component.items()[0] else {
        panic!("first member is the volume support slot");
    };
    assert_eq!(body.visibility(), VisibilitySyntax::Public);
    assert!(matches!(
        body.syntax(),
        SupportSlotSyntax::Volume {
            ambient_dimension: 2
        }
    ));
    assert_eq!(
        &source[body.range().start() as usize..body.range().end() as usize],
        "public support body: volume(ambient_dimension = 2);"
    );

    let ComponentItem::Support(interface) = &component.items()[1] else {
        panic!("second member is the boundary support slot");
    };
    assert!(matches!(
        interface.syntax(),
        SupportSlotSyntax::Boundary { parent } if parent == "body"
    ));
    let ComponentItem::Representation(representation) = &component.items()[2] else {
        panic!("third member is the private Representation");
    };
    assert_eq!(representation.name(), "state_space");

    let Item::Instance(instance) = &document.models()[0].items()[2] else {
        panic!("third model member is the component instance");
    };
    assert_eq!(instance.bindings().len(), 1);
    assert_eq!(instance.bindings()[0].parameter(), "gain");
    assert_eq!(instance.support_bindings().len(), 2);
    assert_eq!(instance.support_bindings()[0].slot(), "body");
    assert_eq!(instance.support_bindings()[0].target(), "fluid");
    assert_eq!(instance.support_bindings()[1].slot(), "interface");
    assert_eq!(instance.support_bindings()[1].target(), "wall");
}

#[test]
fn parser_retains_occurrence_bound_field_slots_and_bindings() {
    let source = r#"component IsotropicBalance2d {
  public support body: volume(ambient_dimension = 2);
  public field slot displacement on body as continuum: m shape spatial_vector;
  public field slot load on body as continuum: kg / (m * s ^ 2) shape spatial_vector;
  public parameter mu: kg / (m * s ^ 2);
}

model Main {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field u on body as space: m shape spatial_vector;
  field f on body as space: kg / (m * s ^ 2) shape spatial_vector;
  instance law: IsotropicBalance2d(mu = 3, support body = body, field displacement = u, field load = f);
}"#;
    let document = parse("field-slots.eqi", source)
        .into_document()
        .expect("Field-slot syntax is valid");
    let component = &document.components()[0];

    let ComponentItem::FieldSlot(displacement) = &component.items()[1] else {
        panic!("second member is the displacement Field slot");
    };
    assert_eq!(displacement.name(), "displacement");
    assert_eq!(displacement.support(), "body");
    assert_eq!(displacement.shape(), Some(&ValueShapeSyntax::SpatialVector));
    assert_eq!(
        &source[displacement.range().start() as usize..displacement.range().end() as usize],
        "public field slot displacement on body as continuum: m shape spatial_vector;"
    );

    let Item::Instance(instance) = &document.models()[0].items()[4] else {
        panic!("fifth model member is the component instance");
    };
    assert_eq!(instance.bindings().len(), 1);
    assert_eq!(instance.support_bindings().len(), 1);
    assert_eq!(instance.field_bindings().len(), 2);
    assert_eq!(instance.field_bindings()[0].slot(), "displacement");
    assert_eq!(instance.field_bindings()[0].target(), "u");
    assert_eq!(instance.field_bindings()[1].slot(), "load");
    assert_eq!(instance.field_bindings()[1].target(), "f");
    assert_eq!(crate::format(&document), format!("{source}\n"));
}

#[test]
fn field_discriminator_does_not_reserve_the_parameter_name_field() {
    let document = parse(
        "field-parameter.eqi",
        "component C { public parameter field: 1; } model m { instance c: C(field = 1); }",
    )
    .into_document()
    .expect("`field = expression` remains a Parameter binding");
    let Item::Instance(instance) = &document.models()[0].items()[0] else {
        panic!("model member is an instance");
    };

    assert_eq!(instance.bindings()[0].parameter(), "field");
    assert!(instance.field_bindings().is_empty());
}

#[test]
fn parser_rejects_non_public_or_non_continuum_field_slots() {
    let private = parse(
        "private-slot.eqi",
        "component C { field slot state on body as continuum: 1; }",
    );
    assert!(private.into_document().is_err());

    let discrete = parse(
        "discrete-slot.eqi",
        "component C { public field slot state on body as discrete: 1; }",
    );
    assert!(discrete.into_document().is_err());
}

#[test]
fn support_discriminator_does_not_reserve_the_parameter_name_support() {
    let document = parse(
        "support-parameter.eqi",
        "component C { public parameter support: 1; } model m { instance c: C(support = 1); }",
    )
    .into_document()
    .expect("`support = expression` remains a Parameter binding");
    let Item::Instance(instance) = &document.models()[0].items()[0] else {
        panic!("model member is an instance");
    };

    assert_eq!(instance.bindings()[0].parameter(), "support");
    assert!(instance.support_bindings().is_empty());
}

#[test]
fn parser_accepts_visibility_typed_declaration_only_documents() {
    let source = r#"public connector Pin = scalar_physical(across = 1, through = A);
private component Internal {}
public component Resistor {}"#;
    let document = parse("library.eqi", source)
        .into_document()
        .expect("library declarations parse without a Model");

    assert!(document.models().is_empty());
    assert_eq!(
        document.connectors()[0].visibility(),
        VisibilitySyntax::Public
    );
    assert_eq!(
        document.components()[0].visibility(),
        VisibilitySyntax::Private
    );
    assert_eq!(
        document.components()[1].visibility(),
        VisibilitySyntax::Public
    );
    assert_eq!(
        &source[document.connectors()[0].range().start() as usize
            ..document.connectors()[0].range().end() as usize],
        "public connector Pin = scalar_physical(across = 1, through = A);"
    );
    assert_eq!(
        &source[document.components()[0].range().start() as usize
            ..document.components()[0].range().end() as usize],
        "private component Internal {}"
    );
}

#[test]
fn parser_retains_public_and_private_model_visibility() {
    let public = parse("entry.eqi", "public model Main {}")
        .into_document()
        .expect("public Model is accepted");
    assert_eq!(public.models().len(), 1);
    assert_eq!(public.models()[0].visibility(), VisibilitySyntax::Public);
    assert_eq!(public.models()[0].range(), TextRange::new(0, 20));
    assert_eq!(crate::format(&public), "public model Main {\n}\n");

    let private = parse("entry.eqi", "private model Main {}")
        .into_document()
        .expect("an explicitly package-local Model is accepted");
    assert_eq!(private.models()[0].visibility(), VisibilitySyntax::Private);
    assert_eq!(private.models()[0].name(), "Main");
}

#[test]
fn parser_discards_illegal_public_members_and_recovers() {
    let source = r#"
component Invalid {
  public relation exposed continuous { 1 = 0; }
  public instance child: Other;
  instance malformed: ;
  parameter retained: 1 = 1;
}
model root {}
"#;
    let result = parse("visibility.eqi", source);
    let document = result.document().expect("recovered compilation unit");

    assert_eq!(result.diagnostics().len(), 3);
    assert_eq!(document.components()[0].items().len(), 1);
    assert!(matches!(
        document.components()[0].items()[0],
        ComponentItem::Parameter(_)
    ));
}

#[test]
fn parser_requires_compilation_unit_definitions_before_models() {
    let result = parse(
        "order.eqi",
        "model first {} component Late {} model second {}",
    );
    let document = result.document().expect("declarations are recovered");

    assert_eq!(result.diagnostics().len(), 1);
    assert_eq!(document.components()[0].name(), "Late");
    assert_eq!(document.models().len(), 2);
}

#[test]
fn parser_retains_field_physical_connector_shapes_ports_and_flux_access() {
    let source = r#"
public connector MechanicalBoundary = field_physical(
  pairing = euclidean_boundary_duality,
  flux = traction: kg / (m * s ^ 2),
  frame = spatial,
  trace = velocity: m / s,
  shape = spatial_vector
);

model coupled {
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = upper);
  representation state_space = continuum;
  field velocity on fluid as state_space: m / s shape [2];
  port interface: conserving MechanicalBoundary over wall;
  relation balance continuous on wall { flux(interface) = 0; }
}
"#;
    let document = parse("field-physical.eqi", source)
        .into_document()
        .expect("field-valued boundary syntax parses");

    let ConnectorSyntax::FieldPhysical {
        trace,
        flux,
        shape,
        frame,
        pairing,
    } = document.connectors()[0].syntax()
    else {
        panic!("field-physical Connector retained");
    };
    assert_eq!(trace.name(), "velocity");
    assert_eq!(flux.name(), "traction");
    assert_eq!(shape, &ValueShapeSyntax::SpatialVector);
    assert_eq!(*frame, FrameSyntax::Spatial);
    assert_eq!(*pairing, BoundaryPairingSyntax::EuclideanBoundaryDuality);

    let Item::Field(field) = &document.models()[0].items()[3] else {
        panic!("fourth item is the shaped Field");
    };
    assert_eq!(field.shape(), Some(&ValueShapeSyntax::Exact(vec![2])));
    let Item::Port(port) = &document.models()[0].items()[4] else {
        panic!("fifth item is the boundary Port");
    };
    let PortSyntax::FieldPhysical { connector, support } = port.syntax() else {
        panic!("field-physical Port retained");
    };
    assert_eq!(connector.as_str(), "MechanicalBoundary");
    assert_eq!(support, "wall");
    let Item::Relation(relation) = &document.models()[0].items()[5] else {
        panic!("sixth item is the Relation");
    };
    assert!(matches!(
        relation.residuals()[0].kind(),
        ExprKind::Call { callee, .. } if callee.as_str() == "flux"
    ));
}

#[test]
fn shaped_fields_never_desugar_a_scalar_initial_value() {
    let valid = parse(
        "shaped-field.eqi",
        "model M { field velocity: m / s shape [2]; }",
    );
    let document = valid.into_document().expect("shaped Field without initial");
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("fixture contains one Field");
    };
    assert_eq!(field.initial(), None);

    let invalid = parse(
        "broadcast.eqi",
        "model M { field velocity: m / s shape [2] = 0; }",
    );
    assert!(invalid.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("non-scalar Field cannot have a scalar initial value")
    }));
}

#[test]
fn field_physical_connector_fields_are_closed_and_exactly_once() {
    let invalid_sources = [
        (
            "duplicate",
            "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant, trace = v: 1, pairing = euclidean_boundary_duality);",
        ),
        (
            "missing",
            "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant);",
        ),
        (
            "unknown",
            "connector C = field_physical(trace = u: 1, flux = f: 1, shape = [], frame = invariant, pairing = euclidean_boundary_duality, channels = 2);",
        ),
    ];

    for (case, source) in invalid_sources {
        let result = parse(format!("{case}.eqi"), source);
        assert!(!result.diagnostics().is_empty(), "{case} must fail closed");
        assert!(result.into_document().is_err(), "{case} cannot compile");
    }
}

#[test]
fn parser_retains_closed_complete_exterior_family_syntax() {
    let source = r#"
component BoundaryLaw {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[boundary in exterior]: conserving MechanicalBoundary over boundary;
  relation natural[boundary in exterior] continuous on boundary {
flux(mechanical[boundary = boundary]) = 0;
  }
  connect conserving [boundary in exterior] child.mechanical[boundary = boundary], mechanical[boundary = boundary];
}

model coupled {
  instance law: BoundaryLaw(
support body = fluid,
support exterior = boundaries(x_lower, x_upper, y_lower, y_upper)
  );
  connect conserving law.mechanical[boundary = x_lower], environment;
}
"#;
    let document = parse("complete-exterior.eqi", source)
        .into_document()
        .expect("restricted boundary-family syntax parses");
    let component = &document.components()[0];

    let ComponentItem::Support(exterior) = &component.items()[1] else {
        panic!("second component member is the complete exterior");
    };
    assert!(matches!(
        exterior.syntax(),
        SupportSlotSyntax::CompleteExterior { parent } if parent == "body"
    ));

    let ComponentItem::PortFamily(port) = &component.items()[2] else {
        panic!("third component member is a Port family");
    };
    assert_eq!(port.binder().member(), "boundary");
    assert_eq!(port.binder().set(), "exterior");

    let ComponentItem::RelationFamily(relation) = &component.items()[3] else {
        panic!("fourth component member is a Relation family");
    };
    assert_eq!(relation.relation().domain(), Some("boundary"));
    let ExprKind::Call { arguments, .. } = relation.relation().residuals()[0].kind() else {
        panic!("family Relation residual contains flux selection");
    };
    assert!(matches!(
        arguments[0].kind(),
        ExprKind::BoundaryPortSelection { selector, .. }
            if selector.member() == "boundary" && selector.target() == "boundary"
    ));

    let ComponentItem::BoundaryConnection(connection) = &component.items()[4] else {
        panic!("fifth component member is a pointwise conserving Connection");
    };
    assert_eq!(connection.binder().expect("binder").set(), "exterior");

    let Item::Instance(instance) = &document.models()[0].items()[0] else {
        panic!("first model member is the component instance");
    };
    assert!(
        instance
            .support_bindings()
            .iter()
            .any(|binding| { binding.slot() == "body" && binding.target() == "fluid" })
    );
    assert_eq!(instance.boundary_set_bindings().len(), 1);
    assert_eq!(
        instance.boundary_set_bindings()[0]
            .members()
            .iter()
            .map(BoundarySetMemberSyntax::target)
            .collect::<Vec<_>>(),
        ["x_lower", "x_upper", "y_lower", "y_upper"]
    );

    let Item::BoundaryConnection(connection) = &document.models()[0].items()[1] else {
        panic!("second model member is a selected conserving Connection");
    };
    assert!(connection.binder().is_none());
    assert_eq!(
        connection.ports()[0].selector().expect("selector").target(),
        "x_lower"
    );
}

#[test]
fn parser_retains_exact_model_spatial_periodic_pairs() {
    let source = r#"
model periodic {
  connect periodic upper.interface, lower.interface;
}
"#;
    let document = parse("periodic.eqi", source)
        .into_document()
        .expect("closed Model spatial-periodic pair parses");
    let Item::BoundaryConnection(connection) = &document.models()[0].items()[0] else {
        panic!("periodic syntax is retained as a boundary Connection");
    };
    assert_eq!(connection.syntax(), ConnectionSyntax::SpatialPeriodic);
    assert_eq!(connection.ports().len(), 2);
    assert!(connection.binder().is_none());

    let component = parse(
        "component-periodic.eqi",
        "component C { connect periodic upper, lower; }",
    );
    assert!(component.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("allowed only in closed Models")
    }));
}

#[test]
fn parser_rejects_boundary_binders_outside_the_closed_family_sites() {
    let invalid_sources = [
        (
            "signal-port",
            "component C { public port p[b in exterior]: signal input 1; }",
        ),
        (
            "periodic-relation",
            "component C { clock c = periodic(period = 1 / 1, phase = 0 / 1); relation r[b in exterior] periodic(c) on b { 1 = 0; } }",
        ),
        (
            "model-relation",
            "model M { relation r[b in exterior] continuous on b { 1 = 0; } }",
        ),
        (
            "model-connection",
            "model M { connect conserving [b in exterior] left, right; }",
        ),
    ];

    for (case, source) in invalid_sources {
        let result = parse(format!("{case}.eqi"), source);
        assert!(!result.diagnostics().is_empty(), "{case} must fail closed");
        assert!(result.into_document().is_err(), "{case} cannot compile");
    }
}

#[test]
fn parser_and_formatter_retain_ordered_dimension_prefix_with_exact_ranges() {
    let source = "dimension Speed = m / s;\ndimension Acceleration = Speed / s;\nmodel M { field velocity: Speed = 0; }";
    let document = parse("dimensions.eqi", source)
        .into_document()
        .expect("dimension prefix parses");

    assert_eq!(document.dimension_syntax().len(), 2);
    let (name, _, range) = document.dimension_syntax().next().expect("first alias");
    assert_eq!(name, "Speed");
    assert_eq!(
        &source[range.start() as usize..range.end() as usize],
        "dimension Speed = m / s;"
    );
    let formatted = crate::format(&document);
    let reparsed = parse("dimensions.eqi", &formatted)
        .into_document()
        .expect("formatted prefix reparses");
    assert_eq!(crate::format(&reparsed), formatted);

    let misplaced = parse(
        "misplaced.eqi",
        "model M { field x: m = 0; } dimension Length = m;",
    );
    assert!(
        misplaced
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message().contains("must form a prefix"))
    );
}
