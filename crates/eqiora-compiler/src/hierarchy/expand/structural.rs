//! Fixed type and folded expression dependencies use exact surviving declaration owners.
use super::*;

impl RootExpansion<'_, '_> {
    pub(super) fn record_structural(
        &mut self,
        owner: &str,
        dependencies: impl IntoIterator<Item = String>,
    ) -> Result<(), Diagnostic> {
        for name in dependencies {
            if self
                .structural_dependencies
                .get(owner)
                .is_some_and(|names| names.contains(&name))
            {
                continue;
            }
            if self.structural_dependency_count >= self.elaborator.limits.max_parameter_terms {
                return Err(hierarchy_error(
                    "static declaration dependencies exceed the Parameter term limit",
                ));
            }
            self.structural_dependencies
                .entry(owner.to_owned())
                .or_default()
                .insert(name);
            self.structural_dependency_count += 1;
        }
        Ok(())
    }

    pub(super) fn record_type_structure(
        &mut self,
        owner: &str,
        file: &str,
        syntax: &eqiora_lang::ValueTypeSyntax,
        values: &super::super::parameters::SymbolicParameterMap,
    ) -> Result<(), Diagnostic> {
        for extent in super::super::parameters::extent_expressions(syntax) {
            let (_, dependencies) = super::super::parameters::structural_extent(
                file, extent, values,
            )?
            .ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    extent.range(),
                    "array extent remained unresolved",
                )
            })?;
            self.record_structural(owner, dependencies)?;
        }
        Ok(())
    }
}
