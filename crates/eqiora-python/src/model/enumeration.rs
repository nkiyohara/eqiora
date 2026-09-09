//! Exact enum selection for the existing immutable Model projection.
use super::{PyModel, validation_error};
use eqiora::kernel::KernelNode;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(super) fn select(
    model: &PyModel,
    py: Python<'_>,
    selection: &str,
) -> PyResult<crate::modeling::enumeration::PyEnum> {
    let document = model
        .document()
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
    let alias = document.aliases().get(selection).copied();
    let definition = document
        .program()
        .nodes()
        .find_map(|node| {
            let KernelNode::Enum(value) = node else {
                return None;
            };
            (alias == Some(value.id().erase()) || value.id().ulid().to_string() == selection)
                .then_some(value)
        })
        .ok_or_else(|| PyValueError::new_err("selection is not an exact Enum in this Model"))?;
    let name = document
        .aliases()
        .iter()
        .find(|(_, id)| **id == definition.id().erase())
        .map(|(name, _)| name.clone());
    Ok(crate::modeling::enumeration::PyEnum {
        name,
        value: definition.clone(),
    })
}
