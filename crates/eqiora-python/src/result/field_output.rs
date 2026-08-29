//! Typed static Field output projected from one accepted Result.

use eqiora::DimExponents;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::array::PyArrayBuffer;
use crate::meshing::PyMesh;
use crate::model::PyModelFieldRef;

pub(crate) struct FieldOutputBlock {
    association: &'static str,
    values: Py<PyArrayBuffer>,
    coefficient_count: usize,
    logical_shape: Vec<usize>,
}

impl FieldOutputBlock {
    pub(super) const fn new(
        association: &'static str,
        values: Py<PyArrayBuffer>,
        coefficient_count: usize,
        logical_shape: Vec<usize>,
    ) -> Self {
        Self {
            association,
            values,
            coefficient_count,
            logical_shape,
        }
    }

    pub(crate) const fn association(&self) -> &'static str {
        self.association
    }

    pub(crate) const fn coefficient_count(&self) -> usize {
        self.coefficient_count
    }

    pub(crate) fn logical_shape(&self) -> &[usize] {
        &self.logical_shape
    }

    pub(crate) fn snapshot(&self, py: Python<'_>) -> PyResult<Vec<f64>> {
        self.values.borrow(py).snapshot(py)
    }
}

/// Immutable coefficients for one exact Model Field on one exact Mesh.
#[pyclass(
    name = "FieldOutput",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyFieldOutput {
    field: Py<PyModelFieldRef>,
    mesh: Py<PyMesh>,
    dimension: DimExponents,
    value_shape: Vec<usize>,
    space: &'static str,
    blocks: Vec<FieldOutputBlock>,
}

impl PyFieldOutput {
    pub(super) fn new(
        field: Py<PyModelFieldRef>,
        mesh: Py<PyMesh>,
        dimension: DimExponents,
        value_shape: Vec<usize>,
        space: &'static str,
        blocks: Vec<FieldOutputBlock>,
    ) -> Self {
        Self {
            field,
            mesh,
            dimension,
            value_shape,
            space,
            blocks,
        }
    }

    pub(crate) fn mesh_handle(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }

    pub(crate) fn field_handle(&self, py: Python<'_>) -> Py<PyModelFieldRef> {
        self.field.clone_ref(py)
    }

    pub(crate) const fn dimension_value(&self) -> DimExponents {
        self.dimension
    }

    pub(crate) fn value_shape_value(&self) -> &[usize] {
        &self.value_shape
    }

    pub(crate) const fn space_value(&self) -> &'static str {
        self.space
    }

    pub(crate) fn blocks(&self) -> &[FieldOutputBlock] {
        &self.blocks
    }

    fn block(&self, association: &str) -> PyResult<&FieldOutputBlock> {
        self.blocks
            .iter()
            .find(|block| block.association == association)
            .ok_or_else(|| PyKeyError::new_err(association.to_owned()))
    }
}

#[pymethods]
impl PyFieldOutput {
    #[getter]
    fn field(&self, py: Python<'_>) -> Py<PyModelFieldRef> {
        self.field.clone_ref(py)
    }

    #[getter]
    fn mesh(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }

    #[getter]
    fn dimension(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        let value = self.dimension;
        (
            value.mass,
            value.length,
            value.time,
            value.current,
            value.temperature,
            value.amount,
            value.luminous_intensity,
        )
    }

    /// Exact mathematical component shape; an empty tuple is scalar.
    #[getter]
    fn value_shape(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.value_shape.iter().copied())?.unbind())
    }

    /// Resolved discrete space or basis family for this Field.
    #[getter]
    const fn space(&self) -> &'static str {
        self.space
    }

    /// Coefficient associations in exact output-block order.
    #[getter]
    fn associations(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.blocks.iter().map(|block| block.association))?.unbind())
    }

    /// Read-only Eqiora-owned coefficients for one exact association.
    #[pyo3(signature = (association, /))]
    fn values(&self, py: Python<'_>, association: &str) -> PyResult<Py<PyArrayBuffer>> {
        Ok(self.block(association)?.values.clone_ref(py))
    }

    /// Number of support entities represented by one coefficient block.
    #[pyo3(signature = (association, /))]
    fn coefficient_count(&self, association: &str) -> PyResult<usize> {
        Ok(self.block(association)?.coefficient_count)
    }

    /// Logical array shape for one coefficient block.
    #[pyo3(signature = (association, /))]
    fn logical_shape(&self, py: Python<'_>, association: &str) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.block(association)?.logical_shape.iter().copied())?.unbind())
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "FieldOutput(field={:?}, space={:?}, associations={:?})",
            self.field.borrow(py).exact_id(),
            self.space,
            self.blocks
                .iter()
                .map(|block| block.association)
                .collect::<Vec<_>>(),
        )
    }
}
