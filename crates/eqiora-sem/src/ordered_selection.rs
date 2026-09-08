//! Reachability of nonsmooth ordered selection in the shared expression DAG.
use eqiora_schema::kernel::{ExprDag, ExprNode};

pub(crate) fn contains(dag: &ExprDag) -> bool {
    let mut selected = Vec::with_capacity(dag.nodes().len());
    for node in dag.nodes() {
        let at = |id: &eqiora_schema::kernel::ExprId| selected[id.index() as usize];
        let value = match node {
            ExprNode::Min(_, _) | ExprNode::Max(_, _) => true,
            ExprNode::Array { elements } => elements.iter().any(at),
            ExprNode::Sample { value, .. }
            | ExprNode::Index { value, .. }
            | ExprNode::Ordinal(value)
            | ExprNode::Not(value)
            | ExprNode::ToReal(value)
            | ExprNode::ToInteger(value)
            | ExprNode::Hold(value)
            | ExprNode::Neg(value)
            | ExprNode::PowI(value, _)
            | ExprNode::UnaryMath(_, value)
            | ExprNode::Gradient(value)
            | ExprNode::Divergence(value)
            | ExprNode::SymmetricPart(value)
            | ExprNode::IsotropicLift(value)
            | ExprNode::Trace(value)
            | ExprNode::NormalComponent(value) => at(value),
            ExprNode::Complex { real: a, imag: b }
            | ExprNode::Compare(_, a, b)
            | ExprNode::And(a, b)
            | ExprNode::Or(a, b)
            | ExprNode::Add(a, b)
            | ExprNode::Sub(a, b)
            | ExprNode::Mul(a, b)
            | ExprNode::Div(a, b)
            | ExprNode::Quotient(a, b)
            | ExprNode::Remainder(a, b) => at(a) || at(b),
            ExprNode::PureOperatorApplication(application) => {
                application.arguments().iter().any(at)
            }
            _ => false,
        };
        selected.push(value);
    }
    dag.roots()
        .iter()
        .any(|root| selected[root.index() as usize])
}
