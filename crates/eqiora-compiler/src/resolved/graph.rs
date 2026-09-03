use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::diagnostics::source_error;

use super::{
    AnalyzedSourceUnit, CompilationModuleId, ResolvedAlias, is_identifier, resolved_error,
};

const MAX_MODULE_IMPORT_DEPTH: usize = 1_024;

fn alias_error(alias: &ResolvedAlias, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    match alias.source_span() {
        Some((file, range)) => source_error(codes::LANGUAGE_LOWERING_ERROR, file, range, message),
        None => resolved_error(message),
    }
}

pub(super) fn validate_graph_shape(
    root: &CompilationModuleId,
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let modules = units
        .iter()
        .map(|unit| unit.module.clone())
        .collect::<BTreeSet<_>>();
    if !modules.contains(root) {
        diagnostics.push(resolved_error(format!(
            "root logical module `{root}` has no source unit"
        )));
    }

    let mut files = BTreeSet::new();
    for unit in units {
        if !files.insert((unit.module.clone(), unit.file.clone())) {
            diagnostics.push(resolved_error(format!(
                "duplicate source unit `{}` in module `{}`",
                unit.file, unit.module
            )));
        }
    }

    let mut alias_index = BTreeMap::new();
    for alias in aliases {
        if alias.alias() == crate::math::ROOT {
            diagnostics.push(alias_error(
                alias,
                "direct alias `math` is reserved for compiler-owned scalar mathematics",
            ));
        }
        if !is_identifier(alias.alias()) {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "direct alias `{}` is not an Eqiora identifier",
                    alias.alias()
                ),
            ));
        }
        if !modules.contains(alias.declaring_module()) {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "direct alias `{}` has unknown declaring module `{}`",
                    alias.alias(),
                    alias.declaring_module()
                ),
            ));
        }
        if !modules.contains(alias.target_module()) {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "direct alias `{}` has unknown target module `{}`",
                    alias.alias(),
                    alias.target_module()
                ),
            ));
        }
        if alias.declaring_module() == alias.target_module() {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "direct alias `{}` cannot target its declaring namespace",
                    alias.alias()
                ),
            ));
        }
        let key = (alias.declaring_module().clone(), alias.alias().to_owned());
        if alias_index.insert(key, alias.target_module()).is_some() {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "duplicate direct alias `{}` in namespace `{}`",
                    alias.alias(),
                    alias.declaring_module()
                ),
            ));
        }

        if units.iter().any(|unit| {
            &unit.module == alias.declaring_module()
                && top_level_names(&unit.document).contains(alias.alias())
        }) {
            diagnostics.push(alias_error(
                alias,
                format!(
                    "import alias `{}` collides with a declaration in module `{}`",
                    alias.alias(),
                    alias.declaring_module()
                ),
            ));
        }
    }

    reject_source_import_cycles(&modules, aliases, diagnostics);
}

fn top_level_names(document: &eqiora_lang::Document) -> BTreeSet<&str> {
    document
        .dimension_syntax()
        .map(|(name, _, _)| name)
        .chain(
            document
                .property_contract_syntax()
                .map(|(_, name, _, _)| name),
        )
        .chain(
            document
                .property_release_syntax()
                .map(|(_, name, _, _, _, _, _, _, _)| name),
        )
        .chain(
            document
                .material_composition_syntax()
                .map(|(_, name, _, _)| name),
        )
        .chain(
            document
                .connectors()
                .iter()
                .map(eqiora_lang::ConnectorDecl::name),
        )
        .chain(
            document
                .components()
                .iter()
                .map(eqiora_lang::ComponentDecl::name),
        )
        .chain(
            document
                .pure_operators()
                .iter()
                .map(eqiora_lang::PureOperatorDecl::name),
        )
        .chain(document.models().iter().map(eqiora_lang::ModelDecl::name))
        .collect()
}

fn reject_source_import_cycles(
    modules: &BTreeSet<CompilationModuleId>,
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut outgoing = modules
        .iter()
        .cloned()
        .map(|module| (module, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for alias in aliases.iter().filter(|alias| {
        alias.is_source_import()
            && modules.contains(alias.declaring_module())
            && modules.contains(alias.target_module())
    }) {
        outgoing
            .get_mut(alias.declaring_module())
            .expect("validated declaring module is indexed")
            .insert(alias.target_module().clone());
    }

    let mut complete = BTreeSet::new();
    for start in modules {
        if complete.contains(start) {
            continue;
        }
        let mut active = BTreeMap::new();
        let mut path = Vec::new();
        let mut stack = vec![(
            start.clone(),
            outgoing[start].iter().cloned().collect::<Vec<_>>(),
            0_usize,
        )];
        active.insert(start.clone(), 0_usize);
        path.push(start.clone());

        while !stack.is_empty() {
            let depth = stack.len();
            let (module, targets, next_index) = stack.last_mut().expect("nonempty traversal stack");
            let target = targets.get(*next_index).cloned();
            *next_index += usize::from(target.is_some());
            let Some(target) = target else {
                let (finished, _, _) = stack.pop().expect("active traversal frame");
                active.remove(&finished);
                path.pop();
                complete.insert(finished);
                continue;
            };
            if complete.contains(&target) {
                continue;
            }
            if let Some(position) = active.get(&target).copied() {
                let mut cycle = path[position..]
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                cycle.push(target.to_string());
                let message = format!("semantic module import cycle: {}", cycle.join(" -> "));
                let closing_alias = aliases.iter().find(|alias| {
                    alias.is_source_import()
                        && alias.declaring_module() == module
                        && alias.target_module() == &target
                });
                diagnostics.push(match closing_alias {
                    Some(alias) => alias_error(alias, message),
                    None => resolved_error(message),
                });
                return;
            }
            if depth >= MAX_MODULE_IMPORT_DEPTH {
                let message = format!(
                    "semantic module import depth exceeds the {MAX_MODULE_IMPORT_DEPTH} module limit"
                );
                let edge = aliases.iter().find(|alias| {
                    alias.is_source_import()
                        && alias.declaring_module() == module
                        && alias.target_module() == &target
                });
                diagnostics.push(match edge {
                    Some(alias) => alias_error(alias, message),
                    None => resolved_error(message),
                });
                return;
            }
            active.insert(target.clone(), path.len());
            path.push(target.clone());
            let targets = outgoing[&target].iter().cloned().collect();
            stack.push((target, targets, 0));
        }
    }
}
