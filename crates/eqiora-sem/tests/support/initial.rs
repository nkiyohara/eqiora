use eqiora_core::entity::kinds;
use eqiora_core::{DynQuantity, Id};
use eqiora_graph::{EdgeKind, Op, Transaction};
use eqiora_schema::kernel::{ExprDagBuilder, ExprNode, KernelNode, RelationDef, SymbolRef};

pub fn initial(field: Id<kinds::Field>, value: DynQuantity) -> KernelNode {
    let mut dag = ExprDagBuilder::new();
    let field = dag.symbol(SymbolRef::Field(field)).unwrap();
    let value = dag.constant(value).unwrap();

    RelationDef::initial(Id::new(), dag.finish([field, value]).unwrap())
        .unwrap()
        .into()
}

pub fn define_all(transaction: &mut Transaction, nodes: impl IntoIterator<Item = KernelNode>) {
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    for node in &nodes {
        transaction.push(Op::DefineKernelNode { node: node.clone() });
    }
    for node in &nodes {
        if let KernelNode::Relation(relation) = node
            && relation.is_initial()
        {
            for expression in relation.expression().nodes() {
                if let ExprNode::Symbol(SymbolRef::Field(field)) = expression {
                    transaction.push(Op::Connect {
                        from: relation.id().erase(),
                        to: field.erase(),
                        edge: EdgeKind::DependsOn,
                    });
                }
            }
        }
    }
}
