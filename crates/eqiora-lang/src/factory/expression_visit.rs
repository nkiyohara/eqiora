//! Scoped traversal of authored value expressions for lexical resolution.
use crate::{ComponentItem, Document, Expr, ExprKind, Item, SignatureItem};

impl super::SourceAstFactory {
    /// Visit value expressions in lexical container scope, after their children.
    #[doc(hidden)]
    pub fn visit_expressions(
        document: &mut Document,
        mut visit: impl FnMut(Option<&str>, &mut Expr),
    ) {
        for release in &mut document.property_releases {
            expression(None, &mut release.source_value, &mut visit);
        }
        for component in &mut document.components {
            let scope = Some(component.name.as_str());
            signature(&mut component.signature, scope, &mut visit);
            for item in &mut component.items {
                match item {
                    ComponentItem::Parameter(value) => {
                        if let Some(value) = &mut value.default {
                            expression(scope, value, &mut visit);
                        }
                    }
                    ComponentItem::Let(value) | ComponentItem::IndexSet(value) => {
                        expression(scope, &mut value.value, &mut visit)
                    }
                    ComponentItem::Relation(value) => {
                        equations(scope, &mut value.equations, &mut visit)
                    }
                    ComponentItem::Initial(value) => {
                        equations(scope, &mut value.equations, &mut visit)
                    }
                    ComponentItem::RelationFamily(value) => {
                        equations(scope, &mut value.relation.equations, &mut visit)
                    }
                    ComponentItem::Instance(value) => {
                        for binding in &mut value.bindings {
                            expression(scope, &mut binding.value, &mut visit);
                        }
                    }
                    ComponentItem::Connection(value) => {
                        for endpoint in &mut value.ports {
                            expression(scope, endpoint, &mut visit);
                        }
                    }
                    _ => {}
                }
            }
        }
        for model in &mut document.models {
            let scope = Some(model.name.as_str());
            signature(&mut model.signature, scope, &mut visit);
            for item in &mut model.items {
                match item {
                    Item::Parameter(value) => expression(scope, &mut value.value, &mut visit),
                    Item::Let(value) | Item::IndexSet(value) => {
                        expression(scope, &mut value.value, &mut visit)
                    }
                    Item::Relation(value) => equations(scope, &mut value.equations, &mut visit),
                    Item::RelationFamily(value) => {
                        equations(scope, &mut value.relation.equations, &mut visit)
                    }
                    Item::Initial(value) => equations(scope, &mut value.equations, &mut visit),
                    Item::Instance(value) => {
                        for binding in &mut value.bindings {
                            expression(scope, &mut binding.value, &mut visit);
                        }
                    }
                    Item::Connection(value) => {
                        for endpoint in &mut value.ports {
                            expression(scope, endpoint, &mut visit);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
fn signature(
    values: &mut [SignatureItem],
    scope: Option<&str>,
    visit: &mut impl FnMut(Option<&str>, &mut Expr),
) {
    for item in values {
        if let SignatureItem::Parameter(parameter) = item
            && let Some(value) = &mut parameter.default
        {
            expression(scope, value, visit);
        }
    }
}
fn equations(
    scope: Option<&str>,
    values: &mut [crate::Equation],
    visit: &mut impl FnMut(Option<&str>, &mut Expr),
) {
    for equation in values {
        expression(scope, &mut equation.left, visit);
        expression(scope, &mut equation.right, visit);
    }
}
fn expression(
    scope: Option<&str>,
    value: &mut Expr,
    visit: &mut impl FnMut(Option<&str>, &mut Expr),
) {
    match &mut value.kind {
        ExprKind::Unary { value, .. }
        | ExprKind::Member { value, .. }
        | ExprKind::Reduction { value, .. } => expression(scope, value, visit),
        ExprKind::Binary { left, right, .. } => {
            expression(scope, left, visit);
            expression(scope, right, visit);
        }
        ExprKind::Index { value, index } => {
            expression(scope, value, visit);
            expression(scope, index, visit);
        }
        ExprKind::Array(values)
        | ExprKind::Call {
            arguments: values, ..
        } => {
            for value in values {
                expression(scope, value, visit);
            }
        }
        _ => {}
    }
    visit(scope, value);
}
