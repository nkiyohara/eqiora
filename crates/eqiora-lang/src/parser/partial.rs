//! Explicit partials use compile-time binding names, not mixed runtime calls.
use super::*;

impl Parser<'_> {
    pub(super) fn parse_partial(&mut self, path: NamePath) -> Option<(Expr, usize)> {
        self.expect(TokenKind::LeftParen, "`(` after partial")?;
        let (value, child_depth) = self.parse_expression_with_depth(0)?;
        self.expect(TokenKind::Comma, "`,` before partial wrt binding")?;
        self.expect_keyword("wrt")?;
        self.expect(TokenKind::Equal, "`=` after wrt")?;
        let wrt = self.parse_name_path("partial independent binding")?;
        let mut holding = Vec::new();
        if self.at(TokenKind::Comma) {
            self.bump();
            self.expect_keyword("holding")?;
            self.expect(TokenKind::Equal, "`=` after holding")?;
            self.expect(TokenKind::LeftParen, "`(` before held-fixed binding set")?;
            while !self.at(TokenKind::RightParen) {
                holding.push(self.parse_name_path("held-fixed binding")?);
                if !self.at(TokenKind::Comma) {
                    break;
                }
                self.bump();
            }
            self.expect(TokenKind::RightParen, "`)` after held-fixed binding set")?;
        }
        let end = self
            .expect(TokenKind::RightParen, "`)` after partial")?
            .range()
            .end();
        Some((
            Expr {
                resolved_enum: None,
                resolved_nominal: None,
                kind: ExprKind::Partial {
                    value: Box::new(value),
                    wrt,
                    holding,
                },
                range: TextRange::new(path.range().start(), end),
            },
            self.parent_depth(child_depth)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{ExprKind, parse};

    #[test]
    fn partial_retains_binding_names_and_canonical_syntax() {
        let parsed = parse(
            "partial.eqi",
            "operator f(input x:1,input y:1):1=partial(x*x*y,wrt=x,holding=(y));",
        );
        let document = parsed.into_document().unwrap();
        let expression = document.pure_operators()[0].body();
        let ExprKind::Partial { wrt, holding, .. } = expression.kind() else {
            panic!("partial")
        };
        assert_eq!(wrt.as_str(), "x");
        assert_eq!(holding[0].as_str(), "y");
        assert_eq!(
            expression.to_source(),
            "partial(x * x * y, wrt = x, holding = (y))"
        );
        for body in [
            "partial(x, wrt = x*x)",
            "partial(x, wrt = x, holding = [y])",
            "partial(x, x)",
        ] {
            assert!(
                !parse(
                    "invalid.eqi",
                    &format!("operator f(input x:1,input y:1):1={body};")
                )
                .diagnostics()
                .is_empty()
            );
        }
    }
}
