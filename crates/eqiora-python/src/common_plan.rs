//! Common typed Model + Mesh + numerical-policy resolution for Python.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMesh,
    ScalarEllipticMethod, ScalarEllipticRunPlan,
};
use eqiora::meshing::MeshTopology;
use eqiora::realization::RealizationRevision;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

use crate::error::{diagnostic_error, internal_diagnostic_error, validation_error};
use crate::model::PyModel;
use crate::panic_boundary;
use crate::realization::PyRealization;

/// Model-unbound request for the current generated Cartesian Mesh family.
#[pyclass(
    name = "Cartesian",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyCartesian {
    cells_per_axis: NonZeroUsize,
}

#[pymethods]
impl PyCartesian {
    #[new]
    #[pyo3(signature = (*, cells_per_axis))]
    fn new(cells_per_axis: usize) -> PyResult<Self> {
        let cells_per_axis = NonZeroUsize::new(cells_per_axis)
            .ok_or_else(|| PyTypeError::new_err("cells_per_axis must be a positive integer"))?;
        Ok(Self { cells_per_axis })
    }

    #[getter]
    fn cells_per_axis(&self) -> usize {
        self.cells_per_axis.get()
    }

    fn __repr__(&self) -> String {
        format!("Cartesian(cells_per_axis={})", self.cells_per_axis)
    }
}

/// Exact effective uniform Cartesian Mesh owned by one resolved Plan.
#[pyclass(
    name = "CartesianMesh",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyCartesianMesh {
    native: ScalarEllipticMesh,
    digest: String,
}

impl PyCartesianMesh {
    fn from_native(py: Python<'_>, native: ScalarEllipticMesh) -> PyResult<Self> {
        let digest = native
            .artifact()
            .digest()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
            .to_string();
        Ok(Self { native, digest })
    }
}

#[pymethods]
impl PyCartesianMesh {
    #[getter]
    fn digest(&self) -> &str {
        &self.digest
    }

    #[getter]
    fn dimension(&self) -> usize {
        self.native.dimension()
    }

    #[getter]
    fn cells_per_axis(&self) -> usize {
        self.native.cells_per_axis().get()
    }

    #[getter]
    fn cell_count(&self) -> usize {
        self.native
            .artifact()
            .mesh()
            .entity_count(self.dimension())
            .expect("an accepted Cartesian Mesh has top-dimensional entities")
    }

    #[getter]
    fn canonical_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.native
            .artifact()
            .canonical_json()
            .map(|bytes| PyBytes::new(py, &bytes))
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    fn __repr__(&self) -> String {
        format!(
            "CartesianMesh(dimension={}, cells_per_axis={}, digest={:?})",
            self.dimension(),
            self.cells_per_axis(),
            self.digest,
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.digest == other.digest)
    }

    fn __hash__(&self) -> isize {
        hash_pair(&self.digest, "cartesian-mesh") as isize
    }
}

/// Continuous tensor-product Q1 Galerkin spatial policy.
#[pyclass(
    name = "Q1",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyQ1;

#[pymethods]
impl PyQ1 {
    #[new]
    const fn new() -> Self {
        Self
    }

    fn __repr__(&self) -> &'static str {
        "Q1()"
    }
}

/// Cell-centred orthogonal two-point-flux finite-volume spatial policy.
#[pyclass(
    name = "CellCenteredTpfa",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyCellCenteredTpfa;

#[pymethods]
impl PyCellCenteredTpfa {
    #[new]
    const fn new() -> Self {
        Self
    }

    fn __repr__(&self) -> &'static str {
        "CellCenteredTpfa()"
    }
}

/// The existing admitted host-serial reproducible CG solve policy.
#[pyclass(
    name = "Linear",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyLinear;

#[pymethods]
impl PyLinear {
    #[new]
    const fn new() -> Self {
        Self
    }

    #[getter]
    const fn algorithm(&self) -> &'static str {
        "conjugate-gradient"
    }

    #[getter]
    const fn preconditioner(&self) -> &'static str {
        "identity"
    }

    #[getter]
    const fn reduction(&self) -> &'static str {
        "reproducible"
    }

    #[getter]
    const fn relative_tolerance(&self) -> f64 {
        1.0e-10
    }

    #[getter]
    const fn absolute_tolerance(&self) -> f64 {
        1.0e-12
    }

    #[getter]
    const fn maximum_iterations(&self) -> usize {
        10_000
    }

    fn __repr__(&self) -> &'static str {
        "Linear(algorithm='conjugate-gradient', preconditioner='identity', reduction='reproducible')"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpatialPolicy {
    Q1,
    CellCenteredTpfa,
}

impl SpatialPolicy {
    const fn method(self) -> ScalarEllipticMethod {
        match self {
            Self::Q1 => ScalarEllipticMethod::FiniteElement,
            Self::CellCenteredTpfa => ScalarEllipticMethod::FiniteVolume,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Q1 => "q1",
            Self::CellCenteredTpfa => "cell-centered-tpfa",
        }
    }

    const fn space(self) -> &'static str {
        match self {
            Self::Q1 => "continuous-lagrange-q1",
            Self::CellCenteredTpfa => "cell-constant",
        }
    }

    const fn quadrature(self) -> &'static str {
        match self {
            Self::Q1 => "gauss-legendre-2-per-axis",
            Self::CellCenteredTpfa => "cell-centroid",
        }
    }
}

/// Immutable common Plan owning its exact Model, Mesh, and effective policies.
#[pyclass(name = "Plan", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub(crate) struct PyPlan {
    document: ModelDocument,
    mesh: PyCartesianMesh,
    spatial: SpatialPolicy,
    native: ScalarEllipticRunPlan,
}

impl PyPlan {
    pub(crate) const fn document(&self) -> &ModelDocument {
        &self.document
    }

    pub(crate) const fn native(&self) -> &ScalarEllipticRunPlan {
        &self.native
    }
}

#[pymethods]
impl PyPlan {
    #[getter]
    fn realization_digest(&self) -> &str {
        self.native.key()
    }

    #[getter]
    fn model_digest(&self) -> &str {
        self.native.model_digest()
    }

    #[getter]
    fn mesh_digest(&self) -> &str {
        &self.mesh.digest
    }

    #[getter]
    fn mesh(&self) -> PyCartesianMesh {
        self.mesh.clone()
    }

    #[getter]
    fn realization(&self) -> PyRealization {
        PyRealization::from_plan(self.native.clone())
    }

    #[getter]
    fn spatial(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.spatial {
            SpatialPolicy::Q1 => Py::new(py, PyQ1).map(Py::into_any),
            SpatialPolicy::CellCenteredTpfa => Py::new(py, PyCellCenteredTpfa).map(Py::into_any),
        }
    }

    #[getter]
    fn solve(&self, py: Python<'_>) -> PyResult<Py<PyLinear>> {
        Py::new(py, PyLinear)
    }

    #[getter]
    fn discretization(&self) -> &'static str {
        self.spatial.name()
    }

    #[getter]
    fn space(&self) -> &'static str {
        self.spatial.space()
    }

    #[getter]
    fn quadrature(&self) -> &'static str {
        self.spatial.quadrature()
    }

    #[getter]
    fn mesh_kind(&self) -> &'static str {
        "generated-cartesian"
    }

    #[getter]
    fn spatial_dimension(&self) -> usize {
        self.native.requirements().spatial_dimension().get()
    }

    #[getter]
    fn scalar_type(&self) -> &'static str {
        "f64"
    }

    #[getter]
    fn vector_layout(&self) -> &'static str {
        "replicated"
    }

    #[getter]
    fn operator_properties(&self) -> &'static str {
        "symmetric-positive-definite"
    }

    #[getter]
    fn schedule(&self) -> &'static str {
        "offline"
    }

    #[getter]
    fn solver_algorithm(&self) -> &'static str {
        "conjugate-gradient"
    }

    #[getter]
    fn solver_backend(&self) -> &'static str {
        self.native.solver_backend()
    }

    #[getter]
    fn solver_backend_version(&self) -> &'static str {
        self.native.solver_backend_version()
    }

    #[getter]
    fn execution_provider(&self) -> &'static str {
        self.native.adapter()
    }

    #[getter]
    fn execution_provider_version(&self) -> &'static str {
        self.native.adapter_version()
    }

    #[getter]
    fn placement(&self) -> &'static str {
        "host-cpu"
    }

    #[getter]
    fn workers(&self) -> usize {
        self.native.intent().workers().get()
    }

    fn __repr__(&self) -> String {
        format!(
            "Plan(spatial={}, model_digest={:?}, mesh_digest={:?}, realization_digest={:?})",
            self.spatial.name(),
            self.model_digest(),
            self.mesh_digest(),
            self.realization_digest(),
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            self.model_digest() == other.model_digest()
                && self.mesh_digest() == other.mesh_digest()
                && self.realization_digest() == other.realization_digest()
        })
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.model_digest().hash(&mut hasher);
        self.mesh_digest().hash(&mut hasher);
        self.realization_digest().hash(&mut hasher);
        hasher.finish() as isize
    }
}

fn hash_pair(first: &str, second: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    first.hash(&mut hasher);
    second.hash(&mut hasher);
    hasher.finish()
}

/// Resolve one supported Model + Mesh + spatial + solve composition.
#[pyfunction]
#[pyo3(name = "resolve_plan")]
#[pyo3(signature = (model, /, *, mesh, spatial, solve))]
fn resolve(
    py: Python<'_>,
    model: &PyModel,
    mesh: PyCartesian,
    spatial: &Bound<'_, PyAny>,
    solve: &Bound<'_, PyAny>,
) -> PyResult<PyPlan> {
    panic_boundary(py, || {
        let spatial = if spatial.extract::<PyRef<'_, PyQ1>>().is_ok() {
            SpatialPolicy::Q1
        } else if spatial.extract::<PyRef<'_, PyCellCenteredTpfa>>().is_ok() {
            SpatialPolicy::CellCenteredTpfa
        } else {
            return Err(PyTypeError::new_err(
                "spatial must be eqiora.fem.Q1 or eqiora.fvm.CellCenteredTpfa",
            ));
        };
        solve.extract::<PyRef<'_, PyLinear>>().map_err(|_| {
            PyTypeError::new_err("solve must be the admitted eqiora.solve.Linear policy")
        })?;
        let document = model
            .document()
            .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?
            .clone();
        let intent = ScalarEllipticIntent::new(
            RealizationRevision::new(1),
            spatial.method(),
            mesh.cells_per_axis,
            NonZeroUsize::MIN,
        );
        let native = document
            .preview_scalar_elliptic_run_with_generated_mesh(
                intent,
                ScalarEllipticExecutionEnvironment::host_serial(),
            )
            .map_err(|diagnostics| diagnostic_error(py, &diagnostics))?;
        let effective_mesh = native.mesh().cloned().ok_or_else(|| {
            internal_diagnostic_error(
                py,
                &[eqiora::Diagnostic::error(
                    eqiora::diagnostic::codes::INTERNAL_FAILURE,
                    "common scalar Plan omitted its resolved effective Mesh",
                )],
            )
        })?;
        Ok(PyPlan {
            document,
            mesh: PyCartesianMesh::from_native(py, effective_mesh)?,
            spatial,
            native,
        })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCartesian>()?;
    module.add_class::<PyCartesianMesh>()?;
    module.add_class::<PyQ1>()?;
    module.add_class::<PyCellCenteredTpfa>()?;
    module.add_class::<PyLinear>()?;
    module.add_class::<PyPlan>()?;
    module.add_function(wrap_pyfunction!(resolve, module)?)?;
    Ok(())
}
