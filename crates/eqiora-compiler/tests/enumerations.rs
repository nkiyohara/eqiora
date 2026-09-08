use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode};

#[test]
fn clocked_enum_state_uses_exact_declared_members_and_shared_selection() {
    let source = "enum Mode {Heating,Cooling,Fault} model M(){clock tick=periodic(1[s]);state mode:Mode at tick;initial{mode=Mode.Heating;}relation update at tick{next(mode)=case pre(mode){Mode.Heating=>Mode.Cooling,Mode.Cooling=>Mode.Fault,Mode.Fault=>Mode.Heating};}}";
    let models = compile("enum.eqi", source).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(
        models[0]
            .transaction()
            .ops()
            .iter()
            .filter(|op| matches!(
                op,
                Op::DefineKernelNode {
                    node: KernelNode::Enum(_)
                }
            ))
            .count(),
        1
    );
    assert!(models[0].transaction().ops().iter().any(|op|matches!(op,Op::DefineKernelNode{node:KernelNode::Relation(relation)} if relation.expression().nodes().iter().any(|node|matches!(node,ExprNode::Select{..})))));
}
