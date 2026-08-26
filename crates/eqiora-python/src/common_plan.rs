//! Root common Plan resolution over exact caller-owned Model and Mesh objects.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::time::Instant;

use eqiora::solver::{LinearSolver, REFERENCE_SOLVER_PROVIDER, SolverPlan};
use eqiora_numerics::{CommonScalarPlan, CommonScalarSpatialPolicy};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::error::{execution_error, validation_error};
use crate::execution::RunIdentity;
use crate::meshing::PyMesh;
use crate::model::{PyModel, PyModelFieldRef};
use crate::result::PyRunResult;

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

/// Closed host-serial reproducible CG policy for this scalar slice.
#[pyclass(
    name = "Linear",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PyLinear {
    native: SolverPlan,
}

#[pymethods]
impl PyLinear {
    #[new]
    #[pyo3(signature = (*, relative_tolerance, absolute_tolerance, maximum_iterations))]
    fn new(
        py: Python<'_>,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: usize,
    ) -> PyResult<Self> {
        let maximum_iterations = NonZeroUsize::new(maximum_iterations)
            .ok_or_else(|| PyTypeError::new_err("maximum_iterations must be a positive integer"))?;
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )
        .map(|native| Self { native })
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))
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
    fn relative_tolerance(&self) -> f64 {
        self.native.relative_tolerance()
    }
    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.native.absolute_tolerance()
    }
    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.native.maximum_iterations().get()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.native == other.native)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.relative_tolerance().to_bits().hash(&mut hasher);
        self.absolute_tolerance().to_bits().hash(&mut hasher);
        self.maximum_iterations().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "Linear(relative_tolerance={}, absolute_tolerance={}, maximum_iterations={})",
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpatialPolicy {
    Q1,
    CellCenteredTpfa,
}

impl SpatialPolicy {
    const fn bridge(self) -> CommonScalarSpatialPolicy {
        match self {
            Self::Q1 => CommonScalarSpatialPolicy::Q1,
            Self::CellCenteredTpfa => CommonScalarSpatialPolicy::CellCenteredTpfa,
        }
    }
    const fn discretization(self) -> &'static str {
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
            Self::CellCenteredTpfa => "cell-centroid/facet-midpoint",
        }
    }
}

/// Immutable common Plan owning one exact Model, Mesh, and effective policy set.
#[pyclass(name = "Plan", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug)]
pub(crate) struct PyPlan {
    native: CommonScalarPlan,
    model: Py<PyModel>,
    mesh: Py<PyMesh>,
    spatial: SpatialPolicy,
    solve: Py<PyLinear>,
}

impl PyPlan {
    pub(crate) const fn native(&self) -> &CommonScalarPlan {
        &self.native
    }
    pub(crate) fn mesh_handle(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }
}

#[pymethods]
impl PyPlan {
    #[getter]
    fn identity(&self) -> &str {
        self.native.identity()
    }
    #[getter]
    fn model_id(&self) -> &str {
        self.native.model_id()
    }
    #[getter]
    fn model_digest(&self) -> &str {
        self.native.model_digest()
    }
    #[getter]
    fn model_revision(&self) -> u64 {
        self.native.model_revision()
    }
    #[getter]
    fn geometry_digest(&self) -> &str {
        self.native.geometry_digest()
    }
    #[getter]
    fn mesh_digest(&self) -> &str {
        self.native.mesh_digest()
    }
    #[getter]
    fn correspondence_digest(&self) -> &str {
        self.native.correspondence_digest()
    }
    #[getter]
    fn production_digest(&self) -> &str {
        self.native.production_digest()
    }
    #[getter]
    fn model(&self, py: Python<'_>) -> Py<PyModel> {
        self.model.clone_ref(py)
    }
    #[getter]
    fn mesh(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh.clone_ref(py)
    }
    #[getter]
    fn field(&self) -> PyModelFieldRef {
        PyModelFieldRef::from_exact(
            self.native.model_digest().to_owned(),
            self.native.field_id().to_owned(),
        )
    }
    #[getter]
    fn spatial(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.spatial {
            SpatialPolicy::Q1 => Py::new(py, PyQ1).map(Py::into_any),
            SpatialPolicy::CellCenteredTpfa => Py::new(py, PyCellCenteredTpfa).map(Py::into_any),
        }
    }
    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinear> {
        self.solve.clone_ref(py)
    }
    #[getter]
    fn discretization(&self) -> &'static str {
        self.spatial.discretization()
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
    const fn mesh_kind(&self) -> &'static str {
        "structured-cartesian"
    }
    #[getter]
    const fn spatial_dimension(&self) -> usize {
        2
    }
    #[getter]
    fn cells(&self) -> (usize, usize) {
        let [x, y] = self.native.cells();
        (x, y)
    }
    #[getter]
    const fn scalar_type(&self) -> &'static str {
        "f64"
    }
    #[getter]
    const fn vector_layout(&self) -> &'static str {
        "replicated"
    }
    #[getter]
    const fn operator_properties(&self) -> &'static str {
        "symmetric-positive-definite"
    }
    #[getter]
    const fn schedule(&self) -> &'static str {
        "offline"
    }
    #[getter]
    const fn solver_algorithm(&self) -> &'static str {
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
    const fn solver_backend(&self) -> &'static str {
        REFERENCE_SOLVER_PROVIDER.id().as_str()
    }
    #[getter]
    const fn solver_backend_version(&self) -> &'static str {
        REFERENCE_SOLVER_PROVIDER.implementation_version()
    }
    #[getter]
    const fn execution_provider(&self) -> &'static str {
        eqiora::solver::SERIAL_EXECUTION_PROVIDER.id().as_str()
    }
    #[getter]
    const fn execution_provider_version(&self) -> &'static str {
        eqiora::solver::SERIAL_EXECUTION_PROVIDER.implementation_version()
    }
    #[getter]
    const fn placement(&self) -> &'static str {
        "host-serial"
    }
    #[getter]
    const fn workers(&self) -> usize {
        1
    }
    #[getter]
    const fn scaling(&self) -> Option<()> {
        None
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.identity() == other.identity())
    }
    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.identity().hash(&mut hasher);
        hasher.finish() as isize
    }
    fn __repr__(&self) -> String {
        format!(
            "Plan(identity={:?}, model_digest={:?}, mesh_digest={:?}, discretization={:?})",
            self.identity(),
            self.model_digest(),
            self.mesh_digest(),
            self.discretization()
        )
    }
}

#[pyfunction(name = "_resolve_plan")]
#[pyo3(signature = (model, /, *, mesh, spatial, solve))]
fn resolve_plan(
    py: Python<'_>,
    model: Py<PyModel>,
    mesh: Py<PyMesh>,
    spatial: &Bound<'_, PyAny>,
    solve: Py<PyLinear>,
) -> PyResult<PyPlan> {
    let spatial = if spatial.extract::<PyRef<'_, PyQ1>>().is_ok() {
        SpatialPolicy::Q1
    } else if spatial.extract::<PyRef<'_, PyCellCenteredTpfa>>().is_ok() {
        SpatialPolicy::CellCenteredTpfa
    } else {
        return Err(PyTypeError::new_err(
            "spatial must be eqiora.fem.Q1() or eqiora.fvm.CellCenteredTpfa()",
        ));
    };
    let solve_ref = solve.borrow(py);
    let model_ref = model.borrow(py);
    let mesh_ref = mesh.borrow(py);
    let owner = mesh_ref
        .authenticated_common_mesh()
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
        .ok_or_else(|| {
            PyTypeError::new_err("mesh must be an authenticated caller-owned common Mesh")
        })?;
    let native = CommonScalarPlan::resolve(
        model_ref.artifact(),
        owner,
        spatial.bridge(),
        solve_ref.native,
    )
    .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
    if native.mesh_digest() != mesh_ref.exact_mesh_digest() {
        return Err(PyTypeError::new_err(
            "resolved Plan did not retain the exact caller Mesh occurrence",
        ));
    }
    drop(solve_ref);
    drop(mesh_ref);
    drop(model_ref);
    Ok(PyPlan {
        native,
        model,
        mesh,
        spatial,
        solve,
    })
}

#[pyfunction(name = "_run_plan")]
#[pyo3(signature = (plan, /))]
fn run_plan(py: Python<'_>, plan: &PyPlan) -> PyResult<PyRunResult> {
    let native = plan.native.clone();
    let (run, elapsed) = py
        .detach(move || {
            let started = Instant::now();
            native.run().map(|run| (run, started.elapsed()))
        })
        .map_err(|diagnostic| execution_error(py, &[diagnostic]))?;
    let identity = RunIdentity::from_common_plan(plan.native());
    PyRunResult::from_common_scalar(
        py,
        identity,
        plan.mesh_handle(py),
        plan.native.field_id().to_owned(),
        plan.native.cells(),
        elapsed,
        run,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyQ1>()?;
    module.add_class::<PyCellCenteredTpfa>()?;
    module.add_class::<PyLinear>()?;
    module.add_class::<PyPlan>()?;
    module.add_function(wrap_pyfunction!(resolve_plan, module)?)?;
    module.add_function(wrap_pyfunction!(run_plan, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eqiora::api::ModelDocument;
    use eqiora::geometry::{
        CanonicalGeometryV1, NamedEntitySet, PlanarOperationGraph, PlanarTopologyHandle,
    };
    use eqiora::{DimExponents, DynQuantity};
    use pyo3::ffi::c_str;
    use pyo3::prelude::*;
    use pyo3::types::{PyDict, PyDictMethods};

    use crate::geometry::PyGeometry;
    use crate::model::PyModel;

    const COMPONENT: &str = r#"
public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

    type SupportBinding<'a> = (
        &'a str,
        &'a NamedEntitySet,
        Option<(&'a str, &'a NamedEntitySet)>,
    );

    fn rectangle_geometry(xmax: f64) -> CanonicalGeometryV1 {
        let graph = PlanarOperationGraph::new();
        let rectangle = graph.rectangle([0.0, xmax], [0.0, 1.0]).unwrap();
        let edges = rectangle.boundaries();
        graph
            .build(
                &rectangle,
                &BTreeMap::from([
                    ("region".to_owned(), vec![rectangle.region().into()]),
                    (
                        "left".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[0])],
                    ),
                    (
                        "right".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[1])],
                    ),
                    (
                        "bottom".to_owned(),
                        vec![PlanarTopologyHandle::from(edges[2])],
                    ),
                    ("top".to_owned(), vec![PlanarTopologyHandle::from(edges[3])]),
                ]),
            )
            .unwrap()
    }

    fn scalar_document(geometry: &CanonicalGeometryV1, source_scale: f64) -> ModelDocument {
        let region = geometry.entity_set("region").unwrap();
        let supports: [SupportBinding<'_>; 5] = [
            ("region", region, None),
            (
                "left",
                geometry.entity_set("left").unwrap(),
                Some(("region", region)),
            ),
            (
                "right",
                geometry.entity_set("right").unwrap(),
                Some(("region", region)),
            ),
            (
                "bottom",
                geometry.entity_set("bottom").unwrap(),
                Some(("region", region)),
            ),
            (
                "top",
                geometry.entity_set("top").unwrap(),
                Some(("region", region)),
            ),
        ];
        let parameters = [
            (
                "wave_number",
                DynQuantity::new(
                    std::f64::consts::PI,
                    DimExponents {
                        length: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "source_scale",
                DynQuantity::new(
                    source_scale,
                    DimExponents {
                        length: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        ModelDocument::compile_external_component(
            "python-common-plan.eqi",
            COMPONENT,
            geometry,
            "PoissonRectangleModel",
            "PoissonRectangle",
            &supports,
            &parameters,
        )
        .unwrap()
    }

    #[test]
    fn installed_root_resolve_and_run_retain_exact_python_objects() -> PyResult<()> {
        Python::initialize();
        Python::attach(|py| {
            let native = pyo3::wrap_pymodule!(crate::_eqiora)(py);
            let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../bindings/python/python/eqiora")
                .canonicalize()?;
            let locals = PyDict::new(py);
            locals.set_item("native", native.bind(py))?;
            locals.set_item("package_directory", package_directory.to_string_lossy())?;
            py.run(
                c_str!(r#"
import importlib.util, pathlib, sys
package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location("eqiora", package_path / "__init__.py", submodule_search_locations=[str(package_path)])
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)

graph = package.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
source = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
mesher = package.meshing.CartesianMesher(cells=(2, 3))
mesh_plan = package.meshing.resolve(source, package.meshing.MeshRequest(mesher))
mesh = package.meshing.generate(source, plan=mesh_plan)
"#),
                None,
                Some(&locals),
            )?;
            let geometry = locals
                .get_item("source")?
                .unwrap()
                .extract::<PyRef<'_, PyGeometry>>()?
                .geometry()
                .clone();
            let model = PyModel::from_document(
                py,
                scalar_document(&geometry, 2.0 * std::f64::consts::PI.powi(2)),
            )?;
            let fresh_model = PyModel::from_document(
                py,
                scalar_document(&geometry, 3.0 * std::f64::consts::PI.powi(2)),
            )?;
            let foreign_model = PyModel::from_document(
                py,
                scalar_document(&rectangle_geometry(2.0), 2.0 * std::f64::consts::PI.powi(2)),
            )?;
            locals.set_item("model", Py::new(py, model)?)?;
            locals.set_item("fresh_model", Py::new(py, fresh_model)?)?;
            locals.set_item("foreign_model", Py::new(py, foreign_model)?)?;
            py.run(
                c_str!(r#"
linear = package.solve.Linear(relative_tolerance=1e-10, absolute_tolerance=1e-12, maximum_iterations=10000)
q1 = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
q1_repeat = package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
tpfa = package.resolve(model, mesh=mesh, spatial=package.fvm.CellCenteredTpfa(), solve=linear)
replayed = package.replay(model.to_json())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
assert q1.model is model
assert q1.mesh is mesh
assert q1.mesh_digest == mesh.digest
assert q1.identity == q1_repeat.identity
assert replayed_plan.identity == q1.identity
assert fresh_model.digest != model.digest
assert fresh_plan.model_digest == fresh_model.digest
assert fresh_plan.identity != q1.identity
assert q1.identity != tpfa.identity
assert q1.cells == (2, 3)
assert q1.scaling is None
assert q1.solve is linear
q1_result = package.run(q1)
tpfa_result = package.run(tpfa)
assert q1_result.model_digest == q1.model_digest
assert q1_result.plan_key == q1.identity
assert q1_result.mesh(q1.field) is mesh
assert q1_result.logical_shape == (3, 4)
assert len(q1_result.values) == 12
try:
    q1_result.run_manifest()
except package.CapabilityError:
    pass
else:
    raise AssertionError("common scalar Result fabricated a durable Run artifact")
assert tpfa_result.logical_shape == (2, 3)
assert len(tpfa_result.values) == 6
try:
    package.resolve(foreign_model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("foreign Model was admitted against the caller Mesh")

foreign_rectangle = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(0.0, 1.0))
foreign_source = graph.build(foreign_rectangle, named_topology={
    "region": foreign_rectangle.region,
    "left": foreign_rectangle.boundaries[0],
    "right": foreign_rectangle.boundaries[1],
    "bottom": foreign_rectangle.boundaries[2],
    "top": foreign_rectangle.boundaries[3],
})
foreign_plan = package.meshing.resolve(foreign_source, package.meshing.MeshRequest(mesher))
foreign_mesh = package.meshing.generate(foreign_source, plan=foreign_plan)
try:
    package.resolve(model, mesh=foreign_mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("same-cardinality foreign Mesh was admitted")
try:
    package.run(model)
except TypeError:
    pass
else:
    raise AssertionError("root run retained a Model-shaped compatibility path")
"#),
                None,
                Some(&locals),
            )
        })
    }
}
