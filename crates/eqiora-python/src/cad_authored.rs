//! Solid-operation projection used by the common Python Geometry graph.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::Diagnostic;
use eqiora::geometry::{
    CadRepairDispositionV1, GeometryBuildReceipt, GeometryFaceHandle, GeometrySolidOperation,
};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyMapping, PyModule, PySequence, PyTuple};

use crate::error::validation_error;
use crate::geometry::digest_to_hex;

/// One immutable solid operation owned by a common Geometry graph session.
#[pyclass(
    name = "GeometrySolidOperation",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub(crate) struct PyGeometrySolidOperation {
    pub(crate) graph: GeometrySolidOperation,
}

impl PartialEq for PyGeometrySolidOperation {
    fn eq(&self, other: &Self) -> bool {
        self.graph == other.graph
    }
}

#[pymethods]
impl PyGeometrySolidOperation {
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
        let handles = self
            .graph
            .face_handles()
            .map_err(|diagnostic| native_error(py, diagnostic))?;
        Ok(PyTuple::new(py, handles.iter().map(GeometryFaceHandle::provenance_key))?.unbind())
    }

    /// Create an opaque handle bound to this exact graph identity.
    fn face_handle(&self, py: Python<'_>, name: &str) -> PyResult<PyGeometryFaceHandle> {
        self.graph
            .face_handle(name)
            .map(|handle| PyGeometryFaceHandle { handle })
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    /// Resolve one graph-bound handle to its stable provenance key.
    fn resolve_face(
        &self,
        py: Python<'_>,
        handle: &PyGeometryFaceHandle,
    ) -> PyResult<&'static str> {
        self.graph
            .resolve_face(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn face_area(&self, py: Python<'_>, handle: &PyGeometryFaceHandle) -> PyResult<f64> {
        self.graph
            .face_area_m2(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn face_boundary_loop_count(
        &self,
        py: Python<'_>,
        handle: &PyGeometryFaceHandle,
    ) -> PyResult<usize> {
        self.graph
            .face_boundary_loop_count(&handle.handle)
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn rectangular_face_vertices(
        &self,
        py: Python<'_>,
        handle: &PyGeometryFaceHandle,
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
        handle: &PyGeometryFaceHandle,
    ) -> PyResult<Option<(f64, f64, f64)>> {
        self.graph
            .rectangular_face_centroid_m(&handle.handle)
            .map(|centroid| centroid.map(Into::into))
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn planar_face_outward_normal(
        &self,
        py: Python<'_>,
        handle: &PyGeometryFaceHandle,
    ) -> PyResult<Option<(f64, f64, f64)>> {
        self.graph
            .planar_face_outward_normal(&handle.handle)
            .map(|normal| normal.map(Into::into))
            .map_err(|diagnostic| native_error(py, diagnostic))
    }

    fn __hash__(&self) -> u64 {
        hash_bytes(self.graph.canonical_bytes())
    }

    fn __repr__(&self) -> String {
        format!(
            "GeometrySolidOperation(graph_digest={:?}, selections={}, cut={})",
            self.graph_digest(),
            self.graph.face_count(),
            self.graph.cut_radius_m().is_some(),
        )
    }
}

/// Opaque authored-face provenance bound to one exact graph digest.
#[pyclass(
    name = "GeometryFaceHandle",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub(crate) struct PyGeometryFaceHandle {
    pub(crate) handle: GeometryFaceHandle,
}

impl PartialEq for PyGeometryFaceHandle {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Eq for PyGeometryFaceHandle {}

#[pymethods]
impl PyGeometryFaceHandle {
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
        self.handle.provenance_key()
    }

    fn __hash__(&self) -> u64 {
        hash_bytes(self.handle.canonical_bytes())
    }

    fn __repr__(&self) -> String {
        format!(
            "GeometryFaceHandle(graph_digest={:?}, provenance_key={:?})",
            self.graph_digest(),
            self.provenance_key(),
        )
    }
}

/// Complete read-only receipt from the bounded native analytic CAD profile.
#[pyclass(
    name = "GeometryBuildReceipt",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub(crate) struct PyGeometryBuildReceipt {
    pub(crate) build: GeometryBuildReceipt,
}

impl PartialEq for PyGeometryBuildReceipt {
    fn eq(&self, other: &Self) -> bool {
        self.build == other.build
    }
}

#[pymethods]
impl PyGeometryBuildReceipt {
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
            "GeometryBuildReceipt(graph_digest={:?}, provider_profile={:?}, repair={:?})",
            self.graph_digest(),
            self.provider_profile(),
            self.repair(),
        )
    }
}

fn handle_tuple(py: Python<'_>, handles: &[GeometryFaceHandle]) -> PyResult<Py<PyTuple>> {
    let projected = handles
        .iter()
        .cloned()
        .map(|handle| Py::new(py, PyGeometryFaceHandle { handle }))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, projected)?.unbind())
}

pub(crate) fn extract_named_topology(
    value: &Bound<'_, PyAny>,
) -> PyResult<BTreeMap<String, Vec<GeometryFaceHandle>>> {
    let mapping = value.cast::<PyMapping>().map_err(|_| {
        PyTypeError::new_err(
            "named_topology must be one mapping from strings to construction handles",
        )
    })?;
    let mut result = BTreeMap::new();
    for item in mapping.items()?.try_iter()? {
        let item = item?;
        let pair = item.cast::<PyTuple>()?;
        let name = pair
            .get_item(0)?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("named_topology keys must be strings"))?;
        let raw = pair.get_item(1)?;
        let handles = if let Ok(handle) = raw.extract::<PyRef<'_, PyGeometryFaceHandle>>() {
            vec![handle.handle.clone()]
        } else {
            let sequence = raw.cast::<PySequence>().map_err(|_| {
                PyTypeError::new_err(
                    "named_topology values must be a construction handle or a sequence of handles",
                )
            })?;
            let mut handles = Vec::with_capacity(sequence.len()?);
            for member in sequence.try_iter()? {
                let member = member?;
                let handle = member
                    .extract::<PyRef<'_, PyGeometryFaceHandle>>()
                    .map_err(|_| {
                        PyTypeError::new_err(
                            "named_topology sequences must contain only construction handles",
                        )
                    })?;
                handles.push(handle.handle.clone());
            }
            handles
        };
        if result.insert(name, handles).is_some() {
            return Err(PyValueError::new_err(
                "named_topology mapping contains a duplicate name",
            ));
        }
    }
    Ok(result)
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

pub(crate) fn extract_rectangle_pair(
    value: &Bound<'_, pyo3::types::PyAny>,
) -> PyResult<(f64, f64)> {
    let extracted = value.extract::<(f64, f64)>();
    match extracted {
        Err(error) if error.is_instance_of::<PyValueError>(value.py()) => {
            if let Ok(tuple) = value.cast::<PyTuple>() {
                let actual = tuple.len();
                if actual != 2 {
                    return Err(PyTypeError::new_err(format!(
                        "expected tuple of length 2, but got tuple of length {actual}"
                    )));
                }
            }
            Err(error)
        }
        result => result,
    }
}

pub(crate) fn extract_sequence_pair(value: &Bound<'_, pyo3::types::PyAny>) -> PyResult<[f64; 2]> {
    let extracted = value.extract::<[f64; 2]>();
    match extracted {
        Err(error) if error.is_instance_of::<PyValueError>(value.py()) => {
            if let Ok(actual) = value.len()
                && actual != 2
            {
                return Err(PyTypeError::new_err(format!(
                    "expected a sequence of length 2 (got {actual})"
                )));
            }
            Err(error)
        }
        result => result,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyGeometrySolidOperation>()?;
    module.add_class::<PyGeometryFaceHandle>()?;
    module.add_class::<PyGeometryBuildReceipt>()
}
