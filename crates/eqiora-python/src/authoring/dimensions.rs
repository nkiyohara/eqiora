//! Structural dimension projection through the existing resolved Module owner.
use super::{PyAstModule, expression::syntax_error};
use crate::modeling::PyDimension;
use eqiora::compiler::{
    CompilationNamespaceId, ResolvedDependency, ResolvedHierarchyInput, ResolvedSourceUnit,
    analyze_resolved_hierarchy,
};
use eqiora::language::{
    ExprKind, NamedDefinitionDecl, SourceAstFactory as Ast, TextRange, ValueTypeSyntax,
    ValueTypeSyntaxKind, VisibilitySyntax,
};
use pyo3::prelude::*;

pub(super) fn declaration(
    name: String,
    value: &ValueTypeSyntax,
    range: TextRange,
) -> PyResult<NamedDefinitionDecl> {
    // A single SI symbol retains unresolved Named syntax until lexical binding;
    // compound dimensions already carry an explicit scalar expression.
    let expression = match value.kind() {
        ValueTypeSyntaxKind::Named(path) => {
            let kind = if path.segments().len() == 1 {
                ExprKind::Name(path.as_str().to_owned())
            } else {
                ExprKind::Path(path.clone())
            };
            Ast::expression(kind, value.range()).map_err(syntax_error)?
        }
        ValueTypeSyntaxKind::Scalar {
            domain: eqiora::ScalarDomain::Real,
            dimension,
        } => dimension.clone(),
        _ => {
            return Err(syntax_error(
                "dimension alias requires a real scalar dimension",
            ));
        }
    };
    Ast::dimension_alias(VisibilitySyntax::Public, name, expression, range).map_err(syntax_error)
}

pub(super) fn resolve(
    module: &PyAstModule,
    root: (String, String),
    units: Vec<(String, String, PyRef<'_, PyAstModule>)>,
    dependencies: Vec<(String, String)>,
    name: &str,
) -> PyResult<PyDimension> {
    let mut declarations = module
        .value
        .document()
        .dimensions()
        .iter()
        .filter(|declaration| declaration.name() == name);
    if !declarations
        .next()
        .is_some_and(|value| value.visibility() == VisibilitySyntax::Public)
        || declarations.next().is_some()
    {
        return Err(syntax_error("import requires one public dimension alias"));
    }
    if units.len() > 256 {
        return Err(syntax_error("module closure exceeds 256 units"));
    }
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
        .map(|(owner, target)| {
            Ok(ResolvedDependency::new(
                CompilationNamespaceId::new([owner]).map_err(syntax_error)?,
                CompilationNamespaceId::new([target]).map_err(syntax_error)?,
            ))
        })
        .collect::<PyResult<Vec<_>>>()?;
    let input = ResolvedHierarchyInput::with_root_module(
        CompilationNamespaceId::new([root.0]).map_err(syntax_error)?,
        root.1.split('.'),
        units,
        dependencies,
    )
    .map_err(syntax_error)?;
    let analysis =
        analyze_resolved_hierarchy(input).map_err(|errors| syntax_error(format!("{errors:?}")))?;
    Ok(PyDimension {
        value: analysis.dimension_alias(name).map_err(syntax_error)?,
    })
}
