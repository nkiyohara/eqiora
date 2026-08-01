//! Nested-forwarding evidence for RFC 0041 complete-exterior Port families.

use std::collections::BTreeSet;

use eqiora::artifact::ModelEnvelope;
use eqiora::compiler::{CompiledModel, ModelSymbols, compile};
use eqiora::entity::kinds;
use eqiora::graph::{GraphStore, InMemoryGraphStore, Op};
use eqiora::kernel::KernelNode;
use eqiora::sem::KernelProgram;
use eqiora::{Entity, RawId, Span};

const FORWARDED: &str = include_str!(
    "../../../../verify/packages/complete-exterior-port-families/models/forwarded.eqi"
);

const FORWARDED_PERMUTED: &str = r#"
public connector MechanicalBoundary = field_physical(
  flux = traction: kg / (m * s ^ 2),
  trace = displacement: m,
  pairing = euclidean_boundary_duality,
  frame = spatial,
  shape = spatial_vector
);

public component ExteriorWrapper {
  public port mechanical[boundary in exterior]:
    conserving MechanicalBoundary over boundary;
  public support exterior: complete_exterior(parent = body);
  public support body: volume(ambient_dimension = 2);

  instance child: ExteriorLaw(
    support exterior = exterior,
    support body = body
  );

  connect conserving [boundary in exterior]
    mechanical[boundary = boundary],
    child.mechanical[boundary = boundary];
}

public component BoundaryTerminal {
  public port mechanical: conserving MechanicalBoundary over face;
  public support face: boundary(parent = body);
  public support body: volume(ambient_dimension = 2);

  relation terminal_law continuous on face {
    trace(mechanical) - trace(mechanical) = 0;
    flux(mechanical) - flux(mechanical) = 0;
  }
}

public component ExteriorLaw {
  relation boundary_law[boundary in exterior] continuous on boundary {
    trace(mechanical[boundary = boundary])
      - trace(mechanical[boundary = boundary]) = 0;
    flux(mechanical[boundary = boundary])
      - flux(mechanical[boundary = boundary]) = 0;
  }

  public port mechanical[boundary in exterior]:
    conserving MechanicalBoundary over boundary;
  public support exterior: complete_exterior(parent = body);
  public support body: volume(ambient_dimension = 2);
}

model Main {
  domain body = box(0, 1, 0, 1);
  domain y_upper = boundary(body, axis = 1, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain x_lower = boundary(body, axis = 0, side = lower);

  instance wrapped: ExteriorWrapper(
    support exterior = boundaries(x_upper, y_lower, x_lower, y_upper),
    support body = body
  );
  instance y_upper_terminal: BoundaryTerminal(
    support face = y_upper,
    support body = body
  );
  instance y_lower_terminal: BoundaryTerminal(
    support face = y_lower,
    support body = body
  );
  instance x_upper_terminal: BoundaryTerminal(
    support face = x_upper,
    support body = body
  );
  instance x_lower_terminal: BoundaryTerminal(
    support face = x_lower,
    support body = body
  );

  connect conserving y_upper_terminal.mechanical,
    wrapped.mechanical[boundary = y_upper];
  connect conserving y_lower_terminal.mechanical,
    wrapped.mechanical[boundary = y_lower];
  connect conserving x_upper_terminal.mechanical,
    wrapped.mechanical[boundary = x_upper];
  connect conserving x_lower_terminal.mechanical,
    wrapped.mechanical[boundary = x_lower];
}
"#;

const CANONICAL_BOUNDARIES: [&str; 4] = [
    "axis=0,side=lower",
    "axis=0,side=upper",
    "axis=1,side=lower",
    "axis=1,side=upper",
];

fn compile_one(file: &str, source: &str) -> CompiledModel {
    let mut compiled = compile(file, source).expect("nested complete exterior compiles");
    assert_eq!(compiled.len(), 1, "fixture has exactly one root Model");
    compiled.pop().expect("one compiled Model")
}

fn count_symbols<I: Entity>(symbols: &ModelSymbols) -> usize {
    symbols
        .iter()
        .filter(|(_, id)| id.downcast::<I>().is_some())
        .count()
}

pub(super) fn admit(compiled: CompiledModel) -> KernelProgram {
    let model = compiled.model();
    let (transaction, _, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("atomic nested-forwarding admission");
    KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect("accepted nested-forwarding Kernel program")
}

pub(super) fn canonical_program(compiled: CompiledModel) -> Vec<u8> {
    ModelEnvelope::from_program(&admit(compiled))
        .expect("canonical model envelope")
        .canonical_json()
        .expect("canonical model bytes")
}

fn source_text<'a>(source: &'a str, span: &Span) -> &'a str {
    let start = usize::try_from(span.start).expect("source start fits usize");
    let end = usize::try_from(span.end).expect("source end fits usize");
    source
        .get(start..end)
        .expect("compiler provenance retains valid UTF-8 source boundaries")
}

#[test]
fn complete_exterior_forwards_through_wrapper_pointwise_to_exact_root_selectors() {
    let compiled = compile_one("forwarded.eqi", FORWARDED);

    assert_eq!(count_symbols::<kinds::Port>(compiled.symbols()), 8);
    assert_eq!(count_symbols::<kinds::Relation>(compiled.symbols()), 8);
    assert_eq!(compiled.physical_exposures().len(), 4);

    let provenance = compiled
        .provenance()
        .expect("hierarchy elaboration emits provenance");

    for boundary in CANONICAL_BOUNDARIES {
        let child_name = format!("wrapped.child.mechanical[{boundary}]");
        let wrapper_name = format!("wrapped.mechanical[{boundary}]");
        let child = compiled
            .symbols()
            .get(&child_name)
            .unwrap_or_else(|| panic!("retained exact child Port `{child_name}`"));
        assert_eq!(
            compiled.symbols().get(&wrapper_name),
            None,
            "the wrapper Port is a projection, not an alias entity"
        );

        let projection = compiled
            .physical_exposures()
            .get(&wrapper_name)
            .unwrap_or_else(|| panic!("exact wrapper projection `{wrapper_name}`"));
        assert_eq!(projection.interior().len(), 1);
        assert_eq!(projection.interior()[0].id().erase(), child);

        let child_provenance = provenance
            .get_by_graph_id(child)
            .unwrap_or_else(|| panic!("retained exact child provenance `{child_name}`"));
        assert!(child_provenance.origins().iter().any(|origin| {
            source_text(FORWARDED, origin.definition_span())
                .contains("public port mechanical[boundary in exterior]")
                && source_text(FORWARDED, origin.instance_span()).contains("instance child")
                && origin.binding_spans().iter().any(|span| {
                    source_text(FORWARDED, span).contains("support exterior = exterior")
                })
                && origin.binding_spans().iter().any(|span| {
                    source_text(FORWARDED, span).contains("support exterior = boundaries")
                })
                && origin
                    .binding_spans()
                    .iter()
                    .any(|span| source_text(FORWARDED, span).contains(boundary_name(boundary)))
        }));

        let projection_provenance = provenance
            .get(projection.exposure())
            .unwrap_or_else(|| panic!("eliminated exact wrapper provenance `{wrapper_name}`"));
        assert!(projection_provenance.origins().iter().any(|origin| {
            source_text(FORWARDED, origin.definition_span())
                .contains("public port mechanical[boundary in exterior]")
                && source_text(FORWARDED, origin.instance_span()).contains("instance wrapped")
                && origin.binding_spans().iter().any(|span| {
                    source_text(FORWARDED, span).contains("support exterior = boundaries")
                })
        }));
    }

    let projected_connections = compiled
        .physical_exposures()
        .iter()
        .map(|projection| projection.connection().id().erase())
        .collect::<BTreeSet<RawId>>();
    assert_eq!(projected_connections.len(), 4);

    let family_relations = compiled
        .symbols()
        .iter()
        .filter(|(name, id)| {
            name.starts_with("wrapped.child.boundary_law[")
                && id.downcast::<kinds::Relation>().is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(family_relations.len(), 4);
    for (_, relation) in family_relations {
        let relation_provenance = provenance
            .get_by_graph_id(relation)
            .expect("generated family Relation provenance");
        assert!(relation_provenance.origins().iter().any(|origin| {
            source_text(FORWARDED, origin.definition_span())
                .contains("relation boundary_law[boundary in exterior]")
                && origin.binding_spans().iter().any(|span| {
                    source_text(FORWARDED, span).contains("support exterior = exterior")
                })
                && origin.binding_spans().iter().any(|span| {
                    source_text(FORWARDED, span).contains("support exterior = boundaries")
                })
        }));
    }

    let mut family_activations = 0;
    let mut pointwise_connections = 0;
    for operation in compiled.transaction().ops() {
        let Op::DefineKernelNode { node } = operation else {
            continue;
        };
        match node {
            KernelNode::Activation(activation) => {
                let Some(activation_provenance) =
                    provenance.get_by_graph_id(activation.id().erase())
                else {
                    continue;
                };
                if activation_provenance.origins().iter().any(|origin| {
                    source_text(FORWARDED, origin.definition_span())
                        .contains("relation boundary_law[boundary in exterior]")
                }) {
                    family_activations += 1;
                }
            }
            KernelNode::Connection(connection) => {
                let connection_provenance = provenance
                    .get_by_graph_id(connection.id().erase())
                    .expect("pointwise Connection provenance");
                let has_family_origin = connection_provenance.origins().iter().any(|origin| {
                    source_text(FORWARDED, origin.definition_span())
                        .contains("connect conserving [boundary in exterior]")
                });
                let has_root_origin = connection_provenance.origins().iter().any(|origin| {
                    source_text(FORWARDED, origin.definition_span())
                        .contains("connect conserving wrapped.mechanical")
                });
                if has_family_origin && has_root_origin {
                    pointwise_connections += 1;
                }
            }
            _ => {}
        }
    }
    assert_eq!(family_activations, 4);
    assert_eq!(pointwise_connections, 4);

    let program = admit(compiled);
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Connection(_)))
            .count(),
        4
    );
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Activation(_)))
            .count(),
        8
    );
}

fn boundary_name(canonical_boundary: &str) -> &'static str {
    match canonical_boundary {
        "axis=0,side=lower" => "x_lower",
        "axis=0,side=upper" => "x_upper",
        "axis=1,side=lower" => "y_lower",
        "axis=1,side=upper" => "y_upper",
        _ => unreachable!("closed canonical boundary fixture"),
    }
}

#[test]
fn forwarding_and_family_permutations_preserve_exact_canonical_order() {
    let first = compile_one("forwarded.eqi", FORWARDED);
    let second = compile_one("relocated/permuted.eqi", FORWARDED_PERMUTED);

    assert_eq!(first.model(), second.model());
    assert_eq!(first.symbols(), second.symbols());
    assert_eq!(first.transaction().label(), second.transaction().label());
    assert_eq!(first.transaction().ops(), second.transaction().ops());
    assert_eq!(
        first.transaction().preconditions(),
        second.transaction().preconditions()
    );
    assert_eq!(first.physical_exposures(), second.physical_exposures());
    assert_eq!(canonical_program(first), canonical_program(second));
}
