use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};

#[test]
fn occurrence_aliases_feed_nested_bindings_without_new_parameters() {
    let source = r#"
component Leaf(parameter gain: 1) {  relation law { gain = 1; } }
component Parent(parameter p: 1) {

  let f = z * p;
  let z = p * p;
  instance leaf: Leaf(gain = f);
}
model M() {
  parameter a: 1 = 2;
  parameter b: 1 = 3;
  instance first: Parent(p = a);
  instance second: Parent(p = b);
}
"#;
    let compiled = compile("component-let.eqi", source).unwrap();
    let model = &compiled[0];
    let ids = [
        model.symbols().get("a").unwrap(),
        model.symbols().get("b").unwrap(),
    ];
    assert!(
        model
            .symbols()
            .iter()
            .all(|(name, _)| !name.ends_with(".f") && !name.ends_with(".z"))
    );
    let nodes = model
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode { node } => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        nodes
            .iter()
            .filter(|node| matches!(node, KernelNode::Parameter(_)))
            .count(),
        2
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| matches!(node, KernelNode::Field(_)))
            .count(),
        0
    );
    let relations = nodes
        .iter()
        .filter_map(|node| match node {
            KernelNode::Relation(r) => Some(r),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(relations.len(), 2);
    let mut dependencies = relations
        .iter()
        .map(|relation| {
            let mut references = relation
                .residuals()
                .nodes()
                .iter()
                .filter_map(|node| match node {
                    ExprNode::Symbol(SymbolRef::Parameter(id)) => Some(id.erase()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            references.sort();
            references.dedup();
            assert_eq!(references.len(), 1);
            references[0]
        })
        .collect::<Vec<_>>();
    dependencies.sort();
    let mut expected = ids.to_vec();
    expected.sort();
    assert_eq!(dependencies, expected);
}

#[test]
fn unused_components_still_reject_invalid_static_aliases() {
    for (signature, body) in [
        ("", "let a = b; let b = a;"),
        ("", "let a = a;"),
        ("", "let a = missing;"),
        ("", "let a = derivative(x);"),
        ("", "let a: m = 1[s];"),
        ("parameter p: 1", "let p = 2;"),
        ("", "let a = 1; let a = 2;"),
        ("parameter p: 1 = a", "let a = 1;"),
        ("", "let a = other.p;"),
    ] {
        let source = format!("component Unused({signature}) {{ {body} }} model M() {{}}");
        assert!(compile("bad.eqi", &source).is_err(), "{body}");
    }
}

#[test]
fn alias_names_are_neither_binding_targets_nor_parent_captures() {
    for source in [
        "component C() { let a = 1; } model M() { instance c: C(a = 2); }",
        "component C() { let a = p; } model M() { parameter p: 1 = 2; instance c: C(); }",
    ] {
        assert!(compile("scope.eqi", source).is_err(), "{source}");
    }
}

#[test]
fn component_alias_assertions_use_common_dimension_resolution() {
    let source = "dimension Length = m; component C() { let distance: Length = 2; relation law { distance = 2[m]; } } model M() { instance c: C(); }";
    compile("dimension.eqi", source).unwrap();
}
