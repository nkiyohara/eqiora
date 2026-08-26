//! Immutable mesh intent and complete resolved provider choices.

use eqiora::artifact::{MeshProductionLineageEnvelopeV1, PlanarMeshQualityV1};
use eqiora::meshing::MeshQualityGate;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes};

use super::gmsh;
use super::request_error;
use super::source_owned::SourceOwnedPlan;
use crate::error::validation_error;
use crate::geometry::{PyGeometry, digest_to_hex};
use crate::panic_boundary;

/// Exact Gmsh provider selection.
#[pyclass(
    name = "GmshMesher",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PyGmshMesher {
    policy: PlanarMeshQualityV1,
}

#[pymethods]
impl PyGmshMesher {
    #[new]
    #[pyo3(signature = (*, maximum_boundary_error=1.0e-4, minimum_mean_ratio=1.0e-5, maximum_boundary_facets=50))]
    fn new(
        py: Python<'_>,
        maximum_boundary_error: f64,
        minimum_mean_ratio: f64,
        maximum_boundary_facets: usize,
    ) -> PyResult<Self> {
        Ok(Self {
            policy: validate_numerical_policy(
                py,
                maximum_boundary_error,
                minimum_mean_ratio,
                maximum_boundary_facets,
            )?,
        })
    }

    #[getter]
    const fn maximum_boundary_error(&self) -> f64 {
        self.policy.maximum_boundary_error_m()
    }

    #[getter]
    const fn minimum_mean_ratio(&self) -> f64 {
        self.policy.minimum_mean_ratio()
    }

    #[getter]
    const fn maximum_boundary_facets(&self) -> usize {
        self.policy.maximum_boundary_facets()
    }

    fn __repr__(&self) -> String {
        provider_repr("GmshMesher", self.policy)
    }
}

/// Deterministic in-process reference provider selection.
#[pyclass(
    name = "ReferenceMesher",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PyReferenceMesher {
    policy: PlanarMeshQualityV1,
}

#[pymethods]
impl PyReferenceMesher {
    #[new]
    #[pyo3(signature = (*, maximum_boundary_error=1.0e-4, minimum_mean_ratio=1.0e-5, maximum_boundary_facets=50))]
    fn new(
        py: Python<'_>,
        maximum_boundary_error: f64,
        minimum_mean_ratio: f64,
        maximum_boundary_facets: usize,
    ) -> PyResult<Self> {
        Ok(Self {
            policy: validate_numerical_policy(
                py,
                maximum_boundary_error,
                minimum_mean_ratio,
                maximum_boundary_facets,
            )?,
        })
    }

    #[getter]
    const fn maximum_boundary_error(&self) -> f64 {
        self.policy.maximum_boundary_error_m()
    }

    #[getter]
    const fn minimum_mean_ratio(&self) -> f64 {
        self.policy.minimum_mean_ratio()
    }

    #[getter]
    const fn maximum_boundary_facets(&self) -> usize {
        self.policy.maximum_boundary_facets()
    }

    fn __repr__(&self) -> String {
        provider_repr("ReferenceMesher", self.policy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MeshProviderPolicy {
    Gmsh(PyGmshMesher),
    Reference(PyReferenceMesher),
}

impl MeshProviderPolicy {
    pub(super) const fn policy(self) -> PlanarMeshQualityV1 {
        match self {
            Self::Gmsh(provider) => provider.policy,
            Self::Reference(provider) => provider.policy,
        }
    }

    fn production_lineage(
        self,
        geometry: &eqiora::geometry::CanonicalGeometryV1,
        mesh: &eqiora::artifact::SimplicialMeshEnvelopeV1,
        correspondence: &eqiora::artifact::GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<MeshProductionLineageEnvelopeV1, eqiora::Diagnostic> {
        match self {
            Self::Gmsh(_) => MeshProductionLineageEnvelopeV1::from_gmsh_4152_resources(
                self.policy(),
                geometry,
                mesh,
                correspondence,
            ),
            Self::Reference(_) => {
                MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
                    self.policy(),
                    geometry,
                    mesh,
                    correspondence,
                )
            }
        }
    }

    pub(super) fn validate_production_lineage(
        self,
        lineage: &MeshProductionLineageEnvelopeV1,
        geometry: &eqiora::geometry::CanonicalGeometryV1,
        mesh: &eqiora::artifact::SimplicialMeshEnvelopeV1,
        correspondence: &eqiora::artifact::GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<(), eqiora::Diagnostic> {
        match self {
            Self::Gmsh(_) => lineage.validate_against_gmsh_4152_resources(
                self.policy(),
                geometry,
                mesh,
                correspondence,
            ),
            Self::Reference(_) => lineage
                .validate_against_planar_circular_hole_reference_v1_resources(
                    self.policy(),
                    geometry,
                    mesh,
                    correspondence,
                ),
        }
    }

    fn to_python(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Gmsh(provider) => Py::new(py, provider).map(Py::into_any),
            Self::Reference(provider) => Py::new(py, provider).map(Py::into_any),
        }
    }

    fn representation(self) -> String {
        match self {
            Self::Gmsh(provider) => provider.__repr__(),
            Self::Reference(provider) => provider.__repr__(),
        }
    }
}

/// Explicit policy for a caller-supplied, untracked Gmsh MSH image.
#[pyclass(
    name = "GmshImport",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PyGmshImport {
    pub(super) maximum_boundary_error: f64,
    pub(super) minimum_mean_ratio: f64,
    pub(super) maximum_boundary_facets: usize,
}

#[pymethods]
impl PyGmshImport {
    #[new]
    #[pyo3(signature = (*, maximum_boundary_error=1.0e-4, minimum_mean_ratio=1.0e-5, maximum_boundary_facets=50))]
    fn new(
        py: Python<'_>,
        maximum_boundary_error: f64,
        minimum_mean_ratio: f64,
        maximum_boundary_facets: usize,
    ) -> PyResult<Self> {
        let _ = validate_numerical_policy(
            py,
            maximum_boundary_error,
            minimum_mean_ratio,
            maximum_boundary_facets,
        )?;
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
            "GmshImport(maximum_boundary_error={}, minimum_mean_ratio={}, maximum_boundary_facets={})",
            self.maximum_boundary_error, self.minimum_mean_ratio, self.maximum_boundary_facets,
        )
    }
}

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
    pub(super) provider: MeshProviderPolicy,
}

#[pymethods]
impl PyMeshRequest {
    #[new]
    #[pyo3(signature = (provider, /))]
    fn new(py: Python<'_>, provider: &Bound<'_, PyAny>) -> PyResult<Self> {
        let provider = if let Ok(provider) = provider.extract::<PyRef<'_, PyGmshMesher>>() {
            MeshProviderPolicy::Gmsh(*provider)
        } else if let Ok(provider) = provider.extract::<PyRef<'_, PyReferenceMesher>>() {
            MeshProviderPolicy::Reference(*provider)
        } else {
            return Err(request_error(
                py,
                "provider must be GmshMesher or ReferenceMesher",
            ));
        };
        Ok(Self { provider })
    }

    #[getter]
    fn provider(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.provider.to_python(py)
    }

    fn __repr__(&self) -> String {
        format!("MeshRequest({})", self.provider.representation())
    }
}

fn validate_numerical_policy(
    py: Python<'_>,
    maximum_boundary_error: f64,
    minimum_mean_ratio: f64,
    maximum_boundary_facets: usize,
) -> PyResult<PlanarMeshQualityV1> {
    if !maximum_boundary_error.is_finite() || maximum_boundary_error <= 0.0 {
        return Err(request_error(
            py,
            "maximum_boundary_error must be finite and positive",
        ));
    }
    PlanarMeshQualityV1::new(
        maximum_boundary_error,
        minimum_mean_ratio,
        maximum_boundary_facets,
    )
    .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))
}

fn provider_repr(name: &str, policy: PlanarMeshQualityV1) -> String {
    format!(
        "{name}(maximum_boundary_error={}, minimum_mean_ratio={}, maximum_boundary_facets={})",
        policy.maximum_boundary_error_m(),
        policy.minimum_mean_ratio(),
        policy.maximum_boundary_facets(),
    )
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
    pub(super) request: PyMeshRequest,
    pub(super) production: MeshProductionLineageEnvelopeV1,
    pub(super) resolved: ResolvedMeshPlan,
}

pub(super) enum ResolvedMeshPlan {
    Gmsh(Box<GmshPlan>),
    SourceOwned(Box<SourceOwnedPlan>),
}

pub(super) struct GmshPlan {
    pub(super) source: eqiora::geometry::CanonicalGeometryV1,
    pub(super) generated: gmsh::GeneratedGmshMesh,
}

#[pymethods]
impl PyMeshPlan {
    #[getter]
    fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[getter]
    fn provider(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.request.provider.to_python(py)
    }

    #[getter]
    fn request(&self, py: Python<'_>) -> PyResult<Py<PyMeshRequest>> {
        Py::new(py, self.request)
    }

    /// Canonical provider occurrence retained by the resolved plan.
    #[getter]
    fn production_lineage_bytes(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        self.production
            .canonical_json()
            .map(|bytes| PyBytes::new(py, &bytes).unbind())
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    /// Identity of the canonical provider occurrence.
    #[getter]
    fn production_lineage_digest(&self, py: Python<'_>) -> PyResult<String> {
        self.production
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    /// Effective number of boundary facets selected by the provider.
    #[getter]
    fn boundary_facets(&self) -> usize {
        match &self.resolved {
            ResolvedMeshPlan::Gmsh(plan) => plan.generated.edge_facets[4].len(),
            ResolvedMeshPlan::SourceOwned(plan) => plan
                .boundary_facets()
                .expect("an admitted source-owned plan retains its circular frontier"),
        }
    }

    /// Measured minimum mean ratio achieved by the resolved mesh.
    #[getter]
    fn achieved_minimum_mean_ratio(&self) -> f64 {
        match &self.resolved {
            ResolvedMeshPlan::Gmsh(plan) => plan
                .generated
                .mesh
                .mesh()
                .quality_report()
                .minimum_mean_ratio(),
            ResolvedMeshPlan::SourceOwned(plan) => {
                plan.mesh.mesh().quality_report().minimum_mean_ratio()
            }
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshPlan(provider={}, source_digest={:?}, boundary_facets={})",
            self.request.provider.representation(),
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
        let policy = request.provider.policy();
        let quality_gate = MeshQualityGate::new(policy.minimum_mean_ratio())
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        let resolved = match request.provider {
            MeshProviderPolicy::Gmsh(_) => {
                let seed = SourceOwnedPlan::resolve(
                    geometry.geometry(),
                    policy.maximum_boundary_error_m(),
                    policy.maximum_boundary_facets(),
                    quality_gate,
                )
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
                let generated =
                    gmsh::generate(geometry.geometry(), &seed.correspondence, quality_gate)
                        .map_err(|diagnostic| {
                            validation_error(py, std::slice::from_ref(&diagnostic))
                        })?;
                ResolvedMeshPlan::Gmsh(Box::new(GmshPlan {
                    source: geometry.geometry().clone(),
                    generated,
                }))
            }
            MeshProviderPolicy::Reference(_) => SourceOwnedPlan::resolve(
                geometry.geometry(),
                policy.maximum_boundary_error_m(),
                policy.maximum_boundary_facets(),
                quality_gate,
            )
            .map(Box::new)
            .map(ResolvedMeshPlan::SourceOwned)
            .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?,
        };
        let (mesh, correspondence) = match &resolved {
            ResolvedMeshPlan::Gmsh(plan) => (&plan.generated.mesh, &plan.generated.correspondence),
            ResolvedMeshPlan::SourceOwned(plan) => (&plan.mesh, &plan.correspondence),
        };
        let production = request
            .provider
            .production_lineage(geometry.geometry(), mesh, correspondence)
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Ok(PyMeshPlan {
            source_digest: digest_to_hex(&geometry.geometry().digest_bytes()),
            request,
            production,
            resolved,
        })
    })
}
