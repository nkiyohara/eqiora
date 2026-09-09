//! Module compilation delegates to the existing selected and resolved compiler owners.

use eqiora::api::ModelDocument;
use eqiora::compiler::{CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::PyAstModule;
use crate::error::{diagnostic_error, panic_boundary};
use crate::geometry::PyGeometry;
use crate::model::PyModel;

#[pyfunction]
#[pyo3(signature = (root, units, *, entry=None, geometry=None, bindings=None))]
pub(super) fn _compile_module(
    py: Python<'_>,
    root: String,
    units: Vec<(String, PyRef<'_, PyAstModule>)>,
    entry: Option<String>,
    geometry: Option<Py<PyGeometry>>,
    bindings: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyModel> {
    panic_boundary(py, || {
        let authority = geometry
            .as_ref()
            .map(|geometry| geometry.borrow(py).geometry().clone());
        let values = crate::static_bindings::extract(bindings, authority.as_ref())?;
        if units.len() > 256 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "module closure exceeds 256 units",
            ));
        }
        for (_, module) in &units {
            module.admit_metadata()?;
        }
        // Module clones share the immutable AST. The resolved owner applies
        // aggregate admission before copying any document for elaboration.
        let units = units
            .into_iter()
            .map(|(name, module)| (name, module.value.clone()))
            .collect::<Vec<_>>();
        let document = py
            .detach(move || {
                let bindings = values
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.borrowed(authority.as_ref())))
                    .collect::<Vec<_>>();
                if let [(name, module)] = units.as_slice()
                    && name == &root
                    && module.document().imports().len() == 0
                {
                    return ModelDocument::compile_module(module, entry.as_deref(), &bindings);
                }
                let error = |message| {
                    vec![eqiora::Diagnostic::error(
                        eqiora::diagnostic::codes::LANGUAGE_LOWERING_ERROR,
                        message,
                    )]
                };
                let entry = entry
                    .ok_or_else(|| error("multi-module compilation requires an explicit entry"))?;
                let namespace = CompilationNamespaceId::new(["eqiora.local_project"])
                    .map_err(|error| vec![error])?;
                let units = units
                    .into_iter()
                    .map(|(name, module)| {
                        ResolvedSourceUnit::from_module(
                            namespace.clone(),
                            format!("src/{}.eqi", name.replace('.', "/")),
                            module,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| vec![error])?;
                let input = ResolvedHierarchyInput::with_root_module(
                    namespace,
                    root.split('.'),
                    units,
                    vec![],
                )
                .map_err(|error| vec![error])?;
                ModelDocument::compile_modules(input, &entry, &bindings)
            })
            .map_err(|errors| diagnostic_error(py, &errors))?;
        match geometry {
            Some(geometry) => PyModel::from_document_with_geometry(py, document, geometry),
            None => PyModel::from_document(py, document),
        }
    })
}
