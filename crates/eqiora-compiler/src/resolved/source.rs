use eqiora_core::Diagnostic;
use eqiora_lang::parse;

use super::{AnalyzedSourceUnit, ResolvedSourceUnit, resolved_error, resolved_source_label};

pub(super) fn analyze_source_unit(
    unit: ResolvedSourceUnit,
    max_provenance_path_bytes: usize,
) -> Result<AnalyzedSourceUnit, Vec<Diagnostic>> {
    if unit.file.is_empty() || unit.file.contains('\0') {
        return Err(vec![resolved_error(
            "resolved source paths must be nonempty and NUL-free",
        )]);
    }

    let parse_file = source_label(&unit);
    check_provenance_path(&parse_file, max_provenance_path_bytes)?;
    let document = match &unit.authored {
        Some(module) => module.document().clone(),
        None => parse(&parse_file, &unit.source).into_document()?,
    };
    let module = unit.module;
    let provenance_file = parse_file;
    check_provenance_path(&provenance_file, max_provenance_path_bytes)?;

    Ok(AnalyzedSourceUnit {
        native: unit.authored.map(std::sync::Arc::new),
        module,
        file: provenance_file,
        source_bytes: unit.input_bytes,
        authored_document: std::sync::Arc::new(document.clone()),
        document,
    })
}

pub(super) fn source_label(unit: &ResolvedSourceUnit) -> String {
    if unit
        .authored
        .as_ref()
        .is_some_and(|module| module.source_file().is_none())
    {
        format!("<module:{}>", unit.module())
    } else {
        resolved_source_label(unit.module(), &unit.file)
    }
}

pub(super) fn native_diagnostics(
    modules: &[(String, eqiora_lang::Module)],
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let module = diagnostic
                .source_span()
                .and_then(|span| modules.iter().find(|(file, _)| file == &span.file));
            module.map_or_else(
                || diagnostic.clone(),
                |(_, module)| crate::diagnostics::native_diagnostic(module, diagnostic.clone()),
            )
        })
        .collect()
}

fn check_provenance_path(path: &str, limit: usize) -> Result<(), Vec<Diagnostic>> {
    if path.len() > limit {
        return Err(vec![resolved_error(format!(
            "package-qualified source path requires {} bytes, exceeding the {limit} byte provenance-path limit",
            path.len()
        ))]);
    }
    Ok(())
}
