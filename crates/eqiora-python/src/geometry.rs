//! Bounded Python authoring over Rust-owned exact geometry meaning.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::geometry::{
    CanonicalGeometryV1, CanonicalPlanarCircularHoleGeometryV2, NamedEntitySet,
};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule, PyTuple};

use crate::error::validation_error;

pub(crate) fn digest_to_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One immutable exact Geometry produced by an accepted authored graph.
///
/// The current constructor path is deliberately bounded to the admitted
/// planar section.  Its public identity and observations do not encode that
/// proving shape as a Python product type.
#[pyclass(
    name = "Geometry",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PyGeometry {
    geometry: GeometryOwner,
}

#[derive(Clone, Debug, PartialEq)]
enum GeometryOwner {
    V1(CanonicalGeometryV1),
    PlanarCircularHoleV2(CanonicalPlanarCircularHoleGeometryV2),
}

impl GeometryOwner {
    fn bounds(&self) -> &[[f64; 2]; 2] {
        match self {
            Self::V1(geometry) => geometry
                .circular_hole_bounds()
                .expect("the admitted planar Geometry has exact Cartesian bounds"),
            Self::PlanarCircularHoleV2(geometry) => geometry.bounds(),
        }
    }

    fn classification_tolerance(&self) -> Option<f64> {
        match self {
            Self::V1(geometry) => Some(geometry.tolerance_m()),
            Self::PlanarCircularHoleV2(_) => None,
        }
    }

    fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::V1(geometry) => geometry.canonical_bytes(),
            Self::PlanarCircularHoleV2(geometry) => geometry.canonical_bytes(),
        }
    }

    fn digest_bytes(&self) -> [u8; 32] {
        match self {
            Self::V1(geometry) => geometry.digest_bytes(),
            Self::PlanarCircularHoleV2(geometry) => geometry.digest_bytes(),
        }
    }

    fn entity_sets(&self) -> &[NamedEntitySet] {
        match self {
            Self::V1(geometry) => geometry.entity_sets(),
            Self::PlanarCircularHoleV2(geometry) => geometry.entity_sets(),
        }
    }

    fn entity_set(&self, name: &str) -> Option<&NamedEntitySet> {
        match self {
            Self::V1(geometry) => geometry.entity_set(name),
            Self::PlanarCircularHoleV2(geometry) => geometry.entity_set(name),
        }
    }
}

/// Immutable named selection bound to one exact Geometry revision.
#[pyclass(
    name = "GeometrySelection",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PyGeometrySelection {
    source_digest: String,
    name: String,
    dimension: usize,
}

#[pymethods]
impl PyGeometrySelection {
    #[getter]
    fn source_digest(&self) -> &str {
        &self.source_digest
    }

    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    #[getter]
    const fn dimension(&self) -> usize {
        self.dimension
    }

    fn __repr__(&self) -> String {
        format!(
            "GeometrySelection(name={:?}, dimension={}, source_digest={:?})",
            self.name, self.dimension, self.source_digest,
        )
    }

    /// Hash the same revision-bound identity used by equality.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.source_digest.hash(&mut hasher);
        self.name.hash(&mut hasher);
        self.dimension.hash(&mut hasher);
        hasher.finish()
    }
}

impl PyGeometrySelection {
    pub(crate) fn bound_source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) fn canonical_name(&self) -> &str {
        &self.name
    }
}

#[pymethods]
impl PyGeometry {
    /// Intrinsic and coordinate dimension of this accepted Geometry.
    #[getter]
    fn dimension(&self) -> usize {
        2
    }

    /// Exact Cartesian bounds, one pair per coordinate axis, in metres.
    #[getter]
    fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        let [[x_lower, x_upper], [y_lower, y_upper]] = *self.geometry.bounds();
        ((x_lower, x_upper), (y_lower, y_upper))
    }

    /// Producer classification tolerance in metres.
    #[getter]
    fn classification_tolerance(&self) -> Option<f64> {
        self.geometry.classification_tolerance()
    }

    /// Exact canonical JSON, without a trailing newline.
    #[getter]
    fn canonical_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, self.geometry.canonical_bytes()).unbind()
    }

    /// Lowercase domain-separated SHA-256 identity.
    #[getter]
    fn digest(&self) -> String {
        digest_to_hex(&self.geometry.digest_bytes())
    }

    /// Canonically ordered semantic selection names.
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
                    format!("Geometry has no selection named {name:?}"),
                );
                validation_error(py, std::slice::from_ref(&diagnostic))
            })
    }

    /// Resolve one canonical name into an immutable revision-bound selection.
    fn selection(&self, py: Python<'_>, name: &str) -> PyResult<PyGeometrySelection> {
        let entity_set = self.geometry.entity_set(name).ok_or_else(|| {
            let diagnostic = Diagnostic::error(
                codes::INVALID_ARTIFACT,
                format!("Geometry has no selection named {name:?}"),
            );
            validation_error(py, std::slice::from_ref(&diagnostic))
        })?;
        Ok(PyGeometrySelection {
            source_digest: self.digest(),
            name: entity_set.name().to_owned(),
            dimension: entity_set.dimension(),
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
            "Geometry(dimension={}, bounds={:?}, selections={}, digest={:?})",
            self.dimension(),
            self.bounds(),
            self.geometry.entity_sets().len(),
            self.digest(),
        )
    }
}

impl PyGeometry {
    pub(crate) const fn from_geometry(geometry: CanonicalGeometryV1) -> Self {
        Self {
            geometry: GeometryOwner::V1(geometry),
        }
    }

    pub(crate) const fn from_planar_circular_hole_v2(
        geometry: CanonicalPlanarCircularHoleGeometryV2,
    ) -> Self {
        Self {
            geometry: GeometryOwner::PlanarCircularHoleV2(geometry),
        }
    }

    pub(crate) fn geometry_v1(&self) -> Option<&CanonicalGeometryV1> {
        match &self.geometry {
            GeometryOwner::V1(geometry) => Some(geometry),
            GeometryOwner::PlanarCircularHoleV2(_) => None,
        }
    }

    pub(crate) fn digest_bytes(&self) -> [u8; 32] {
        self.geometry.digest_bytes()
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyGeometry>()?;
    module.add_class::<PyGeometrySelection>()
}
