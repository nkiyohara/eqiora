use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

fn source(initial: &str, next: &str, extra: &str) -> String {
    format!(
        "model M() {{ clock tick=periodic(1[s]); {extra} state memory:array<integer,2> at tick; initial {{memory={initial};}} relation update at tick {{next(memory)={next};}} }}"
    )
}

#[test]
fn integer_array_literals_keep_exact_components_in_initial_and_update_equations() {
    let models = compile(
        "integer-array.eqi",
        &source(
            "[9007199254740993,-9223372036854775808]",
            "pre(memory)+[1,2]",
            "",
        ),
    )
    .unwrap();
    let integers = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation.expression().nodes()),
            _ => None,
        })
        .flatten()
        .filter_map(|node| match node {
            ExprNode::Constant(value) => value.integer_scalar_value(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(integers.contains(&9007199254740993));
    assert!(integers.contains(&i64::MIN));
    assert!(integers.contains(&1));
    assert!(integers.contains(&2));
}

#[test]
fn integer_array_context_rejects_fraction_overflow_wrong_shape_and_real_parameters() {
    for (initial, next, extra) in [
        ("[1.5,2]", "pre(memory)", ""),
        ("[9223372036854775808,2]", "pre(memory)", ""),
        ("[1,2,3]", "pre(memory)", ""),
        ("[1,2]", "pre(memory)+[real,1]", "parameter real:1=2;"),
        ("[real,2]", "pre(memory)", "parameter real:1=2;"),
        ("[1,2]", "pre(memory)+1", ""),
    ] {
        let errors = compile("integer-array.eqi", &source(initial, next, extra)).unwrap_err();
        assert!(
            errors.iter().all(|error| error.source_span().is_some()),
            "{errors:?}"
        );
    }
}

#[test]
fn nested_integer_arrays_retain_each_axis() {
    compile("nested.eqi", "model M(){clock tick=periodic(1[s]);state memory:array<array<integer,2>,2> at tick;initial{memory=[[9007199254740993,2],[3,4]];}relation r at tick{next(memory)=pre(memory)+[[1,2],[3,4]];}}").unwrap();
}

#[test]
fn indexed_integer_state_operands_keep_literal_context_inside_array_updates() {
    compile("indexed-array.eqi", "model M(input drive:array<integer,2> at tick){clock tick=periodic(1[s]);state a:array<integer,2> at tick;state b:array<integer,2> at tick;initial{pre(a)=[9007199254740993,2];pre(b)=[10,20];}relation update at tick{next(a)=[pre(a)[0]+drive[0],pre(b)[1]];next(b)=[pre(a)[1],pre(b)[0]+drive[1]+1];}}").unwrap();
}

#[test]
fn integer_parameter_anchors_mixed_literal_arrays_without_coercing_real_parameters() {
    compile(
        "mixed-array.eqi",
        &source(
            "[seed,2]",
            "[pre(memory)[0],3]",
            "parameter seed:integer=9007199254740993;",
        ),
    )
    .unwrap();
    compile("nested-mixed.eqi", "model M(){parameter seed:integer=9007199254740993;clock tick=periodic(1[s]);state a:array<array<integer,2>,2> at tick;initial{a=[[seed,2],[3,4]];}relation r at tick{next(a)=pre(a);}}").unwrap();
}
