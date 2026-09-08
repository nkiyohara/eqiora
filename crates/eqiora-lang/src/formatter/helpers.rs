use core::fmt::Write;

use crate::ast::BoundaryPortSelectorSyntax;

pub(super) fn format_boundary_port_selector(
    selector: &BoundaryPortSelectorSyntax,
    output: &mut crate::formatter::comments::Output,
) {
    write!(output, "[{} = {}]", selector.member, selector.target).expect("String write");
}

pub(super) fn format_scalar_physical(
    across_name: &str,
    across: &crate::ValueTypeSyntax,
    through_name: &str,
    through: &crate::ValueTypeSyntax,
    output: &mut crate::formatter::comments::Output,
) {
    write!(output, "scalar_physical(across {across_name}: ").expect("String write");
    super::value_type::format_value_type(across, output);
    write!(output, ", through {through_name}: ").expect("String write");
    super::value_type::format_value_type(through, output);
    output.push(')');
}

pub(super) fn write_indent(output: &mut crate::formatter::comments::Output, indent: usize) {
    output.extend(core::iter::repeat_n(' ', indent));
}
