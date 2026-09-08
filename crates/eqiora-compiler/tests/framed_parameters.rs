use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::KernelNode;

#[test]
fn framed_vector_retains_order_without_hidden_fields() {
    let models = compile("frame.eqi", "model M() { parameter force:vector<V,2>=tensor_value(frame=body,components=[2,3]); domain body=box(0,1,0,1); relation r { force=force; } }").unwrap();
    let values = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(value),
            } => Some(value.value()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].components().unwrap().collect::<Vec<_>>(),
        vec![(2.0, 0.0), (3.0, 0.0)]
    );
    assert!(!models[0].transaction().ops().iter().any(|op| matches!(
        op,
        Op::DefineKernelNode {
            node: KernelNode::Field(_)
        }
    )));
}

#[test]
fn explicit_frames_disambiguate_matrix_and_channel_values() {
    let source = "model M() { domain a=box(0,1,0,1);domain b=box(0,2,0,2); parameter matrix:tensor<complex<Pa>,2,2>=tensor_value(frame=b,components=[[math.complex(2,11),math.complex(3,13)],[math.complex(5,17),math.complex(7,19)]]); parameter channels:array<vector<V,2>,2>=[tensor_value(frame=a,components=[2,3]),tensor_value(frame=a,components=[5,7])]; relation r { matrix=matrix;channels=channels; } }";
    let models = compile("frames.eqi", source).unwrap();
    let values = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(value),
            } => Some(value.value().components().unwrap().collect::<Vec<_>>()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(values.contains(&vec![(2., 11.), (3., 13.), (5., 17.), (7., 19.)]));
    assert!(values.contains(&vec![(2., 0.), (3., 0.), (5., 0.), (7., 0.)]));
}

#[test]
fn missing_ambiguous_and_foreign_frames_reject() {
    for (domains, initializer, message) in [
        ("", "0", "frame support"),
        (
            "domain a=box(0,1,0,1);domain b=box(0,1,0,1);",
            "0",
            "ambiguous",
        ),
        (
            "domain a=box(0,1,0,1);",
            "tensor_value(frame=foreign,components=[2,3])",
            "existing Cartesian support",
        ),
    ] {
        let source = format!(
            "model M() {{ {domains} parameter p:vector<1,2>={initializer};relation r{{p=p;}} }}"
        );
        let errors = compile("frames.eqi", &source).unwrap_err();
        assert!(
            errors.iter().any(|e| e.message().contains(message)),
            "{errors:?}"
        );
    }
}

#[test]
fn component_frames_rebind_and_preserve_uniform_parameter_dependencies() {
    let source = "component C(support body:volume(ambient_dimension=2),parameter p:vector<V,2>=tensor_value(frame=body,components=[2,3])) { relation r { p=p; } } model M() { domain a=box(0,1,0,1);domain b=box(0,2,0,2);instance first:C(body=a);instance second:C(body=b,p=tensor_value(frame=b,components=[5,7])); }";
    let models = compile("occurrences.eqi", source).unwrap();
    let values = models[0]
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
        .filter_map(|node| match node {
            eqiora_schema::kernel::ExprNode::Constant(value) => {
                Some(value.components().unwrap().collect::<Vec<_>>())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(values.contains(&vec![(2., 0.), (3., 0.)]), "{values:?}");
    assert!(values.contains(&vec![(5., 0.), (7., 0.)]), "{values:?}");
}

#[test]
fn constructor_never_freezes_mutable_parameter_components() {
    let errors=compile("mutable.eqi","model M() {domain body=box(0,1,0,1);parameter x:1=2;parameter p:vector<1,2>=tensor_value(frame=body,components=[x,0]);relation r{p=p;} }").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.message().contains("named value references")),
        "{errors:?}"
    );
}
