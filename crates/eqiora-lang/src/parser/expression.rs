//! Ordered expression parsing with bounded recursion and constructed-tree depth.

use super::*;

// Matches the compiler's existing canonical expression-depth admission. This
// guard runs before allocating a deeper tree or entering another recursive call.
const MAX_EXPRESSION_DEPTH: usize = 256;

impl Parser<'_> {
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
        let (mut left, mut depth) = if self.at(TokenKind::Minus) {
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
        } else if self.at(TokenKind::Number) {
            let expression = self.parse_quantity_or_number()?;
            // The unit grammar owns the separate dimension tree; the quantity
            // is one scalar operand in this value-expression depth bound.
            (expression, 1)
        } else if self.at(TokenKind::Identifier) {
            let token = self.bump();
            let name = token.text().to_owned();
            let path = if self.at(TokenKind::Dot) {
                self.parse_name_path_from_first(token, "qualified name segment")?
            } else {
                NamePath::single(name, token.range())
            };
            if self.at(TokenKind::LeftParen) {
                self.bump();
                if self.at(TokenKind::RightParen) {
                    self.error_here("operator call requires at least one argument");
                    return None;
                }
                let (first, mut child_depth) = self.parse_expression_with_depth(0)?;
                let mut arguments = vec![first];
                while self.at(TokenKind::Comma) {
                    self.bump();
                    let (argument, depth) = self.parse_expression_with_depth(0)?;
                    child_depth = child_depth.max(depth);
                    arguments.push(argument);
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
            } else if self.at(TokenKind::LeftBracket) {
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
        } else if self.at(TokenKind::LeftParen) {
            let start = self.bump().range().start();
            let (mut expression, depth) = self.parse_expression_with_depth(0)?;
            let end = self
                .expect(TokenKind::RightParen, "`)` after expression")?
                .range()
                .end();
            expression.range = TextRange::new(start, end);
            (expression, depth)
        } else {
            self.error_here("expected expression");
            return None;
        };
        loop {
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
}
