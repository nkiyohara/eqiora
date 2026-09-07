use core::fmt::Write;

use super::{format_expression, separate_declaration};
use crate::ast::{ComponentDecl, Document, VisibilitySyntax};

pub(super) fn format_properties(
    document: &Document,
    output: &mut crate::formatter::comments::Output,
    count: &mut usize,
) {
    for dimension in &document.dimensions {
        separate_declaration(output, count);
        super::compile_time::format_dimension(dimension, output);
    }
    for (declaration, (visibility, name, dimension, _)) in document
        .property_contracts
        .iter()
        .zip(document.property_contract_syntax())
    {
        separate_declaration(output, count);
        output.begin(&declaration.comments);
        if visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        writeln!(output, "property contract {name} {{").expect("String write");
        output.push_str("  scalar value: ");
        format_expression(dimension, 0, output);
        output.push_str(";\n}\n");
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
        writeln!(output, "property release {name} implements {contract} {{").expect("String write");
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
        writeln!(output, "material composition {name} {{").expect("String write");
        for (binding, (property, release, _)) in declaration.properties.iter().zip(properties) {
            output.begin(&binding.comments);
            writeln!(output, "  property {property} = {release};").expect("String write");
            output.end();
        }
        output.push_str("}\n");
        output.end();
    }
}

pub(super) fn format_component_requirements(
    component: &ComponentDecl,
    output: &mut crate::formatter::comments::Output,
) {
    for (declaration, (name, contract, _)) in component
        .property_requirements
        .iter()
        .zip(component.property_requirement_syntax())
    {
        output.begin(&declaration.comments);
        writeln!(output, "  public property {name}: {contract};").expect("String write");
        output.end();
    }
}
