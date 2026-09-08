//! Lazy conditional value expressions using the shared bounded expression parser.
use super::*;

impl Parser<'_> {
    pub(super) fn parse_select(&mut self) -> Option<(Expr, usize)> {
        let start = self.expect_keyword("if")?.range().start();
        let (condition, condition_depth) = self.parse_expression_with_depth(0)?;
        self.expect_keyword("then")?;
        let (then_value, then_depth) = self.parse_expression_with_depth(0)?;
        self.expect_keyword("else")?;
        let (else_value, else_depth) = self.parse_expression_with_depth(0)?;
        let depth = self.parent_depth(condition_depth.max(then_depth).max(else_depth))?;
        let range = TextRange::new(start, else_value.range().end());
        Some((
            Expr {
                resolved_enum: None,
                resolved_nominal: None,
                kind: ExprKind::Select {
                    condition: Box::new(condition),
                    then_value: Box::new(then_value),
                    else_value: Box::new(else_value),
                },
                range,
            },
            depth,
        ))
    }
}
