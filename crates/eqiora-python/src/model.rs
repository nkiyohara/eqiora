//! Immutable Python model identities and optimistic value edits.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::api::{
    ModelDocument, ModelFieldRef, ModelParameterRef, StructuralSemanticFingerprint, ValueEditPlan,
};
use eqiora::diagnostic::codes;
use eqiora::{Diagnostic, EntityKind, RawId};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

use crate::error::{diagnostic_error, internal_diagnostic_error, panic_boundary, validation_error};

/// Exact identity of one immutable canonical Model artifact.
#[pyclass(
    name = "Revision",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyRevision {
    model_id: String,
    digest: String,
    number: u64,
}

/// Alpha-normalized comparison/cache evidence, never exact Model identity.
#[pyclass(
    name = "StructuralSemanticFingerprint",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyStructuralSemanticFingerprint {
    value: StructuralSemanticFingerprint,
}

#[pymethods]
impl PyStructuralSemanticFingerprint {
    /// Exact generation of the structural projection.
    #[getter]
    fn generation(&self) -> &'static str {
        self.value.generation().as_str()
    }

    /// Domain-separated digest of the alpha-normalized projection.
    #[getter]
    fn digest(&self) -> &str {
        self.value.digest()
    }

    fn __repr__(&self) -> String {
        format!(
            "StructuralSemanticFingerprint(generation={:?}, digest={:?})",
            self.generation(),
            self.digest()
        )
    }
}

#[pymethods]
impl PyRevision {
    /// Canonical typed Model ontology identity.
    #[getter]
    fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Domain-separated canonical Model content digest.
    #[getter]
    fn digest(&self) -> &str {
        &self.digest
    }

    /// Semantic graph revision serialized by the Model artifact.
    #[getter]
    const fn number(&self) -> u64 {
        self.number
    }

    fn __repr__(&self) -> String {
        format!(
            "Revision(model_id={:?}, number={}, digest={:?})",
            self.model_id, self.number, self.digest
        )
    }
}

/// One immutable, exact-base value edit prepared by the shared Rust facade.
#[pyclass(
    name = "ValueEdit",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyValueEdit {
    plan: ValueEditPlan,
}

/// Exact canonical Parameter selected from one immutable Model.
#[pyclass(
    name = "ParameterRef",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyModelParameterRef {
    pub(crate) value: ModelParameterRef,
    model_digest: String,
    id: String,
}

impl PartialEq for PyModelParameterRef {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for PyModelParameterRef {}

impl Hash for PyModelParameterRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.model_digest.hash(state);
        self.id.hash(state);
    }
}

#[pymethods]
impl PyModelParameterRef {
    /// Exact canonical Model artifact digest.
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }

    /// Stable canonical Parameter ULID.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    fn __repr__(&self) -> String {
        format!(
            "ParameterRef(id={:?}, model_digest={:?})",
            self.id, self.model_digest
        )
    }
}

/// Exact canonical Field selected from one immutable Model.
#[pyclass(
    name = "FieldRef",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyModelFieldRef {
    pub(crate) value: ModelFieldRef,
    model_digest: String,
    id: String,
}

impl PartialEq for PyModelFieldRef {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for PyModelFieldRef {}

impl Hash for PyModelFieldRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.model_digest.hash(state);
        self.id.hash(state);
    }
}

#[pymethods]
impl PyModelFieldRef {
    /// Exact canonical Model artifact digest.
    #[getter]
    fn model_digest(&self) -> &str {
        &self.model_digest
    }

    /// Stable canonical Field ULID.
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }

    fn __repr__(&self) -> String {
        format!(
            "FieldRef(id={:?}, model_digest={:?})",
            self.id, self.model_digest
        )
    }
}

#[pymethods]
impl PyValueEdit {
    /// Exact-plan key over the base artifact and ordered transaction wire.
    #[getter]
    fn key(&self) -> String {
        self.plan.key()
    }

    /// Canonical identity of the immutable base Model content.
    #[getter]
    fn base_digest(&self) -> &str {
        self.plan.base_digest()
    }

    /// Exact graph revision required by the optimistic precondition.
    #[getter]
    fn base_revision(&self) -> u64 {
        self.plan.base_revision().0
    }

    /// Stable canonical target identity.
    #[getter]
    fn target_id(&self) -> String {
        self.plan.target().ulid().to_string()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.plan.key() == other.plan.key())
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        hash_value(&self.plan.key()) as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "ValueEdit(base_revision={}, target_id={:?}, key={:?})",
            self.base_revision(),
            self.target_id(),
            self.key()
        )
    }
}

/// One immutable, validated canonical Model revision.
#[pyclass(name = "Model", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub(crate) struct PyModel {
    document: ModelDocument,
    revision: PyRevision,
}

impl PyModel {
    pub(crate) fn from_document(py: Python<'_>, document: ModelDocument) -> PyResult<Self> {
        let reference = document
            .artifact_reference()
            .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))?;
        Ok(Self {
            revision: PyRevision {
                model_id: reference.model().ulid().to_string(),
                digest: reference.artifact().to_string(),
                number: reference.semantic_revision().get(),
            },
            document,
        })
    }

    pub(crate) fn document(&self) -> &ModelDocument {
        &self.document
    }

    fn resolve_edit_target(&self, target: &str) -> Result<RawId, Diagnostic> {
        if let Some(&id) = self.document.aliases().get(target) {
            return Ok(id);
        }
        self.document
            .program()
            .nodes()
            .map(|node| node.id())
            .find(|id| {
                matches!(id.kind(), EntityKind::Field | EntityKind::Parameter)
                    && id.ulid().to_string() == target
            })
            .ok_or_else(|| {
                Diagnostic::error(
                    codes::NODE_NOT_FOUND,
                    format!(
                        "value-edit target {target:?} is neither a Field/Parameter alias nor an exact canonical ID in this Model revision"
                    ),
                )
            })
    }
}

#[pymethods]
impl PyModel {
    /// Define a model from immutable native declarations.
    #[staticmethod]
    #[pyo3(signature = (name, *declarations))]
    fn define(
        py: Python<'_>,
        name: String,
        declarations: &Bound<'_, pyo3::types::PyTuple>,
    ) -> PyResult<Self> {
        panic_boundary(py, || {
            let document = crate::modeling::define_model(py, name, declarations)?;
            Self::from_document(py, document)
        })
    }

    /// Canonical, versioned Model artifact bytes.
    fn to_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        panic_boundary(py, || {
            self.document
                .canonical_json()
                .map(|bytes| PyBytes::new(py, &bytes))
                .map_err(|diagnostic| internal_diagnostic_error(py, &[diagnostic]))
        })
    }

    /// Exact immutable artifact-bound revision identity.
    #[getter]
    fn revision(&self) -> PyRevision {
        self.revision.clone()
    }

    /// Typed Semantic Model identity retained by the canonical artifact.
    #[getter]
    fn model_id(&self) -> &str {
        &self.revision.model_id
    }

    /// Domain-separated semantic content digest.
    #[getter]
    fn digest(&self) -> &str {
        &self.revision.digest
    }

    /// Alpha-normalized structural evidence, separate from exact artifact identity.
    #[getter]
    fn structural_fingerprint(&self, py: Python<'_>) -> PyResult<PyStructuralSemanticFingerprint> {
        panic_boundary(py, || {
            let document = self.document.clone();
            py.detach(move || document.structural_fingerprint())
                .map(|value| PyStructuralSemanticFingerprint { value })
                .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
        })
    }

    /// Compare structural meaning without changing exact Model equality.
    fn structurally_equivalent(&self, py: Python<'_>, other: &PyModel) -> PyResult<bool> {
        panic_boundary(py, || {
            let left = self.document.clone();
            let right = other.document.clone();
            py.detach(move || left.structurally_equivalent(&right))
                .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))
        })
    }

    /// Stable ULIDs of Fields that can appear in results.
    #[getter]
    fn field_ids(&self) -> Vec<String> {
        self.document
            .program()
            .nodes()
            .filter(|node| node.id().kind() == EntityKind::Field)
            .map(|node| node.id().ulid().to_string())
            .collect()
    }

    /// Stable ULIDs of Parameters addressable by value edits.
    #[getter]
    fn parameter_ids(&self) -> Vec<String> {
        self.document
            .program()
            .nodes()
            .filter(|node| node.id().kind() == EntityKind::Parameter)
            .map(|node| node.id().ulid().to_string())
            .collect()
    }

    /// Resolve a source alias or exact ULID once into an exact Parameter role.
    fn parameter(&self, py: Python<'_>, selection: &str) -> PyResult<PyModelParameterRef> {
        panic_boundary(py, || {
            let value = self
                .document
                .parameter_ref(selection)
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            Ok(PyModelParameterRef {
                model_digest: value.model().artifact().to_string(),
                id: value.id().ulid().to_string(),
                value,
            })
        })
    }

    /// Resolve a source alias or exact ULID once into an exact Field role.
    fn field(&self, py: Python<'_>, selection: &str) -> PyResult<PyModelFieldRef> {
        panic_boundary(py, || {
            let value = self
                .document
                .field_ref(selection)
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            Ok(PyModelFieldRef {
                model_digest: value.model().artifact().to_string(),
                id: value.id().ulid().to_string(),
                value,
            })
        })
    }

    /// Prepare an exact-base scalar value edit without mutating this Model.
    fn preview_value_edit(
        &self,
        py: Python<'_>,
        target: &str,
        value: f64,
    ) -> PyResult<PyValueEdit> {
        panic_boundary(py, || {
            let target = self
                .resolve_edit_target(target)
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            self.document
                .preview_value_edit(target, value)
                .map(|plan| PyValueEdit { plan })
                .map_err(|diagnostic| validation_error(py, &[diagnostic]))
        })
    }

    /// Atomically commit an exact-base edit into a new immutable child Model.
    fn commit(&self, py: Python<'_>, edit: &PyValueEdit) -> PyResult<Self> {
        panic_boundary(py, || {
            let document = self.document.clone();
            let plan = edit.plan.clone();
            let child = py
                .detach(move || document.commit_value_edit(plan))
                .map_err(|diagnostics| validation_error(py, &diagnostics))?
                .into_document();
            Self::from_document(py, child)
        })
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.revision == other.revision)
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        hash_value(&self.revision) as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "Model(model_id={:?}, revision={}, digest={:?})",
            self.revision.model_id, self.revision.number, self.revision.digest
        )
    }
}

fn hash_value(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyStructuralSemanticFingerprint>()?;
    module.add_class::<PyRevision>()?;
    module.add_class::<PyValueEdit>()?;
    module.add_class::<PyModelParameterRef>()?;
    module.add_class::<PyModelFieldRef>()?;
    module.add_class::<PyModel>()?;
    Ok(())
}
