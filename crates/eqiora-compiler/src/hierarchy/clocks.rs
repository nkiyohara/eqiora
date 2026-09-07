//! Exact declared clocks projected through existing definition and occurrence scopes.
use super::scope::{Scope, SymbolKind};
use eqiora_lang::{
    ComponentDecl, ComponentItem, ExprKind, InstanceDecl, Item, ModelDecl, SignatureItem,
};
use eqiora_schema::kernel::RationalTime;

pub(super) fn component(
    file: &str,
    declaration: &ComponentDecl,
    name: &str,
) -> Option<Option<RationalTime>> {
    if declaration
        .signature()
        .iter()
        .any(|item| matches!(item,SignatureItem::Clock(value) if value.name()==name))
    {
        return Some(None);
    }
    declaration.items().iter().find_map(|item| match item {
        ComponentItem::Clock(value) if value.name() == name => {
            crate::units::lower_clock(file, value.period(), value.phase())
                .ok()
                .map(|(period, _)| Some(period))
        }
        _ => None,
    })
}
pub(super) fn model(
    file: &str,
    declaration: &ModelDecl,
    name: &str,
) -> Option<Option<RationalTime>> {
    if declaration
        .signature()
        .iter()
        .any(|item| matches!(item,SignatureItem::Clock(value) if value.name()==name))
    {
        return Some(None);
    }
    declaration.items().iter().find_map(|item| match item {
        Item::Clock(value) if value.name() == name => {
            crate::units::lower_clock(file, value.period(), value.phase())
                .ok()
                .map(|(period, _)| Some(period))
        }
        _ => None,
    })
}
pub(super) fn occurrence(scope: &Scope, name: &str) -> Option<Option<RationalTime>> {
    match scope.symbol(name)?.kind {
        SymbolKind::Clock(period) => Some(Some(period)),
        _ => None,
    }
}
pub(super) fn component_occurrence(
    file: &str,
    declaration: &ComponentDecl,
    instance: &InstanceDecl,
    parent: &Scope,
    name: &str,
) -> Option<Option<RationalTime>> {
    match component(file, declaration, name)? {
        Some(period) => Some(Some(period)),
        None => instance
            .bindings()
            .iter()
            .find(|binding| binding.name() == name)
            .and_then(|binding| match binding.value().kind() {
                ExprKind::Name(target) => occurrence(parent, target),
                _ => None,
            }),
    }
}
