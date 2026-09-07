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

    pub(super) fn parse_relation(&mut self) -> Option<RelationDecl> {
        match self.parse_relation_inner(false)? {
            ParsedRelation::Ordinary(relation) => Some(relation),
            ParsedRelation::Family(_) => unreachable!("model Relations reject family binders"),
        }
    }

    pub(super) fn parse_component_relation(&mut self) -> Option<ParsedRelation> {
        self.parse_relation_inner(true)
    }

    fn parse_relation_inner(&mut self, allow_family: bool) -> Option<ParsedRelation> {
        let start = self.expect_keyword("relation")?.range().start();
        let name = self.expect_identifier("Relation name")?.text().to_owned();
        let binder = if self.at(TokenKind::LeftBracket) {
            if !allow_family {
                self.error_here("boundary family binders are allowed only in Components");
                return None;
            }
            Some(self.parse_boundary_family_binder()?)
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
            ActivationSyntax::Periodic(
                self.expect_identifier("Relation Clock activation")?
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
            equations,
            range: TextRange::new(start, end),
        };
        let Some(binder) = binder else {
            return Some(ParsedRelation::Ordinary(relation));
        };
        if !matches!(relation.activation(), ActivationSyntax::Continuous) {
            self.error_here("a boundary Relation family must be continuous");
            return None;
        }
        if relation.domain() != Some(binder.member()) {
            self.error_here(
                "a boundary Relation family must be declared on its bound boundary member",
            );
            return None;
        }
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
