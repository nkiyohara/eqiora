//! Bounded Python projection of one Rust-owned chordal reference mesh.

use eqiora::artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora::geometry::{CanonicalGeometryV1, NamedEntitySet};
use eqiora::meshing::MeshQualityGate;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyTuple};

use crate::error::validation_error;
use crate::geometry::{PyRectangleWithCircularHole, digest_to_hex};
use crate::panic_boundary;

/// One in-process, exact-source-bound chordal reference mesh.
///
/// The Python value retains the accepted durable source-to-resource binding;
/// its getters remain bounded projections rather than a second mesh meaning.
#[pyclass(
    name = "CircularHoleChordalMesh",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyCircularHoleChordalMesh {
    accepted: AcceptedCircularHoleChordalRealizationV1,
}

#[pymethods]
impl PyCircularHoleChordalMesh {
    /// Lowercase domain-separated identity of the retained exact source.
    #[getter]
    fn source_digest(&self) -> String {
        digest_to_hex(&self.accepted.source().digest_bytes())
    }

    /// Lowercase domain-separated identity of the inner mesh artifact.
    #[getter]
    fn mesh_digest(&self, py: Python<'_>) -> PyResult<String> {
        self.accepted
            .mesh()
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    /// Canonical JSON of the inner mesh artifact, without source binding.
    #[getter]
    fn mesh_canonical_json(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        self.accepted
            .mesh()
            .canonical_json()
            .map(|bytes| PyBytes::new(py, &bytes).unbind())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
    }

    /// Topological and coordinate dimension of the full-dimensional mesh.
    #[getter]
    fn dimension(&self) -> usize {
        self.accepted.mesh().dimension()
    }

    /// Number of accepted mesh vertices.
    #[getter]
    fn vertex_count(&self) -> usize {
        self.accepted.mesh().mesh().vertices().len()
    }

    /// Number of accepted top-dimensional cells.
    #[getter]
    fn cell_count(&self) -> usize {
        self.accepted.mesh().mesh().cells().len()
    }

    /// Caller-requested maximum circular-boundary error in metres.
    #[getter]
    fn requested_max_boundary_error(&self) -> f64 {
        self.accepted.requested_max_boundary_error_m()
    }

    /// Precommitted binary64 evaluation allowance in metres.
    #[getter]
    fn boundary_evaluation_allowance(&self) -> f64 {
        self.accepted.boundary_evaluation_allowance_m()
    }

    /// Accepted circular-boundary error bound in metres.
    #[getter]
    fn boundary_error_bound(&self) -> f64 {
        self.accepted.boundary_error_bound_m()
    }

    /// Number of straight segments realizing the exact circular boundary.
    #[getter]
    fn circle_segments(&self) -> usize {
        self.accepted.circle_segments()
    }

    /// Exact-circle minus chordal-loop area in square metres.
    #[getter]
    fn circle_area_deficit(&self) -> f64 {
        self.accepted.circle_area_deficit_m2()
    }

    /// Exact-circle minus chordal-loop perimeter in metres.
    #[getter]
    fn circle_perimeter_deficit(&self) -> f64 {
        self.accepted.circle_perimeter_deficit_m()
    }

    /// Minimum mean-ratio threshold required during mesh admission.
    #[getter]
    fn required_minimum_mean_ratio(&self) -> f64 {
        self.accepted.envelope().required_minimum_mean_ratio()
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

    /// Canonically ordered selection names retained from the exact source.
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
}

impl PyCircularHoleChordalMesh {
    pub(crate) const fn source(&self) -> &CanonicalGeometryV1 {
        self.accepted.source()
    }

    pub(crate) const fn accepted(&self) -> &AcceptedCircularHoleChordalRealizationV1 {
        &self.accepted
    }
}

/// Derive one bounded chordal reference mesh from exact circular-hole meaning.
#[pyfunction]
#[pyo3(signature = (
    geometry,
    *,
    max_boundary_error=1.0e-4,
    required_minimum_mean_ratio=1.0e-5,
    max_segments=50
))]
pub(crate) fn circular_hole_chordal(
    py: Python<'_>,
    geometry: &PyRectangleWithCircularHole,
    max_boundary_error: f64,
    required_minimum_mean_ratio: f64,
    max_segments: usize,
) -> PyResult<PyCircularHoleChordalMesh> {
    panic_boundary(py, || {
        let source = geometry.geometry();
        let quality_gate = MeshQualityGate::new(required_minimum_mean_ratio)
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        let accepted = AcceptedCircularHoleChordalRealizationV1::from_reference(
            source,
            max_boundary_error,
            max_segments,
            quality_gate,
        )
        .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        Ok(PyCircularHoleChordalMesh { accepted })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCircularHoleChordalMesh>()?;
    module.add_function(wrap_pyfunction!(circular_hole_chordal, module)?)?;
    Ok(())
}
