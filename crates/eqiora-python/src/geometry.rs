//! Bounded Python authoring over Rust-owned exact geometry meaning.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CanonicalCircularHoleGeometryV1, EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet,
};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyTuple};

use crate::error::validation_error;

pub(crate) fn digest_to_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One exact rectangle-minus-circle value with fixed semantic selection roles.
#[pyclass(
    name = "RectangleWithCircularHole",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PyRectangleWithCircularHole {
    geometry: CanonicalCircularHoleGeometryV1,
}

#[pymethods]
impl PyRectangleWithCircularHole {
    #[new]
    #[pyo3(signature = (
        *,
        bounds,
        circle_center,
        circle_radius,
        tolerance,
        region,
        x_lower,
        x_upper,
        y_lower,
        y_upper,
        hole
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        bounds: [[f64; 2]; 2],
        circle_center: [f64; 2],
        circle_radius: f64,
        tolerance: f64,
        region: String,
        x_lower: String,
        x_upper: String,
        y_lower: String,
        y_upper: String,
        hole: String,
    ) -> PyResult<Self> {
        let mut boundaries = BTreeMap::<String, Vec<usize>>::new();
        for (name, entity) in [
            (x_lower, 0),
            (x_upper, 1),
            (y_lower, 2),
            (y_upper, 3),
            (hole, 4),
        ] {
            boundaries.entry(name).or_default().push(entity);
        }
        let mut entity_sets = boundaries
            .into_iter()
            .map(|(name, members)| NamedEntitySet::new(name, EDGE_DIMENSION, members))
            .collect::<Vec<_>>();
        entity_sets.push(NamedEntitySet::new(region, FACE_DIMENSION, vec![0]));

        let geometry = CanonicalCircularHoleGeometryV1::new(
            bounds,
            circle_center,
            circle_radius,
            entity_sets,
            tolerance,
        )
        .map_err(|diagnostic| validation_error(py, std::slice::from_ref(&diagnostic)))?;
        Ok(Self { geometry })
    }

    /// Exact Cartesian bounds, x axis then y axis, in metres.
    #[getter]
    fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        let [[x_lower, x_upper], [y_lower, y_upper]] = *self.geometry.bounds();
        ((x_lower, x_upper), (y_lower, y_upper))
    }

    /// Exact circle centre in metres.
    #[getter]
    fn circle_center(&self) -> (f64, f64) {
        self.geometry.circle_center().into()
    }

    /// Exact circle radius in metres.
    #[getter]
    fn circle_radius(&self) -> f64 {
        self.geometry.circle_radius_m()
    }

    /// Producer classification tolerance in metres.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.geometry.tolerance_m()
    }

    /// Exact canonical JSON, without a trailing newline.
    #[getter]
    fn canonical_json(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, self.geometry.canonical_bytes()).unbind()
    }

    /// Lowercase domain-separated SHA-256 identity.
    #[getter]
    fn digest(&self) -> String {
        digest_to_hex(&self.geometry.digest_bytes())
    }

    /// Canonically ordered fixed-role selection names.
    #[getter]
    fn selection_names(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(
            py,
            self.geometry.entity_sets().iter().map(NamedEntitySet::name),
        )?
        .unbind())
    }

    /// Exact topological dimension of one fixed-role selection.
    fn selection_dimension(&self, py: Python<'_>, name: &str) -> PyResult<usize> {
        self.geometry
            .entity_set(name)
            .map(NamedEntitySet::dimension)
            .ok_or_else(|| {
                let diagnostic = Diagnostic::error(
                    codes::INVALID_ARTIFACT,
                    format!("exact circular-hole geometry has no selection named {name:?}"),
                );
                validation_error(py, std::slice::from_ref(&diagnostic))
            })
    }

    /// Hash the same canonical identity used by equality.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.geometry.canonical_bytes().hash(&mut hasher);
        hasher.finish()
    }

    fn __repr__(&self) -> String {
        format!(
            "RectangleWithCircularHole(bounds={:?}, circle_center={:?}, \
             circle_radius={}, tolerance={}, selections={}, digest={:?})",
            self.bounds(),
            self.circle_center(),
            self.circle_radius(),
            self.tolerance(),
            self.geometry.entity_sets().len(),
            self.digest(),
        )
    }
}

impl PyRectangleWithCircularHole {
    pub(crate) const fn geometry(&self) -> &CanonicalCircularHoleGeometryV1 {
        &self.geometry
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRectangleWithCircularHole>()
}
