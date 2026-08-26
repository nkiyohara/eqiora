//! Immutable accepted Mesh publication and NumPy projections.

use std::sync::Mutex;

use eqiora::Diagnostic;
use eqiora::artifact::{
    AcceptedCircularHoleChordalRealizationV1, CartesianMeshEnvelopeV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, MeshProductionLineageEnvelopeV1, RealizationEnvelopeV1,
    SimplicialMeshEnvelopeV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::{CanonicalGeometryV1, NamedEntitySet};
use eqiora::meshing::{MeshEntity, MeshTopology};
use eqiora_numerics::AuthenticatedCommonMesh;
use numpy::PyArray2;
use pyo3::exceptions::{PyOverflowError, PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyBytes, PyDict, PyTuple};
use sha2::{Digest, Sha256};

use super::plan::{PyGmshImport, PyMeshPlan, ResolvedMeshPlan};
use super::request_error;
use crate::error::{diagnostic_error, validation_error};
use crate::geometry::{PyGeometry, PyGeometrySelection, digest_to_hex};
use crate::matrix::ReadOnlyMatrix;
use crate::notebook_mime::{TEXT_MIME, WIDGET_MIME, select_mime_types};
use crate::panic_boundary;

const REFERENCE_SOURCE_DIGEST: &str =
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";
const REFERENCE_CANONICAL_BYTES: usize = 42_388;
const REFERENCE_CANONICAL_RAW_SHA256: &str =
    "9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2";
const REFERENCE_MESH_DIGEST: &str =
    "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b";
const REFERENCE_COORDINATES_SHA256: &str =
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d";
const REFERENCE_TRIANGLES_SHA256: &str =
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642";
const MESH_DIGEST_DOMAIN: &[u8] = b"eqiora.simplicial-mesh-envelope/v1\0";
const UNSUPPORTED_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: this N1 viewer supports only the exact accepted Gmsh 4.15.2 circular-hole Mesh (662 vertices, 1210 triangles).";
const CORRUPT_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: the installed Eqiora Notebook presentation runtime or assets are incomplete. Reinstall eqiora[notebook].";

/// Immutable source-bound accepted Mesh.
#[pyclass(
    name = "Mesh",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object,
    weakref
)]
pub(crate) struct PyMesh {
    source: AcceptedMeshSource,
    lineage: MeshLineage,
    canonical_bytes: Vec<u8>,
    coordinates: ReadOnlyMatrix<f64>,
    cells: ReadOnlyMatrix<u32>,
    presentation: Mutex<PresentationState>,
}

enum PresentationState {
    Empty,
    Creating,
    Ready(Py<PyAny>),
}

enum AcceptedMeshSource {
    Chordal {
        accepted: Box<AcceptedCircularHoleChordalRealizationV1>,
        external_import: Option<Box<ExternalImportLineage>>,
    },
    SourceOwned {
        geometry: Box<CanonicalGeometryV1>,
        mesh: Box<SimplicialMeshEnvelopeV1>,
        correspondence: Box<GeometryMeshCorrespondenceEnvelopeV1>,
        production: Box<MeshProductionLineageEnvelopeV1>,
        provider_observation: SourceOwnedProviderObservation,
    },
    SourceOwnedCartesian {
        geometry: Box<CanonicalGeometryV1>,
        mesh: Box<CartesianMeshEnvelopeV1>,
        correspondence: Box<GeometryMeshCorrespondenceEnvelopeV1>,
        production: Box<MeshProductionLineageEnvelopeV1>,
    },
    Cartesian,
}

enum SourceOwnedProviderObservation {
    Reference,
    Gmsh4152 { output: Box<[u8]> },
}

/// Exact authenticated Cartesian resources admitted for downstream native consumers.
pub(crate) struct AuthenticatedCartesianResources<'a> {
    pub(crate) geometry: &'a CanonicalGeometryV1,
    pub(crate) mesh: &'a CartesianMeshEnvelopeV1,
    pub(crate) correspondence: &'a GeometryMeshCorrespondenceEnvelopeV1,
    pub(crate) production: &'a MeshProductionLineageEnvelopeV1,
}

struct ExternalImportLineage {
    canonical_bytes: Vec<u8>,
    digest: String,
}

struct MeshLineage {
    source_digest: String,
    realized_geometry_digest: String,
    mesh_digest: String,
    correspondence_digest: String,
    realization_digest: Option<String>,
    dimension: usize,
    vertex_count: usize,
    cell_count: usize,
}

#[pymethods]
impl PyMesh {
    pub(crate) fn exact_mesh_digest(&self) -> &str {
        &self.lineage.mesh_digest
    }

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

    /// Canonical provider occurrence that produced this common Mesh.
    #[getter]
    fn production_lineage_bytes(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        let production = self.production_lineage().ok_or_else(|| {
            capability_error(py, "this Mesh has no common production-lineage artifact")
        })?;
        production
            .canonical_json()
            .map(|bytes| PyBytes::new(py, &bytes).unbind())
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    /// Identity of the canonical provider occurrence.
    #[getter]
    fn production_lineage_digest(&self, py: Python<'_>) -> PyResult<String> {
        let production = self.production_lineage().ok_or_else(|| {
            capability_error(py, "this Mesh has no common production-lineage artifact")
        })?;
        production
            .digest()
            .map(|digest| digest.to_string())
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    /// Identity of the complete exact-source realization binding.
    #[getter]
    fn realization_digest(&self, py: Python<'_>) -> PyResult<&str> {
        self.lineage.realization_digest.as_deref().ok_or_else(|| {
            capability_error(
                py,
                "realization_digest is unavailable for a source-owned Geometry v2 Mesh",
            )
        })
    }

    /// Canonical external-import manifest, or None for non-imported Meshes.
    #[getter]
    fn external_import_manifest_bytes(&self, py: Python<'_>) -> Option<Py<PyBytes>> {
        self.external_import()
            .map(|lineage| PyBytes::new(py, &lineage.canonical_bytes).unbind())
    }

    /// Identity of the external-import manifest, or None otherwise.
    #[getter]
    fn external_import_manifest_digest(&self) -> Option<&str> {
        self.external_import()
            .map(|lineage| lineage.digest.as_str())
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
            AcceptedMeshSource::Chordal { accepted, .. } => {
                Ok(accepted.mesh().mesh().quality_report().minimum_mean_ratio())
            }
            AcceptedMeshSource::SourceOwned { mesh, .. } => {
                Ok(mesh.mesh().quality_report().minimum_mean_ratio())
            }
            AcceptedMeshSource::SourceOwnedCartesian { .. } | AcceptedMeshSource::Cartesian => {
                Err(capability_error(
                    py,
                    "minimum_mean_ratio is not defined for this Cartesian Mesh",
                ))
            }
        }
    }

    #[getter]
    fn selection_names(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let names = match &self.source {
            AcceptedMeshSource::Chordal { accepted, .. } => accepted
                .source()
                .entity_sets()
                .iter()
                .map(NamedEntitySet::name)
                .collect::<Vec<_>>(),
            AcceptedMeshSource::SourceOwned { geometry, .. } => geometry
                .entity_sets()
                .iter()
                .map(NamedEntitySet::name)
                .collect::<Vec<_>>(),
            AcceptedMeshSource::SourceOwnedCartesian { geometry, .. } => geometry
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
    fn selection_entity_count(&self, py: Python<'_>, name: &Bound<'_, PyAny>) -> PyResult<usize> {
        let selection;
        let (name, expected_dimension) = if let Ok(name) = name.extract::<&str>() {
            (name, None)
        } else if let Ok(value) = name.extract::<PyRef<'_, PyGeometrySelection>>() {
            selection = value;
            if selection.bound_source_digest() != self.lineage.source_digest {
                let diagnostic = Diagnostic::error(
                    codes::INVALID_ARTIFACT,
                    "GeometrySelection belongs to a foreign or stale Geometry revision",
                );
                return Err(validation_error(py, std::slice::from_ref(&diagnostic)));
            }
            (
                selection.canonical_name(),
                Some(selection.canonical_dimension()),
            )
        } else {
            return Err(PyTypeError::new_err(
                "name must be a str or GeometrySelection",
            ));
        };
        match &self.source {
            AcceptedMeshSource::Chordal { accepted, .. } => accepted
                .correspondence()
                .region_entity_set_entities(accepted.realized_geometry(), name)
                .and_then(|entities| validated_entity_count(entities, expected_dimension))
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic))),
            AcceptedMeshSource::SourceOwned {
                geometry,
                correspondence,
                ..
            } => correspondence
                .planar_circular_hole_v2_entity_set_entities(geometry, name)
                .and_then(|entities| validated_entity_count(entities, expected_dimension))
                .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic))),
            AcceptedMeshSource::SourceOwnedCartesian {
                geometry,
                correspondence,
                ..
            } => correspondence
                .planar_rectangle_v2_entity_set_entities(geometry, name)
                .and_then(|entities| validated_entity_count(entities, expected_dimension))
                .map_err(|diagnostic| validation_error(py, &[diagnostic])),
            AcceptedMeshSource::Cartesian => Err(capability_error(
                py,
                "this Cartesian Mesh publishes no named selection membership",
            )),
        }
    }

    fn __repr__(&self, _py: Python<'_>) -> PyResult<String> {
        Ok(self.representation())
    }

    #[pyo3(signature = (include=None, exclude=None))]
    fn _repr_mimebundle_(
        slf: Py<Self>,
        py: Python<'_>,
        include: Option<&Bound<'_, PyAny>>,
        exclude: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let selected = select_mime_types(py, include, exclude)?;
        let output = PyDict::new(py);
        if selected.is_empty() {
            return Ok(output.unbind());
        }

        let mesh = slf.get();
        let representation = mesh.representation();
        if !selected.contains(WIDGET_MIME) {
            if selected.contains(TEXT_MIME) {
                output.set_item(TEXT_MIME, representation)?;
            }
            return Ok(output.unbind());
        }

        if !mesh.is_exact_notebook_reference() {
            if selected.contains(TEXT_MIME) {
                output.set_item(
                    TEXT_MIME,
                    format!("{representation}\n{UNSUPPORTED_NOTEBOOK_MESSAGE}"),
                )?;
            }
            return Ok(output.unbind());
        }

        let coordinates = mesh.coordinates.numpy(py)?;
        let triangles = mesh.cells.numpy(py)?;
        let token = PyDict::new(py);
        token.set_item("source_digest", &mesh.lineage.source_digest)?;
        token.set_item("canonical_bytes", PyBytes::new(py, &mesh.canonical_bytes))?;
        token.set_item("canonical_raw_sha256", REFERENCE_CANONICAL_RAW_SHA256)?;
        token.set_item("mesh_digest", &mesh.lineage.mesh_digest)?;
        token.set_item("coordinates", coordinates.bind(py))?;
        token.set_item("triangles", triangles.bind(py))?;
        token.set_item("coordinates_sha256", REFERENCE_COORDINATES_SHA256)?;
        token.set_item("triangles_sha256", REFERENCE_TRIANGLES_SHA256)?;

        let current = {
            let mut state = mesh
                .presentation
                .lock()
                .map_err(|_| PyRuntimeError::new_err("Mesh presentation lock is poisoned"))?;
            match std::mem::replace(&mut *state, PresentationState::Creating) {
                PresentationState::Empty => None,
                PresentationState::Ready(delegate) => Some(delegate),
                PresentationState::Creating => {
                    *state = PresentationState::Creating;
                    if selected.contains(TEXT_MIME) {
                        output.set_item(
                            TEXT_MIME,
                            format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                        )?;
                    }
                    return Ok(output.unbind());
                }
            }
        };

        let outcome = call_presentation_adapter(py, slf.bind(py), &token, current.as_ref());
        match outcome {
            Ok(AdapterOutcome::Absent) => {
                mesh.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(TEXT_MIME, representation)?;
                }
            }
            Ok(AdapterOutcome::Unsupported) => {
                if let Some(delegate) = current {
                    close_delegate(py, &delegate);
                }
                mesh.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(
                        TEXT_MIME,
                        format!("{representation}\n{UNSUPPORTED_NOTEBOOK_MESSAGE}"),
                    )?;
                }
            }
            Ok(AdapterOutcome::Rich {
                delegate,
                widget_view,
            }) => {
                mesh.set_presentation_state(PresentationState::Ready(delegate))?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(TEXT_MIME, representation)?;
                }
                output.set_item(WIDGET_MIME, widget_view)?;
            }
            Err(delegate) => {
                if let Some(delegate) = delegate.or(current) {
                    close_delegate(py, &delegate);
                }
                mesh.set_presentation_state(PresentationState::Empty)?;
                if selected.contains(TEXT_MIME) {
                    output.set_item(
                        TEXT_MIME,
                        format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                    )?;
                }
            }
        }
        Ok(output.unbind())
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
            source: AcceptedMeshSource::Chordal {
                accepted: Box::new(accepted),
                external_import: None,
            },
            lineage: MeshLineage {
                source_digest,
                realized_geometry_digest,
                mesh_digest,
                correspondence_digest,
                realization_digest: Some(realization_digest),
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
            presentation: Mutex::new(PresentationState::Empty),
        })
    }

    fn from_source_owned(
        py: Python<'_>,
        plan: &super::source_owned::SourceOwnedPlan,
        production: &MeshProductionLineageEnvelopeV1,
    ) -> PyResult<Self> {
        plan.revalidate(&plan.source)
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Self::from_source_parts(
            py,
            &plan.source,
            &plan.mesh,
            &plan.correspondence,
            production,
            SourceOwnedProviderObservation::Reference,
        )
    }

    fn from_source_parts(
        py: Python<'_>,
        source: &CanonicalGeometryV1,
        accepted_mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        production: &MeshProductionLineageEnvelopeV1,
        provider_observation: SourceOwnedProviderObservation,
    ) -> PyResult<Self> {
        let dimension = accepted_mesh.dimension();
        let mesh = accepted_mesh.mesh();
        let vertex_count = mesh.vertices().len();
        let cell_count = mesh.cells().len();
        let (coordinates, cells) = project_simplicial_mesh(py, mesh, dimension)?;
        let source_digest = digest_to_hex(&source.digest_bytes());
        let mesh_digest = accepted_mesh
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let correspondence_digest = correspondence
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let canonical_bytes = accepted_mesh
            .canonical_json()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        Ok(Self {
            source: AcceptedMeshSource::SourceOwned {
                geometry: Box::new(source.clone()),
                mesh: Box::new(accepted_mesh.clone()),
                correspondence: Box::new(correspondence.clone()),
                production: Box::new(production.clone()),
                provider_observation,
            },
            lineage: MeshLineage {
                source_digest: source_digest.clone(),
                realized_geometry_digest: source_digest,
                mesh_digest,
                correspondence_digest,
                realization_digest: None,
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
            presentation: Mutex::new(PresentationState::Empty),
        })
    }

    fn from_source_owned_cartesian(
        py: Python<'_>,
        source: &CanonicalGeometryV1,
        accepted_mesh: &CartesianMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        production: &MeshProductionLineageEnvelopeV1,
    ) -> PyResult<Self> {
        let policy = production.cartesian_cells().ok_or_else(|| {
            request_error(
                py,
                "Cartesian MeshPlan has a non-Cartesian production policy",
            )
        })?;
        correspondence
            .validate_against_planar_rectangle_v2_cartesian(source, accepted_mesh, policy.cells())
            .and_then(|()| {
                production.validate_against_structured_cartesian_v1_resources(
                    policy,
                    source,
                    accepted_mesh,
                    correspondence,
                )
            })
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        let dimension = accepted_mesh.dimension();
        let native = accepted_mesh.mesh();
        let vertex_count = native
            .entity_count(0)
            .ok_or_else(|| PyRuntimeError::new_err("Cartesian Mesh omitted its vertices"))?;
        let cell_count = native.entity_count(dimension).ok_or_else(|| {
            PyRuntimeError::new_err("Cartesian Mesh omitted its top-dimensional cells")
        })?;
        let (coordinates, cells) =
            project_cartesian_mesh(py, native, dimension, vertex_count, cell_count)?;
        let source_digest = digest_to_hex(&source.digest_bytes());
        let mesh_digest = accepted_mesh
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let correspondence_digest = correspondence
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let canonical_bytes = accepted_mesh
            .canonical_json()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        let published = Self {
            source: AcceptedMeshSource::SourceOwnedCartesian {
                geometry: Box::new(source.clone()),
                mesh: Box::new(accepted_mesh.clone()),
                correspondence: Box::new(correspondence.clone()),
                production: Box::new(production.clone()),
            },
            lineage: MeshLineage {
                source_digest: source_digest.clone(),
                realized_geometry_digest: source_digest,
                mesh_digest,
                correspondence_digest,
                realization_digest: None,
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
            presentation: Mutex::new(PresentationState::Empty),
        };
        let authenticated = published
            .authenticated_cartesian_resources()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .ok_or_else(|| PyRuntimeError::new_err("Cartesian publication lost its resources"))?;
        let _ = (
            authenticated.geometry,
            authenticated.mesh,
            authenticated.correspondence,
            authenticated.production,
        );
        Ok(published)
    }

    fn from_imported(py: Python<'_>, imported: super::gmsh::ImportedGmshMesh) -> PyResult<Self> {
        let manifest_digest = imported
            .manifest
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        let manifest_bytes = imported
            .manifest
            .canonical_json()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        let mut mesh = Self::from_accepted(py, imported.accepted)?;
        let AcceptedMeshSource::Chordal {
            external_import, ..
        } = &mut mesh.source
        else {
            unreachable!("from_accepted always publishes a chordal Mesh")
        };
        *external_import = Some(Box::new(ExternalImportLineage {
            canonical_bytes: manifest_bytes,
            digest: manifest_digest,
        }));
        Ok(mesh)
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
                realization_digest: Some(realization_digest),
                dimension,
                vertex_count,
                cell_count,
            },
            canonical_bytes,
            coordinates,
            cells,
            presentation: Mutex::new(PresentationState::Empty),
        })
    }

    pub(crate) fn accepted_chordal(
        &self,
        py: Python<'_>,
    ) -> PyResult<&AcceptedCircularHoleChordalRealizationV1> {
        match &self.source {
            AcceptedMeshSource::Chordal { accepted, .. } => Ok(accepted),
            AcceptedMeshSource::SourceOwned { .. } => Err(capability_error(
                py,
                "this operation requires the legacy accepted affine-triangle realization",
            )),
            AcceptedMeshSource::SourceOwnedCartesian { .. } => Err(capability_error(
                py,
                "this operation requires an accepted affine-triangle Mesh",
            )),
            AcceptedMeshSource::Cartesian => Err(capability_error(
                py,
                "this operation requires an accepted affine-triangle Mesh",
            )),
        }
    }

    fn representation(&self) -> String {
        format!(
            "Mesh(dimension={}, vertices={}, cells={}, digest={:?})",
            self.dimension(),
            self.vertex_count(),
            self.cell_count(),
            self.digest(),
        )
    }

    fn is_exact_notebook_reference(&self) -> bool {
        if !matches!(self.source, AcceptedMeshSource::Chordal { .. })
            || self.lineage.source_digest != REFERENCE_SOURCE_DIGEST
            || self.canonical_bytes.len() != REFERENCE_CANONICAL_BYTES
            || self.lineage.mesh_digest != REFERENCE_MESH_DIGEST
        {
            return false;
        }
        let raw = Sha256::digest(&self.canonical_bytes);
        if hex_digest(&raw) != REFERENCE_CANONICAL_RAW_SHA256 {
            return false;
        }
        let mut framed = Sha256::new();
        framed.update(MESH_DIGEST_DOMAIN);
        framed.update(&self.canonical_bytes);
        hex_digest(&framed.finalize()) == REFERENCE_MESH_DIGEST
    }

    fn external_import(&self) -> Option<&ExternalImportLineage> {
        match &self.source {
            AcceptedMeshSource::Chordal {
                external_import, ..
            } => external_import.as_deref(),
            AcceptedMeshSource::SourceOwned { .. } => None,
            AcceptedMeshSource::SourceOwnedCartesian { .. } => None,
            AcceptedMeshSource::Cartesian => None,
        }
    }

    fn production_lineage(&self) -> Option<&MeshProductionLineageEnvelopeV1> {
        match &self.source {
            AcceptedMeshSource::SourceOwned { production, .. } => Some(production),
            AcceptedMeshSource::SourceOwnedCartesian { production, .. } => Some(production),
            AcceptedMeshSource::Chordal { .. } | AcceptedMeshSource::Cartesian => None,
        }
    }

    fn gmsh_provider_output(&self) -> Option<&[u8]> {
        match &self.source {
            AcceptedMeshSource::SourceOwned {
                provider_observation: SourceOwnedProviderObservation::Gmsh4152 { output },
                ..
            } => Some(output),
            _ => None,
        }
    }

    /// Return only exact replay-authenticated Cartesian resources.
    pub(crate) fn authenticated_cartesian_resources(
        &self,
    ) -> Result<Option<AuthenticatedCartesianResources<'_>>, Diagnostic> {
        let AcceptedMeshSource::SourceOwnedCartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } = &self.source
        else {
            return Ok(None);
        };
        let policy = production.cartesian_cells().ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "Cartesian Mesh has a non-Cartesian production policy",
            )
        })?;
        correspondence.validate_against_planar_rectangle_v2_cartesian(
            geometry,
            mesh,
            policy.cells(),
        )?;
        production.validate_against_structured_cartesian_v1_resources(
            policy,
            geometry,
            mesh,
            correspondence,
        )?;
        Ok(Some(AuthenticatedCartesianResources {
            geometry,
            mesh,
            correspondence,
            production,
        }))
    }

    /// Reauthenticate this exact published occurrence for the common resolver.
    pub(crate) fn authenticated_common_mesh(
        &self,
    ) -> Result<Option<AuthenticatedCommonMesh>, Diagnostic> {
        match &self.source {
            AcceptedMeshSource::SourceOwnedCartesian {
                geometry,
                mesh,
                correspondence,
                production,
            } => AuthenticatedCommonMesh::structured_cartesian(
                (**geometry).clone(),
                (**mesh).clone(),
                (**correspondence).clone(),
                (**production).clone(),
            )
            .map(Some),
            AcceptedMeshSource::SourceOwned {
                geometry,
                mesh,
                correspondence,
                production,
                provider_observation: SourceOwnedProviderObservation::Reference,
            } => AuthenticatedCommonMesh::planar_reference(
                (**geometry).clone(),
                (**mesh).clone(),
                (**correspondence).clone(),
                (**production).clone(),
            )
            .map(Some),
            AcceptedMeshSource::SourceOwned {
                geometry,
                production,
                provider_observation: SourceOwnedProviderObservation::Gmsh4152 { output },
                ..
            } => {
                let policy = production.planar_mesh_quality().ok_or_else(|| {
                    Diagnostic::error(
                        codes::INVALID_ARTIFACT,
                        "Gmsh common Mesh has a non-planar production policy",
                    )
                })?;
                AuthenticatedCommonMesh::gmsh_4152((**geometry).clone(), policy, output.to_vec())
                    .map(Some)
            }
            AcceptedMeshSource::Chordal { .. } | AcceptedMeshSource::Cartesian => Ok(None),
        }
    }

    fn set_presentation_state(&self, next: PresentationState) -> PyResult<()> {
        let mut state = self
            .presentation
            .lock()
            .map_err(|_| PyRuntimeError::new_err("Mesh presentation lock is poisoned"))?;
        *state = next;
        Ok(())
    }
}

/// Import one complete Gmsh MSH 4.1 image into the common accepted Mesh.
#[pyfunction]
#[pyo3(signature = (geometry, source, /, *, policy))]
pub(super) fn import_gmsh(
    py: Python<'_>,
    geometry: &PyGeometry,
    source: &[u8],
    policy: PyRef<'_, PyGmshImport>,
) -> PyResult<PyMesh> {
    panic_boundary(py, || {
        let geometry = geometry.geometry().clone();
        let source = source.to_vec();
        let policy = *policy;
        let imported = py.detach(move || {
            let quality_gate = eqiora::meshing::MeshQualityGate::new(policy.minimum_mean_ratio)?;
            let reference = AcceptedCircularHoleChordalRealizationV1::from_reference(
                &geometry,
                policy.maximum_boundary_error,
                policy.maximum_boundary_facets,
                quality_gate,
            )?;
            super::gmsh::import(&source, &reference, quality_gate)
        });
        imported
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
            .and_then(|imported| PyMesh::from_imported(py, imported))
    })
}

enum AdapterOutcome {
    Absent,
    Unsupported,
    Rich {
        delegate: Py<PyAny>,
        widget_view: Py<PyAny>,
    },
}

fn call_presentation_adapter(
    py: Python<'_>,
    mesh: &Bound<'_, PyMesh>,
    token: &Bound<'_, PyDict>,
    current: Option<&Py<PyAny>>,
) -> Result<AdapterOutcome, Option<Py<PyAny>>> {
    let module = py.import("eqiora._presentation").map_err(|_| None)?;
    let adapter = module.getattr("mesh_mimebundle").map_err(|_| None)?;
    let current = current.map_or_else(|| py.None(), |value| value.clone_ref(py));
    let result = adapter.call1((mesh, token, current)).map_err(|_| None)?;
    let tuple = result.cast::<PyTuple>().map_err(|_| None)?;
    if tuple.len() != 3 {
        return Err(tuple.get_item(1).ok().map(Bound::unbind));
    }
    let status = tuple
        .get_item(0)
        .and_then(|value| value.extract::<String>())
        .map_err(|_| tuple.get_item(1).ok().map(Bound::unbind))?;
    if status == "absent"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Absent);
    }
    if status == "unsupported"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Unsupported);
    }
    if status != "rich" {
        return Err(tuple.get_item(1).ok().and_then(|value| {
            if value.is_none() {
                None
            } else {
                Some(value.unbind())
            }
        }));
    }
    let delegate = tuple.get_item(1).map_err(|_| None)?;
    if delegate.is_none() {
        return Err(None);
    }
    let delegate = delegate.unbind();
    let hook_result = tuple
        .get_item(2)
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let hook_tuple = hook_result
        .cast::<PyTuple>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if hook_tuple.len() != 2
        || !hook_tuple
            .get_item(1)
            .is_ok_and(|value| value.is_instance_of::<PyDict>())
    {
        return Err(Some(delegate));
    }
    let data = hook_tuple
        .get_item(0)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .cast_into::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let widget_view = data
        .get_item(WIDGET_MIME)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .ok_or_else(|| Some(delegate.clone_ref(py)))?;
    let widget = widget_view
        .cast::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if widget.len() != 3
        || widget
            .get_item("version_major")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(2)
        || widget
            .get_item("version_minor")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(0)
        || widget
            .get_item("model_id")
            .ok()
            .flatten()
            .and_then(|value| value.extract::<String>().ok())
            .is_none_or(|model_id| model_id.is_empty())
    {
        return Err(Some(delegate));
    }
    Ok(AdapterOutcome::Rich {
        delegate,
        widget_view: widget_view.unbind(),
    })
}

fn close_delegate(py: Python<'_>, delegate: &Py<PyAny>) {
    let _ = delegate.bind(py).call_method0("close");
}

fn exact_u8(value: Bound<'_, PyAny>) -> Option<u8> {
    if value.is_instance_of::<PyBool>() {
        None
    } else {
        value.extract::<u8>().ok()
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    panic_boundary(py, || match &plan.resolved {
        ResolvedMeshPlan::Gmsh(resolved) => {
            if geometry.geometry() != &resolved.source {
                return Err(request_error(
                    py,
                    "MeshPlan belongs to a different exact Geometry",
                ));
            }
            let super::plan::MeshProviderPolicy::Gmsh(provider) = plan.request.provider else {
                unreachable!("Gmsh resolved plan retains Gmsh provider policy")
            };
            super::gmsh::revalidate_generated(
                &resolved.source,
                &resolved.generated,
                provider.policy,
            )
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            plan.request
                .provider
                .validate_production_lineage(
                    &plan.production,
                    &resolved.source,
                    &resolved.generated.mesh,
                    &resolved.generated.correspondence,
                )
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            let published = PyMesh::from_source_parts(
                py,
                &resolved.source,
                &resolved.generated.mesh,
                &resolved.generated.correspondence,
                &plan.production,
                SourceOwnedProviderObservation::Gmsh4152 {
                    output: resolved
                        .generated
                        .provider_output
                        .clone()
                        .into_boxed_slice(),
                },
            )?;
            if published.gmsh_provider_output()
                != Some(resolved.generated.provider_output.as_slice())
            {
                return Err(PyRuntimeError::new_err(
                    "published Gmsh Mesh lost its exact provider observation",
                ));
            }
            Ok(published)
        }
        ResolvedMeshPlan::SourceOwned(resolved) => {
            resolved
                .revalidate(geometry.geometry())
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            plan.request
                .provider
                .validate_production_lineage(
                    &plan.production,
                    &resolved.source,
                    &resolved.mesh,
                    &resolved.correspondence,
                )
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            PyMesh::from_source_owned(py, resolved, &plan.production)
        }
        ResolvedMeshPlan::Cartesian(resolved) => {
            let super::plan::MeshProviderPolicy::Cartesian(provider) = plan.request.provider else {
                unreachable!("Cartesian resolved plan retains Cartesian provider policy")
            };
            resolved
                .revalidate(geometry.geometry(), provider.policy)
                .and_then(|()| {
                    plan.production
                        .validate_against_structured_cartesian_v1_resources(
                            provider.policy,
                            &resolved.source,
                            &resolved.mesh,
                            &resolved.correspondence,
                        )
                })
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            PyMesh::from_source_owned_cartesian(
                py,
                &resolved.source,
                &resolved.mesh,
                &resolved.correspondence,
                &plan.production,
            )
        }
    })
}

fn validated_entity_count(
    entities: Vec<MeshEntity>,
    expected_dimension: Option<usize>,
) -> Result<usize, Diagnostic> {
    if expected_dimension.is_some_and(|dimension| {
        entities
            .iter()
            .any(|entity| entity.dimension() != dimension)
    }) {
        return Err(Diagnostic::error(
            codes::INVALID_ARTIFACT,
            "GeometrySelection dimension differs from correspondence-owned membership",
        ));
    }
    Ok(entities.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_bound_selection_dimension_must_match_correspondence_membership() {
        assert!(
            validated_entity_count(vec![MeshEntity::new(1, 0)], Some(2)).is_err(),
            "dimension-wrong correspondence membership must reject"
        );
        assert_eq!(
            validated_entity_count(vec![MeshEntity::new(1, 0)], Some(1)).unwrap(),
            1
        );
    }
}
