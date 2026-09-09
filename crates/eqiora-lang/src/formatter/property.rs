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
            "property contract {}(): ",
            declaration.comments.named(name)
        )
        .expect("String write");
        super::value_type::format_value_type(value_type, output);
        output.push_str(" {\n  derivatives value_only;\n}\n");
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
            "property release {} implements {contract} {{",
            declaration.comments.named(name)
        )
        .expect("String write");
        output.push_str("  value = ");
        format_expression(value, 0, output);
        output.push_str(";\n  source_unit: ");
        format_expression(source_dimension, 0, output);
        output.push_str(" = ");
        format_expression(scale, 0, output);
        write!(
            output,
            ";\n  validity = unconditional;\n  citation = {citation};\n  license = {license};\n}}\n"
        )
        .expect("String write");
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
