//! Closed record declarations. Clock ownership belongs to the complete bus use.
use super::*;
use crate::{RecordDecl, RecordMemberDecl};
use std::collections::HashSet;

impl Parser<'_> {
    pub(super) fn parse_record(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<RecordDecl> {
        self.expect_keyword("record")?;
        let name = self.declaration_name("record name")?.text().to_owned();
        self.expect(TokenKind::LeftBrace, "`{` before record members")?;
        let mut members = Vec::new();
        let mut seen = HashSet::new();
        while !self.at(TokenKind::RightBrace) {
            if members.len() >= 65_536 {
                self.error_here("record exceeds the 65536-member limit");
                return None;
            }
            let token = self.declaration_name("record member")?;
            if token.text() == "_" || !seen.insert(token.text().to_owned()) {
                self.error_token(&token, "record requires distinct non-wildcard members");
                return None;
            }
            self.expect(TokenKind::Colon, "`:` before record member type")?;
            let value_type = self.parse_value_type()?;
            let range = TextRange::new(token.range().start(), value_type.range().end());
            members.push(RecordMemberDecl {
                comments: Default::default(),
                name: token.text().to_owned(),
                value_type,
                range,
            });
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        if members.is_empty() {
            self.error_here("record requires at least one member");
            return None;
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after record members")?
            .range()
            .end();
        Some(RecordDecl {
            comments: Default::default(),
            visibility,
            name,
            members,
            range: TextRange::new(start, end),
        })
    }
}
