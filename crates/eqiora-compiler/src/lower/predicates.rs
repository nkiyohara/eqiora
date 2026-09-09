//! Authored comparison projection into the shared Kernel vocabulary.
use super::*;

/// Map authored comparison syntax to the one Kernel predicate vocabulary.
pub(crate) fn comparison_operator(
    operator: BinaryOp,
) -> Option<eqiora_schema::kernel::ComparisonOp> {
    use eqiora_schema::kernel::ComparisonOp;
    Some(match operator {
        BinaryOp::Equal => ComparisonOp::Equal,
        BinaryOp::NotEqual => ComparisonOp::NotEqual,
        BinaryOp::Less => ComparisonOp::Less,
        BinaryOp::LessEqual => ComparisonOp::LessEqual,
        BinaryOp::Greater => ComparisonOp::Greater,
        BinaryOp::GreaterEqual => ComparisonOp::GreaterEqual,
        _ => return None,
    })
}
