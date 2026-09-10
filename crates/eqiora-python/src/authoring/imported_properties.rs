//! Read-only property descriptors use the ordinary resolved compiler admission.
use super::PyAstModule;
use super::expression::syntax_error;
use crate::modeling::PyValueType;
use eqiora::compiler::{
    CompilationNamespaceId, ResolvedDependency, ResolvedHierarchyInput, ResolvedSourceUnit,
    analyze_resolved_hierarchy,
};
use eqiora::language::{Module, VisibilitySyntax};
use pyo3::prelude::*;

pub(super) type ContractDescriptor = (
    PyValueType,
    Vec<(String, PyValueType)>,
    String,
    Option<String>,
);

pub(super) fn contract(
    root: (String, String),
    units: Vec<(String, String, PyRef<'_, PyAstModule>)>,
    dependencies: Vec<(String, String)>,
    name: &str,
) -> PyResult<ContractDescriptor> {
    if units.len() > 256 {
        return Err(syntax_error("module closure exceeds 256 units"));
    }
    let (package, root) = root;
    let namespace = CompilationNamespaceId::new([package]).map_err(syntax_error)?;
    let units = units
        .into_iter()
        .map(|(package, name, module)| {
            module.admit_metadata()?;
            ResolvedSourceUnit::from_module(
                CompilationNamespaceId::new([package]).map_err(syntax_error)?,
                format!("src/{}.eqi", name.replace('.', "/")),
                module.value.clone(),
            )
            .map_err(syntax_error)
        })
        .collect::<PyResult<Vec<_>>>()?;
    let dependencies = dependencies
        .into_iter()
        .map(|(declaring, target)| {
            Ok(ResolvedDependency::new(
                CompilationNamespaceId::new([declaring]).map_err(syntax_error)?,
                CompilationNamespaceId::new([target]).map_err(syntax_error)?,
            ))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let input =
        ResolvedHierarchyInput::with_root_module(namespace, root.split('.'), units, dependencies)
            .map_err(syntax_error)?;
    let analyzed =
        analyze_resolved_hierarchy(input).map_err(|errors| syntax_error(format!("{errors:?}")))?;
    let descriptor = analyzed
        .property_contract_descriptor(name)
        .map_err(syntax_error)?;
    Ok((
        PyValueType {
            value: descriptor.value_type().clone(),
        },
        descriptor
            .inputs()
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    PyValueType {
                        value: value.clone(),
                    },
                )
            })
            .collect(),
        descriptor.derivatives().as_str().to_owned(),
        descriptor.branch().map(str::to_owned),
    ))
}

pub(super) fn release(module: &Module, name: &str) -> PyResult<String> {
    let mut found = module
        .document()
        .property_release_syntax()
        .filter(|(_, candidate, ..)| *candidate == name);
    let (visibility, _, contract, ..) = found
        .next()
        .ok_or_else(|| syntax_error("import requires an exact property release"))?;
    if found.next().is_some() || visibility != VisibilitySyntax::Public {
        return Err(syntax_error("import requires one public property release"));
    }
    Ok(contract.to_string())
}
