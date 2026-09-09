//! Authored formal partials use the common typed scalar evaluator.
use eqiora::compiler::compile;
use eqiora::graph::Op;
use eqiora::ir::ScalarOperatorIr;
use eqiora::kernel::{KernelNode, SymbolRef};

fn residuals(source: &str) -> Vec<f64> {
    let models = compile("partial.eqi", source).unwrap();
    let operations = models[0].transaction().ops();
    let parameters = operations
        .iter()
        .filter_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(parameter),
            } => Some((parameter.id(), parameter.value().clone())),
            _ => None,
        })
        .collect::<std::collections::HashMap<_, _>>();
    operations
        .iter()
        .filter_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation.expression()),
            _ => None,
        })
        .flat_map(|dag| {
            ScalarOperatorIr::lower(dag)
                .unwrap()
                .evaluate_typed(dag.roots(), &mut |symbol| match symbol {
                    SymbolRef::Parameter(id) => parameters.get(&id).cloned(),
                    _ => None,
                })
                .unwrap()
                .into_iter()
                .map(|value| value.real_scalar_value().unwrap().value())
        })
        .collect::<Vec<_>>()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| pair[0] - pair[1])
        .collect()
}

#[test]
fn authored_two_input_partials_and_composition_have_independent_analytic_values() {
    let source = include_str!("../../../verify/language/formal-partials/models/composition.eqi");
    assert_eq!(residuals(source), vec![0.; 3]);
}

#[test]
fn parameter_alias_does_not_cut_dependency_or_merge_equal_independent_inputs() {
    let source = include_str!("../../../verify/language/formal-partials/models/aliases.eqi");
    assert_eq!(residuals(source), vec![0.; 3]);
}

#[test]
fn conductivity_partial_has_conductivity_per_temperature_dimension() {
    let source = include_str!("../../../verify/language/formal-partials/models/conductivity.eqi");
    for value in residuals(source) {
        assert!(value.abs() < 1e-15);
    }
}

#[test]
fn aliases_foreign_bindings_conflicting_holding_and_implicit_solution_partials_fail() {
    for expression in [
        "partial(x*x,wrt=z)",
        "partial(x,wrt=foreign)",
        "partial(x,wrt=x,holding=(x))",
        "partial(x,wrt=x,holding=(y,y))",
        "partial(x,wrt=x,holding=(z))",
        "partial(x,wrt=v)",
        "partial(derivative(v),wrt=x)",
        "partial(math.sin(x),wrt=x)",
    ] {
        let source = format!(
            "model M() {{ parameter x:1=3; parameter y:1=3; let z=x; variable v:1; relation r {{ {expression}=0; }} }}"
        );
        assert!(compile("invalid.eqi", &source).is_err(), "{expression}");
    }
}

#[test]
fn sequential_partial_selectors_retain_distinct_live_values() {
    let source = r#"model M() {
 parameter x:1=3; parameter y:1=5;
 relation r {
   partial(x*x,wrt=x)=6;
   partial(x*y*y,wrt=y)=30;
   partial(x*x*y,wrt=x)=30;
 }
}"#;
    assert_eq!(residuals(source), vec![0.; 3]);
}

#[test]
fn component_binding_preserves_independent_parent_parameter_directions() {
    let source = r#"
component C(parameter x:1, parameter y:1) {
 relation r {
   partial(x*x*y,wrt=x,holding=(y))=2*x*y;
   partial(x*x*y,wrt=y,holding=(x))=x*x;
 }
}
model M() {
 parameter p:1=3; parameter q:1=3;
 instance forward:C(x=p,y=q);
 instance reverse:C(x=q,y=p);
}
"#;
    assert_eq!(residuals(source), vec![0.; 4]);
    // Unequal values also expose accidental reuse across reversed bindings.
    assert_eq!(residuals(&source.replace("q:1=3", "q:1=5")), vec![0.; 4]);
}

#[test]
fn component_bindings_cannot_manufacture_independence_or_conflicting_holding() {
    for bindings in ["x=p,y=p", "x=3,y=q", "x=p+q,y=q"] {
        let source = format!(
            "component C(parameter x:1, parameter y:1) {{ relation r {{ partial(x*x*y,wrt=x,holding=(y))=0; }} }} model M() {{ parameter p:1=3; parameter q:1=3; instance c:C({bindings}); }}"
        );
        assert!(compile("dependent.eqi", &source).is_err(), "{bindings}");
    }
}

#[test]
fn shared_alias_partials_preserve_the_bounded_source_dag() {
    let source = |squarings: usize, expected: u32| {
        let mut source = String::from("model M() { parameter p:1=1; let a0=p;");
        for index in 1..=squarings {
            source.push_str(&format!("let a{index}=a{}*a{};", index - 1, index - 1));
        }
        source.push_str(&format!(
            "relation r {{ partial(a{squarings},wrt=p)={expected}; }} }}"
        ));
        source
    };
    // a7=p^128, hence a7_p=128 at p=1. Every operation is exact here.
    assert_eq!(residuals(&source(7, 128)), vec![0.0]);
    // a24=p^(2^24) exceeds the existing formal-exponent bound of 255.
    // Shared aliases must reach that declared gate without expanding a tree.
    let failure = compile("partial-bound.eqi", &source(24, 16777216)).unwrap_err();
    assert!(
        failure.iter().any(|diagnostic| diagnostic
            .message()
            .contains("formal exponent exceeds its limit")),
        "{failure:?}"
    );
}
