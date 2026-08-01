//! Transparent Python projection of the closed authored-CAD graph.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CadAuthoredBuild, CadAuthoredFaceHandle, CadAuthoredFaceSelection, CadAuthoredGraph,
    CadRepairDispositionV1, ConstrainedRectangleV1,
};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyTuple};

use crate::error::validation_error;
use crate::geometry::digest_to_hex;

/// One immutable native-owned authored-CAD operation graph.
#[pyclass(
    name = "CadAuthoredGraph",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PyCadAuthoredGraph {
    graph: CadAuthoredGraph,
}

#[pymethods]
impl PyCadAuthoredGraph {
    /// Construct the closed rectangle, face, and positive-z extrusion history.
    #[staticmethod]
    #[pyo3(signature = (
        *,
        x_bounds,
        y_bounds,
        plane_z,
        depth,
        modeling_tolerance
    ))]
    fn rectangle_extrusion(
        py: Python<'_>,
        x_bounds: (f64, f64),
        y_bounds: (f64, f64),
        plane_z: f64,
        depth: f64,
        modeling_tolerance: f64,
    ) -> PyResult<Self> {
        let sketch = ConstrainedRectangleV1::new(x_bounds, y_bounds, plane_z)
            .map_err(|diagnostic| native_error(py, diagnostic))?;
        let graph = CadAuthoredGraph::new(sketch, depth, modeling_tolerance)
            .map_err(|diagnostic| native_error(py, diagnostic))?;
        Ok(Self { graph })
    }

    /// Replay either frozen canonical graph wire through the native decoder.
    #[staticmethod]
    fn decode_canonical(py: Python<'_>, data: &[u8]) -> PyResult<Self> {
        CadAuthoredGraph::decode_canonical(data)
            .map(|graph| Self { graph })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    /// Return a successor graph containing the one admitted circular cut.
    #[pyo3(signature = (*, center, radius, boolean_tolerance))]
    fn circular_through_cut(
        &self,
        py: Python<'_>,
        center: [f64; 2],
        radius: f64,
        boolean_tolerance: f64,
    ) -> PyResult<Self> {
        self.graph
            .circular_through_cut(center, radius, boolean_tolerance)
            .map(|graph| Self { graph })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    /// Exact compact canonical graph bytes owned by Rust.
    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, self.graph.canonical_bytes()).unbind()
    }

    /// Domain-separated authored-graph identity, never output-Geometry identity.
    #[getter]
    fn graph_digest(&self) -> String {
        digest_to_hex(&self.graph.digest_bytes())
    }

    #[getter]
    fn x_bounds(&self) -> (f64, f64) {
        self.graph.sketch().x_bounds_m()
    }

    #[getter]
    fn y_bounds(&self) -> (f64, f64) {
        self.graph.sketch().y_bounds_m()
    }

    #[getter]
    fn plane_z(&self) -> f64 {
        self.graph.sketch().plane_z_m()
    }

    #[getter]
    fn extrusion_depth(&self) -> f64 {
        self.graph.extrusion_depth_m()
    }

    #[getter]
    fn requested_modeling_tolerance(&self) -> f64 {
        self.graph.requested_modeling_tolerance_m()
    }

    #[getter]
    fn requested_boolean_tolerance(&self) -> Option<f64> {
        self.graph.requested_boolean_tolerance_m()
    }

    #[getter]
    fn cut_center(&self) -> Option<(f64, f64)> {
        self.graph.cut_center_m().map(Into::into)
    }

    #[getter]
    fn cut_radius(&self) -> Option<f64> {
        self.graph.cut_radius_m()
    }

    #[getter]
    fn bounds(&self) -> ((f64, f64), (f64, f64), (f64, f64)) {
        let [x, y, z] = self.graph.output().bounds_m();
        (x, y, z)
    }

    #[getter]
    fn vertex_count(&self) -> Option<usize> {
        self.graph.vertex_count()
    }

    #[getter]
    fn edge_count(&self) -> Option<usize> {
        self.graph.edge_count()
    }

    #[getter]
    fn face_count(&self) -> usize {
        self.graph.face_count()
    }

    #[getter]
    fn closed_shell_count(&self) -> usize {
        self.graph.closed_shell_count()
    }

    #[getter]
    fn body_count(&self) -> usize {
        self.graph.body_count()
    }

    #[getter]
    fn genus(&self) -> usize {
        self.graph.genus()
    }

    #[getter]
    fn volume(&self) -> f64 {
        self.graph.volume_m3()
    }

    #[getter]
    fn surface_area(&self) -> f64 {
        self.graph.surface_area_m2()
    }

    #[getter]
    fn repair(&self) -> &'static str {
        repair_name(self.graph.repair_disposition())
    }

    /// Provenance keys in the owner's canonical order.
    #[getter]
    fn selection_names(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(
            py,
            self.graph
                .selection_inventory()
                .iter()
                .map(|selection| selection.provenance_key()),
        )?
        .unbind())
    }

    /// Create an opaque handle bound to this exact graph identity.
    fn face_handle(&self, py: Python<'_>, name: &str) -> PyResult<PyCadAuthoredFaceHandle> {
        let selection = selection_from_name(&self.graph, name)
            .ok_or_else(|| native_error(py, unknown_selection(name)))?;
        self.graph
            .face_handle(selection)
            .map(|handle| PyCadAuthoredFaceHandle { handle })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    /// Resolve one graph-bound handle to its stable provenance key.
    fn resolve_face(
        &self,
        py: Python<'_>,
        handle: &PyCadAuthoredFaceHandle,
    ) -> PyResult<&'static str> {
        self.graph
            .resolve_face(&handle.handle)
            .map(CadAuthoredFaceSelection::provenance_key)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn face_area(&self, py: Python<'_>, handle: &PyCadAuthoredFaceHandle) -> PyResult<f64> {
        self.graph
            .face_area_m2(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn face_boundary_loop_count(
        &self,
        py: Python<'_>,
        handle: &PyCadAuthoredFaceHandle,
    ) -> PyResult<usize> {
        self.graph
            .face_boundary_loop_count(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn rectangular_face_vertices(
        &self,
        py: Python<'_>,
        handle: &PyCadAuthoredFaceHandle,
    ) -> PyResult<Option<Py<PyTuple>>> {
        let vertices = self
            .graph
            .rectangular_face_vertices_m(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))?;

        vertices
            .map(|vertices| {
                Ok(
                    PyTuple::new(py, vertices.into_iter().map(Into::<(f64, f64, f64)>::into))?
                        .unbind(),
                )
            })
            .transpose()
    }

    fn rectangular_face_centroid(
        &self,
        py: Python<'_>,
        handle: &PyCadAuthoredFaceHandle,
    ) -> PyResult<Option<(f64, f64, f64)>> {
        self.graph
            .rectangular_face_centroid_m(&handle.handle)
            .map(|centroid| centroid.map(Into::into))
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn planar_face_outward_normal(
        &self,
        py: Python<'_>,
        handle: &PyCadAuthoredFaceHandle,
    ) -> PyResult<Option<(f64, f64, f64)>> {
        self.graph
            .planar_face_outward_normal(&handle.handle)
            .map(|normal| normal.map(Into::into))
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    /// Execute and admit the bounded analytic provider profile.
    fn build(&self, py: Python<'_>) -> PyResult<PyCadAuthoredBuild> {
        self.graph
            .build_analytic()
            .map(|build| PyCadAuthoredBuild { build })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn __hash__(&self) -> u64 {
        hash_bytes(self.graph.canonical_bytes())
    }

    fn __repr__(&self) -> String {
        format!(
            "CadAuthoredGraph(graph_digest={:?}, selections={}, cut={})",
            self.graph_digest(),
            self.graph.selection_inventory().len(),
            self.graph.cut_radius_m().is_some(),
        )
    }
}

/// Opaque authored-face provenance bound to one exact graph digest.
#[pyclass(
    name = "CadAuthoredFaceHandle",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PyCadAuthoredFaceHandle {
    handle: CadAuthoredFaceHandle,
}

#[pymethods]
impl PyCadAuthoredFaceHandle {
    #[staticmethod]
    fn decode_canonical(py: Python<'_>, data: &[u8]) -> PyResult<Self> {
        CadAuthoredFaceHandle::decode_canonical(data)
            .map(|handle| Self { handle })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, self.handle.canonical_bytes()).unbind()
    }

    #[getter]
    fn graph_digest(&self) -> String {
        digest_to_hex(&self.handle.graph_digest_bytes())
    }

    #[getter]
    fn provenance_key(&self) -> &'static str {
        self.handle.selection().provenance_key()
    }

    fn __hash__(&self) -> u64 {
        hash_bytes(self.handle.canonical_bytes())
    }

    fn __repr__(&self) -> String {
        format!(
            "CadAuthoredFaceHandle(graph_digest={:?}, provenance_key={:?})",
            self.graph_digest(),
            self.provenance_key(),
        )
    }
}

/// Complete read-only receipt from the bounded native analytic CAD profile.
#[pyclass(
    name = "CadAuthoredBuild",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PyCadAuthoredBuild {
    build: CadAuthoredBuild,
}

#[pymethods]
impl PyCadAuthoredBuild {
    #[getter]
    fn graph_digest(&self) -> String {
        digest_to_hex(&self.build.graph_digest_bytes())
    }

    #[getter]
    fn provider_profile(&self) -> &'static str {
        self.build.provider_profile()
    }

    #[getter]
    fn requested_modeling_tolerance(&self) -> f64 {
        self.build.requested_modeling_tolerance_m()
    }

    #[getter]
    fn requested_boolean_tolerance(&self) -> Option<f64> {
        self.build.requested_boolean_tolerance_m()
    }

    #[getter]
    fn effective_boolean_tolerance(&self) -> Option<f64> {
        self.build.effective_boolean_tolerance_m()
    }

    #[getter]
    fn maximum_position_discrepancy(&self) -> f64 {
        self.build.maximum_position_discrepancy_m()
    }

    #[getter]
    fn maximum_area_discrepancy(&self) -> f64 {
        self.build.maximum_area_discrepancy_m2()
    }

    #[getter]
    fn maximum_volume_discrepancy(&self) -> f64 {
        self.build.maximum_volume_discrepancy_m3()
    }

    #[getter]
    fn repair(&self) -> &'static str {
        repair_name(self.build.repair_disposition())
    }

    #[getter]
    fn retained_unchanged(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.retained_unchanged())
    }

    #[getter]
    fn retained_modified(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.retained_modified())
    }

    #[getter]
    fn created(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.created())
    }

    #[getter]
    fn deleted(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.deleted())
    }

    #[getter]
    fn split(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.split())
    }

    #[getter]
    fn merged(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        handle_tuple(py, self.build.merged())
    }

    fn __repr__(&self) -> String {
        format!(
            "CadAuthoredBuild(graph_digest={:?}, provider_profile={:?}, repair={:?})",
            self.graph_digest(),
            self.provider_profile(),
            self.repair(),
        )
    }
}

fn handle_tuple(py: Python<'_>, handles: &[CadAuthoredFaceHandle]) -> PyResult<Py<PyTuple>> {
    let projected = handles
        .iter()
        .cloned()
        .map(|handle| Py::new(py, PyCadAuthoredFaceHandle { handle }))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, projected)?.unbind())
}

fn selection_from_name(graph: &CadAuthoredGraph, name: &str) -> Option<CadAuthoredFaceSelection> {
    graph
        .selection_inventory()
        .iter()
        .find(|selection| selection.provenance_key() == name)
        .cloned()
}

fn unknown_selection(name: &str) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_ARTIFACT,
        format!("authored CAD graph has no provenance selection named {name:?}"),
    )
}

fn native_error(py: Python<'_>, diagnostic: Diagnostic) -> PyErr {
    validation_error(py, std::slice::from_ref(&diagnostic))
}

const fn repair_name(repair: CadRepairDispositionV1) -> &'static str {
    match repair {
        CadRepairDispositionV1::None => "none",
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCadAuthoredGraph>()?;
    module.add_class::<PyCadAuthoredFaceHandle>()?;
    module.add_class::<PyCadAuthoredBuild>()
}
