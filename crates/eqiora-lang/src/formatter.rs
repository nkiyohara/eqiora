//! One canonical source style derived from the recovered syntax tree.
//! Declaration-specific modules keep the top-level traversal readable.
use core::fmt::Write;

mod cartesian;
mod compile_time;
mod document;
mod formulation;
mod helpers;
mod property;
mod relation;

use crate::ast::{
    BinaryOp, BoundaryConnectionDecl, BoundaryFamilyBinderSyntax, BoundaryPairingSyntax,
    BoundaryPortReferenceSyntax, BoundarySideSyntax, ClockDecl, ComponentItem,
    ComponentPortFamilyDecl, ConnectionDecl, ConnectionSyntax, ConnectorSyntax, Document,
    DomainSyntax, Expr, ExprKind, FieldDecl, FrameSyntax, InstanceDecl, Item, PortSyntax,
    PureOperatorBinaryOp, PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind,
    PureValueClassSyntax, RepresentationSyntax, SignalDirectionSyntax, SupportSlotSyntax, UnaryOp,
    ValueShapeSyntax, VisibilitySyntax,
};
use cartesian::format_cartesian_coordinate;
use compile_time::{format_let, format_parameter};
use formulation::format_component;
use helpers::{
    format_boundary_port_selector, format_name_paths, format_scalar_physical, write_indent,
};
use property::{format_component_requirements, format_properties};
use relation::{format_relation, format_relation_family};

/// Format a syntax tree, canonicalizing comment-free documents and preserving commented source.
#[must_use]
pub fn format(document: &Document) -> String {
    if let Some(source) = document.retained_source() {
        return source.to_owned();
    }
    let mut output = String::new();
    let mut declaration_count = document::format_header(document, &mut output);
    format_properties(document, &mut output, &mut declaration_count);
    for connector in &document.connectors {
        separate_declaration(&mut output, &mut declaration_count);
        if connector.visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        write!(output, "connector {} = ", connector.name).expect("String write");
        match &connector.syntax {
            ConnectorSyntax::ScalarPhysical {
                across_dimension,
                through_dimension,
            } => format_scalar_physical(across_dimension, through_dimension, &mut output),
            ConnectorSyntax::FieldPhysical {
                trace,
                flux,
                shape,
                frame,
                pairing,
            } => {
                output.push_str("field_physical(\n  trace = ");
                write!(output, "{}: ", trace.name).expect("String write");
                format_expression(&trace.dimension, 0, &mut output);
                output.push_str(",\n  flux = ");
                write!(output, "{}: ", flux.name).expect("String write");
                format_expression(&flux.dimension, 0, &mut output);
                output.push_str(",\n  shape = ");
                format_value_shape(shape, &mut output);
                output.push_str(",\n  frame = ");
                output.push_str(match frame {
                    FrameSyntax::Invariant => "invariant",
                    FrameSyntax::Spatial => "spatial",
                });
                output.push_str(",\n  pairing = ");
                output.push_str(match pairing {
                    BoundaryPairingSyntax::EuclideanBoundaryDuality => "euclidean_boundary_duality",
                });
                output.push_str("\n)");
            }
        }
        output.push_str(";\n");
    }
    for operator in &document.pure_operators {
        separate_declaration(&mut output, &mut declaration_count);
        format_pure_operator(operator, &mut output);
    }
    for component in &document.components {
        separate_declaration(&mut output, &mut declaration_count);
        format_component(component, &mut output);
    }
    for model in &document.models {
        separate_declaration(&mut output, &mut declaration_count);
        if model.visibility == VisibilitySyntax::Public {
            output.push_str("public ");
        }
        writeln!(output, "model {} {{", model.name).expect("String writes cannot fail");
        for item in &model.items {
            format_item(item, 2, &mut output);
        }
        output.push_str("}\n");
    }
    output
}
fn format_pure_operator(declaration: &PureOperatorDecl, output: &mut String) {
    if declaration.visibility == VisibilitySyntax::Public {
        output.push_str("public ");
    }
    write!(output, "pure operator {}(", declaration.name).expect("String write");
    for (index, formal) in declaration.formals.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "{}: ", formal.name).expect("String write");
        format_pure_value_class(&formal.value_class, output);
    }
    output.push_str(") -> ");
    format_pure_value_class(&declaration.result, output);
    output.push_str(" = ");
    format_pure_operator_expression(&declaration.body, 0, output);
    output.push_str(";\n");
}
fn format_pure_value_class(value_class: &PureValueClassSyntax, output: &mut String) {
    match value_class {
        PureValueClassSyntax::Scalar => output.push_str("scalar"),
        PureValueClassSyntax::Spatial { rank } => {
            write!(output, "spatial[{}]", rank.value).expect("String write");
        }
    }
}
fn format_pure_operator_expression(
    expression: &PureOperatorExpr,
    parent_precedence: u8,
    output: &mut String,
) {
    let precedence = pure_operator_expression_precedence(expression);
    let parenthesize = precedence < parent_precedence;
    if parenthesize {
        output.push('(');
    }
    match &expression.kind {
        PureOperatorExprKind::Rational {
            numerator,
            denominator,
        } => {
            write!(
                output,
                "rational({}, {})",
                numerator.value, denominator.value
            )
            .expect("String write");
        }
        PureOperatorExprKind::Component {
            formal,
            result_axes,
            ..
        } => {
            write!(output, "component({formal}").expect("String write");
            for axis in result_axes {
                write!(output, ", {}", axis.value).expect("String write");
            }
            output.push(')');
        }
        PureOperatorExprKind::Delta {
            left_axis,
            right_axis,
        } => {
            write!(output, "delta({}, {})", left_axis.value, right_axis.value)
                .expect("String write");
        }
        PureOperatorExprKind::Neg(value) => {
            output.push('-');
            format_pure_operator_expression(value, precedence, output);
        }
        PureOperatorExprKind::Binary { op, left, right } => {
            let (symbol, left_precedence, right_precedence) = match op {
                PureOperatorBinaryOp::Add => (" + ", precedence, precedence + 1),
                PureOperatorBinaryOp::Sub => (" - ", precedence, precedence + 1),
                PureOperatorBinaryOp::Mul => (" * ", precedence, precedence + 1),
            };
            format_pure_operator_expression(left, left_precedence, output);
            output.push_str(symbol);
            format_pure_operator_expression(right, right_precedence, output);
        }
    }
    if parenthesize {
        output.push(')');
    }
}

fn pure_operator_expression_precedence(expression: &PureOperatorExpr) -> u8 {
    match &expression.kind {
        PureOperatorExprKind::Binary {
            op: PureOperatorBinaryOp::Add | PureOperatorBinaryOp::Sub,
            ..
        } => 1,
        PureOperatorExprKind::Binary {
            op: PureOperatorBinaryOp::Mul,
            ..
        } => 3,
        PureOperatorExprKind::Neg(_) => 5,
        PureOperatorExprKind::Rational { .. }
        | PureOperatorExprKind::Component { .. }
        | PureOperatorExprKind::Delta { .. } => 7,
    }
}

fn separate_declaration(output: &mut String, declaration_count: &mut usize) {
    output.push_str(if *declaration_count == 0 { "" } else { "\n" });
    *declaration_count += 1;
}

fn format_component_item(item: &ComponentItem, indent: usize, output: &mut String) {
    match item {
        ComponentItem::Parameter(declaration) => {
            write_indent(output, indent);
            if declaration.visibility == VisibilitySyntax::Public {
                output.push_str("public ");
            }
            write!(output, "parameter {}: ", declaration.name).expect("String write");
            format_expression(&declaration.dimension, 0, output);
            if let Some(default) = &declaration.default {
                output.push_str(" = ");
                format_expression(default, 0, output);
            }
            output.push_str(";\n");
        }
        ComponentItem::Port(declaration) => {
            write_indent(output, indent);
            if declaration.visibility == VisibilitySyntax::Public {
                output.push_str("public ");
            }
            write!(output, "port {}: ", declaration.name).expect("String write");
            format_port_syntax(&declaration.syntax, output);
            output.push_str(";\n");
        }
        ComponentItem::PortFamily(declaration) => {
            format_component_port_family(declaration, indent, output);
        }
        ComponentItem::Support(declaration) => {
            write_indent(output, indent);
            if declaration.visibility == VisibilitySyntax::Public {
                output.push_str("public ");
            }
            write!(output, "support {}: ", declaration.name).expect("String write");
            match &declaration.syntax {
                SupportSlotSyntax::Volume { ambient_dimension } => {
                    write!(output, "volume(ambient_dimension = {ambient_dimension})")
                        .expect("String write");
                }
                SupportSlotSyntax::Boundary { parent } => {
                    write!(output, "boundary(parent = {parent})").expect("String write");
                }
                SupportSlotSyntax::CompleteExterior { parent } => {
                    write!(output, "complete_exterior(parent = {parent})").expect("String write");
                }
            }
            output.push_str(";\n");
        }
        ComponentItem::FieldSlot(declaration) => {
            write_indent(output, indent);
            write!(
                output,
                "public field slot {} on {} as continuum: ",
                declaration.name, declaration.support
            )
            .expect("String write");
            format_expression(&declaration.dimension, 0, output);
            if let Some(shape) = &declaration.shape {
                output.push_str(" shape ");
                format_value_shape(shape, output);
            }
            output.push_str(";\n");
        }
        ComponentItem::Representation(declaration) => {
            format_representation(declaration, indent, output);
        }
        ComponentItem::Field(declaration) => format_field(declaration, indent, output),
        ComponentItem::Clock(declaration) => format_clock(declaration, indent, output),
        ComponentItem::Relation(declaration) => format_relation(declaration, indent, output),
        ComponentItem::RelationFamily(declaration) => {
            format_relation_family(declaration, indent, output);
        }
        ComponentItem::Connection(declaration) => format_connection(declaration, indent, output),
        ComponentItem::BoundaryConnection(declaration) => {
            format_boundary_connection(declaration, indent, output);
        }
        ComponentItem::Instance(declaration) => format_instance(declaration, indent, output),
    }
}

fn format_item(item: &Item, indent: usize, output: &mut String) {
    match item {
        Item::Domain(declaration) => {
            write_indent(output, indent);
            write!(output, "domain {} = ", declaration.name).expect("String write");
            match &declaration.syntax {
                DomainSyntax::CartesianBox(bounds) => {
                    output.push_str("box(");
                    for (index, (lower, upper)) in bounds.iter().enumerate() {
                        if index != 0 {
                            output.push_str(", ");
                        }
                        format_cartesian_coordinate(lower, output);
                        output.push_str(", ");
                        format_cartesian_coordinate(upper, output);
                    }
                    output.push(')');
                }
                DomainSyntax::Boundary { parent, axis, side } => {
                    write!(output, "boundary({parent}, axis = {axis}, side = ")
                        .expect("String write");
                    output.push_str(match side {
                        BoundarySideSyntax::Lower => "lower",
                        BoundarySideSyntax::Upper => "upper",
                    });
                    output.push(')');
                }
                DomainSyntax::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                } => format_scalar_physical(across_dimension, through_dimension, output),
            }
            output.push_str(";\n");
        }
        Item::Representation(declaration) => {
            format_representation(declaration, indent, output);
        }
        Item::Field(declaration) => format_field(declaration, indent, output),
        Item::Parameter(declaration) => format_parameter(declaration, indent, output),
        Item::Let(declaration) => format_let(declaration, indent, output),
        Item::Port(declaration) => {
            write_indent(output, indent);
            write!(output, "port {}: ", declaration.name).expect("String write");
            format_port_syntax(&declaration.syntax, output);
            output.push_str(";\n");
        }
        Item::Clock(declaration) => format_clock(declaration, indent, output),
        Item::Relation(declaration) => format_relation(declaration, indent, output),
        Item::Connection(declaration) => format_connection(declaration, indent, output),
        Item::BoundaryConnection(declaration) => {
            format_boundary_connection(declaration, indent, output);
        }
        Item::Boundary(declaration) => {
            write_indent(output, indent);
            output.push_str("boundary ");
            format_name_paths(&declaration.ports, output);
            output.push_str(";\n");
        }
        Item::Instance(declaration) => format_instance(declaration, indent, output),
    }
}

fn format_representation(
    declaration: &crate::ast::RepresentationDecl,
    indent: usize,
    output: &mut String,
) {
    write_indent(output, indent);
    write!(output, "representation {} = ", declaration.name).expect("String write");
    match declaration.syntax {
        RepresentationSyntax::Continuum => output.push_str("continuum"),
    }
    output.push_str(";\n");
}

fn format_field(declaration: &FieldDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    write!(output, "field {}", declaration.name).expect("String write");
    if let (Some(domain), Some(representation)) = (&declaration.domain, &declaration.representation)
    {
        write!(output, " on {domain} as {representation}").expect("String write");
    }
    output.push_str(": ");
    format_expression(&declaration.dimension, 0, output);
    if let Some(shape) = &declaration.shape {
        output.push_str(" shape ");
        format_value_shape(shape, output);
    }
    if let Some(initial) = declaration.initial {
        writeln!(output, " = {};", format_number(initial)).expect("String write");
    } else {
        output.push_str(";\n");
    }
}

fn format_value_shape(shape: &ValueShapeSyntax, output: &mut String) {
    match shape {
        ValueShapeSyntax::Scalar => output.push_str("[]"),
        ValueShapeSyntax::Exact(extents) => {
            output.push('[');
            for (index, extent) in extents.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                write!(output, "{extent}").expect("String write");
            }
            output.push(']');
        }
        ValueShapeSyntax::SpatialVector => output.push_str("spatial_vector"),
    }
}

fn format_port_syntax(syntax: &PortSyntax, output: &mut String) {
    match syntax {
        PortSyntax::Signal {
            direction: SignalDirectionSyntax::Input,
            dimension,
        } => {
            output.push_str("signal input ");
            format_expression(dimension, 0, output);
        }
        PortSyntax::Signal {
            direction: SignalDirectionSyntax::Output,
            dimension,
        } => {
            output.push_str("signal output ");
            format_expression(dimension, 0, output);
        }
        PortSyntax::ConservingMarker { dimension } => {
            output.push_str("conserving ");
            format_expression(dimension, 0, output);
        }
        PortSyntax::ScalarPhysical { domain } => {
            write!(output, "conserving on {domain}").expect("String write");
        }
        PortSyntax::ScalarPhysicalConnector { connector } => {
            write!(output, "conserving on {connector}").expect("String write");
        }
        PortSyntax::FieldPhysical { connector, support } => {
            write!(output, "conserving {connector} over {support}").expect("String write");
        }
    }
}

fn format_component_port_family(
    declaration: &ComponentPortFamilyDecl,
    indent: usize,
    output: &mut String,
) {
    let port = &declaration.port;
    write_indent(output, indent);
    if port.visibility == VisibilitySyntax::Public {
        output.push_str("public ");
    }
    write!(output, "port {}", port.name).expect("String write");
    format_boundary_family_binder(&declaration.binder, output);
    output.push_str(": ");
    format_port_syntax(&port.syntax, output);
    output.push_str(";\n");
}

fn format_boundary_family_binder(binder: &BoundaryFamilyBinderSyntax, output: &mut String) {
    write!(output, "[{} in {}]", binder.member, binder.set).expect("String write");
}

fn format_clock(declaration: &ClockDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    writeln!(
        output,
        "clock {} = periodic(period = {} / {}, phase = {} / {});",
        declaration.name,
        declaration.period.numerator,
        declaration.period.denominator,
        declaration.phase.numerator,
        declaration.phase.denominator
    )
    .expect("String write");
}

fn format_connection(declaration: &ConnectionDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    match declaration.syntax {
        ConnectionSyntax::Signal => {
            output.push_str("connect signal ");
            if let Some((source, targets)) = declaration.ports.split_first() {
                write!(output, "{source} -> ").expect("String write");
                format_name_paths(targets, output);
            }
        }
        ConnectionSyntax::Conserving => {
            output.push_str("connect conserving ");
            format_name_paths(&declaration.ports, output);
        }
        ConnectionSyntax::SpatialPeriodic => {
            output.push_str("connect periodic ");
            format_name_paths(&declaration.ports, output);
        }
    }
    output.push_str(";\n");
}

fn format_boundary_connection(
    declaration: &BoundaryConnectionDecl,
    indent: usize,
    output: &mut String,
) {
    write_indent(output, indent);
    output.push_str(match declaration.syntax {
        ConnectionSyntax::Conserving => "connect conserving",
        ConnectionSyntax::SpatialPeriodic => "connect periodic",
        ConnectionSyntax::Signal => "connect signal",
    });
    if let Some(binder) = &declaration.binder {
        output.push(' ');
        format_boundary_family_binder(binder, output);
    }
    output.push(' ');
    for (index, port) in declaration.ports.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        format_boundary_port_reference(port, output);
    }
    output.push_str(";\n");
}

fn format_boundary_port_reference(reference: &BoundaryPortReferenceSyntax, output: &mut String) {
    write!(output, "{}", reference.port).expect("String write");
    if let Some(selector) = &reference.selector {
        format_boundary_port_selector(selector, output);
    }
}

fn format_instance(declaration: &InstanceDecl, indent: usize, output: &mut String) {
    write_indent(output, indent);
    write!(
        output,
        "instance {}: {}",
        declaration.name, declaration.definition
    )
    .expect("String write");
    if declaration.has_bindings() {
        output.push('(');
        let mut separated = false;
        for (index, binding) in declaration.bindings.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            write!(output, "{} = ", binding.parameter).expect("String write");
            format_expression(&binding.value, 0, output);
            separated = true;
        }
        for binding in &declaration.support_bindings {
            if separated {
                output.push_str(", ");
            }
            write!(output, "support {} = {}", binding.slot, binding.target).expect("String write");
            separated = true;
        }
        for binding in &declaration.boundary_set_bindings {
            if separated {
                output.push_str(", ");
            }
            write!(output, "support {} = boundaries(", binding.slot).expect("String write");
            for (index, member) in binding.members.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                output.push_str(&member.target);
            }
            output.push(')');
            separated = true;
        }
        for (index, binding) in declaration.field_bindings.iter().enumerate() {
            if separated || index != 0 {
                output.push_str(", ");
            }
            write!(output, "field {} = {}", binding.slot, binding.target).expect("String write");
            separated = true;
        }
        for binding in &declaration.property_bindings {
            if separated {
                output.push_str(", ");
            }
            write!(
                output,
                "property {} = {}",
                binding.property, binding.release
            )
            .expect("String write");
            separated = true;
        }
        if let Some(material) = declaration.material_binding_syntax() {
            if separated {
                output.push_str(", ");
            }
            write!(output, "material = {material}").expect("String write");
        }
        output.push(')');
    }
    output.push_str(";\n");
}

fn format_expression(expression: &Expr, parent_precedence: u8, output: &mut String) {
    let precedence = expression_precedence(expression);
    let parenthesize = precedence < parent_precedence;
    if parenthesize {
        output.push('(');
    }
    match &expression.kind {
        ExprKind::Number(value) => output.push_str(&format_number(*value)),
        ExprKind::Quantity { value, unit } => {
            output.push_str(&format_number(*value));
            output.push_str(" [");
            format_expression(unit, 0, output);
            output.push(']');
        }
        ExprKind::Name(name) => output.push_str(name),
        ExprKind::Path(path) => write!(output, "{path}").expect("String write"),
        ExprKind::BoundaryPortSelection { port, selector } => {
            write!(output, "{port}").expect("String write");
            format_boundary_port_selector(selector, output);
        }
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => {
            output.push('-');
            format_expression(value, precedence, output);
        }
        ExprKind::Binary { op, left, right } => {
            let (symbol, left_precedence, right_precedence) = match op {
                BinaryOp::Add => (" + ", precedence, precedence + 1),
                BinaryOp::Sub => (" - ", precedence, precedence + 1),
                BinaryOp::Mul => (" * ", precedence, precedence + 1),
                BinaryOp::Div => (" / ", precedence, precedence + 1),
                BinaryOp::Pow => (" ^ ", precedence + 1, precedence),
            };
            format_expression(left, left_precedence, output);
            output.push_str(symbol);
            format_expression(right, right_precedence, output);
        }
        ExprKind::Call { callee, arguments } => {
            write!(output, "{callee}").expect("String write");
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                format_expression(argument, 0, output);
            }
            output.push(')');
        }
    }
    if parenthesize {
        output.push(')');
    }
}

fn expression_precedence(expression: &Expr) -> u8 {
    match &expression.kind {
        ExprKind::Binary {
            op: BinaryOp::Add | BinaryOp::Sub,
            ..
        } => 1,
        ExprKind::Binary {
            op: BinaryOp::Mul | BinaryOp::Div,
            ..
        } => 3,
        ExprKind::Binary {
            op: BinaryOp::Pow, ..
        } => 7,
        ExprKind::Unary { .. } => 9,
        ExprKind::Number(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Name(_)
        | ExprKind::Path(_)
        | ExprKind::BoundaryPortSelection { .. }
        | ExprKind::Call { .. } => 11,
    }
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests;
