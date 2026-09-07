use core::fmt::Write;

use crate::ast::{DimensionDecl, LetDecl, ParameterDecl};

use super::{format_expression, write_indent};

pub(super) fn format_dimension(
    declaration: &DimensionDecl,
    output: &mut crate::formatter::comments::Output,
) {
    output.begin(&declaration.comments);
    write!(output, "dimension {} = ", declaration.name).expect("String write");
    format_expression(&declaration.expression, 0, output);
    output.push_str(";\n");
    output.end();
}

pub(super) fn format_parameter(
    declaration: &ParameterDecl,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    write_indent(output, indent);
    write!(output, "parameter {}: ", declaration.name).expect("String write");
    super::value_type::format_value_type(&declaration.value_type, output);
    output.push_str(" = ");
    format_expression(&declaration.value, 0, output);
    output.push_str(";\n");
}

pub(super) fn format_let(
    declaration: &LetDecl,
    indent: usize,
    output: &mut crate::formatter::comments::Output,
) {
    write_indent(output, indent);
    write!(output, "let {}", declaration.name).expect("String write");
    if let Some(value_type) = &declaration.value_type {
        output.push_str(": ");
        super::value_type::format_value_type(value_type, output);
    }
    output.push_str(" = ");
    format_expression(&declaration.value, 0, output);
    output.push_str(";\n");
}
