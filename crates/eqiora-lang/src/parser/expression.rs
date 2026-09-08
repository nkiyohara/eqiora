//! Ordered expression parsing with bounded recursion and constructed-tree depth.

use super::*;

// Matches the compiler's existing canonical expression-depth admission. This
// guard runs before allocating a deeper tree or entering another recursive call.
const MAX_EXPRESSION_DEPTH: usize = 256;

impl Parser<'_> {
    pub(super) fn parse_exact_expression(&mut self) -> Option<Expr> {
        let previous = self.exact_numeric;
        self.exact_numeric = true;
        let result = self.parse_expression(0);
        self.exact_numeric = previous;
        result
    }

    pub(super) fn parse_expression(&mut self, minimum_binding_power: u8) -> Option<Expr> {
        self.parse_expression_with_depth(minimum_binding_power)
            .map(|(expression, _)| expression)
    }

    fn parse_expression_with_depth(&mut self, minimum_binding_power: u8) -> Option<(Expr, usize)> {
        if self.expression_recursion >= MAX_EXPRESSION_DEPTH {
            self.error_here("expression recursion exceeds the 256-level limit");
            return None;
        }
        self.expression_recursion += 1;
        let parsed = self.parse_expression_inner(minimum_binding_power);
        self.expression_recursion -= 1;
        parsed
    }

    fn parent_depth(&mut self, child_depth: usize) -> Option<usize> {
        if child_depth >= MAX_EXPRESSION_DEPTH {
            self.error_here("expression tree exceeds the 256-level limit");
            None
        } else {
            Some(child_depth + 1)
        }
    }

    fn parse_expression_inner(&mut self, minimum_binding_power: u8) -> Option<(Expr, usize)> {
        let (mut left, mut depth) = self.parse_primary()?;
        loop {
            if self.at(TokenKind::Dot) {
                if !matches!(
                    left.kind(),
                    ExprKind::Index { .. } | ExprKind::Member { .. }
                ) {
                    self.error_here("member access requires an indexed component occurrence");
                    return None;
                }
                self.bump();
                let member = self.expect_identifier("indexed occurrence member")?;
                depth = self.parent_depth(depth)?;
                let range = TextRange::new(left.range.start(), member.range().end());
                left = Expr {
                    kind: ExprKind::Member {
                        value: Box::new(left),
                        member: member.text().to_owned(),
                    },
                    range,
                };
                continue;
            }
            if self.at(TokenKind::LeftBracket) {
                self.bump();
                let (index, index_depth) = self.parse_expression_with_depth(0)?;
                depth = self.parent_depth(depth.max(index_depth))?;
                let end = self
                    .expect(TokenKind::RightBracket, "`]` after index")?
                    .range()
                    .end();
                let range = TextRange::new(left.range.start(), end);
                left = Expr {
                    kind: ExprKind::Index {
                        value: Box::new(left),
                        index: Box::new(index),
                    },
                    range,
                };
                continue;
            }
            let (operator, left_power, right_power) = match self.current().kind() {
                TokenKind::Plus => (BinaryOp::Add, 1, 2),
                TokenKind::Minus => (BinaryOp::Sub, 1, 2),
                TokenKind::Star => (BinaryOp::Mul, 3, 4),
                TokenKind::Slash => (BinaryOp::Div, 3, 4),
                TokenKind::Caret => (BinaryOp::Pow, 7, 6),
                _ => break,
            };
            if left_power < minimum_binding_power {
                break;
            }
            self.bump();
            let (right, right_depth) = self.parse_expression_with_depth(right_power)?;
            depth = self.parent_depth(depth.max(right_depth))?;
            let range = TextRange::new(left.range.start(), right.range.end());
            left = Expr {
                kind: ExprKind::Binary {
                    op: operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                range,
            };
        }
        Some((left, depth))
    }
    fn parse_primary(&mut self) -> Option<(Expr, usize)> {
        let result = if self.at(TokenKind::Minus) {
            let start = self.bump().range().start();
            // Power binds inside unary minus, including a signed right power:
            // -x^2 is -(x^2), while x^-2 is x^(-2).
            let (value, child_depth) = self.parse_expression_with_depth(6)?;
            let depth = self.parent_depth(child_depth)?;
            (
                Expr {
                    range: TextRange::new(start, value.range.end()),
                    kind: ExprKind::Unary {
                        op: UnaryOp::Neg,
                        value: Box::new(value),
                    },
                },
                depth,
            )
        } else if self.at(TokenKind::LeftBracket) {
            return self.parse_array();
        } else if self.at(TokenKind::Number) {
            let expression = self.parse_quantity_or_number()?;
            // The unit grammar owns the separate dimension tree; the quantity
            // is one scalar operand in this value-expression depth bound.
            (expression, 1)
        } else if self.at(TokenKind::Identifier) {
            return self.parse_named();
        } else if self.at(TokenKind::LeftParen) {
            return self.parse_group();
        } else {
            self.error_here("expected expression");
            return None;
        };
        Some(result)
    }

    fn parse_array(&mut self) -> Option<(Expr, usize)> {
        let result = {
            let start = self.bump().range().start();
            if self.at(TokenKind::RightBracket) {
                self.error_here("array literal requires at least one element");
                return None;
            }
            let (first, mut child_depth) = self.parse_expression_with_depth(0)?;
            let mut elements = vec![first];
            while self.at(TokenKind::Comma) {
                self.bump();
                let (element, depth) = self.parse_expression_with_depth(0)?;
                child_depth = child_depth.max(depth);
                elements.push(element);
            }
            let depth = self.parent_depth(child_depth)?;
            let end = self
                .expect(TokenKind::RightBracket, "`]` after array elements")?
                .range()
                .end();
            (
                Expr {
                    kind: ExprKind::Array(elements),
                    range: TextRange::new(start, end),
                },
                depth,
            )
        };
        Some(result)
    }

    fn parse_named(&mut self) -> Option<(Expr, usize)> {
        let result = {
            let token = self.bump();
            let name = token.text().to_owned();
            let path = if self.at(TokenKind::Dot) {
                self.parse_name_path_from_first(token, "qualified name segment")?
            } else {
                NamePath::single(name, token.range())
            };
            if self.at(TokenKind::LeftParen) {
                self.bump();
                let mut arguments = Vec::new();
                let mut child_depth = 0;
                if self.at(TokenKind::RightParen) {
                    if path.as_str() != "boundaries" {
                        self.error_here("operator call requires at least one argument");
                        return None;
                    }
                } else {
                    loop {
                        let (argument, depth) = self.parse_expression_with_depth(0)?;
                        child_depth = child_depth.max(depth);
                        arguments.push(argument);
                        if !self.at(TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                    }
                }
                let depth = self.parent_depth(child_depth)?;
                let end = self
                    .expect(TokenKind::RightParen, "`)` after operator arguments")?
                    .range()
                    .end();
                (
                    Expr {
                        kind: ExprKind::Call {
                            callee: path.clone(),
                            arguments,
                        },
                        range: TextRange::new(path.range().start(), end),
                    },
                    depth,
                )
            } else if self.at_boundary_selection() {
                let selector = self.parse_boundary_port_selector()?;
                let range = TextRange::new(path.range().start(), selector.range().end());
                (
                    Expr {
                        kind: ExprKind::BoundaryPortSelection {
                            port: Box::new(path),
                            selector: Box::new(selector),
                        },
                        range,
                    },
                    1,
                )
            } else {
                let range = path.range();
                let kind = if path.is_qualified() {
                    ExprKind::Path(path)
                } else {
                    ExprKind::Name(path.as_str().to_owned())
                };
                (Expr { kind, range }, 1)
            }
        };
        Some(result)
    }

    fn parse_group(&mut self) -> Option<(Expr, usize)> {
        let result = {
            let start = self.bump().range().start();
            let (mut expression, depth) = self.parse_expression_with_depth(0)?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after expression")?
                .range()
                .end();
            expression.range = TextRange::new(start, end);
            (expression, depth)
        };
        Some(result)
    }

    fn at_boundary_selection(&mut self) -> bool {
        if !self.at(TokenKind::LeftBracket) {
            return false;
        }
        let mut tokens = self.tokens[self.cursor..]
            .iter()
            .filter(|token| !token.kind().is_trivia());
        tokens.next();
        matches!(tokens.next().map(Token::kind), Some(TokenKind::Identifier))
            && matches!(tokens.next().map(Token::kind), Some(TokenKind::Equal))
    }
}
