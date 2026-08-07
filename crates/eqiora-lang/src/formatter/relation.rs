//! Canonical Relation formatting, including natural equality projection.

use core::fmt::Write;

use crate::ast::{
    ActivationSyntax, BinaryOp, Expr, ExprKind, RelationDecl, RelationFamilyDecl, UnaryOp,
};

use super::{format_boundary_family_binder, format_expression, write_indent};

pub(super) fn format_relation(declaration: &RelationDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    write!(output, "relation {} ", declaration.name).expect("String write");
    match &declaration.activation {
        ActivationSyntax::Continuous => output.push_str("continuous"),
        ActivationSyntax::Periodic(clock) => {
            write!(output, "periodic({clock})").expect("String write");
        }
    }
    if let Some(domain) = &declaration.domain {
        write!(output, " on {domain}").expect("String write");
    }
    output.push_str(" {\n");
    format_residuals(&declaration.residuals, indent, output);
    write_indent(output, indent);
    output.push_str("}\n");
}

pub(super) fn format_relation_family(
    declaration: &RelationFamilyDecl,
    indent: usize,
    output: &mut String,
) {
    let relation = &declaration.relation;
    write_indent(output, indent);
    write!(output, "relation {}", relation.name).expect("String write");
    format_boundary_family_binder(&declaration.binder, output);
    output.push_str(" continuous");
    if let Some(domain) = &relation.domain {
        write!(output, " on {domain}").expect("String write");
    }
    output.push_str(" {\n");
    format_residuals(&relation.residuals, indent, output);
    write_indent(output, indent);
    output.push_str("}\n");
}

fn format_residuals(residuals: &[Expr], indent: usize, output: &mut String) {
    for residual in residuals {
        write_indent(output, indent + 2);
        format_residual(residual, output);
        output.push_str(";\n");
    }
}

fn format_residual(residual: &Expr, output: &mut String) {
    let ExprKind::Binary {
        op: BinaryOp::Sub,
        left,
        right,
    } = residual.kind()
    else {
        format_expression(residual, 0, output);
        output.push_str(" = 0");
        return;
    };

    format_expression(left, 0, output);
    output.push_str(" = ");
    if collides_with_legacy_zero_sentinel(right) {
        output.push('(');
        format_expression(right, 0, output);
        output.push(')');
    } else {
        format_expression(right, 0, output);
    }
}

fn collides_with_legacy_zero_sentinel(expression: &Expr) -> bool {
    match expression.kind() {
        ExprKind::Number(value) => *value == 0.0,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            matches!(value.kind(), ExprKind::Number(number) if *number == 0.0)
        }
        _ => false,
    }
}
