//! Recovery at the existing declaration delimiters.

use super::{Parser, TokenKind};

impl Parser<'_> {
    pub(super) fn recover_item(&mut self) {
        while !self.at(TokenKind::Eof) && !self.at(TokenKind::RightBrace) {
            if self.at(TokenKind::Semicolon) {
                self.bump();
                return;
            }
            self.bump();
        }
    }

    pub(super) fn recover_top_level(&mut self) {
        while !self.at(TokenKind::Eof)
            && !self.at_keyword("property")
            && !self.at_keyword("connector")
            && !self.at_keyword("component")
            && !self.at_keyword("enum")
            && !self.at_keyword("operator")
            && !self.at_keyword("model")
        {
            self.bump();
        }
    }
}
