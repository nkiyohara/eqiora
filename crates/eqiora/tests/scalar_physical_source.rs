use eqiora::artifact::ModelEnvelope;
use eqiora::compiler::compile;
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore};
use eqiora::kernel::{DomainKind, ExprNode, KernelNode, PortPayload, SymbolRef};
use eqiora::sem::KernelProgram;

const SOURCE: &str = r#"
model physical_pair {
  domain electrical = scalar_physical(
    across = kg * m ^ 2 / (s ^ 3 * A),
    through = A
  );
  port left: conserving on electrical;
  port right: conserving on electrical;

  relation left_component continuous {
    across(left) = 0;
  }
  relation right_component continuous {
    through(right) = 0;
  }

  connect conserving left, right;
}
"#;

#[test]
fn source_constructs_one_nominal_current_physical_program() {
    let mut compiled = compile("physical_pair.eqi", SOURCE).expect("typed physical source");
    let compiled = compiled.pop().expect("one model");
    let domain = compiled.symbols().get("electrical").expect("Domain ID");
    let left = compiled.symbols().get("left").expect("left Port ID");
    let right = compiled.symbols().get("right").expect("right Port ID");
    let left_relation = compiled
        .symbols()
        .get("left_component")
        .expect("left Relation ID");
    let right_relation = compiled
        .symbols()
        .get("right_component")
        .expect("right Relation ID");
    let (transaction, model, _) = compiled.into_parts();

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("atomic source commit");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model)
        .expect("source satisfies the physical closure contract");

    let KernelNode::Domain(domain_definition) = program.node(domain).expect("Domain definition")
    else {
        panic!("source Domain has the canonical kind");
    };
    assert!(matches!(
        domain_definition.kind(),
        DomainKind::ScalarPhysical {
            across_dimension,
            through_dimension,
        } if across_dimension.exponents() == [(1, 1), (2, 1), (-3, 1), (-1, 1), (0, 1), (0, 1), (0, 1)]
            && through_dimension.exponents() == [(0, 1), (0, 1), (0, 1), (1, 1), (0, 1), (0, 1), (0, 1)]
    ));

    for port in [left, right] {
        let KernelNode::Port(definition) = program.node(port).expect("Port definition") else {
            panic!("source Port has the canonical kind");
        };
        assert_eq!(
            definition.payload(),
            PortPayload::ScalarPhysical {
                domain: domain.downcast().expect("typed Domain ID"),
            }
        );
    }

    let expected_symbols = [
        (
            left_relation,
            SymbolRef::Across(left.downcast().expect("typed Port ID")),
        ),
        (
            right_relation,
            SymbolRef::Through(right.downcast().expect("typed Port ID")),
        ),
    ];
    for (relation, expected_symbol) in expected_symbols {
        let KernelNode::Relation(definition) = program.node(relation).expect("Relation definition")
        else {
            panic!("source Relation has the canonical kind");
        };
        assert!(
            definition
                .residuals()
                .nodes()
                .iter()
                .any(|node| matches!(node, ExprNode::Symbol(symbol) if *symbol == expected_symbol))
        );
    }

    for (relation, port) in [(left_relation, left), (right_relation, right)] {
        for kind in [EdgeKind::DependsOn, EdgeKind::HasPort] {
            assert!(
                program.edges().iter().any(|edge| edge.from() == relation
                    && edge.to() == port
                    && edge.kind() == kind)
            );
        }
    }

    let envelope = ModelEnvelope::from_program(&program).expect("physical model needs v2");
    let restored = envelope.to_program().expect("v2 round trip validates");
    assert_eq!(restored.nodes().count(), program.nodes().count());
    assert_eq!(restored.edges(), program.edges());
}
