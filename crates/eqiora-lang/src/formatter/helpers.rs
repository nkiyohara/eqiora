use core::fmt::Write;

use crate::ast::{BoundaryPortSelectorSyntax, NamePath};

pub(super) fn format_boundary_port_selector(
    selector: &BoundaryPortSelectorSyntax,
    output: &mut String,
) {
    write!(output, "[{} = {}]", selector.member, selector.target).expect("String write");
}

pub(super) fn format_scalar_physical(
    across: &crate::ValueTypeSyntax,
    through: &crate::ValueTypeSyntax,
    output: &mut String,
) {
    output.push_str("scalar_physical(across = ");
    super::value_type::format_value_type(across, output);
    output.push_str(", through = ");
    super::value_type::format_value_type(through, output);
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
