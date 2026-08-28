//! Immutable mesh intent and complete resolved provider choices.

use eqiora::artifact::{
    AffineTriangleMeshCellsV1, CartesianMeshCellsV1, MeshProductionLineageEnvelopeV1,
    PlanarMeshQualityV1,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyTuple};

use super::gmsh;
use super::request_error;
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
    pub(super) policy: PlanarMeshQualityV1,
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

/// Deterministic structured Cartesian provider selection.
#[pyclass(
    name = "CartesianMesher",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PyCartesianMesher {
    pub(super) policy: CartesianMeshCellsV1,
}

#[pymethods]
impl PyCartesianMesher {
    #[new]
    #[pyo3(signature = (*, cells))]
    fn new(py: Python<'_>, cells: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let parsed = parse_cells(py, cells)?;
        let policy = CartesianMeshCellsV1::new(parsed)
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Ok(Self { policy })
    }

    #[getter]
    const fn cells(&self) -> (usize, usize) {
        let [nx, ny] = self.policy.cells();
        (nx, ny)
    }

    fn __repr__(&self) -> String {
        let (nx, ny) = self.cells();
        format!("CartesianMesher(cells=({nx}, {ny}))")
    }
}

/// Deterministic fixed-diagonal affine-triangle provider selection.
#[pyclass(
    name = "AffineTriangleMesher",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PyAffineTriangleMesher {
    pub(super) policy: AffineTriangleMeshCellsV1,
}

#[pymethods]
impl PyAffineTriangleMesher {
    #[new]
    #[pyo3(signature = (*, cells))]
    fn new(py: Python<'_>, cells: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let policy = AffineTriangleMeshCellsV1::new(parse_cells(py, cells)?)
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Ok(Self { policy })
    }

    #[getter]
    const fn cells(&self) -> (usize, usize) {
        let [nx, ny] = self.policy.cells();
        (nx, ny)
    }

    #[getter]
    const fn diagonal(&self) -> &'static str {
        self.policy.diagonal()
    }

    fn __repr__(&self) -> String {
        let (nx, ny) = self.cells();
        format!("AffineTriangleMesher(cells=({nx}, {ny}))")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MeshProviderPolicy {
    Gmsh(PyGmshMesher),
    Cartesian(PyCartesianMesher),
    AffineTriangle(PyAffineTriangleMesher),
}

impl MeshProviderPolicy {
    pub(super) fn production_lineage(
        self,
        geometry: &eqiora::geometry::CanonicalGeometryV1,
        mesh: &eqiora::artifact::SimplicialMeshEnvelopeV1,
        correspondence: &eqiora::artifact::GeometryMeshCorrespondenceEnvelopeV1,
    ) -> Result<MeshProductionLineageEnvelopeV1, eqiora::Diagnostic> {
        match self {
            Self::Gmsh(provider) => MeshProductionLineageEnvelopeV1::from_gmsh_4152_resources(
                provider.policy,
                geometry,
                mesh,
                correspondence,
            ),
            Self::Cartesian(_) => unreachable!("Cartesian lineage uses Cartesian resources"),
            Self::AffineTriangle(provider) => {
                MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
                    provider.policy,
                    geometry,
                    mesh,
                    correspondence,
                )
            }
        }
    }

    fn to_python(self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            Self::Gmsh(provider) => Py::new(py, provider).map(Py::into_any),
            Self::Cartesian(provider) => Py::new(py, provider).map(Py::into_any),
            Self::AffineTriangle(provider) => Py::new(py, provider).map(Py::into_any),
        }
    }

    fn representation(self) -> String {
        match self {
            Self::Gmsh(provider) => provider.__repr__(),
            Self::Cartesian(provider) => provider.__repr__(),
            Self::AffineTriangle(provider) => provider.__repr__(),
        }
    }
}

fn extract_provider(py: Python<'_>, provider: &Bound<'_, PyAny>) -> PyResult<MeshProviderPolicy> {
    if let Ok(provider) = provider.extract::<PyRef<'_, PyGmshMesher>>() {
        Ok(MeshProviderPolicy::Gmsh(*provider))
    } else if let Ok(provider) = provider.extract::<PyRef<'_, PyCartesianMesher>>() {
        Ok(MeshProviderPolicy::Cartesian(*provider))
    } else if let Ok(provider) = provider.extract::<PyRef<'_, PyAffineTriangleMesher>>() {
        Ok(MeshProviderPolicy::AffineTriangle(*provider))
    } else {
        Err(request_error(
            py,
            "provider must be GmshMesher, CartesianMesher, or AffineTriangleMesher",
        ))
    }
}

fn parse_cells(py: Python<'_>, cells: &Bound<'_, PyTuple>) -> PyResult<[usize; 2]> {
    if cells.len() != 2 {
        return Err(request_error(py, "cells must contain exactly (nx, ny)"));
    }
    let mut parsed = [0; 2];
    for (axis, value) in cells.iter().enumerate() {
        if value.is_instance_of::<PyBool>() {
            return Err(request_error(py, "cells must contain positive integers"));
        }
        parsed[axis] = value
            .extract::<usize>()
            .map_err(|_| request_error(py, "cells must contain positive integers"))?;
    }
    Ok(parsed)
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
    pub(super) source: eqiora::geometry::CanonicalGeometryV1,
    pub(super) provider: MeshProviderPolicy,
    pub(super) planned: PlannedMesh,
}

pub(super) enum PlannedMesh {
    Gmsh(gmsh::GmshSizingReceipt),
    Cartesian { boundary_facets: usize },
    AffineTriangle { boundary_facets: usize },
}

#[pymethods]
impl PyMeshPlan {
    #[getter]
    fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[getter]
    fn provider(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.provider.to_python(py)
    }

    /// Planned number of boundary facets selected by the provider policy.
    #[getter]
    fn boundary_facets(&self) -> usize {
        match (&self.planned, self.provider) {
            (PlannedMesh::Gmsh(sizing), MeshProviderPolicy::Gmsh(_)) => sizing.circle_segments(),
            (PlannedMesh::Cartesian { boundary_facets }, MeshProviderPolicy::Cartesian(_))
            | (
                PlannedMesh::AffineTriangle { boundary_facets },
                MeshProviderPolicy::AffineTriangle(_),
            ) => *boundary_facets,
            _ => unreachable!("planned mesh and provider remain paired"),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MeshPlan(provider={}, source_digest={:?}, boundary_facets={})",
            self.provider.representation(),
            self.source_digest,
            self.boundary_facets(),
        )
    }
}

/// Resolve one complete provider plan for the exact supplied Geometry.
#[pyfunction]
#[pyo3(signature = (geometry, provider, /))]
pub(super) fn resolve(
    py: Python<'_>,
    geometry: &PyGeometry,
    provider: &Bound<'_, PyAny>,
) -> PyResult<PyMeshPlan> {
    panic_boundary(py, || {
        let provider = extract_provider(py, provider)?;
        let planned = match provider {
            MeshProviderPolicy::Gmsh(provider) => {
                let sizing =
                    gmsh::plan(geometry.geometry(), provider.policy).map_err(|diagnostic| {
                        validation_error(py, std::slice::from_ref(&diagnostic))
                    })?;
                PlannedMesh::Gmsh(sizing)
            }
            MeshProviderPolicy::Cartesian(provider) => {
                if geometry.geometry().planar_rectangle_bounds().is_none() {
                    return Err(request_error(
                        py,
                        "CartesianMesher requires planar rectangle Geometry v2",
                    ));
                }
                PlannedMesh::Cartesian {
                    boundary_facets: rectangle_boundary_facets(py, provider.policy.cells())?,
                }
            }
            MeshProviderPolicy::AffineTriangle(provider) => {
                let is_rectangle = geometry.geometry().planar_rectangle_bounds().is_some();
                let is_partition = geometry
                    .geometry()
                    .planar_adjacent_rectangle_partition()
                    .is_some();
                if !is_rectangle && !is_partition {
                    return Err(request_error(
                        py,
                        "AffineTriangleMesher requires planar rectangle or adjacent-partition Geometry v2",
                    ));
                }
                if is_partition && provider.policy.cells() != [2, 2] {
                    return Err(request_error(
                        py,
                        "the admitted adjacent-partition AffineTriangleMesher plan requires cells=(2, 2)",
                    ));
                }
                PlannedMesh::AffineTriangle {
                    boundary_facets: rectangle_boundary_facets(py, provider.policy.cells())?,
                }
            }
        };
        Ok(PyMeshPlan {
            source_digest: digest_to_hex(&geometry.geometry().digest_bytes()),
            source: geometry.geometry().clone(),
            provider,
            planned,
        })
    })
}

fn rectangle_boundary_facets(py: Python<'_>, cells: [usize; 2]) -> PyResult<usize> {
    cells[0]
        .checked_add(cells[1])
        .and_then(|sum| sum.checked_mul(2))
        .ok_or_else(|| request_error(py, "planned rectangle boundary-facet count overflows usize"))
}
