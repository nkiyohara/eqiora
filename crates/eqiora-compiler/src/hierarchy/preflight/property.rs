//! One property catalog with explicit caller/callee lexical namespaces.
use super::*;

impl Elaborator<'_> {
    fn property_namespace<'a>(
        &'a self,
        namespace: &'a DefinitionNamespace,
    ) -> Result<&'a crate::resolved::CompilationModuleId, Diagnostic> {
        match namespace {
            DefinitionNamespace::Resolved(module) => Ok(module),
            DefinitionNamespace::Local => self.property_local_module.as_ref().ok_or_else(|| {
                super::super::hierarchy_error("property scope has no exact local module")
            }),
        }
    }

    pub(in crate::hierarchy) fn resolve_property_contract(
        &self,
        namespace: &DefinitionNamespace,
        path: &eqiora_lang::NamePath,
        file: &str,
    ) -> Result<&crate::property::Contract, Diagnostic> {
        self.property_catalog.contract(
            self.property_namespace(namespace)?,
            path,
            &self.property_aliases,
            file,
        )
    }

    pub(in crate::hierarchy) fn property_contract_identity(
        &self,
        namespace: &DefinitionNamespace,
        path: &eqiora_lang::NamePath,
        file: &str,
    ) -> Result<String, Diagnostic> {
        self.property_catalog.contract_identity(
            self.property_namespace(namespace)?,
            path,
            &self.property_aliases,
            file,
        )
    }

    pub(in crate::hierarchy) fn bind_property(
        &self,
        requirement_namespace: &DefinitionNamespace,
        requirement: &eqiora_lang::ComponentPropertyDecl,
        binding_namespace: &DefinitionNamespace,
        value: &eqiora_lang::Expr,
        file: &str,
    ) -> Result<eqiora_schema::kernel::PropertyRelease, Vec<Diagnostic>> {
        self.property_catalog.bind(
            self.property_namespace(requirement_namespace)
                .map_err(|error| vec![error])?,
            requirement,
            self.property_namespace(binding_namespace)
                .map_err(|error| vec![error])?,
            value,
            &self.property_aliases,
            file,
        )
    }
}
