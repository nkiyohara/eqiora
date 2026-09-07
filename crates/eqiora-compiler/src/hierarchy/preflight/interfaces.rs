//! Owned signature declarations reuse ordinary Parameter and Port elaboration.
use super::*;
use eqiora_lang::{SignalDirectionSyntax, SignatureItem, SourceAstFactory, VisibilitySyntax};

fn signal(value: &eqiora_lang::FieldDecl, direction: SignalDirectionSyntax) -> PortSyntax {
    PortSyntax::Signal {
        direction,
        value_type: value.value_type().clone(),
        domain: value.domain().map(str::to_owned),
        activation: value.activation().clone(),
    }
}

pub(super) fn component_items(declaration: &ComponentDecl) -> std::sync::Arc<[ComponentItem]> {
    declaration
        .signature()
        .iter()
        .filter_map(|item| match item {
            SignatureItem::Parameter(value) => Some(ComponentItem::Parameter(value.clone())),
            SignatureItem::Port(value) => Some(ComponentItem::Port(value.clone())),
            SignatureItem::PortFamily(value) => Some(ComponentItem::PortFamily(value.clone())),
            SignatureItem::Input(value) | SignatureItem::Output(value) => {
                Some(ComponentItem::Port(
                    SourceAstFactory::component_port(
                        VisibilitySyntax::Public,
                        value.name(),
                        signal(
                            value,
                            if matches!(item, SignatureItem::Input(_)) {
                                SignalDirectionSyntax::Input
                            } else {
                                SignalDirectionSyntax::Output
                            },
                        ),
                        value.range(),
                    )
                    .expect("checked signature endpoint"),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .into()
}

pub(super) fn model_items(declaration: &ModelDecl) -> std::sync::Arc<[Item]> {
    declaration
        .signature()
        .iter()
        .filter_map(|item| match item {
            SignatureItem::Parameter(value) => value.default().map(|default| {
                Item::Parameter(
                    SourceAstFactory::parameter(
                        value.name(),
                        value.value_type().clone(),
                        default.clone(),
                        value.range(),
                    )
                    .expect("checked signature parameter"),
                )
            }),
            SignatureItem::Port(value) => Some(Item::Port(
                SourceAstFactory::port(value.name(), value.syntax().clone(), value.range())
                    .expect("checked signature port"),
            )),
            SignatureItem::Input(value) | SignatureItem::Output(value) => Some(Item::Port(
                SourceAstFactory::port(
                    value.name(),
                    signal(
                        value,
                        if matches!(item, SignatureItem::Input(_)) {
                            SignalDirectionSyntax::Input
                        } else {
                            SignalDirectionSyntax::Output
                        },
                    ),
                    value.range(),
                )
                .expect("checked signature endpoint"),
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into()
}
