use eqiora_core::Diagnostic;
use eqiora_lang::parse;

use crate::diagnostics::source_error;

use super::{
    AnalyzedSourceUnit, CompilationModuleId, ModuleName, ResolvedSourceUnit, resolved_error,
    resolved_source_label,
};

pub(super) fn analyze_source_unit(
    unit: ResolvedSourceUnit,
    max_provenance_path_bytes: usize,
) -> Result<AnalyzedSourceUnit, Vec<Diagnostic>> {
    if unit.file.is_empty() || unit.file.contains('\0') {
        return Err(vec![resolved_error(
            "resolved source paths must be nonempty and NUL-free",
        )]);
    }

    let parse_file = resolved_source_label(unit.module(), &unit.file);
    check_provenance_path(&parse_file, max_provenance_path_bytes)?;
    let document = parse(&parse_file, &unit.source).into_document()?;
    let module = declared_module(&unit, &document, &parse_file)?;
    let provenance_file = resolved_source_label(&module, &unit.file);
    check_provenance_path(&provenance_file, max_provenance_path_bytes)?;

    Ok(AnalyzedSourceUnit {
        module,
        file: provenance_file,
        source_bytes: unit.source.len(),
        document,
    })
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

fn declared_module(
    unit: &ResolvedSourceUnit,
    document: &eqiora_lang::Document,
    file: &str,
) -> Result<CompilationModuleId, Vec<Diagnostic>> {
    let Some((name, range)) = document.module() else {
        return Ok(unit.module.clone());
    };
    let name = ModuleName::new(name.segments())
        .map_err(|error| vec![source_error(error.code(), file, range, error.message())])?;
    let declared = CompilationModuleId::new(unit.module.owner().clone(), name);
    if unit.module_from_host && declared != unit.module {
        return Err(vec![source_error(
            eqiora_core::diagnostic::codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            format!(
                "source declares module `{declared}` but its resolved graph assigns `{}`",
                unit.module
            ),
        )]);
    }
    Ok(declared)
}
