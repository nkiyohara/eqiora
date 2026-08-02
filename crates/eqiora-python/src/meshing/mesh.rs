//! Immutable accepted Mesh publication and NumPy projections.

use eqiora::artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora::geometry::{CanonicalGeometryV1, NamedEntitySet};
use numpy::PyArray2;
use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};

use super::plan::PyMeshPlan;
use super::request_error;
use crate::error::validation_error;
use crate::geometry::{PyGeometry, digest_to_hex};
use crate::matrix::ReadOnlyMatrix;
use crate::panic_boundary;

/// Immutable source-bound accepted Mesh.
#[pyclass(name = "Mesh", module = "eqiora._eqiora", frozen, skip_from_py_object)]
pub(crate) struct PyMesh {
    accepted: AcceptedCircularHoleChordalRealizationV1,
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
}

#[pymethods]
impl PyMesh {
    /// Exact Geometry identity retained by the source binding.
    #[getter]
    fn source_digest(&self) -> String {
        digest_to_hex(&self.accepted.source().digest_bytes())
    }

    /// Identity of the accepted common mesh artifact.
    #[getter]
    fn digest(&self, py: Python<'_>) -> PyResult<String> {
        self.accepted
            .mesh()
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    /// Identity of the exact Geometry-to-Mesh correspondence artifact.
    #[getter]
    fn correspondence_digest(&self, py: Python<'_>) -> PyResult<String> {
        self.accepted
            .correspondence()
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    /// Identity of the complete exact-source realization binding.
    #[getter]
    fn realization_digest(&self, py: Python<'_>) -> PyResult<String> {
        self.accepted
            .envelope()
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    /// Canonical bytes of the accepted common Mesh artifact.
    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        self.accepted
            .mesh()
            .canonical_json()
            .map(|bytes| PyBytes::new(py, &bytes).unbind())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    #[getter]
    fn dimension(&self) -> usize {
        self.accepted.mesh().dimension()
    }

    #[getter]
    fn vertex_count(&self) -> usize {
        self.accepted.mesh().mesh().vertices().len()
    }

    #[getter]
    fn cell_count(&self) -> usize {
        self.accepted.mesh().mesh().cells().len()
    }

    /// Canonically ordered, read-only coordinates in coherent SI units.
    #[getter]
    fn coordinates(&self, py: Python<'_>) -> PyResult<Py<PyArray2<f64>>> {
        self.coordinates.numpy(py)
    }

    /// Canonically ordered, read-only top-cell connectivity.
    #[getter]
    fn cells(&self, py: Python<'_>) -> PyResult<Py<PyArray2<u32>>> {
        self.cells.numpy(py)
    }

    /// Minimum mean ratio measured over every accepted cell.
    #[getter]
    fn minimum_mean_ratio(&self) -> f64 {
        self.accepted
            .mesh()
            .mesh()
            .quality_report()
            .minimum_mean_ratio()
    }

    #[getter]
    fn selection_names(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(
            py,
            self.accepted
                .source()
                .entity_sets()
                .iter()
                .map(NamedEntitySet::name),
        )?
        .unbind())
    }

    /// Count mesh entities proven to realize one exact-source selection.
    fn selection_entity_count(&self, py: Python<'_>, name: &str) -> PyResult<usize> {
        self.accepted
            .correspondence()
            .region_entity_set_entities(self.accepted.realized_geometry(), name)
            .map(|entities| entities.len())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Mesh(dimension={}, vertices={}, cells={}, digest={:?})",
            self.dimension(),
            self.vertex_count(),
            self.cell_count(),
            self.digest(py)?,
        ))
    }
}

impl PyMesh {
    fn from_accepted(
        py: Python<'_>,
        accepted: AcceptedCircularHoleChordalRealizationV1,
    ) -> PyResult<Self> {
        let mesh = accepted.mesh().mesh();
        let dimension = accepted.mesh().dimension();
        let vertex_count = mesh.vertices().len();
        let mut coordinates = Vec::with_capacity(vertex_count * dimension);
        for coordinate in mesh.vertices() {
            if coordinate.len() != dimension {
                return Err(request_error(
                    py,
                    "accepted Mesh coordinate dimension is inconsistent",
                ));
            }
            coordinates.extend(coordinate);
        }

        let cell_count = mesh.cells().len();
        let cell_width = mesh.cells().first().map_or(0, Vec::len);
        let mut cells = Vec::with_capacity(cell_count * cell_width);
        for cell in mesh.cells() {
            if cell.len() != cell_width {
                return Err(request_error(
                    py,
                    "accepted Mesh cell arity is inconsistent",
                ));
            }
            for &vertex in cell {
                cells.push(u32::try_from(vertex).map_err(|_| {
                    PyOverflowError::new_err("Mesh vertex index exceeds Python uint32")
                })?);
            }
        }

        Ok(Self {
            accepted,
            coordinates: ReadOnlyMatrix::new(vertex_count, dimension, coordinates),
            cells: ReadOnlyMatrix::new(cell_count, cell_width, cells),
        })
    }

    pub(crate) const fn source(&self) -> &CanonicalGeometryV1 {
        self.accepted.source()
    }

    pub(crate) const fn accepted(&self) -> &AcceptedCircularHoleChordalRealizationV1 {
        &self.accepted
    }
}

/// Publish the exact accepted Mesh owned by a resolved plan.
#[pyfunction]
#[pyo3(signature = (geometry, /, *, plan))]
pub(super) fn generate(
    py: Python<'_>,
    geometry: &PyGeometry,
    plan: &PyMeshPlan,
) -> PyResult<PyMesh> {
    panic_boundary(py, || {
        if geometry.geometry() != plan.accepted.source() {
            return Err(request_error(
                py,
                "MeshPlan belongs to a different exact Geometry",
            ));
        }
        PyMesh::from_accepted(py, plan.accepted.clone())
    })
}
