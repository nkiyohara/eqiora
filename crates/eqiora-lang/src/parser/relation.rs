//! Relation declaration and natural-equation parsing.

use crate::ast::{
    ActivationSyntax, Equation, InitialDecl, RelationDecl, RelationFamilyDecl, TextRange,
};
use crate::lexer::TokenKind;

use super::Parser;

pub(super) enum ParsedRelation {
    Ordinary(RelationDecl),
    Family(RelationFamilyDecl),
}

impl Parser<'_> {
    pub(super) fn parse_initial(&mut self) -> Option<InitialDecl> {
        let start = self.expect_keyword("initial")?.range().start();
        self.expect(TokenKind::LeftBrace, "`{` before initialization equations")?;
        let mut equations = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            equations.push(self.parse_relation_statement()?);
        }
        if equations.is_empty() {
            self.error_here("initial requires at least one equation");
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after initialization")?
            .range()
            .end();
        Some(InitialDecl {
            comments: Default::default(),
            equations,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_component_relation(&mut self) -> Option<ParsedRelation> {
        let start = self.expect_keyword("relation")?.range().start();
        let name = self.declaration_name("Relation name")?.text().to_owned();
        let binder = if self.at(TokenKind::LeftBracket) {
            Some(self.parse_index_family_binder()?)
        } else {
            None
        };
        let domain = if self.at_keyword("on") {
            self.bump();
            Some(self.expect_identifier("Relation Domain")?.text().to_owned())
        } else {
            None
        };
        let activation = if self.at_keyword("at") {
            self.bump();
            ActivationSyntax::Named(
                self.expect_identifier("Relation activation")?
                    .text()
                    .to_owned(),
            )
        } else {
            ActivationSyntax::Continuous
        };
        self.expect(TokenKind::LeftBrace, "`{` before equations")?;
        let mut equations = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            equations.push(self.parse_relation_statement()?);
        }
        if equations.is_empty() {
            self.error_here("Relation requires at least one equation");
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after Relation")?
            .range()
            .end();
        let relation = RelationDecl {
            comments: Default::default(),
            name,
            activation,
            domain,
            body: crate::ast::RelationBody::Equations(equations),
            range: TextRange::new(start, end),
        };
        let Some(binder) = binder else {
            return Some(ParsedRelation::Ordinary(relation));
        };
        Some(ParsedRelation::Family(RelationFamilyDecl {
            relation,
            binder,
        }))
    }

    fn parse_relation_statement(&mut self) -> Option<Equation> {
        let left = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` after Relation left-hand expression")?;
        let right = self.parse_expression(0)?;
        self.expect(TokenKind::Semicolon, "`;` after equation")?;
        let range = TextRange::new(left.range().start(), right.range().end());
        Some(Equation { left, right, range })
    }
}
