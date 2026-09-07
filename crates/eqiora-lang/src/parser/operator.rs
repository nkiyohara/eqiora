//! Parsing for exact pure-operator declarations.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_pure_operator(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PureOperatorDecl> {
        self.expect_keyword("pure")?;
        self.expect_keyword("operator")?;
        let name = self
            .expect_identifier("pure operator name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftParen, "`(` after pure operator name")?;
        if self.at(TokenKind::RightParen) {
            self.error_here("pure operator requires at least one formal");
            return None;
        }
        let mut formals = vec![self.parse_pure_operator_formal()?];
        while self.at(TokenKind::Comma) {
            self.bump();
            formals.push(self.parse_pure_operator_formal()?);
        }
        self.expect(TokenKind::RightParen, "`)` after pure operator formals")?;
        self.expect(TokenKind::Arrow, "`->` before pure operator result")?;
        let result = self.parse_pure_value_class()?;
        self.expect(TokenKind::Equal, "`=` before pure operator body")?;
        let body = self.parse_pure_operator_expression(0)?;
        let end = self
            .expect(TokenKind::Semicolon, "`;` after pure operator")?
            .range()
            .end();
        Some(PureOperatorDecl {
            comments: Default::default(),
            visibility,
            name,
            formals,
            result,
            body,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_operator_formal(&mut self) -> Option<PureOperatorFormal> {
        let name = self.expect_identifier("pure operator formal")?;
        let start = name.range().start();
        self.expect(TokenKind::Colon, "`:` after pure operator formal")?;
        let value_class = self.parse_pure_value_class()?;
        let end = self.previous_significant_range().end();
        Some(PureOperatorFormal {
            comments: Default::default(),
            name: name.text().to_owned(),
            value_class,
            range: TextRange::new(start, end),
        })
    }

    fn parse_pure_value_class(&mut self) -> Option<PureValueClassSyntax> {
        if self.at_keyword("scalar") {
            self.bump();
            Some(PureValueClassSyntax::Scalar)
        } else if self.at_keyword("spatial") {
            self.bump();
            self.expect(TokenKind::LeftBracket, "`[` after `spatial`")?;
            let rank = self.parse_exact_integer("spatial rank")?;
            self.expect(TokenKind::RightBracket, "`]` after spatial rank")?;
            Some(PureValueClassSyntax::Spatial { rank })
        } else {
            self.error_here("expected `scalar` or `spatial[rank]`");
            None
        }
    }

    fn parse_pure_operator_expression(
        &mut self,
        minimum_binding_power: u8,
    ) -> Option<PureOperatorExpr> {
        let mut left = if self.at(TokenKind::Minus) {
            let start = self.bump().range().start();
            let value = self.parse_pure_operator_expression(5)?;
            PureOperatorExpr {
                range: TextRange::new(start, value.range.end()),
                kind: PureOperatorExprKind::Neg(Box::new(value)),
            }
        } else if self.at_keyword("rational") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `rational`")?;
            let numerator = self.parse_exact_integer("rational numerator")?;
            self.expect(TokenKind::Comma, "`,` between rational integers")?;
            let denominator = self.parse_exact_integer("rational denominator")?;
            if denominator.value == 0 {
                let token = self.tokens[self.cursor.saturating_sub(1)].clone();
                self.error_token(&token, "rational denominator must be nonzero");
                return None;
            }
            let end = self
                .expect(TokenKind::RightParen, "`)` after rational literal")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Rational {
                    numerator,
                    denominator,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at_keyword("component") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `component`")?;
            let formal = self.expect_identifier("component formal")?;
            let mut result_axes = Vec::new();
            while self.at(TokenKind::Comma) {
                self.bump();
                result_axes.push(self.parse_exact_integer("component result axis")?);
            }
            let end = self
                .expect(TokenKind::RightParen, "`)` after component selection")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Component {
                    formal: formal.text().to_owned(),
                    formal_range: formal.range(),
                    result_axes,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at_keyword("delta") {
            let start = self.bump().range().start();
            self.expect(TokenKind::LeftParen, "`(` after `delta`")?;
            let left_axis = self.parse_exact_integer("delta left axis")?;
            self.expect(TokenKind::Comma, "`,` between delta axes")?;
            let right_axis = self.parse_exact_integer("delta right axis")?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after delta axes")?
                .range()
                .end();
            PureOperatorExpr {
                kind: PureOperatorExprKind::Delta {
                    left_axis,
                    right_axis,
                },
                range: TextRange::new(start, end),
            }
        } else if self.at(TokenKind::LeftParen) {
            let start = self.bump().range().start();
            let mut expression = self.parse_pure_operator_expression(0)?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after pure operator expression")?
                .range()
                .end();
            expression.range = TextRange::new(start, end);
            expression
        } else {
            self.error_here(
                "expected `rational`, `component`, `delta`, or parenthesized pure operator expression",
            );
            return None;
        };

        loop {
            let (operator, left_power, right_power) = match self.current().kind() {
                TokenKind::Plus => (PureOperatorBinaryOp::Add, 1, 2),
                TokenKind::Minus => (PureOperatorBinaryOp::Sub, 1, 2),
                TokenKind::Star => (PureOperatorBinaryOp::Mul, 3, 4),
                _ => break,
            };
            if left_power < minimum_binding_power {
                break;
            }
            self.bump();
            let right = self.parse_pure_operator_expression(right_power)?;
            let range = TextRange::new(left.range.start(), right.range.end());
            left = PureOperatorExpr {
                kind: PureOperatorExprKind::Binary {
                    op: operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                range,
            };
        }
        Some(left)
    }

    fn parse_exact_integer(&mut self, expected: &str) -> Option<ExactIntegerSyntax> {
        let token = self.expect(TokenKind::Number, expected)?;
        match token.text().parse::<u64>() {
            Ok(value) => Some(ExactIntegerSyntax {
                spelling: token.text().to_owned(),
                value,
                range: token.range(),
            }),
            Err(_) => {
                self.error_token(
                    &token,
                    format!("{expected} must be an exact unsigned integer"),
                );
                None
            }
        }
    }
}
