//! The existing lexical finite-type owner applied to explicit module imports.
use super::*;
use crate::resolved::{AnalyzedSourceUnit, ResolvedAlias};

pub(crate) fn bind_resolved(
    units: &mut [AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> Result<(), Vec<Diagnostic>> {
    let mut definitions = BTreeMap::new();
    let mut public = BTreeMap::new();
    for unit in units.iter() {
        let namespace =
            crate::enumeration::resolved_namespace(&unit.module).map_err(|error| vec![error])?;
        definitions.insert(
            unit.module.clone(),
            finite_spaces(&unit.file, &unit.document, &namespace, |name| {
                unit.native
                    .as_ref()
                    .and_then(|module| module.nominal_identity(name))
            })?,
        );
        public.insert(
            unit.module.clone(),
            unit.document
                .finite_spaces()
                .iter()
                .filter(|declaration| {
                    declaration.visibility() == eqiora_lang::VisibilitySyntax::Public
                })
                .map(|declaration| declaration.name().to_owned())
                .collect::<std::collections::BTreeSet<_>>(),
        );
    }
    for unit in units {
        let mut visible = definitions[&unit.module].clone();
        for alias in aliases
            .iter()
            .filter(|alias| alias.declaring_module() == &unit.module)
        {
            for (name, value) in &definitions[alias.target_module()] {
                if public[alias.target_module()].contains(name) {
                    visible.insert(format!("{}.{}", alias.alias(), name), value.clone());
                }
            }
        }
        bind_finite_types(&unit.file, &mut unit.document, &visible)?;
        bind_finite_expressions(&unit.file, &mut unit.document, &visible)?;
        let namespace =
            crate::enumeration::resolved_namespace(&unit.module).map_err(|error| vec![error])?;
        bind_local_index_types(&unit.file, &mut unit.document, &namespace, |name| {
            unit.native
                .as_ref()
                .and_then(|module| module.nominal_identity(name))
        })?;
    }
    Ok(())
}
