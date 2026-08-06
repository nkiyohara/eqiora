//! Relation declaration and natural-equation parsing.

use crate::ast::{
    ActivationSyntax, BinaryOp, Expr, ExprKind, RelationDecl, RelationFamilyDecl, TextRange,
};
use crate::lexer::TokenKind;

use super::Parser;

pub(super) enum ParsedRelation {
    Ordinary(RelationDecl),
    Family(RelationFamilyDecl),
}

impl Parser<'_> {
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
        let activation = if self.at_keyword("continuous") {
            self.bump();
            ActivationSyntax::Continuous
        } else if self.at_keyword("periodic") {
            self.bump();
            self.expect(TokenKind::LeftParen, "`(` after `periodic`")?;
            let clock = self
                .expect_identifier("periodic ClockDomain name")?
                .text()
                .to_owned();
            self.expect(TokenKind::RightParen, "`)` after ClockDomain name")?;
            ActivationSyntax::Periodic(clock)
        } else {
            self.error_here("expected `continuous` or `periodic(clock)` Activation");
            return None;
        };
        let domain = if self.at_keyword("on") {
            self.bump();
            Some(self.expect_identifier("Relation Domain")?.text().to_owned())
        } else {
            None
        };
        self.expect(TokenKind::LeftBrace, "`{` before residuals")?;
        let mut residuals = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            residuals.push(self.parse_relation_statement()?);
        }
        if residuals.is_empty() {
            self.error_here("Relation requires at least one residual");
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after Relation")?
            .range()
            .end();
        let relation = RelationDecl {
            name,
            activation,
            domain,
            residuals,
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

    fn parse_relation_statement(&mut self) -> Option<Expr> {
        let left = self.parse_expression(0)?;
        self.expect(TokenKind::Equal, "`=` after Relation left-hand expression")?;
        if self.at_legacy_zero_sentinel() {
            self.parse_signed_number()?;
            self.expect(TokenKind::Semicolon, "`;` after residual")?;
            return Some(left);
        }

        let right = self.parse_expression(0)?;
        self.expect(TokenKind::Semicolon, "`;` after residual")?;
        let range = TextRange::new(left.range().start(), right.range().end());
        Some(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Sub,
                left: Box::new(left),
                right: Box::new(right),
            },
            range,
        })
    }

    fn at_legacy_zero_sentinel(&self) -> bool {
        let mut tokens = self.tokens[self.cursor..]
            .iter()
            .filter(|token| !token.kind().is_trivia());
        let Some(first) = tokens.next() else {
            return false;
        };
        let (negative, number) = if first.kind() == TokenKind::Minus {
            let Some(number) = tokens.next() else {
                return false;
            };
            (true, number)
        } else {
            (false, first)
        };
        if number.kind() != TokenKind::Number
            || !tokens
                .next()
                .is_some_and(|token| token.kind() == TokenKind::Semicolon)
        {
            return false;
        }
        number
            .text()
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && if negative { -value } else { value } == 0.0)
    }
}
