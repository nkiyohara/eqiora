//! Category-free named occurrence arguments.
use super::*;
impl Parser<'_> {
    pub(super) fn parse_instance(&mut self) -> Option<InstanceDecl> {
        let start = self.expect_keyword("instance")?.range().start();
        let name = self.declaration_name("instance name")?.text().to_owned();
        let family = if self.at(TokenKind::LeftBracket) {
            Some(self.parse_index_family_binder()?)
        } else {
            None
        };
        self.expect(TokenKind::Colon, "`:` before component definition")?;
        let definition = self.parse_name_path("component definition name")?;
        self.expect(TokenKind::LeftParen, "`(` before named bindings")?;
        let mut bindings = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            let token = self.expect_identifier("target signature name")?;
            let name = token.text().to_owned();
            let binding_start = token.range().start();
            self.expect(TokenKind::Equal, "`=` in named binding")?;
            let value = self.parse_expression(0)?;
            bindings.push(NamedBindingDecl {
                comments: Default::default(),
                name,
                range: TextRange::new(binding_start, value.range().end()),
                value,
            });
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RightParen, "`)` after named bindings")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after instance")?
            .range()
            .end();
        Some(InstanceDecl {
            comments: Default::default(),
            name,
            definition,
            family,
            bindings,
            range: TextRange::new(start, end),
        })
    }
}
