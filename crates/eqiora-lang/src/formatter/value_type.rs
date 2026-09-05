use core::fmt::Write;

use eqiora_core::ScalarDomain;

use crate::{ValueTypeSyntax, ValueTypeSyntaxKind};

use super::format_expression;

impl ValueTypeSyntax {
    /// Emit this type using the canonical source constructors.
    #[must_use]
    pub fn to_source(&self) -> String {
        let mut output = String::new();
        format_value_type(self, &mut output);
        output
    }
}

pub(super) fn format_value_type(value: &ValueTypeSyntax, output: &mut String) {
    match value.kind() {
        ValueTypeSyntaxKind::Scalar { domain, dimension } => {
            if *domain == ScalarDomain::Complex {
                output.push_str("complex<");
            }
            format_expression(dimension, 0, output);
            if *domain == ScalarDomain::Complex {
                output.push('>');
            }
        }
        ValueTypeSyntaxKind::Vector { scalar, extent } => {
            output.push_str("vector<");
            format_value_type(scalar, output);
            write!(output, ", {extent}>").expect("String write");
        }
        ValueTypeSyntaxKind::Tensor { scalar, extents } => {
            output.push_str("tensor<");
            format_value_type(scalar, output);
            for extent in extents {
                write!(output, ", {extent}").expect("String write");
            }
            output.push('>');
        }
        ValueTypeSyntaxKind::Array { element, extent } => {
            output.push_str("array<");
            format_value_type(element, output);
            write!(output, ", {extent}>").expect("String write");
        }
    }
}
