//! Canonical concrete periodic clock declarations.

use super::Parser;
use crate::ast::{ClockDecl, Expr, ExprKind, TextRange};
use crate::lexer::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_clock(&mut self) -> Option<ClockDecl> {
        let start = self.expect_keyword("clock")?.range().start();
        let name = self.declaration_name("ClockDomain name")?.text().to_owned();
        self.expect(TokenKind::Equal, "`=` before ClockDomain definition")?;
        self.expect_keyword("periodic")?;
        self.expect(TokenKind::LeftParen, "`(` after `periodic`")?;
        let period = self.parse_exact_expression()?;
        let phase = if self.at(TokenKind::Comma) {
            self.bump();
            self.expect_keyword("phase")?;
            self.expect(TokenKind::Equal, "`=` after `phase`")?;
            self.parse_exact_expression()?
        } else {
            let range = self.current().range();
            Expr {
                resolved_enum: None,
                resolved_nominal: None,
                kind: ExprKind::Quantity {
                    value: crate::DecimalLiteral::parse("0").expect("exact zero"),
                    unit: Box::new(Expr {
                        resolved_enum: None,
                        resolved_nominal: None,
                        kind: ExprKind::Name("s".into()),
                        range,
                    }),
                },
                range,
            }
        };
        self.expect(TokenKind::RightParen, "`)` after periodic clock")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after ClockDomain")?
            .range()
            .end();
        Some(ClockDecl {
            comments: Default::default(),
            name,
            period,
            phase,
            range: TextRange::new(start, end),
        })
    }
}
