//! A bounded family expands into fixed, independent ordinary occurrences.

use eqiora::api::ModelDocument;
use eqiora::kernel::{FieldRole, KernelNode, RationalTime};
use eqiora::sem::{Interpreter, ReferenceConfig};

const CELL: &str = r#"
component Cell(parameter value: integer, clock tick: periodic, output y: integer at tick) {
  state memory: integer at tick;
  initial { memory = value; }
  relation update at tick {
    next(memory) = pre(memory) + 1;
    y = next(memory);
  }
}
"#;

fn source(instances: &str, first: &str, second: &str) -> String {
    format!(
        r#"{CELL}
model Pair(output first: integer at tick, output second: integer at tick) {{
  clock tick = periodic(1[s]);
  parameter n: integer = 2;
  indexset Stages = range(n);
  {instances}
  relation expose at tick {{ first = {first}; second = {second}; }}
}}"#
    )
}

#[test]
fn indexed_and_explicit_cells_follow_the_same_equations_without_resizing() {
    let indexed = source(
        "instance cell[i in Stages]: Cell(value = ordinal(i), tick = tick);",
        "cell[index(Stages, 0)].y",
        "cell[index(Stages, 1)].y",
    );
    let explicit = source(
        "instance cell0: Cell(value = 0, tick = tick); instance cell1: Cell(value = 1, tick = tick);",
        "cell0.y",
        "cell1.y",
    );
    for source in [indexed, explicit] {
        let document = ModelDocument::compile("bounded-family.eqi", &source).unwrap();
        let aliases = document.aliases().clone();
        let digest = document.digest().unwrap();
        let state_ids = document
            .program()
            .nodes()
            .filter_map(|node| {
                if let KernelNode::Field(field) = node {
                    (field.role() == FieldRole::State).then_some(field.id())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(state_ids.len(), 2);
        assert_ne!(state_ids[0], state_ids[1]);
        let outputs = [aliases["first"], aliases["second"]];
        let interpreter = Interpreter::new();
        let config = ReferenceConfig::new(2.0, 1.0).unwrap();
        let mut session = interpreter
            .execution_session(document.program(), config, [])
            .unwrap();
        assert_eq!(session.advance_ticks(1).unwrap(), 1);
        let mut resumed = interpreter
            .resume_execution(document.program(), &session.checkpoint())
            .unwrap();
        assert_eq!(resumed.advance_ticks(2).unwrap(), 2);
        // The two equations are m_i(k+1)=m_i(k)+1 with m_0(0)=0,
        // m_1(0)=1. Phase-zero publication uses the newly accepted values.
        for (tick, expected) in [[1_i64, 2], [2, 3], [3, 4]].into_iter().enumerate() {
            for (output, expected) in outputs.into_iter().zip(expected) {
                let (time, value) = resumed.output(output, tick as u64).unwrap();
                assert_eq!(time, RationalTime::new(tick as u64, 1).unwrap());
                assert_eq!(value.integer_scalar_value(), Some(expected));
            }
        }
        assert_eq!(document.aliases(), &aliases);
        assert_eq!(document.digest().unwrap(), digest);
        assert_eq!(
            document
                .program()
                .nodes()
                .filter(|node| matches!(node, KernelNode::Field(_)))
                .count(),
            2
        );
    }
}

#[test]
fn equal_extent_does_not_make_foreign_index_sets_interchangeable() {
    let valid = source(
        "indexset Other = range(2); instance cell[i in Stages]: Cell(value = ordinal(i), tick = tick);",
        "cell[index(Stages, 0)].y",
        "cell[index(Stages, 1)].y",
    );
    ModelDocument::compile("index-owner.eqi", &valid).unwrap();
    for invalid in [
        valid.replace("cell[index(Stages, 0)]", "cell[index(Other, 0)]"),
        valid.replace("cell[index(Stages, 0)]", "cell[index(Stages, -1)]"),
        valid.replace("cell[index(Stages, 0)]", "cell[index(Stages, 2)]"),
    ] {
        let diagnostics = ModelDocument::compile("index-owner.eqi", &invalid).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_span().is_some()),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn changing_a_structural_extent_requires_recompilation_even_after_replay() {
    use eqiora::compiler::compile;
    use eqiora::graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
    use eqiora::sem::KernelProgram;
    use eqiora::{DimExponents, ScalarDomain, ValueLiteral, ValueType};

    let source = source(
        "instance cell[i in Stages]: Cell(value = ordinal(i), tick = tick);",
        "cell[index(Stages, 0)].y",
        "cell[index(Stages, 1)].y",
    );
    let replacement = ValueLiteral::from_integer(
        ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS)
            .expect("valid scalar type"),
        3,
    )
    .unwrap();
    let document = ModelDocument::compile("fixed-extent.eqi", &source).unwrap();
    let replay = ModelDocument::replay(&document.canonical_json().unwrap()).unwrap();
    let extent = document.aliases()["n"];
    for document in [&document, &replay] {
        let digest = document.digest().unwrap();
        assert!(
            document
                .preview_value_edit(extent, replacement.clone())
                .is_err()
        );
        assert_eq!(document.digest().unwrap(), digest);
    }
    let compiled = compile("fixed-extent.eqi", &source).unwrap().pop().unwrap();
    let target = compiled.symbols().get("n").unwrap();
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let before = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let mut update = Transaction::new("attempt to resize already compiled occurrences");
    update.push(Op::SetValue {
        target,
        value: replacement,
    });
    assert!(store.commit(update).is_err());
    let after = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    assert_eq!(before.revision(), after.revision());
    assert_eq!(
        before.nodes().collect::<Vec<_>>(),
        after.nodes().collect::<Vec<_>>()
    );
    assert_eq!(before.typed_value(target), after.typed_value(target));
}
