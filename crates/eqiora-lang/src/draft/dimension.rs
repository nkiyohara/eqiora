use super::*;

pub(super) fn dimension_expression(
    dimension: DimExponents,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> Expr {
    crate::factory::value_literal::dimension_expression(dimension, || ranges.allocate(path, paths))
}
