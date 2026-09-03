use eqiora_core::Diagnostic;
use eqiora_lang::{ConnectionDecl, Item, ModelDecl, VisibilitySyntax};

use super::{
    Budget, Encoder, MODEL_CONNECTION_ITEM_TAG, encode_container_records, encode_model_item,
    encode_name, encode_visibility,
};

pub(super) fn encode_model(
    declaration: &ModelDecl,
    budget: &mut Budget,
) -> Result<Vec<u8>, Diagnostic> {
    budget.account_members(declaration.items().len(), "model")?;
    let members = encode_container_records(
        declaration.items(),
        budget,
        model_connection,
        encode_model_item,
        MODEL_CONNECTION_ITEM_TAG,
    )?;
    let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| encoder.records(&members))?;
    if declaration.visibility() == VisibilitySyntax::Public {
        encoder.field(3, |encoder| {
            encode_visibility(encoder, declaration.visibility())
        })?;
    }
    encoder.finish()
}

pub(super) fn model_connection(item: &Item) -> Option<&ConnectionDecl> {
    match item {
        Item::Connection(declaration) => Some(declaration),
        _ => None,
    }
}
