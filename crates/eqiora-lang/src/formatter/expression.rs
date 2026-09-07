//! Canonical expression rendering with explicit precedence and quantity islands.

use super::{format_boundary_port_selector, format_number};
use crate::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use core::fmt::Write;

pub(super) fn format_expression(
    expression: &Expr,
    parent_precedence: u8,
    output: &mut crate::formatter::comments::Output,
) {
    let precedence = expression_precedence(expression);
    let parenthesize = precedence < parent_precedence;
    if parenthesize {
        output.push('(');
    }
    match &expression.kind {
        ExprKind::Number(value) => output.push_str(&format_number(*value)),
        ExprKind::Quantity { value, unit } => {
            output.push_str(&value.canonical_text());
            output.push_str(" [");
            format_expression(unit, 0, output);
            output.push(']');
        }
        ExprKind::Name(name) => output.push_str(name),
        ExprKind::Path(path) => write!(output, "{path}").expect("String write"),
        ExprKind::BoundaryPortSelection { port, selector } => {
            write!(output, "{port}").expect("String write");
            format_boundary_port_selector(selector, output);
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            output.push('-');
            format_expression(value, precedence, output);
        }
        ExprKind::Binary { op, left, right } => {
            let (symbol, left_precedence, right_precedence) = match op {
                BinaryOp::Add => (" + ", precedence, precedence + 1),
                BinaryOp::Sub => (" - ", precedence, precedence + 1),
                BinaryOp::Mul => (" * ", precedence, precedence + 1),
                BinaryOp::Div => (" / ", precedence, precedence + 1),
                BinaryOp::Pow => (" ^ ", precedence + 1, 6),
            };
            format_expression(left, left_precedence, output);
            output.push_str(symbol);
            format_expression(right, right_precedence, output);
        }
        ExprKind::Array(elements) => {
            output.push('[');
            for (index, element) in elements.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                format_expression(element, 0, output);
            }
            output.push(']');
        }
        ExprKind::Index { value, index } => {
            // A bare number followed by `[` is always a quantity island.
            let group_number = matches!(value.kind(), ExprKind::Number(_));
            if group_number {
                output.push('(');
            }
            format_expression(value, 11, output);
            if group_number {
                output.push(')');
            }
            output.push('[');
            format_expression(index, 0, output);
            output.push(']');
        }
        ExprKind::Call { callee, arguments } => {
            write!(output, "{callee}").expect("String write");
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                format_expression(argument, 0, output);
            }
            output.push(')');
        }
    }
    if parenthesize {
        output.push(')');
    }
}

fn expression_precedence(expression: &Expr) -> u8 {
    match &expression.kind {
        ExprKind::Binary {
            op: BinaryOp::Add | BinaryOp::Sub,
            ..
        } => 1,
        ExprKind::Binary {
            op: BinaryOp::Mul | BinaryOp::Div,
            ..
        } => 3,
        ExprKind::Binary {
            op: BinaryOp::Pow, ..
        } => 7,
        ExprKind::Unary { .. } => 6,
        // Native source factories may store a negative literal directly rather
        // than as Unary(Neg). Its printed sign still needs a grouped power base.
        ExprKind::Number(value) if *value < 0.0 => 6,
        ExprKind::Quantity { value, .. } if value.is_negative() => 6,
        ExprKind::Number(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Name(_)
        | ExprKind::Path(_)
        | ExprKind::BoundaryPortSelection { .. }
        | ExprKind::Call { .. }
        | ExprKind::Array(_)
        | ExprKind::Index { .. } => 11,
    }
}
