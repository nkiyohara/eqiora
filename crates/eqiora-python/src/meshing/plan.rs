//! Immutable mesh intent and complete resolved provider choices.

use eqiora::artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora::meshing::MeshQualityGate;
use pyo3::prelude::*;

use super::request_error;
use crate::error::validation_error;
use crate::geometry::{PyGeometry, digest_to_hex};
use crate::panic_boundary;

const REFERENCE_PROVIDER: &str = "eqiora.reference.chordal-triangle/v1";

/// Immutable caller intent for the currently admitted planar mesh provider.
#[pyclass(
    name = "MeshRequest",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PyMeshRequest {
    maximum_boundary_error: f64,
    minimum_mean_ratio: f64,
    maximum_boundary_facets: usize,
}

#[pymethods]
impl PyMeshRequest {
    #[new]
    #[pyo3(signature = (
        *,
        maximum_boundary_error=1.0e-4,
        minimum_mean_ratio=1.0e-5,
        maximum_boundary_facets=50
    ))]
    fn new(
        py: Python<'_>,
        maximum_boundary_error: f64,
        minimum_mean_ratio: f64,
        maximum_boundary_facets: usize,
    ) -> PyResult<Self> {
        if !maximum_boundary_error.is_finite() || maximum_boundary_error <= 0.0 {
            return Err(request_error(
                py,
                "maximum_boundary_error must be finite and positive",
            ));
        }
        MeshQualityGate::new(minimum_mean_ratio)
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        if maximum_boundary_facets < 8 {
            return Err(request_error(
                py,
                "maximum_boundary_facets must be at least 8 for the admitted provider",
            ));
        }
        Ok(Self {
            maximum_boundary_error,
            minimum_mean_ratio,
            maximum_boundary_facets,
        })
    }

    #[getter]
    const fn maximum_boundary_error(&self) -> f64 {
        self.maximum_boundary_error
    }

    #[getter]
    const fn minimum_mean_ratio(&self) -> f64 {
        self.minimum_mean_ratio
    }

    #[getter]
    const fn maximum_boundary_facets(&self) -> usize {
        self.maximum_boundary_facets
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshRequest(maximum_boundary_error={}, minimum_mean_ratio={}, \
             maximum_boundary_facets={})",
            self.maximum_boundary_error, self.minimum_mean_ratio, self.maximum_boundary_facets,
        )
    }
}

/// Complete resolved provider choice bound to one exact Geometry.
///
/// Resolution owns the deterministic accepted resource privately so
/// `generate` can only publish the exact resource inspected through this plan.
#[pyclass(
    name = "MeshPlan",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyMeshPlan {
    source_digest: String,
    request: PyMeshRequest,
    pub(super) accepted: AcceptedCircularHoleChordalRealizationV1,
}

#[pymethods]
impl PyMeshPlan {
    #[getter]
    fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[getter]
    const fn provider(&self) -> &'static str {
        REFERENCE_PROVIDER
    }

    #[getter]
    fn request(&self, py: Python<'_>) -> PyResult<Py<PyMeshRequest>> {
        Py::new(py, self.request)
    }

    /// Effective number of boundary facets selected by the provider.
    #[getter]
    fn boundary_facets(&self) -> usize {
        self.accepted.circle_segments()
    }

    /// Measured accepted boundary approximation error in metres.
    #[getter]
    const fn boundary_error_bound(&self) -> f64 {
        self.accepted.boundary_error_bound_m()
    }

    /// Binary64 allowance used to evaluate the boundary error receipt.
    #[getter]
    const fn boundary_evaluation_allowance(&self) -> f64 {
        self.accepted.boundary_evaluation_allowance_m()
    }

    /// Measured minimum mean ratio achieved by the resolved mesh.
    #[getter]
    fn achieved_minimum_mean_ratio(&self) -> f64 {
        self.accepted
            .mesh()
            .mesh()
            .quality_report()
            .minimum_mean_ratio()
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshPlan(provider={:?}, source_digest={:?}, boundary_facets={})",
            REFERENCE_PROVIDER,
            self.source_digest,
            self.boundary_facets(),
        )
    }
}

/// Resolve one complete provider plan for the exact supplied Geometry.
#[pyfunction]
#[pyo3(signature = (geometry, request, /))]
pub(super) fn resolve(
    py: Python<'_>,
    geometry: &PyGeometry,
    request: PyRef<'_, PyMeshRequest>,
) -> PyResult<PyMeshPlan> {
    panic_boundary(py, || {
        let request = *request;
        let quality_gate = MeshQualityGate::new(request.minimum_mean_ratio)
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        let accepted = AcceptedCircularHoleChordalRealizationV1::from_reference(
            geometry.geometry(),
            request.maximum_boundary_error,
            request.maximum_boundary_facets,
            quality_gate,
        )
        .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        Ok(PyMeshPlan {
            source_digest: digest_to_hex(&geometry.geometry().digest_bytes()),
            request,
            accepted,
        })
    })
}
