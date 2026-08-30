//! Closed authored-Formulation parsing.

use crate::ast::{FormulationDecl, FormulationSyntax, TextRange};
use crate::lexer::TokenKind;

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_formulation(&mut self) -> Option<FormulationDecl> {
        let start = self.expect_keyword("form")?.range().start();
        let kind = if self.at_keyword("primal") {
            self.bump();
            FormulationSyntax::Primal
        } else {
            self.error_here("expected `primal` after `form`");
            return None;
        };
        self.expect_keyword("for")?;
        let relation = self
            .expect_identifier("Formulation Relation")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftBrace, "`{` before authored Formulation")?;
        let left = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` in authored Formulation")?;
        let right = self.parse_expression(0)?;
        self.expect(
            TokenKind::Semicolon,
            "`;` after authored Formulation equality",
        )?;
        let end = self
            .expect(
                TokenKind::RightBrace,
                "`}` after the single authored Formulation equality",
            )?
            .range()
            .end();
        Some(FormulationDecl {
            kind,
            relation,
            left,
            right,
            range: TextRange::new(start, end),
        })
    }
}
