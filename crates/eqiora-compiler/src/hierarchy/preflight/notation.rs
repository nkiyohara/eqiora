//! Source-indexed presentation metadata, kept outside physical identity.
use eqiora_lang::{Document, Notation};
use std::collections::BTreeMap;

pub(super) fn index(file: &str, document: &Document) -> BTreeMap<(String, u32, u32), Notation> {
    document
        .notations()
        .map(|(range, notation)| {
            (
                (file.to_owned(), range.start(), range.end()),
                notation.clone(),
            )
        })
        .collect()
}
