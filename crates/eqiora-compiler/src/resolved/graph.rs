use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;

use super::{
    AnalyzedSourceUnit, CompilationNamespaceId, ResolvedAlias, is_identifier, resolved_error,
};

pub(super) fn validate_graph_shape(
    root: &CompilationNamespaceId,
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let namespaces = units
        .iter()
        .map(|unit| unit.namespace.clone())
        .collect::<BTreeSet<_>>();
    if !namespaces.contains(root) {
        diagnostics.push(resolved_error(format!(
            "root compilation namespace `{root}` has no source unit"
        )));
    }

    let mut files = BTreeSet::new();
    for unit in units {
        if !files.insert((unit.namespace.clone(), unit.file.clone())) {
            diagnostics.push(resolved_error(format!(
                "duplicate source unit `{}` in namespace `{}`",
                unit.file, unit.namespace
            )));
        }
    }

    let mut alias_index = BTreeMap::new();
    for alias in aliases {
        if !is_identifier(alias.alias()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` is not an Eqiora identifier",
                alias.alias()
            )));
        }
        if !namespaces.contains(alias.declaring()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` has unknown declaring namespace `{}`",
                alias.alias(),
                alias.declaring()
            )));
        }
        if !namespaces.contains(alias.target()) {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` has unknown target namespace `{}`",
                alias.alias(),
                alias.target()
            )));
        }
        if alias.declaring() == alias.target() {
            diagnostics.push(resolved_error(format!(
                "direct alias `{}` cannot target its declaring namespace",
                alias.alias()
            )));
        }
        let key = (alias.declaring().clone(), alias.alias().to_owned());
        if alias_index.insert(key, alias.target()).is_some() {
            diagnostics.push(resolved_error(format!(
                "duplicate direct alias `{}` in namespace `{}`",
                alias.alias(),
                alias.declaring()
            )));
        }
    }
}
