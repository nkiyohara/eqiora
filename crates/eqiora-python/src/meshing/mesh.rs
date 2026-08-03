//! Immutable accepted Mesh publication and NumPy projections.

use eqiora::Diagnostic;
use eqiora::artifact::{
    AcceptedCircularHoleChordalRealizationV1, CartesianMeshEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, RealizationEnvelopeV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::NamedEntitySet;
use eqiora::meshing::{MeshEntity, MeshTopology};
use numpy::PyArray2;
use pyo3::exceptions::{PyOverflowError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};

use super::plan::PyMeshPlan;
use super::request_error;
use crate::error::{diagnostic_error, validation_error};
use crate::geometry::{PyGeometry, digest_to_hex};
use crate::matrix::ReadOnlyMatrix;
use crate::panic_boundary;

/// Immutable source-bound accepted Mesh.
#[pyclass(name = "Mesh", module = "eqiora._eqiora", frozen, skip_from_py_object)]
pub(crate) struct PyMesh {
    source: AcceptedMeshSource,
    lineage: MeshLineage,
    canonical_bytes: Vec<u8>,
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
}

enum AcceptedMeshSource {
    Chordal(Box<AcceptedCircularHoleChordalRealizationV1>),
    Cartesian,
}

struct MeshLineage {
    source_digest: String,
    realized_geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    realization_digest: String,
    dimension: usize,
    vertex_count: usize,
    cell_count: usize,
}

#[pymethods]
impl PyMesh {
    /// Exact Geometry identity retained by the source binding.
    #[getter]
    fn source_digest(&self) -> &str {
        &self.lineage.source_digest
    }

    /// Identity of the realized straight-edged geometry artifact.
    #[getter]
    fn realized_geometry_digest(&self) -> &str {
        &self.lineage.realized_geometry_digest
    }

    /// Identity of the accepted common mesh artifact.
    #[getter]
    fn digest(&self) -> &str {
        &self.lineage.mesh_digest
    }

    /// Identity of the exact Geometry-to-Mesh correspondence artifact.
    #[getter]
    fn correspondence_digest(&self) -> &str {
        &self.lineage.correspondence_digest
    }

    /// Identity of the complete exact-source realization binding.
    #[getter]
    fn realization_digest(&self) -> &str {
        &self.lineage.realization_digest
    }

    /// Canonical bytes of the accepted common Mesh artifact.
    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, &self.canonical_bytes).unbind()
    }

    #[getter]
    fn dimension(&self) -> usize {
        self.lineage.dimension
    }

    #[getter]
    fn vertex_count(&self) -> usize {
        self.lineage.vertex_count
    }

    #[getter]
    fn cell_count(&self) -> usize {
        self.lineage.cell_count
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
    fn minimum_mean_ratio(&self, py: Python<'_>) -> PyResult<f64> {
        match &self.source {
            AcceptedMeshSource::Chordal(accepted) => {
                Ok(accepted.mesh().mesh().quality_report().minimum_mean_ratio())
            }
            AcceptedMeshSource::Cartesian => Err(capability_error(
                py,
                "minimum_mean_ratio is not defined for this Cartesian Mesh",
            )),
        }
    }

    #[getter]
    fn selection_names(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let names = match &self.source {
            AcceptedMeshSource::Chordal(accepted) => accepted
                .source()
                .entity_sets()
                .iter()
                .map(NamedEntitySet::name)
                .collect::<Vec<_>>(),
            AcceptedMeshSource::Cartesian => {
                // This accepted Cartesian Mesh publishes no named selections.
                Vec::new()
            }
        };
        Ok(PyTuple::new(py, names)?.unbind())
    }

    /// Count mesh entities proven to realize one exact-source selection.
    fn selection_entity_count(&self, py: Python<'_>, name: &str) -> PyResult<usize> {
        match &self.source {
            AcceptedMeshSource::Chordal(accepted) => accepted
                .correspondence()
                .region_entity_set_entities(accepted.realized_geometry(), name)
                .map(|entities| entities.len())
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic))),
            AcceptedMeshSource::Cartesian => Err(capability_error(
                py,
                "this Cartesian Mesh publishes no named selection membership",
            )),
        }
    }

    fn __repr__(&self, _py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "Mesh(dimension={}, vertices={}, cells={}, digest={:?})",
            self.dimension(),
            self.vertex_count(),
            self.cell_count(),
            self.digest(),
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
        let cell_count = mesh.cells().len();
        let (coordinates, cells) = project_simplicial_mesh(py, mesh, dimension)?;
        let source_digest = digest_to_hex(&accepted.source().digest_bytes());
        let realized_geometry_digest = accepted
            .realized_geometry()
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let mesh_digest = accepted
            .mesh()
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let correspondence_digest = accepted
            .correspondence()
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let realization_digest = accepted
            .envelope()
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let canonical_bytes = accepted
            .mesh()
            .canonical_json()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;

        Ok(Self {
            source: AcceptedMeshSource::Chordal(Box::new(accepted)),
            lineage: MeshLineage {
                source_digest,
                realized_geometry_digest,
                mesh_digest,
                correspondence_digest,
                realization_digest,
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
        })
    }

    pub(crate) fn from_cartesian(
        py: Python<'_>,
        geometry: GeometryIdentityEnvelopeV1,
        mesh: CartesianMeshEnvelopeV1,
        correspondence: GeometryMeshCorrespondenceEnvelopeV1,
        realization: RealizationEnvelopeV1,
    ) -> PyResult<Self> {
        let dimension = mesh.dimension();
        let native = mesh.mesh();
        let vertex_count = native
            .entity_count(0)
            .ok_or_else(|| PyRuntimeError::new_err("Cartesian Mesh omitted its vertices"))?;
        let cell_count = native.entity_count(dimension).ok_or_else(|| {
            PyRuntimeError::new_err("Cartesian Mesh omitted its top-dimensional cells")
        })?;
        let (coordinates, cells) =
            project_cartesian_mesh(py, native, dimension, vertex_count, cell_count)?;
        let geometry_digest = geometry
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let mesh_digest = mesh
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let correspondence_digest = correspondence
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let realization_digest = realization
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let canonical_bytes = mesh
            .canonical_json()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Ok(Self {
            source: AcceptedMeshSource::Cartesian,
            lineage: MeshLineage {
                source_digest: geometry_digest.clone(),
                realized_geometry_digest: geometry_digest,
                mesh_digest,
                correspondence_digest,
                realization_digest,
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
        })
    }

    pub(crate) fn accepted_chordal(
        &self,
        py: Python<'_>,
    ) -> PyResult<&AcceptedCircularHoleChordalRealizationV1> {
        match &self.source {
            AcceptedMeshSource::Chordal(accepted) => Ok(accepted),
            AcceptedMeshSource::Cartesian => Err(capability_error(
                py,
                "this operation requires an accepted affine-triangle Mesh",
            )),
        }
    }
}

fn project_simplicial_mesh(
    py: Python<'_>,
    mesh: &eqiora::meshing::SimplicialMesh,
    dimension: usize,
) -> PyResult<(ReadOnlyMatrix<f64>, ReadOnlyMatrix<u32>)> {
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
        cells.extend(
            cell.iter()
                .map(|&vertex| mesh_index(vertex))
                .collect::<PyResult<Vec<_>>>()?,
        );
    }
    Ok((
        ReadOnlyMatrix::new(vertex_count, dimension, coordinates),
        ReadOnlyMatrix::new(cell_count, cell_width, cells),
    ))
}

fn project_cartesian_mesh(
    py: Python<'_>,
    mesh: &eqiora::meshing::CartesianMesh,
    dimension: usize,
    vertex_count: usize,
    cell_count: usize,
) -> PyResult<(ReadOnlyMatrix<f64>, ReadOnlyMatrix<u32>)> {
    let mut coordinates = Vec::with_capacity(vertex_count * dimension);
    for index in 0..vertex_count {
        let coordinate = mesh
            .vertex_coordinates(MeshEntity::new(0, index))
            .ok_or_else(|| request_error(py, "Cartesian Mesh omitted a vertex coordinate"))?;
        if coordinate.len() != dimension {
            return Err(request_error(
                py,
                "Cartesian Mesh coordinate dimension is inconsistent",
            ));
        }
        coordinates.extend(coordinate);
    }
    let cell_width = 1_usize
        .checked_shl(
            u32::try_from(dimension)
                .map_err(|_| PyOverflowError::new_err("Mesh dimension exceeds uint32"))?,
        )
        .ok_or_else(|| PyOverflowError::new_err("Mesh cell arity exceeds local usize"))?;
    let mut cells = Vec::with_capacity(cell_count * cell_width);
    for index in 0..cell_count {
        let vertices = mesh
            .entity_vertices(MeshEntity::new(dimension, index))
            .ok_or_else(|| request_error(py, "Cartesian Mesh omitted cell connectivity"))?;
        if vertices.len() != cell_width {
            return Err(request_error(
                py,
                "Cartesian Mesh cell arity is inconsistent",
            ));
        }
        cells.extend(
            vertices
                .iter()
                .map(|vertex| mesh_index(vertex.index()))
                .collect::<PyResult<Vec<_>>>()?,
        );
    }
    Ok((
        ReadOnlyMatrix::new(vertex_count, dimension, coordinates),
        ReadOnlyMatrix::new(cell_count, cell_width, cells),
    ))
}

fn mesh_index(index: usize) -> PyResult<u32> {
    u32::try_from(index)
        .map_err(|_| PyOverflowError::new_err("Mesh vertex index exceeds Python uint32"))
}

fn capability_error(py: Python<'_>, message: &str) -> PyErr {
    diagnostic_error(py, &[Diagnostic::error(codes::NOT_IMPLEMENTED, message)])
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
