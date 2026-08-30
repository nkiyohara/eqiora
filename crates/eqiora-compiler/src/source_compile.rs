//! Source parsing and dispatch to flat or hierarchical lowering.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Item, parse};

use crate::diagnostics::source_error;
use crate::lower::{CompiledModel, lower_model};

/// Parse and type-lower every model in one source file.
///
/// # Errors
/// Returns accumulated parser diagnostics, or type/lowering diagnostics from
/// every model that could be independently checked.
pub fn compile(file: &str, source: &str) -> Result<Vec<CompiledModel>, Vec<Diagnostic>> {
    let document = parse(file, source).into_compilation_document()?;
    if let Some(component) = document
        .components()
        .iter()
        .find(|component| component.formulations().len() != 0)
    {
        let range = component
            .formulations()
            .next()
            .expect("non-empty authored forms")
            .3;
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            "authored Component formulations require fresh external-Geometry component compilation",
        )]);
    }
    let has_hierarchy = !document.connectors().is_empty()
        || !document.components().is_empty()
        || !document.pure_operators().is_empty()
        || document.models().iter().any(|model| {
            model
                .items()
                .iter()
                .any(|item| matches!(item, Item::Instance(_) | Item::Let(_)))
        });
    if has_hierarchy {
        return crate::hierarchy::compile_hierarchy(file, source.len(), &document);
    }
    let mut compiled = Vec::new();
    let mut diagnostics = Vec::new();
    for model in document.models() {
        match lower_model(file, model) {
            Ok(value) => compiled.push(value),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(compiled)
    } else {
        Err(diagnostics)
    }
}
