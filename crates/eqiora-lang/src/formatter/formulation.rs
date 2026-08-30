//! Canonical formatting for authored mathematical formulations.

use core::fmt::Write;

use crate::ast::{FormulationDecl, FormulationSyntax};

use super::{format_expression, write_indent};

pub(super) fn format_formulation(
    declaration: &FormulationDecl,
    indent: usize,
    output: &mut String,
) {
    write_indent(output, indent);
    output.push_str("form ");
    output.push_str(match declaration.kind {
        FormulationSyntax::Primal => "primal",
    });
    writeln!(output, " for {} {{", declaration.relation).expect("String write");
    write_indent(output, indent + 2);
    format_expression(&declaration.left, 0, output);
    output.push_str(" = ");
    format_expression(&declaration.right, 0, output);
    output.push_str(";\n");
    write_indent(output, indent);
    output.push_str("}\n");
}
