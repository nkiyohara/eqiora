//! Parsing for exact pure-operator declarations.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_pure_operator(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PureOperatorDecl> {
        self.expect_keyword("operator")?;
        let name = self
            .declaration_name("pure operator name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftParen, "`(` after pure operator name")?;
        if self.at(TokenKind::RightParen) {
            self.error_here("pure operator requires at least one formal");
            return None;
        }
        let mut formals = vec![self.parse_pure_operator_formal()?];
        while self.at(TokenKind::Comma) {
            self.bump();
            formals.push(self.parse_pure_operator_formal()?);
        }
        self.expect(TokenKind::RightParen, "`)` after pure operator formals")?;
        self.expect(TokenKind::Colon, "`:` before operator result")?;
        let result = self.parse_pure_value_class()?;
        self.expect(TokenKind::Equal, "`=` before pure operator body")?;
        let body = self.parse_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after pure operator")?
            .range()
            .end();
        Some(PureOperatorDecl {
            comments: Default::default(),
            visibility,
            name,
            formals,
            result,
            body,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_operator_formal(&mut self) -> Option<PureOperatorFormal> {
        let start = self.expect_keyword("input")?.range().start();
        let name = self.declaration_name("operator formal")?;
        self.expect(TokenKind::Colon, "`:` after pure operator formal")?;
        let value_class = self.parse_pure_value_class()?;
        let end = self.previous_significant_range().end();
        Some(PureOperatorFormal {
            comments: Default::default(),
            name: name.text().to_owned(),
            value_class,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_value_class(&mut self) -> Option<PureValueClassSyntax> {
        if self.at_keyword("scalar") {
            self.bump();
            Some(PureValueClassSyntax::Scalar)
        } else if self.at_keyword("spatial") {
            self.bump();
            self.expect(TokenKind::LeftBracket, "`[` after `spatial`")?;
            let rank = self.parse_exact_integer("spatial rank")?;
            self.expect(TokenKind::RightBracket, "`]` after spatial rank")?;
            Some(PureValueClassSyntax::Spatial { rank })
        } else {
            self.parse_value_type().map(PureValueClassSyntax::Typed)
        }
    }

    fn parse_exact_integer(&mut self, expected: &str) -> Option<ExactIntegerSyntax> {
        let token = self.expect(TokenKind::Number, expected)?;
        match token.text().parse::<u64>() {
            Ok(value) => Some(ExactIntegerSyntax {
                spelling: token.text().to_owned(),
                value,
                range: token.range(),
            }),
            Err(_) => {
                self.error_token(
                    &token,
                    format!("{expected} must be an exact unsigned integer"),
                );
                None
            }
        }
    }
}
