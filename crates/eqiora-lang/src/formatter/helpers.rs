use core::fmt::Write;

use super::format_expression;
use crate::ast::{BoundaryPortSelectorSyntax, Expr, NamePath};

pub(super) fn format_boundary_port_selector(
    selector: &BoundaryPortSelectorSyntax,
    output: &mut String,
) {
    write!(output, "[{} = {}]", selector.member, selector.target).expect("String write");
}

pub(super) fn format_scalar_physical(across: &Expr, through: &Expr, output: &mut String) {
    output.push_str("scalar_physical(across = ");
    format_expression(across, 0, output);
    output.push_str(", through = ");
    format_expression(through, 0, output);
    output.push(')');
}

pub(super) fn format_name_paths(paths: &[NamePath], output: &mut String) {
    for (index, path) in paths.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "{path}").expect("String write");
    }
}

pub(super) fn write_indent(output: &mut String, indent: usize) {
    output.extend(core::iter::repeat_n(' ', indent));
}
