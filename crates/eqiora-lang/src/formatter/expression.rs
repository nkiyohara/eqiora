//! Canonical expression rendering with explicit precedence and quantity islands.

use super::format_boundary_port_selector;
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
        ExprKind::Member { value, member } => {
            format_expression(value, 19, output);
            write!(output, ".{member}").expect("String write");
        }
        ExprKind::Boolean(value) => output.push_str(if *value { "true" } else { "false" }),
        ExprKind::Number(value) => output.push_str(&value.canonical_text()),
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
        ExprKind::Unary {
            op: UnaryOp::Not,
            value,
        } => {
            output.push_str("not ");
            format_expression(value, precedence, output);
        }
        ExprKind::Binary { op, left, right } => {
            let (symbol, left_precedence, right_precedence) = match op {
                BinaryOp::Add => (" + ", precedence, precedence + 1),
                BinaryOp::Sub => (" - ", precedence, precedence + 1),
                BinaryOp::Mul => (" * ", precedence, precedence + 1),
                BinaryOp::Div => (" / ", precedence, precedence + 1),
                BinaryOp::Pow => (" ^ ", precedence + 1, 14),
                BinaryOp::And => (" and ", precedence, precedence + 1),
                BinaryOp::Or => (" or ", precedence, precedence + 1),
                BinaryOp::Equal => (" == ", precedence + 1, precedence + 1),
                BinaryOp::NotEqual => (" != ", precedence + 1, precedence + 1),
                BinaryOp::Less => (" < ", precedence + 1, precedence + 1),
                BinaryOp::LessEqual => (" <= ", precedence + 1, precedence + 1),
                BinaryOp::Greater => (" > ", precedence + 1, precedence + 1),
                BinaryOp::GreaterEqual => (" >= ", precedence + 1, precedence + 1),
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
            format_expression(value, 19, output);
            if group_number {
                output.push(')');
            }
            output.push('[');
            format_expression(index, 0, output);
            output.push(']');
        }
        ExprKind::Reduction {
            operation,
            binder,
            value,
        } => {
            output.push_str(match operation {
                crate::ReductionOp::Sum => "sum(",
                crate::ReductionOp::Product => "product(",
                crate::ReductionOp::Min => "min(",
                crate::ReductionOp::Max => "max(",
            });
            format_expression(value, 0, output);
            write!(
                output,
                ", over = ({} in {}))",
                binder.member(),
                binder.set()
            )
            .expect("String write");
        }
        ExprKind::Call { callee, arguments } => {
            write!(output, "{callee}").expect("String write");
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                if callee.as_str() == "tensor_value" {
                    output.push_str(if index == 0 {
                        "frame = "
                    } else {
                        "components = "
                    });
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
        } => 9,
        ExprKind::Binary {
            op: BinaryOp::Mul | BinaryOp::Div,
            ..
        } => 11,
        ExprKind::Binary {
            op: BinaryOp::Pow, ..
        } => 15,
        ExprKind::Unary {
            op: UnaryOp::Neg, ..
        } => 14,
        ExprKind::Unary {
            op: UnaryOp::Not, ..
        } => 5,
        ExprKind::Binary {
            op: BinaryOp::Or, ..
        } => 1,
        ExprKind::Binary {
            op: BinaryOp::And, ..
        } => 3,
        ExprKind::Binary { .. } => 7,
        // Native source factories may store a negative literal directly rather
        // than as Unary(Neg). Its printed sign still needs a grouped power base.
        ExprKind::Number(value) if value.is_negative() => 14,
        ExprKind::Quantity { value, .. } if value.is_negative() => 14,
        ExprKind::Member { .. } => 19,
        ExprKind::Boolean(_)
        | ExprKind::Number(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Name(_)
        | ExprKind::Path(_)
        | ExprKind::BoundaryPortSelection { .. }
        | ExprKind::Call { .. }
        | ExprKind::Reduction { .. }
        | ExprKind::Array(_)
        | ExprKind::Index { .. } => 19,
    }
}
