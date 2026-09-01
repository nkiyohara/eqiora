use core::fmt::Write;

use crate::ast::{DimensionDecl, LetDecl, ParameterDecl};

use super::{format_expression, format_number, write_indent};

pub(super) fn format_dimension(declaration: &DimensionDecl, output: &mut String) {
    write!(output, "dimension {} = ", declaration.name).expect("String write");
    format_expression(&declaration.expression, 0, output);
    output.push_str(";\n");
}

pub(super) fn format_parameter(declaration: &ParameterDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    write!(output, "parameter {}: ", declaration.name).expect("String write");
    format_expression(&declaration.dimension, 0, output);
    writeln!(output, " = {};", format_number(declaration.initial)).expect("String write");
}

pub(super) fn format_let(declaration: &LetDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    write!(output, "let {}", declaration.name).expect("String write");
    if let Some(dimension) = &declaration.dimension {
        output.push_str(": ");
        format_expression(dimension, 0, output);
    }
    output.push_str(" = ");
    format_expression(&declaration.value, 0, output);
    output.push_str(";\n");
}
