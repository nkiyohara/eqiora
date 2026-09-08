//! Module enumeration declarations and exhaustive value cases.
use super::*;
use crate::{CaseArm, EnumDecl};
use std::collections::HashSet;

impl Parser<'_> {
    pub(super) fn parse_enumeration(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<EnumDecl> {
        self.expect_keyword("enum")?;
        let name = self.expect_identifier("enum name")?.text().to_owned();
        self.expect(TokenKind::LeftBrace, "`{` before enum tags")?;
        let mut tags = Vec::new();
        let mut seen = HashSet::new();
        while !self.at(TokenKind::RightBrace) {
            if tags.len() >= 65_536 {
                self.error_here("enum exceeds the 65536-tag limit");
                return None;
            }
            let token = self.expect_identifier("enum tag")?;
            if token.text() == "_" || !seen.insert(token.text().to_owned()) {
                self.error_token(
                    &token,
                    "enum tags must be distinct non-wildcard identifiers",
                );
                return None;
            }
            tags.push(NamePath::single(token.text().to_owned(), token.range()));
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        if tags.is_empty() {
            self.error_here("enum requires at least one tag");
            return None;
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after enum tags")?
            .range()
            .end();
        Some(EnumDecl {
            comments: Default::default(),
            visibility,
            name,
            tags,
            range: TextRange::new(start, end),
        })
    }
    pub(super) fn parse_case(&mut self) -> Option<(Expr, usize)> {
        let start = self.expect_keyword("case")?.range().start();
        let (value, mut child_depth) = self.parse_expression_with_depth(0)?;
        self.expect(TokenKind::LeftBrace, "`{` before case arms")?;
        let mut arms = Vec::new();
        let mut seen = HashSet::new();
        while !self.at(TokenKind::RightBrace) {
            if arms.len() >= 65_536 {
                self.error_here("case exceeds the 65536-arm limit");
                return None;
            }
            let pattern = self.parse_name_path("qualified enum tag")?;
            if !pattern.is_qualified()
                || pattern.segments().any(|segment| segment == "_")
                || !seen.insert(pattern.as_str().to_owned())
            {
                self.error_here("case requires distinct qualified enum tags without wildcards");
                return None;
            }
            self.expect(TokenKind::FatArrow, "`=>` after case pattern")?;
            let (arm_value, depth) = self.parse_expression_with_depth(0)?;
            child_depth = child_depth.max(depth);
            let range = TextRange::new(pattern.range().start(), arm_value.range().end());
            arms.push(CaseArm {
                pattern,
                value: arm_value,
                range,
            });
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        if arms.is_empty() {
            self.error_here("case requires at least one arm");
            return None;
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after case arms")?
            .range()
            .end();
        let depth = self.parent_depth(child_depth)?;
        Some((
            Expr {
                resolved_nominal: None,
                kind: ExprKind::Case {
                    value: Box::new(value),
                    arms,
                },
                range: TextRange::new(start, end),
            },
            depth,
        ))
    }
}
