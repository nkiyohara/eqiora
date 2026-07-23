//! Exact-package alias evidence for RFC 0041 family elaboration.

use eqiora::compiler::{
    CompilationNamespaceId, CompiledModel, ResolvedAlias, ResolvedHierarchyInput,
    ResolvedSourceUnit, analyze_resolved_hierarchy,
};

const DEPENDENCY: &str = r#"
public connector MechanicalBoundary = field_physical(
  trace = displacement: m,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);

public component ExteriorLaw {
  public support body: volume(ambient_dimension = 2);
  public support exterior: complete_exterior(parent = body);
  public port mechanical[boundary in exterior]:
    conserving MechanicalBoundary over boundary;
  relation boundary_law[boundary in exterior] continuous on boundary {
    trace(mechanical[boundary = boundary])
      - trace(mechanical[boundary = boundary]) = 0;
    flux(mechanical[boundary = boundary])
      - flux(mechanical[boundary = boundary]) = 0;
  }
}
"#;

fn namespace(name: &str) -> CompilationNamespaceId {
    CompilationNamespaceId::new([name]).expect("bounded test namespace")
}

fn root_source(alias: &str) -> String {
    format!(
        r#"
public component BoundaryTerminal {{
  public support body: volume(ambient_dimension = 2);
  public support face: boundary(parent = body);
  public port mechanical: conserving {alias}.MechanicalBoundary over face;
  relation terminal_law continuous on face {{
    trace(mechanical) - trace(mechanical) = 0;
    flux(mechanical) - flux(mechanical) = 0;
  }}
}}

model Main {{
  domain body = box(0, 1, 0, 1);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);

  instance solid: {alias}.ExteriorLaw(
    support body = body,
    support exterior = boundaries(y_upper, x_lower, y_lower, x_upper)
  );
  instance x_lower_terminal: BoundaryTerminal(
    support body = body,
    support face = x_lower
  );
  instance x_upper_terminal: BoundaryTerminal(
    support body = body,
    support face = x_upper
  );
  instance y_lower_terminal: BoundaryTerminal(
    support body = body,
    support face = y_lower
  );
  instance y_upper_terminal: BoundaryTerminal(
    support body = body,
    support face = y_upper
  );

  connect conserving solid.mechanical[boundary = x_lower],
    x_lower_terminal.mechanical;
  connect conserving solid.mechanical[boundary = x_upper],
    x_upper_terminal.mechanical;
  connect conserving solid.mechanical[boundary = y_lower],
    y_lower_terminal.mechanical;
  connect conserving solid.mechanical[boundary = y_upper],
    y_upper_terminal.mechanical;
}}
"#
    )
}

fn compile_with_alias(alias: &str) -> CompiledModel {
    let root = namespace("root-package-digest");
    let dependency = namespace("mechanics-package-digest");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            ResolvedSourceUnit::new(root.clone(), "root/main.eqi", root_source(alias)),
            ResolvedSourceUnit::new(dependency.clone(), "mechanics/exterior.eqi", DEPENDENCY),
        ],
        vec![ResolvedAlias::new(root, alias, dependency)],
    );
    analyze_resolved_hierarchy(input)
        .expect("exact hierarchy analysis")
        .validate_definitions()
        .expect("exact hierarchy definition proof")
        .compile_root("Main")
        .expect("aliased complete-exterior occurrence")
}

#[test]
fn dependency_alias_spelling_cannot_change_family_meaning() {
    let short = compile_with_alias("solid");
    let descriptive = compile_with_alias("linear_elasticity");

    assert_eq!(short.model(), descriptive.model());
    assert_eq!(short.symbols(), descriptive.symbols());
    assert_eq!(short.transaction().ops(), descriptive.transaction().ops());
    assert_eq!(
        super::forwarding::canonical_program(short),
        super::forwarding::canonical_program(descriptive)
    );
}
