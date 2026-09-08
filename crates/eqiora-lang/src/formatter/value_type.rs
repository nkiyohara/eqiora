use core::fmt::Write;

use eqiora_core::ScalarDomain;

use crate::{ValueTypeSyntax, ValueTypeSyntaxKind};

use super::format_expression;

impl ValueTypeSyntax {
    /// Emit this type using the canonical source constructors.
    #[must_use]
    pub fn to_source(&self) -> String {
        let mut output = super::comments::Output::default();
        format_value_type(self, &mut output);
        output.finish()
    }
}

pub(super) fn format_value_type(
    value: &ValueTypeSyntax,
    output: &mut crate::formatter::comments::Output,
) {
    match value.kind() {
        ValueTypeSyntaxKind::Coordinates(name) => {
            write!(output, "coordinates<integer, {name}>").expect("String write");
        }
        ValueTypeSyntaxKind::Counts(name) => {
            write!(output, "counts<{name}>").expect("String write");
        }
        ValueTypeSyntaxKind::Index(name) => {
            write!(output, "index<{name}>").expect("String write");
        }
        ValueTypeSyntaxKind::Scalar { domain, dimension } => {
            if *domain == ScalarDomain::Boolean {
                output.push_str("bool");
                return;
            }
            if *domain == ScalarDomain::Integer {
                output.push_str("integer");
                return;
            }
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
