use crate::ast::{LetDecl, ParameterDecl, TextRange};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_parameter(&mut self) -> Option<ParameterDecl> {
        let start = self.expect_keyword("parameter")?.range().start();
        let name = self
            .expect_identifier("declaration name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before dimension")?;
        let dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` before value")?;
        let initial = self.parse_signed_number()?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after declaration")?
            .range()
            .end();
        Some(ParameterDecl {
            name,
            dimension,
            initial,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_let(&mut self) -> Option<LetDecl> {
        let start = self.expect_keyword("let")?.range().start();
        let name = self.expect_identifier("alias name")?.text().to_owned();
        self.expect(TokenKind::Colon, "`:` before alias dimension")?;
        let dimension = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` before alias expression")?;
        let value = self.parse_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after declaration")?
            .range()
            .end();
        Some(LetDecl {
            name,
            dimension,
            value,
            range: TextRange::new(start, end),
        })
    }
}
