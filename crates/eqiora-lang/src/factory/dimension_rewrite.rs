use crate::ast::{ComponentItem, ConnectorSyntax, Document, DomainSyntax, Expr, Item, PortSyntax};

use super::SourceAstFactory;

impl SourceAstFactory {
    /// Rewrite every physical-dimension expression in one owned source unit.
    ///
    /// This compiler-facing transform deliberately excludes ordinary value
    /// expressions. It lets semantic dimension aliases erase once before the
    /// existing lowerers without exposing mutable declaration fields.
    #[doc(hidden)]
    pub fn rewrite_dimension_expressions(
        document: &mut Document,
        mut rewrite: impl FnMut(&Expr) -> Expr,
    ) {
        for declaration in &mut document.property_contracts {
            declaration.dimension = rewrite(&declaration.dimension);
        }
        for declaration in &mut document.property_releases {
            declaration.source_dimension = rewrite(&declaration.source_dimension);
        }
        for connector in &mut document.connectors {
            rewrite_connector(&mut connector.syntax, &mut rewrite);
        }
        for component in &mut document.components {
            for item in &mut component.items {
                rewrite_component_item(item, &mut rewrite);
            }
        }
        for model in &mut document.models {
            for item in &mut model.items {
                rewrite_item(item, &mut rewrite);
            }
        }
    }
}

fn rewrite_connector(syntax: &mut ConnectorSyntax, rewrite: &mut impl FnMut(&Expr) -> Expr) {
    match syntax {
        ConnectorSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            *across_dimension = rewrite(across_dimension);
            *through_dimension = rewrite(through_dimension);
        }
        ConnectorSyntax::FieldPhysical { trace, flux, .. } => {
            trace.dimension = rewrite(&trace.dimension);
            flux.dimension = rewrite(&flux.dimension);
        }
    }
}

fn rewrite_component_item(item: &mut ComponentItem, rewrite: &mut impl FnMut(&Expr) -> Expr) {
    match item {
        ComponentItem::Parameter(declaration) => {
            declaration.dimension = rewrite(&declaration.dimension);
        }
        ComponentItem::Port(declaration) => rewrite_port(&mut declaration.syntax, rewrite),
        ComponentItem::PortFamily(declaration) => {
            rewrite_port(&mut declaration.port.syntax, rewrite);
        }
        ComponentItem::FieldSlot(declaration) => {
            declaration.dimension = rewrite(&declaration.dimension);
        }
        ComponentItem::Field(declaration) => {
            declaration.dimension = rewrite(&declaration.dimension);
        }
        ComponentItem::Instance(_)
        | ComponentItem::Support(_)
        | ComponentItem::Representation(_)
        | ComponentItem::Clock(_)
        | ComponentItem::Relation(_)
        | ComponentItem::RelationFamily(_)
        | ComponentItem::Connection(_)
        | ComponentItem::BoundaryConnection(_) => {}
    }
}

fn rewrite_item(item: &mut Item, rewrite: &mut impl FnMut(&Expr) -> Expr) {
    match item {
        Item::Domain(declaration) => {
            if let DomainSyntax::ScalarPhysical {
                across_dimension,
                through_dimension,
            } = &mut declaration.syntax
            {
                *across_dimension = rewrite(across_dimension);
                *through_dimension = rewrite(through_dimension);
            }
        }
        Item::Field(declaration) => declaration.dimension = rewrite(&declaration.dimension),
        Item::Parameter(declaration) => declaration.dimension = rewrite(&declaration.dimension),
        Item::Let(declaration) => {
            if let Some(dimension) = &mut declaration.dimension {
                *dimension = rewrite(dimension);
            }
        }
        Item::Port(declaration) => rewrite_port(&mut declaration.syntax, rewrite),
        Item::Representation(_)
        | Item::Clock(_)
        | Item::Relation(_)
        | Item::Connection(_)
        | Item::BoundaryConnection(_)
        | Item::Boundary(_)
        | Item::Instance(_) => {}
    }
}

fn rewrite_port(syntax: &mut PortSyntax, rewrite: &mut impl FnMut(&Expr) -> Expr) {
    match syntax {
        PortSyntax::Signal { dimension, .. } | PortSyntax::ConservingMarker { dimension } => {
            *dimension = rewrite(dimension);
        }
        _ => {}
    }
}
