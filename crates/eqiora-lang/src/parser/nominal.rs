//! Parsing for finite nominal declarations and bounded family binders.

use super::*;
use crate::ast::nominal::{FiniteSpaceDecl, IndexFamilyBinderSyntax, IndexSetDecl};
use std::collections::BTreeSet;

impl Parser<'_> {
    pub(super) fn parse_finite_space(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<FiniteSpaceDecl> {
        self.expect_keyword("space")?;
        let name = self
            .expect_identifier("finite space name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` before finite space definition")?;
        self.expect_keyword("orthonormal")?;
        self.expect(TokenKind::LeftParen, "`(` before finite space labels")?;
        let mut labels = Vec::new();
        let mut distinct = BTreeSet::new();
        loop {
            let label = self.expect_identifier("finite space basis label")?;
            if !distinct.insert(label.text().to_owned()) {
                self.error_token(&label, "finite space basis labels must be distinct");
                return None;
            }
            labels.push(label.text().to_owned());
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RightParen, "`)` after finite space labels")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after finite space")?
            .range()
            .end();
        Some(FiniteSpaceDecl {
            comments: Default::default(),
            visibility,
            name,
            labels,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_index_set(&mut self) -> Option<IndexSetDecl> {
        let start = self.expect_keyword("indexset")?.range().start();
        let name = self.expect_identifier("index set name")?.text().to_owned();
        self.expect(TokenKind::Equal, "`=` before index set definition")?;
        self.expect_keyword("range")?;
        self.expect(TokenKind::LeftParen, "`(` before index set extent")?;
        let extent = self.parse_expression(0)?;
        self.expect(TokenKind::RightParen, "`)` after index set extent")?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after index set")?
            .range()
            .end();
        Some(IndexSetDecl {
            comments: Default::default(),
            name,
            extent,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_index_family_binder(&mut self) -> Option<IndexFamilyBinderSyntax> {
        let start = self
            .expect(TokenKind::LeftBracket, "`[` before index family binder")?
            .range()
            .start();
        let binder = self
            .expect_identifier("index family binder")?
            .text()
            .to_owned();
        self.expect_keyword("in")?;
        let set = self.parse_name_path("index family set")?;
        let end = self
            .expect(TokenKind::RightBracket, "`]` after index family binder")?
            .range()
            .end();
        Some(IndexFamilyBinderSyntax {
            binder,
            set,
            range: TextRange::new(start, end),
        })
    }
}
