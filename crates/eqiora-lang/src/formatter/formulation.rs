//! Canonical formatting for authored mathematical formulations.

use core::fmt::Write;

use crate::ast::formulation::FormulationDecl;
use crate::ast::{ComponentDecl, VisibilitySyntax};

use super::{
    format_component_item, format_component_requirements, format_expression, write_indent,
};

pub(super) fn format_component(component: &ComponentDecl, output: &mut String) {
    if component.visibility == VisibilitySyntax::Public {
        output.push_str("public ");
    }
    writeln!(output, "component {} {{", component.name).expect("String write");
    format_component_requirements(component, output);
    for item in &component.items {
        format_component_item(item, 2, output);
    }
    for formulation in &component.formulations {
        format_formulation(formulation, 2, output);
    }
    output.push_str("}\n");
}

pub(super) fn format_formulation(
    declaration: &FormulationDecl,
    indent: usize,
    output: &mut String,
) {
    write_indent(output, indent);
    output.push_str("form ");
    output.push_str("primal");
    writeln!(output, " for {} {{", declaration.relation).expect("String write");
    write_indent(output, indent + 2);
    format_expression(&declaration.left, 0, output);
    output.push_str(" = ");
    format_expression(&declaration.right, 0, output);
    output.push_str(";\n");
    write_indent(output, indent);
    output.push_str("}\n");
}

#[cfg(test)]
mod tests {
    use crate::{format, parse};

    #[test]
    fn primal_form_has_one_canonical_roundtrip() {
        let source = "component D{public support region:volume(ambient_dimension=2);representation s=continuum;field u on region as s:1=0;relation balance continuous on region{-div(grad(u))=f;}form primal for balance{integrate(region,dot(grad(test(u)),grad(u)))=integrate(region,test(u)*f);}}";
        let first = parse("form.eqi", source).into_document().unwrap();
        let formatted = format(&first);
        let second = parse("form.eqi", &formatted).into_document().unwrap();

        assert_eq!(format(&second), formatted);
        assert!(formatted.contains("form primal for balance {\n"));
        assert!(formatted.contains(
            "integrate(region, dot(grad(test(u)), grad(u))) = integrate(region, test(u) * f);"
        ));
    }
}
