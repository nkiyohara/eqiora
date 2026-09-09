use core::fmt::Write;

use super::{format_expression, separate_declaration};
use crate::ast::{Document, VisibilitySyntax};

pub(super) fn format_properties(
    document: &Document,
    output: &mut crate::formatter::comments::Output,
    count: &mut usize,
) {
    for dimension in &document.dimensions {
        separate_declaration(output, count);
        super::compile_time::format_dimension(dimension, output);
    }
    for (declaration, (visibility, name, value_type, _)) in document
        .property_contracts
        .iter()
        .zip(document.property_contract_syntax())
    {
        separate_declaration(output, count);
        output.begin(&declaration.comments);
        if visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        write!(
            output,
            "property contract {}(",
            declaration.comments.named(name)
        )
        .expect("String write");
        for (index, (name, input_type)) in declaration.inputs.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            write!(output, "input {name}: ").expect("String write");
            super::value_type::format_value_type(input_type, output);
        }
        output.push_str("): ");
        super::value_type::format_value_type(value_type, output);
        output.push_str(" {\n  derivatives ");
        output.push_str(declaration.derivatives.as_str());
        output.push_str(";\n");
        if let Some(branch) = &declaration.branch {
            writeln!(output, "  branch {branch};").expect("String write");
        }
        output.push_str("}\n");
        output.end();
    }
    for (
        declaration,
        (visibility, name, contract, value, source_dimension, scale, citation, license, _),
    ) in document
        .property_releases
        .iter()
        .zip(document.property_release_syntax())
    {
        separate_declaration(output, count);
        output.begin(&declaration.comments);
        if visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        writeln!(
            output,
            "property release {}: {contract} {{",
            declaration.comments.named(name)
        )
        .expect("String write");
        match value {
            crate::PropertySourceSyntax::Expression(value) => {
                output.push_str("  analytic {\n    value = ");
                format_expression(value, 0, output);
                output.push_str(";\n");
                if let Some(dimension) = source_dimension {
                    output.push_str("    source_unit: ");
                    format_expression(dimension, 0, output);
                    output.push_str(" = ");
                    format_expression(scale, 0, output);
                    output.push_str(";\n");
                }
                output.push_str("  }\n");
            }
            crate::PropertySourceSyntax::Table(table) => {
                write!(
                    output,
                    "  table {{\n    data {};\n    axis {}: ",
                    table.data(),
                    table.axis()
                )
                .expect("String write");
                format_expression(table.axis_dimension(), 0, output);
                write!(output, ";\n    value {}: ", table.value()).expect("String write");
                format_expression(table.value_dimension(), 0, output);
                output.push_str(";\n    interpolation piecewise_affine;\n    preprocessing identity;\n    missing reject;\n    knot_derivative reject;\n    endpoint_derivative reject;\n  }\n");
            }
        }
        if let crate::PropertySourceSyntax::Table(table) = value {
            write!(output, "  validity {} in [", table.axis()).expect("String write");
            format_expression(&table.validity()[0], 0, output);
            output.push_str(", ");
            format_expression(&table.validity()[1], 0, output);
            writeln!(output, "];\n  outside reject;\n  branch {};\n  citation {citation};\n  license {license};\n}}", declaration.branch.as_ref().expect("table branch")).expect("String write");
        } else {
            output.push_str("  validity ");
            if let Some(validity) = &declaration.validity {
                format_validity(validity, output);
            } else {
                output.push_str("unconditional");
            }
            writeln!(output, ";\n  outside reject;\n  branch {};\n  citation {citation};\n  license {license};\n}}", declaration.branch.as_ref().expect("admitted branch")).expect("String write");
        }
        output.end();
    }
    for (declaration, (visibility, name, properties, _)) in document
        .material_compositions
        .iter()
        .zip(document.material_composition_syntax())
    {
        separate_declaration(output, count);
        output.begin(&declaration.comments);
        if visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        writeln!(
            output,
            "material composition {} {{",
            declaration.comments.named(name)
        )
        .expect("String write");
        for (binding, (property, release, _)) in declaration.properties.iter().zip(properties) {
            output.begin(&binding.comments);
            writeln!(output, "  property {property} = {release};").expect("String write");
            output.end();
        }
        output.push_str("}\n");
        output.end();
    }
}

fn format_validity(value: &crate::Expr, output: &mut crate::formatter::comments::Output) {
    use crate::{BinaryOp, ExprKind};
    if let ExprKind::Binary {
        op: BinaryOp::And,
        left,
        right,
    } = value.kind()
        && let ExprKind::Binary {
            op: BinaryOp::GreaterEqual,
            left: low_input,
            right: low,
        } = left.kind()
        && let ExprKind::Binary {
            op: BinaryOp::LessEqual,
            left: high_input,
            right: high,
        } = right.kind()
        && let (ExprKind::Name(first), ExprKind::Name(second)) =
            (low_input.kind(), high_input.kind())
        && first == second
    {
        write!(output, "{first} in [").expect("String write");
        format_expression(low, 0, output);
        output.push_str(", ");
        format_expression(high, 0, output);
        output.push(']');
    } else {
        format_expression(value, 0, output);
    }
}
