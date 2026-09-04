use crate::diagnostics::{source_error, stable_sort};

use super::*;

/// Parse and globally analyze every source unit of an exact resolved graph.
///
/// # Errors
/// Returns all parser and global namespace diagnostics together. Analysis
/// creates no graph transaction and performs no package I/O.
pub fn analyze_resolved_hierarchy(
    input: ResolvedHierarchyInput,
) -> Result<AnalyzedResolvedHierarchy, Vec<Diagnostic>> {
    analyze_resolved_hierarchy_with_cancellation(input, || false)
        .map(|analysis| analysis.expect("non-cancellable analysis produces a result"))
}

pub(super) fn analyze_resolved_hierarchy_with_cancellation(
    input: ResolvedHierarchyInput,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Option<AnalyzedResolvedHierarchy>, Vec<Diagnostic>> {
    if is_cancelled() {
        return Ok(None);
    }
    preflight_resolved_hierarchy(
        input.units.iter().map(|unit| unit.source.len()),
        input.aliases.len(),
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    let limits = HierarchyLimits::default();
    let mut diagnostics = Vec::new();
    let mut units = Vec::new();
    for unit in input.units {
        if is_cancelled() {
            return Ok(None);
        }
        match source::analyze_source_unit(unit, limits.provenance.max_source_path_bytes) {
            Ok(unit) => units.push(unit),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if is_cancelled() {
        return Ok(None);
    }

    let authored_imports = units.iter().try_fold(0_usize, |count, unit| {
        count.checked_add(unit.document.imports().len())
    });
    let alias_count = authored_imports.and_then(|count| input.aliases.len().checked_add(count));
    if alias_count.is_none_or(|count| count > MAX_ALIASES) {
        diagnostics.push(resolved_error(format!(
            "resolved hierarchy exceeds the {MAX_ALIASES} direct-alias limit"
        )));
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }

    let mut aliases = input.aliases;
    aliases
        .try_reserve(authored_imports.expect("checked authored import count"))
        .map_err(|_| vec![resolved_error("cannot reserve authored module imports")])?;
    for unit in &units {
        if is_cancelled() {
            return Ok(None);
        }
        for (import_module, import_alias, import_range) in unit.document.imports() {
            let module = match ModuleName::new(import_module.segments()) {
                Ok(module) => module,
                Err(error) => {
                    diagnostics.push(source_error(
                        error.code(),
                        &unit.file,
                        import_range,
                        error.message(),
                    ));
                    continue;
                }
            };
            aliases.push(ResolvedAlias::authored_import(
                unit.module.clone(),
                import_alias,
                CompilationModuleId::new(unit.module.owner().clone(), module),
                &unit.file,
                import_range,
            ));
        }
    }

    graph::validate_graph_shape(&input.root, &units, &aliases, &mut diagnostics);
    if is_cancelled() {
        return Ok(None);
    }
    if !diagnostics.is_empty() {
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }
    let mut analysis = AnalyzedResolvedHierarchy {
        root: input.root,
        units,
        aliases,
        canonical_declarations: Box::new([]),
        declaration_locations: Box::new([]),
        reference_locations: Box::new([]),
        property_bindings: Box::new([]),
    };
    let canonical_units = analysis.units.clone();
    for unit in &mut analysis.units {
        if is_cancelled() {
            return Ok(None);
        }
        if let Err(mut errors) =
            crate::dimensions::elaborate_dimension_aliases_in_place(&unit.file, &mut unit.document)
        {
            diagnostics.append(&mut errors);
        }
    }
    if !diagnostics.is_empty() {
        stable_sort(&mut diagnostics);
        return Err(diagnostics);
    }
    analysis.property_bindings =
        crate::property::validate_and_elaborate(&mut analysis.units, &analysis.aliases)?;
    if is_cancelled() {
        return Ok(None);
    }
    crate::hierarchy::validate_resolved_hierarchy(&analysis, limits)?;
    if is_cancelled() {
        return Ok(None);
    }
    analysis.canonical_declarations =
        collect_canonical_declarations(&canonical_units, &analysis.aliases, &mut diagnostics)
            .into_boxed_slice();
    if is_cancelled() {
        return Ok(None);
    }
    analysis.declaration_locations = declaration::collect_declaration_locations(
        &canonical_units,
        &analysis.canonical_declarations,
    )
    .into_boxed_slice();
    analysis.reference_locations = declaration::collect_reference_locations(
        &canonical_units,
        &analysis.aliases,
        &analysis.canonical_declarations,
    )?
    .into_boxed_slice();
    if is_cancelled() {
        return Ok(None);
    }
    if diagnostics.is_empty() {
        Ok(Some(analysis))
    } else {
        stable_sort(&mut diagnostics);
        Err(diagnostics)
    }
}
