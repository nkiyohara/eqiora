use core::fmt::Write;

use crate::ast::BoundaryPortSelectorSyntax;

pub(super) fn format_boundary_port_selector(
    selector: &BoundaryPortSelectorSyntax,
    output: &mut crate::formatter::comments::Output,
) {
    write!(output, "[{} = {}]", selector.member, selector.target).expect("String write");
}

pub(super) fn format_scalar_physical(
    across: &crate::ValueTypeSyntax,
    through: &crate::ValueTypeSyntax,
    output: &mut crate::formatter::comments::Output,
) {
    output.push_str("scalar_physical(across = ");
    super::value_type::format_value_type(across, output);
    output.push_str(", through = ");
    super::value_type::format_value_type(through, output);
    output.push(')');
}

pub(super) fn write_indent(output: &mut crate::formatter::comments::Output, indent: usize) {
    output.extend(core::iter::repeat_n(' ', indent));
}
