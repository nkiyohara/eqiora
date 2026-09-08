//! Reachability of nonsmooth ordered selection in the shared expression DAG.
use eqiora_schema::kernel::{ExprDag, ExprNode};

pub(crate) fn contains(dag: &ExprDag) -> bool {
    let mut selected = Vec::with_capacity(dag.nodes().len());
    for node in dag.nodes() {
        let at = |id: &eqiora_schema::kernel::ExprId| selected[id.index() as usize];
        let value = matches!(node, ExprNode::Min(_, _) | ExprNode::Max(_, _))
            || crate::program::signal_activation::operands(node)
                .iter()
                .any(at);
        selected.push(value);
    }
    dag.roots()
        .iter()
        .any(|root| selected[root.index() as usize])
}
