//! The admitted Component requirements share existing exact interface owners.

use super::Parser;
use crate::ast::{ClockRequirementDecl, ComponentItem, TextRange, VisibilitySyntax};
use crate::lexer::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_component_signature(&mut self) -> Option<Vec<ComponentItem>> {
        self.expect(
            TokenKind::LeftParen,
            "`(` before Component signature (use `()` when empty)",
        )?;
        let mut requirements = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            let start = self.current().range().start();
            let requirement = if self.at_keyword("support") {
                ComponentItem::Support(self.parse_support_slot(start, VisibilitySyntax::Public)?)
            } else if self.at_keyword("variable") || self.at_keyword("state") {
                ComponentItem::FieldRequirement(self.parse_field(false)?)
            } else if self.at_keyword("clock") {
                self.bump();
                let token = self.expect_identifier("required clock name")?;
                ComponentItem::ClockRequirement(ClockRequirementDecl {
                    comments: Default::default(),
                    name: token.text().to_owned(),
                    range: TextRange::new(start, token.range().end()),
                })
            } else {
                self.error_here("expected support, variable, state, or clock requirement");
                return None;
            };
            requirements.push(requirement);
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RightParen, "`)` after Component signature")?;
        Some(requirements)
    }
}
