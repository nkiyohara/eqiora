//! Canonical formatting of ordered authored equalities.

use core::fmt::Write;

use crate::ast::{ActivationSyntax, Equation, RelationDecl, RelationFamilyDecl};

use super::{format_boundary_family_binder, format_expression, write_indent};

pub(super) fn format_relation(
    declaration: &RelationDecl,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    write_indent(output, indent);
    write!(output, "relation {}", declaration.name).expect("String write");
    format_body(declaration, indent, output);
}

pub(super) fn format_relation_family(
    declaration: &RelationFamilyDecl,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    write_indent(output, indent);
    write!(output, "relation {}", declaration.relation.name).expect("String write");
    format_boundary_family_binder(&declaration.binder, output);
    format_body(&declaration.relation, indent, output);
}

fn format_body(
    declaration: &RelationDecl,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    if let Some(domain) = &declaration.domain {
        write!(output, " on {domain}").expect("String write");
    }
    if let ActivationSyntax::Named(clock) = &declaration.activation {
        write!(output, " at {clock}").expect("String write");
    }
    output.push_str(" {\n");
    for equation in &declaration.equations {
        format_equation(equation, indent, output);
    }
    write_indent(output, indent);
    output.push_str("}\n");
}

fn format_equation(
    equation: &Equation,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    write_indent(output, indent + 2);
    format_expression(equation.left(), 0, output);
    output.push_str(" = ");
    format_expression(equation.right(), 0, output);
    output.push_str(";\n");
}
