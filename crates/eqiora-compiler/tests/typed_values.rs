use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

fn reject(source: &str, message: &str) {
    eqiora_lang::parse("typed.eqi", source)
        .into_document()
        .unwrap();
    let errors = compile("typed.eqi", source).unwrap_err();
    assert!(
        errors.iter().any(|error| error.message().contains(message)),
        "{errors:?}"
    );
    assert!(errors.iter().all(|error| error.source_span().is_some()));
}

#[test]
fn complete_parameter_values_keep_imaginary_parts_and_channel_order() {
    let models = compile("typed.eqi", "model M() { parameter z:complex<V>=math.complex(2,3); parameter channels:array<complex<V>,2>=[math.complex(1,4),math.complex(2,-3)]; relation r { z=channels[1]; } }").unwrap();
    let values = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(value),
            } => Some(
                value
                    .value()
                    .components()
                    .expect("real or complex fixture components")
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(values.contains(&vec![(2.0, 3.0)]));
    assert!(values.contains(&vec![(1.0, 4.0), (2.0, -3.0)]));
    assert!(models[0].transaction().ops().iter().any(|op| matches!(op,Op::DefineKernelNode{node:KernelNode::Relation(value)} if value.expression().nodes().iter().any(|node| matches!(node, ExprNode::Index{index:1,..})))));
}

#[test]
fn runtime_constructors_keep_original_parameter_dependencies() {
    let models=compile("typed.eqi", "model M() { parameter p:1=2; variable x:complex<1>; let data=[math.complex(p,1),math.complex(3,p)]; let result=data[0]+data[1]; relation r { x=result; } }").unwrap();
    let nodes = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Relation(value),
            } => Some(value.expression().nodes()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert!(
        nodes
            .iter()
            .any(|node| matches!(node, ExprNode::Array { .. }))
    );
    assert!(
        nodes
            .iter()
            .any(|node| matches!(node, ExprNode::Complex { .. }))
    );
    assert!(nodes.iter().any(|node| matches!(
        node,
        ExprNode::Symbol(eqiora_schema::kernel::SymbolRef::Parameter(_))
    )));
}

#[test]
fn indexes_are_constant_channels_and_do_not_narrow_complex_values() {
    reject(
        "model M() { parameter p:array<1,2>=[1,2]; relation r { p[2]=0; } }",
        "index",
    );
    reject(
        "model M() { parameter p:array<1,2>=[1,2]; parameter index:1=0; relation r { p[index]=0; } }",
        "index",
    );
    reject(
        "model M() { parameter p:array<1,2>=[1,2]; let n:integer=-1; relation r { p[n]=0; } }",
        "index",
    );
    reject(
        "model M() { parameter p:1=math.complex(1,2); relation r { p=0; } }",
        "real",
    );
    reject(
        "model M() { parameter p:array<V,2>=[1+2,3]; relation r { p=0; } }",
        "dimension",
    );
}

#[test]
fn static_channel_operations_and_constant_indexes_share_component_resolution() {
    let source = "component C(parameter values:array<complex<V>,2>=[math.complex(2,3),math.complex(4,-1)]) {  let chosen=values[which]/2; let which:integer=1-1; variable out:complex<V>; relation r { out=chosen; } } model M() { instance c: C(); }";
    compile("typed.eqi", source).unwrap();
    reject(
        "component C(parameter values:array<1,2>) {  let choice=to_integer(0*values[0]); let selected=values[choice]; } model M() { variable x:1; relation r { x=0; } }",
        "index",
    );
    reject(
        "model M() { parameter ragged:array<array<1,2>,2>=[[1,2],[3]]; relation r { ragged=0; } }",
        "shape",
    );
}

#[test]
fn typed_properties_preserve_normalized_complex_channels_and_nominal_contracts() {
    use eqiora_compiler::{
        CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit,
        analyze_resolved_hierarchy,
    };
    let source = "property contract Coefficients(): array<complex<V>,2> { derivatives value_only; } property release Reference implements Coefficients { value=[math.complex(2,3),math.complex(4,-1)]; source_unit: V=2; validity=unconditional; citation=org.example.reference; license=org.example.license; } component C(property data:Coefficients) {  variable x:complex<V>; relation r { x=data[0]; } } model M() { instance c:C(data =Reference); }";
    let root = CompilationNamespaceId::new(["root", "1", "typed-property"]).unwrap();
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![ResolvedSourceUnit::new(root, "src/main.eqi", source).unwrap()],
        vec![],
    );
    let analyzed = analyze_resolved_hierarchy(input).unwrap();
    assert_eq!(
        analyzed
            .property_bindings()
            .next()
            .unwrap()
            .5
            .components()
            .expect("real or complex fixture components")
            .collect::<Vec<_>>(),
        vec![(4.0, 6.0), (8.0, -2.0)]
    );
    analyzed
        .validate_definitions()
        .unwrap()
        .compile_root("M")
        .unwrap();
}

#[test]
fn repeated_array_aliases_cannot_expand_past_the_existing_source_type_budget() {
    let mut source = String::from("model M() { let a0=[1,2];");
    for index in 1..18 {
        source.push_str(&format!("let a{index}=[a{},a{}];", index - 1, index - 1));
    }
    source.push_str("variable x:1; relation r { x=0; } }");
    reject(&source, "65536");
}

#[test]
fn nested_initializer_zero_inherits_only_the_declared_element_shape() {
    let models=compile("typed.eqi", "model M() { parameter data:array<array<complex<V>,2>,2>=[0,[math.complex(1,2),3]]; relation r { data=0; } }").unwrap();
    let value = models[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(value),
            } => Some(value.value()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        value
            .components()
            .expect("real or complex fixture components")
            .collect::<Vec<_>>(),
        vec![(0.0, 0.0), (0.0, 0.0), (1.0, 2.0), (3.0, 0.0)]
    );
    reject(
        "model M() { parameter data:array<array<1,2>,2>=[1,[2,3]]; relation r { data=0; } }",
        "shaped",
    );
    reject(
        "model M() { let data=[0,[1,2]]; variable x:1; relation r { x=0; } }",
        "types",
    );
}
