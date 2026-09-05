use super::Parser;
use crate::ast::{BinaryOp, Expr, ExprKind, TextRange, UnaryOp};
use crate::lexer::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_signed_quantity_literal(&mut self) -> Option<Expr> {
        let start = self.current().range().start();
        let negative = self.at(TokenKind::Minus);
        if negative {
            self.bump();
        }
        if !self.at(TokenKind::Number) {
            self.error_here("expected numeric quantity literal");
            return None;
        }
        let mut expression = self.parse_quantity_or_number()?;
        if negative {
            match &mut expression.kind {
                ExprKind::Number(value) | ExprKind::Quantity { value, .. } => *value = -*value,
                _ => unreachable!("numeric parser returns a literal"),
            }
        }
        expression.range = TextRange::new(start, expression.range.end());
        Some(expression)
    }

    pub(super) fn parse_quantity_or_number(&mut self) -> Option<Expr> {
        let token = self.bump();
        let value = self.parse_f64(&token)?;
        if !self.at(TokenKind::LeftBracket) {
            return Some(Expr {
                kind: ExprKind::Number(value),
                range: token.range(),
            });
        }
        self.bump();
        let unit = self.parse_dimension_expression()?;
        let end = self
            .expect(TokenKind::RightBracket, "`]` after input unit")?
            .range()
            .end();
        Some(Expr {
            kind: ExprKind::Quantity {
                value,
                unit: Box::new(unit),
            },
            range: TextRange::new(token.range().start(), end),
        })
    }

    pub(super) fn parse_dimension_expression(&mut self) -> Option<Expr> {
        self.parse_dimension_at_depth(0)
    }

    fn parse_dimension_at_depth(&mut self, depth: usize) -> Option<Expr> {
        let mut left = self.parse_dimension_power(depth)?;
        while matches!(self.current().kind(), TokenKind::Star | TokenKind::Slash) {
            let op = if self.bump().kind() == TokenKind::Star {
                BinaryOp::Mul
            } else {
                BinaryOp::Div
            };
            let right = self.parse_dimension_power(depth)?;
            left = dimension_binary(op, left, right);
        }
        Some(left)
    }

    fn parse_dimension_power(&mut self, depth: usize) -> Option<Expr> {
        let mut base = match self.current().kind() {
            TokenKind::LeftParen => {
                if depth >= 256 {
                    self.error_here(
                        "source resource limit exceeded: dimension nesting exceeds 256",
                    );
                    return None;
                }
                let start = self.bump().range().start();
                let mut expression = self.parse_dimension_at_depth(depth + 1)?;
                let end = self
                    .expect(TokenKind::RightParen, "`)` after dimension")?
                    .range()
                    .end();
                expression.range = TextRange::new(start, end);
                expression
            }
            TokenKind::Identifier => {
                let first = self.bump();
                let path = self.parse_name_path_from_first(first, "dimension name")?;
                let range = path.range();
                let kind = if path.is_qualified() {
                    ExprKind::Path(path)
                } else {
                    ExprKind::Name(path.as_str().to_owned())
                };
                Expr { kind, range }
            }
            TokenKind::Number if self.current().text() == "1" => {
                let token = self.bump();
                Expr {
                    kind: ExprKind::Number(1.0),
                    range: token.range(),
                }
            }
            _ => {
                self.error_here("dimension must use `1`, dimension names, products, quotients, and exact rational powers");
                return None;
            }
        };
        if self.at(TokenKind::Caret) {
            self.bump();
            let exponent = if self.at(TokenKind::LeftParen) {
                let start = self.bump().range().start();
                let numerator = self.parse_dimension_integer(true)?;
                self.expect(TokenKind::Slash, "`/` in rational dimension exponent")?;
                let denominator = self.parse_dimension_integer(false)?;
                let end = self
                    .expect(
                        TokenKind::RightParen,
                        "`)` after rational dimension exponent",
                    )?
                    .range()
                    .end();
                let mut ratio = dimension_binary(BinaryOp::Div, numerator, denominator);
                ratio.range = TextRange::new(start, end);
                ratio
            } else {
                self.parse_dimension_integer(true)?
            };
            base = dimension_binary(BinaryOp::Pow, base, exponent);
        }
        Some(base)
    }

    fn parse_dimension_integer(&mut self, signed: bool) -> Option<Expr> {
        let start = self.current().range().start();
        let negative = signed && self.at(TokenKind::Minus);
        if signed && matches!(self.current().kind(), TokenKind::Minus | TokenKind::Plus) {
            self.bump();
        }
        let token = self.expect(
            TokenKind::Number,
            "integer dimension exponent with positive denominator",
        )?;
        if token.text().len() > 256 {
            self.error_token(
                &token,
                "source resource limit exceeded: dimension exponent token exceeds 256 bytes",
            );
            return None;
        }
        let value = token
            .text()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| token.text().parse::<i32>().ok())
            .flatten();
        let Some(value) = value.filter(|value| signed || *value > 0) else {
            self.error_token(&token, "dimension exponent requires integer tokens bounded by 2147483647 and a positive denominator");
            return None;
        };
        let number = Expr {
            kind: ExprKind::Number(f64::from(value)),
            range: token.range(),
        };
        Some(if negative {
            Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(number),
                },
                range: TextRange::new(start, token.range().end()),
            }
        } else {
            Expr {
                range: TextRange::new(start, token.range().end()),
                ..number
            }
        })
    }
}

fn dimension_binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr {
        range: TextRange::new(left.range.start(), right.range.end()),
        kind: ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::{format, parse};

    #[test]
    fn quantity_islands_round_trip_without_falling_back_to_indexing() {
        for literal in [
            "10 [ms]",
            "1[m ^ (-1 / 2)]",
            "2 [Hz ^ (-2 / 4)]",
            "-3 [kg * m / s ^ 2]",
        ] {
            let source = format!("model M {{ relation value continuous {{ {literal} = 0; }} }}");
            let document = parse("quantity.eqi", &source).into_document().unwrap();
            let formatted = format(&document);
            let replay = parse("formatted.eqi", &formatted).into_document().unwrap();
            assert_eq!(format(&replay), formatted);
        }
        for literal in ["1 []", "1 [m + s]", "1 [m ^ (1 / 0)]", "1 [m"] {
            let source = format!("model M {{ relation value continuous {{ {literal} = 0; }} }}");
            assert!(
                parse("invalid.eqi", &source).into_document().is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn dimension_resources_reject_before_deep_recursion_or_integer_conversion() {
        let nested = |depth: usize| {
            format!(
                "dimension D = {}m{}; model M {{}}",
                "(".repeat(depth),
                ")".repeat(depth)
            )
        };
        assert!(parse("limit.eqi", &nested(256)).into_document().is_ok());
        for source in [
            nested(257),
            nested(10_000),
            format!("dimension D = m ^ {}1; model M {{}}", "0".repeat(256)),
        ] {
            let parsed = parse("excess.eqi", &source);
            assert!(
                parsed
                    .diagnostics()
                    .iter()
                    .any(|d| d.message().contains("source resource limit exceeded"))
            );
            assert!(parsed.into_document().is_err());
        }
        let at_limit = format!("dimension D = m ^ {}1; model M {{}}", "0".repeat(255));
        assert!(parse("token-limit.eqi", &at_limit).into_document().is_ok());
    }

    #[test]
    fn dimension_exponents_accept_exact_bounded_integer_tokens() {
        for dimension in [
            "m ^ +2",
            "m ^ -2147483647",
            "m ^ 2147483647",
            "m ^ (2 / 4)",
            "m ^ (-1 / 2)",
            "m ^ (+1 / 2)",
            "m ^ (0 / 2147483647)",
            "(m ^ 2) ^ (1 / 2)",
        ] {
            let source = format!("dimension D = {dimension}; model M {{ parameter x: D = 1; }}");
            let document = parse("dimension.eqi", &source)
                .into_document()
                .expect(dimension);
            let formatted = format(&document);
            let reparsed = parse("formatted.eqi", &formatted)
                .into_document()
                .expect(&formatted);
            assert_eq!(format(&reparsed), formatted);
        }
    }

    #[test]
    fn invalid_raw_exponents_reject_before_rounding_or_reduction() {
        for dimension in [
            "m ^ 2147483648",
            "m ^ -2147483648",
            "m ^ (2147483648 / 2147483648)",
            "m ^ (0 / 2147483648)",
            "m ^ 2147483647.0000000001",
            "m ^ 1.0",
            "m ^ 1e0",
            "m ^ (1 / 2.0)",
            "m ^ (1 / 0)",
            "m ^ (1 / -2)",
            "m ^ (1 / +2)",
            "m ^ (2)",
            "m ^ (1 + 1)",
            "m ^ 2 ^ 3",
        ] {
            for source in [
                format!("dimension D = {dimension}; model M {{}}"),
                format!("model M {{ parameter x: {dimension} = 1; }}"),
                format!("model M {{ let x: {dimension} = 1; }}"),
                format!("component C {{ public parameter x: {dimension}; }} model M {{}}"),
            ] {
                assert!(
                    parse("invalid.eqi", &source).into_document().is_err(),
                    "{source}"
                );
            }
        }
    }
}
