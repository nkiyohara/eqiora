//! Immutable mesh intent and complete resolved provider choices.

use eqiora::Diagnostic;
use eqiora::artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora::diagnostic::codes;
use eqiora::meshing::MeshQualityGate;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use super::gmsh;
use super::request_error;
use super::source_owned::{self, SourceOwnedPlan};
use crate::error::{diagnostic_error, validation_error};
use crate::geometry::{PyGeometry, digest_to_hex};
use crate::panic_boundary;

const GMSH_PROVIDER: &str = "eqiora.gmsh-cli/4.15.2";

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
    pub(super) maximum_boundary_error: f64,
    pub(super) minimum_mean_ratio: f64,
    pub(super) maximum_boundary_facets: usize,
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
    pub(super) resolved: ResolvedMeshPlan,
}

pub(super) enum ResolvedMeshPlan {
    Legacy(Box<AcceptedCircularHoleChordalRealizationV1>),
    SourceOwned(Box<SourceOwnedPlan>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MeshRoute {
    Gmsh,
    SourceOwned,
}

#[pymethods]
impl PyMeshPlan {
    #[getter]
    fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[getter]
    fn provider(&self) -> &'static str {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(_) => GMSH_PROVIDER,
            ResolvedMeshPlan::SourceOwned(_) => source_owned::PROVIDER,
        }
    }

    #[getter]
    fn request(&self, py: Python<'_>) -> PyResult<Py<PyMeshRequest>> {
        Py::new(py, self.request)
    }

    /// Effective number of boundary facets selected by the provider.
    #[getter]
    fn boundary_facets(&self) -> usize {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(accepted) => accepted.circle_segments(),
            ResolvedMeshPlan::SourceOwned(plan) => plan
                .boundary_facets()
                .expect("an admitted source-owned plan retains its circular frontier"),
        }
    }

    /// Measured accepted boundary approximation error in metres.
    #[getter]
    fn boundary_error_bound(&self, py: Python<'_>) -> PyResult<f64> {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(accepted) => Ok(accepted.boundary_error_bound_m()),
            ResolvedMeshPlan::SourceOwned(_) => Err(unsupported(
                py,
                "boundary_error_bound is unavailable without an accepted chordal realization",
            )),
        }
    }

    /// Binary64 allowance used to evaluate the boundary error receipt.
    #[getter]
    fn boundary_evaluation_allowance(&self, py: Python<'_>) -> PyResult<f64> {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(accepted) => Ok(accepted.boundary_evaluation_allowance_m()),
            ResolvedMeshPlan::SourceOwned(_) => Err(unsupported(
                py,
                "boundary_evaluation_allowance is unavailable without an accepted chordal realization",
            )),
        }
    }

    /// Canonical bytes of the complete accepted source-to-mesh binding.
    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(accepted) => accepted
                .envelope()
                .canonical_json()
                .map(|bytes| PyBytes::new(py, &bytes).unbind())
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic))),
            ResolvedMeshPlan::SourceOwned(_) => Err(unsupported(
                py,
                "canonical_bytes are unavailable without an accepted chordal realization",
            )),
        }
    }

    /// Measured minimum mean ratio achieved by the resolved mesh.
    #[getter]
    fn achieved_minimum_mean_ratio(&self) -> f64 {
        match &self.resolved {
            ResolvedMeshPlan::Legacy(accepted) => {
                accepted.mesh().mesh().quality_report().minimum_mean_ratio()
            }
            ResolvedMeshPlan::SourceOwned(plan) => {
                plan.mesh.mesh().quality_report().minimum_mean_ratio()
            }
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshPlan(provider={:?}, source_digest={:?}, boundary_facets={})",
            self.provider(),
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
        let resolved = if route(geometry.geometry()) == MeshRoute::Gmsh {
            let reference = AcceptedCircularHoleChordalRealizationV1::from_reference(
                geometry.geometry(),
                request.maximum_boundary_error,
                request.maximum_boundary_facets,
                quality_gate,
            )
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
            let accepted = gmsh::generate(&reference, quality_gate)
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
            ResolvedMeshPlan::Legacy(Box::new(accepted))
        } else {
            SourceOwnedPlan::resolve(
                geometry.geometry(),
                request.maximum_boundary_error,
                request.maximum_boundary_facets,
                quality_gate,
            )
            .map(Box::new)
            .map(ResolvedMeshPlan::SourceOwned)
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?
        };
        Ok(PyMeshPlan {
            source_digest: digest_to_hex(&geometry.geometry().digest_bytes()),
            request,
            resolved,
        })
    })
}

fn route(geometry: &eqiora::geometry::CanonicalGeometryV1) -> MeshRoute {
    if geometry.classification_tolerance_m().is_some() {
        MeshRoute::Gmsh
    } else {
        MeshRoute::SourceOwned
    }
}

fn unsupported(py: Python<'_>, message: &str) -> PyErr {
    diagnostic_error(py, &[Diagnostic::error(codes::NOT_IMPLEMENTED, message)])
}

#[cfg(test)]
mod tests {
    use eqiora::geometry::{CanonicalGeometryV1, NamedEntitySet};

    use super::*;

    #[test]
    fn classification_bearing_geometry_keeps_the_exact_legacy_gmsh_route() {
        let geometry = CanonicalGeometryV1::from_circular_hole(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.2, 0.2],
            0.05,
            vec![
                NamedEntitySet::new("fluid", 2, vec![0]),
                NamedEntitySet::new("inlet", 1, vec![0]),
                NamedEntitySet::new("outlet", 1, vec![1]),
                NamedEntitySet::new("walls", 1, vec![2, 3]),
                NamedEntitySet::new("cylinder", 1, vec![4]),
            ],
            1.0e-12,
        )
        .unwrap();
        assert_eq!(route(&geometry), MeshRoute::Gmsh);
        assert_eq!(GMSH_PROVIDER, "eqiora.gmsh-cli/4.15.2");
    }
}
