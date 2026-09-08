//! Private crossing events; guard typing belongs to the common compiler.
use super::{Parser, TokenKind};
use crate::{EventDecl, TextRange};
use eqiora_schema::kernel::EventDirection;

impl Parser<'_> {
    pub(super) fn parse_event(&mut self) -> Option<EventDecl> {
        let start = self.expect_keyword("event")?.range().start();
        let name = self.expect_identifier("event name")?.text().to_owned();
        self.expect(TokenKind::Equal, "`=` before event definition")?;
        self.expect_keyword("crossing")?;
        self.expect(TokenKind::LeftParen, "`(` after crossing")?;
        let guard = self.parse_expression(0)?;
        self.expect(TokenKind::Comma, "`,` before crossing direction")?;
        self.expect_keyword("direction")?;
        self.expect(TokenKind::Equal, "`=` before crossing direction")?;
        let token = self.expect_identifier("crossing direction")?;
        let direction = match token.text() {
            "any" => EventDirection::Any,
            "rising" => EventDirection::Rising,
            "falling" => EventDirection::Falling,
            _ => {
                self.error_token(
                    &token,
                    "expected crossing direction `any`, `rising`, or `falling`",
                );
                return None;
            }
        };
        self.expect(TokenKind::RightParen, "`)` after crossing direction")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after event")?
            .range()
            .end();
        Some(EventDecl {
            comments: Default::default(),
            name,
            guard,
            direction,
            range: TextRange::new(start, end),
        })
    }
}
