use crate::parse;

use super::*;

#[test]
fn signed_parameter_quantities_round_trip_with_exact_unit_powers() {
    let source = "model M { parameter value: m ^ (1 / 2) = -4[(mm ^ 2) ^ (1 / 4)]; }";
    let document = parse("quantity.eqi", source).into_document().unwrap();
    let formatted = format(&document);
    assert!(formatted.contains("-4 [(mm ^ 2) ^ (1 / 4)]"));
    let reparsed = parse("quantity.eqi", &formatted).into_document().unwrap();
    assert_eq!(format(&reparsed), formatted);
    for value in ["1[]", "1[m", "1[m ^ (1 / 0)]", "1 + 2", "-1[ms] + 2"] {
        let source = format!("model M {{ parameter value: m = {value}; }}");
        assert!(
            parse("invalid.eqi", &source).into_document().is_err(),
            "{source}"
        );
    }
}

#[test]
fn canonical_format_is_idempotent() {
    let source = "model m{field x:1=0;relation r continuous{derivative(x)-(1+x*2)=0;}}";
    let first = parse("m.eqi", source).into_document().expect("parse");
    let formatted = format(&first);
    let second = parse("m.eqi", &formatted)
        .into_document()
        .expect("formatted parse");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("derivative(x) = 1 + x * 2;"));
}

#[test]
fn pure_operators_format_before_consumers_with_canonical_exact_integers() {
    let source = r#"component Consumer {}
public pure operator dyadic(left: spatial[01], right: scalar) -> spatial[2] = component(left, 00) * component(right) + rational(03, 04) * delta(0, 01);
model M {
  field u: 1 = 0;
  relation law continuous { catalog.dyadic(u, u) = 0; }
}"#;
    let document = parse("pure-format.eqi", source)
        .into_document()
        .expect("pure operator source");
    let formatted = format(&document);
    let reparsed = parse("pure-format.eqi", &formatted)
        .into_document()
        .expect("canonical pure operator source");

    assert_eq!(format(&reparsed), formatted);
    assert!(
        formatted.find("pure operator").expect("operator")
            < formatted.find("component Consumer").expect("component")
    );
    assert!(formatted.contains(
        "dyadic(left: spatial[1], right: scalar) -> spatial[2] = component(left, 0) * component(right) + rational(3, 4) * delta(0, 1);"
    ));
    assert!(formatted.contains("catalog.dyadic(u, u)"));
}

#[test]
fn spatial_source_roundtrips_with_explicit_scope() {
    let source = r#"
model bar {
  domain body = box(0, 2);
  domain fixed = boundary(body, axis = 0, side = lower);
  representation space = continuum;
  field u on body as space: m = 0;
  relation clamp continuous on fixed { trace(u) = 0; }
}
"#;
    let first = parse("bar.eqi", source).into_document().expect("parse");
    let formatted = format(&first);
    let second = parse("bar.eqi", &formatted)
        .into_document()
        .expect("formatted parse");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("relation clamp continuous on fixed"));
}

#[test]
fn coordinate_math_roundtrips_without_losing_structure() {
    let source = r#"
model poisson {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: 1 = 0;
  parameter wave_number: 1 / m = 3.141592653589793;
  relation balance continuous on interval {
-div(grad(u)) - math.sin(wave_number * coordinate(0)) = 0;
  }
}
"#;
    let first = parse("poisson.eqi", source).into_document().expect("parse");
    let formatted = format(&first);
    let second = parse("poisson.eqi", &formatted)
        .into_document()
        .expect("formatted parse");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("math.sin(wave_number * coordinate(0))"));
}

#[test]
fn scalar_physical_source_roundtrips_without_weakening_legacy_markers() {
    let source = r#"
model circuit {
  domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port positive: conserving on electrical;
  port legacy: conserving A;
  port dimensionless: conserving 1;
  relation source continuous { across(positive) = 0; }
}
"#;
    let first = parse("circuit.eqi", source)
        .into_document()
        .expect("physical source parses");
    let formatted = format(&first);
    let second = parse("circuit.eqi", &formatted)
        .into_document()
        .expect("formatted physical source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains(
        "domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);"
    ));
    assert!(formatted.contains("port positive: conserving on electrical;"));
    assert!(formatted.contains("port legacy: conserving A;"));
    assert!(formatted.contains("port dimensionless: conserving 1;"));
}

#[test]
fn component_source_roundtrips_with_canonical_qualified_names() {
    let source = r#"
connector Pin=scalar_physical(across=kg*m^2/(s^3*A),through=A);
component Resistor{
public parameter resistance:kg*m^2/(s^3*A^2);
public port positive:conserving on Pin;
public port negative:conserving on Pin;
relation law continuous{across(positive)-across(negative)-resistance*through(positive)=0;}
}
component Pair{
public parameter resistance:kg*m^2/(s^3*A^2)=2;
public port positive:conserving on Pin;
instance inner:Library.Resistor(resistance=resistance);
connect conserving positive,inner.positive;
}
model circuit{
instance r2:Pair(resistance=2);
instance r4:Pair(resistance=4);
connect conserving r2.positive,r4.positive;
}
"#;
    let first = parse("component.eqi", source)
        .into_document()
        .expect("component source parses");
    let formatted = format(&first);
    let second = parse("component.eqi", &formatted)
        .into_document()
        .expect("formatted component source parses");
    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("connector Pin = scalar_physical"));
    assert!(formatted.contains("instance inner: Library.Resistor(resistance = resistance);"));
    assert!(formatted.contains("connect conserving r2.positive, r4.positive;"));
}

#[test]
fn component_support_source_roundtrips_with_discriminated_bindings() {
    let source = r#"component BoundaryState{
public support body:volume(ambient_dimension=2);
public support interface:boundary(parent=body);
representation state_space=continuum;
field state on body as state_space:1=0;
}
model coupled{
domain fluid=box(0,1,0,1);
domain wall=boundary(fluid,axis=0,side=lower);
instance probe:BoundaryState(support interface=wall,gain=2,support body=fluid);
}"#;
    let first = parse("supports.eqi", source)
        .into_document()
        .expect("support source parses");
    let formatted = format(&first);
    let second = parse("supports.eqi", &formatted)
        .into_document()
        .expect("formatted support source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("public support body: volume(ambient_dimension = 2);"));
    assert!(formatted.contains("public support interface: boundary(parent = body);"));
    assert!(formatted.contains("representation state_space = continuum;"));
    assert!(formatted.contains(
        "instance probe: BoundaryState(gain = 2, support interface = wall, support body = fluid);"
    ));
}

#[test]
fn package_visibility_roundtrips_in_canonical_source() {
    let source = r#"private connector Hidden=scalar_physical(across=1,through=A);
public connector Pin=scalar_physical(across=1,through=A);
private component Internal{}
public component Resistor{}"#;
    let first = parse("library.eqi", source)
        .into_document()
        .expect("declaration-only source parses");
    let formatted = format(&first);
    let second = parse("relocated/library.eqi", &formatted)
        .into_document()
        .expect("canonical declaration-only source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("connector Hidden = scalar_physical"));
    assert!(!formatted.contains("private connector"));
    assert!(formatted.contains("public connector Pin = scalar_physical"));
    assert!(formatted.contains("component Internal {\n}"));
    assert!(!formatted.contains("private component"));
    assert!(formatted.contains("public component Resistor {\n}"));
    assert_eq!(
        first.connectors()[0].visibility(),
        second.connectors()[0].visibility()
    );
    assert_eq!(
        first.components()[1].visibility(),
        second.components()[1].visibility()
    );
}

#[test]
fn field_physical_source_has_one_fixed_readable_order() {
    let source = r#"public connector MechanicalBoundary=field_physical(pairing=euclidean_boundary_duality,frame=spatial,shape=[2],flux=traction:kg/(m*s^2),trace=velocity:m/s);
component Wall {
public support wall:boundary(parent=body);
public support body:volume(ambient_dimension=2);
public port interface:conserving MechanicalBoundary over wall;
field velocity: m/s shape spatial_vector;
relation load continuous { flux(interface)=0; }
}"#;
    let first = parse("boundary.eqi", source)
        .into_document()
        .expect("field-physical source parses");
    let formatted = format(&first);
    let second = parse("boundary.eqi", &formatted)
        .into_document()
        .expect("canonical field-physical source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains(
        "field_physical(\n  trace = velocity: m / s,\n  flux = traction: kg / (m * s ^ 2),\n  shape = [2],\n  frame = spatial,\n  pairing = euclidean_boundary_duality\n);"
    ));
    assert!(formatted.contains("public port interface: conserving MechanicalBoundary over wall;"));
    assert!(formatted.contains("field velocity: m / s shape spatial_vector;"));
    assert!(formatted.contains("flux(interface) = 0;"));
}

#[test]
fn complete_exterior_families_have_one_closed_canonical_spelling() {
    let source = r#"component BoundaryLaw{
public support body:volume(ambient_dimension=2);
public support exterior:complete_exterior(parent=body);
public port mechanical[boundary in exterior]:conserving MechanicalBoundary over boundary;
relation natural[boundary in exterior] continuous on boundary{flux(mechanical[boundary=boundary])=0;}
connect conserving[boundary in exterior] child.mechanical[boundary=boundary],mechanical[boundary=boundary];
}
model coupled{
instance law:BoundaryLaw(support body=fluid,support exterior=boundaries(x_lower,x_upper,y_lower,y_upper));
connect conserving law.mechanical[boundary=x_lower],environment;
}"#;
    let first = parse("complete-exterior.eqi", source)
        .into_document()
        .expect("boundary-family source parses");
    let formatted = format(&first);
    let second = parse("complete-exterior.eqi", &formatted)
        .into_document()
        .expect("canonical boundary-family source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("public support exterior: complete_exterior(parent = body);"));
    assert!(formatted.contains(
        "public port mechanical[boundary in exterior]: conserving MechanicalBoundary over boundary;"
    ));
    assert!(formatted.contains("relation natural[boundary in exterior] continuous on boundary {"));
    assert!(formatted.contains(
        "connect conserving [boundary in exterior] child.mechanical[boundary = boundary], mechanical[boundary = boundary];"
    ));
    assert!(
        formatted.contains("support exterior = boundaries(x_lower, x_upper, y_lower, y_upper)")
    );
    assert!(
        formatted.contains("connect conserving law.mechanical[boundary = x_lower], environment;")
    );
}

#[test]
fn spatial_periodic_pair_has_one_closed_canonical_spelling() {
    let source = "model M{connect periodic upper.interface,lower.interface;}";
    let first = parse("periodic.eqi", source)
        .into_document()
        .expect("spatial-periodic source parses");
    let formatted = format(&first);
    let second = parse("periodic.eqi", &formatted)
        .into_document()
        .expect("canonical spatial-periodic source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("connect periodic upper.interface, lower.interface;"));
}

#[test]
fn legacy_support_relations_and_connections_keep_their_canonical_spelling() {
    let source = r#"component Legacy{
public support body:volume(ambient_dimension=2);
public support wall:boundary(parent=body);
public port interface:conserving MechanicalBoundary over wall;
relation law continuous on wall{flux(interface)=0;}
connect conserving interface,child.interface;
}
model use_legacy{
instance legacy:Legacy(support body=fluid,support wall=fixed);
connect conserving legacy.interface,environment;
}"#;
    let first = parse("legacy.eqi", source)
        .into_document()
        .expect("legacy source parses");
    let formatted = format(&first);
    let second = parse("legacy.eqi", &formatted)
        .into_document()
        .expect("canonical legacy source parses");

    assert_eq!(format(&second), formatted);
    assert!(formatted.contains("public support wall: boundary(parent = body);"));
    assert!(formatted.contains("public port interface: conserving MechanicalBoundary over wall;"));
    assert!(formatted.contains("relation law continuous on wall {"));
    assert!(formatted.contains("connect conserving interface, child.interface;"));
    assert!(!formatted.contains("boundaries("));
    assert!(!formatted.contains("[boundary"));
}
