//! Root common Plan resolution over exact caller-owned Model and Mesh objects.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use eqiora::backends::faer::{FAER_SOLVER_PROVIDER, FaerLinearSolver};
use eqiora::solver::{LinearSolver, REFERENCE_SOLVER_PROVIDER, ReductionPolicy, SolverPlan};
use eqiora_numerics::{
    CommonScalarPlan, CommonSpatialPolicy, CommonSteadyStokesPlan, resolve_common_plan,
};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule, PyTuple};

use crate::error::{execution_error, validation_error};
use crate::execution::RunIdentity;
use crate::meshing::PyMesh;
use crate::model::{PyModel, PyModelFieldRef};
use crate::result::PyRunResult;

mod policy;
use policy::{PyCellCenteredTpfa, PyLinear, PyMiniP1, PyQ1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpatialPolicy {
    Q1,
    CellCenteredTpfa,
    MiniP1,
}

impl SpatialPolicy {
    const fn native(self) -> CommonSpatialPolicy {
        match self {
            Self::Q1 => CommonSpatialPolicy::Q1,
            Self::CellCenteredTpfa => CommonSpatialPolicy::CellCenteredTpfa,
            Self::MiniP1 => CommonSpatialPolicy::MiniP1,
        }
    }
    const fn discretization(self) -> &'static str {
        match self {
            Self::Q1 => "q1",
            Self::CellCenteredTpfa => "cell-centered-tpfa",
            Self::MiniP1 => "mini-p1",
        }
    }
    const fn space(self) -> &'static str {
        match self {
            Self::Q1 => "continuous-lagrange-q1",
            Self::CellCenteredTpfa => "cell-constant",
            Self::MiniP1 => "(simplex-p1-bubble)^2/continuous-lagrange-p1",
        }
    }
    const fn quadrature(self) -> &'static str {
        match self {
            Self::Q1 => "gauss-legendre-2-per-axis",
            Self::CellCenteredTpfa => "cell-centroid/facet-midpoint",
            Self::MiniP1 => "triangle-duffy-gauss-legendre-3-per-axis",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CommonPlanKind {
    Scalar(Box<CommonScalarPlan>),
    SteadyStokes(Box<CommonSteadyStokesPlan>),
}

impl CommonPlanKind {
    fn identity(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.identity(),
            Self::SteadyStokes(plan) => plan.identity(),
        }
    }
    fn model_id(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.model_id(),
            Self::SteadyStokes(plan) => plan.model_id(),
        }
    }
    fn model_digest(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.model_digest(),
            Self::SteadyStokes(plan) => plan.model_digest(),
        }
    }
    const fn model_revision(&self) -> u64 {
        match self {
            Self::Scalar(plan) => plan.model_revision(),
            Self::SteadyStokes(plan) => plan.model_revision(),
        }
    }
    fn geometry_digest(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.geometry_digest(),
            Self::SteadyStokes(plan) => plan.geometry_digest(),
        }
    }
    fn mesh_digest(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.mesh_digest(),
            Self::SteadyStokes(plan) => plan.mesh_digest(),
        }
    }
    fn correspondence_digest(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.correspondence_digest(),
            Self::SteadyStokes(plan) => plan.correspondence_digest(),
        }
    }
    fn production_digest(&self) -> &str {
        match self {
            Self::Scalar(plan) => plan.production_digest(),
            Self::SteadyStokes(plan) => plan.production_digest(),
        }
    }
    const fn effective_solver(&self) -> SolverPlan {
        match self {
            Self::Scalar(plan) => plan.linear(),
            Self::SteadyStokes(plan) => plan.linear(),
        }
    }
}

/// Immutable common Plan owning one exact Model, Mesh, and effective policy set.
#[pyclass(name = "Plan", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug)]
pub(crate) struct PyPlan {
    native: CommonPlanKind,
    model: Py<PyModel>,
    mesh: Py<PyMesh>,
    spatial: SpatialPolicy,
    solve: Py<PyLinear>,
}

impl PyPlan {
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
    fn field(&self) -> Option<PyModelFieldRef> {
        let CommonPlanKind::Scalar(plan) = &self.native else {
            return None;
        };
        Some(PyModelFieldRef::from_exact(
            plan.model_digest().to_owned(),
            plan.field_id().to_owned(),
        ))
    }
    #[getter]
    fn fields(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let model_digest = self.native.model_digest().to_owned();
        let fields = match &self.native {
            CommonPlanKind::Scalar(plan) => vec![PyModelFieldRef::from_exact(
                model_digest,
                plan.field_id().to_owned(),
            )],
            CommonPlanKind::SteadyStokes(plan) => vec![
                PyModelFieldRef::from_exact(
                    model_digest.clone(),
                    plan.velocity_field_id().to_owned(),
                ),
                PyModelFieldRef::from_exact(model_digest, plan.pressure_field_id().to_owned()),
            ],
        };
        Ok(PyTuple::new(py, fields)?.unbind())
    }
    #[getter]
    fn velocity_field(&self) -> Option<PyModelFieldRef> {
        let CommonPlanKind::SteadyStokes(plan) = &self.native else {
            return None;
        };
        Some(PyModelFieldRef::from_exact(
            plan.model_digest().to_owned(),
            plan.velocity_field_id().to_owned(),
        ))
    }
    #[getter]
    fn pressure_field(&self) -> Option<PyModelFieldRef> {
        let CommonPlanKind::SteadyStokes(plan) = &self.native else {
            return None;
        };
        Some(PyModelFieldRef::from_exact(
            plan.model_digest().to_owned(),
            plan.pressure_field_id().to_owned(),
        ))
    }
    #[getter]
    fn spatial(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.spatial {
            SpatialPolicy::Q1 => Py::new(py, PyQ1).map(Py::into_any),
            SpatialPolicy::CellCenteredTpfa => Py::new(py, PyCellCenteredTpfa).map(Py::into_any),
            SpatialPolicy::MiniP1 => Py::new(py, PyMiniP1).map(Py::into_any),
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
    fn mesh_kind(&self) -> &'static str {
        match self.native {
            CommonPlanKind::Scalar(_) => "structured-cartesian",
            CommonPlanKind::SteadyStokes(_) => "imported-affine-simplicial",
        }
    }
    #[getter]
    const fn spatial_dimension(&self) -> usize {
        2
    }
    #[getter]
    fn cells(&self) -> Option<(usize, usize)> {
        match &self.native {
            CommonPlanKind::Scalar(plan) => {
                let [x, y] = plan.cells();
                Some((x, y))
            }
            CommonPlanKind::SteadyStokes(_) => None,
        }
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
    fn operator_properties(&self) -> &'static str {
        match self.native {
            CommonPlanKind::Scalar(_) => "symmetric-positive-definite",
            CommonPlanKind::SteadyStokes(_) => "symmetric-indefinite",
        }
    }
    #[getter]
    const fn schedule(&self) -> &'static str {
        "offline"
    }
    #[getter]
    fn solver_algorithm(&self) -> &'static str {
        match self.native.effective_solver().algorithm() {
            LinearSolver::ConjugateGradient => "conjugate-gradient",
            LinearSolver::MinimumResidual => "minimum-residual",
            LinearSolver::BiConjugateGradientStabilized => "bicgstab",
            LinearSolver::SparseLu => "sparse-lu",
        }
    }
    #[getter]
    const fn preconditioner(&self) -> &'static str {
        "identity"
    }
    #[getter]
    fn reduction(&self) -> &'static str {
        match self.native.effective_solver().reduction() {
            ReductionPolicy::Reproducible => "reproducible",
            ReductionPolicy::Fast => "fast",
        }
    }
    #[getter]
    fn solver_backend(&self) -> &'static str {
        match self.native {
            CommonPlanKind::Scalar(_) => REFERENCE_SOLVER_PROVIDER.id().as_str(),
            CommonPlanKind::SteadyStokes(_) => FAER_SOLVER_PROVIDER.id().as_str(),
        }
    }
    #[getter]
    fn solver_backend_version(&self) -> &'static str {
        match self.native {
            CommonPlanKind::Scalar(_) => REFERENCE_SOLVER_PROVIDER.implementation_version(),
            CommonPlanKind::SteadyStokes(_) => FAER_SOLVER_PROVIDER.implementation_version(),
        }
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
#[pyo3(signature = (model, /, *, mesh, spatial, solve, scaling=None))]
fn resolve_plan(
    py: Python<'_>,
    model: Py<PyModel>,
    mesh: Py<PyMesh>,
    spatial: &Bound<'_, PyAny>,
    solve: Py<PyLinear>,
    scaling: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPlan> {
    let spatial = if spatial.extract::<PyRef<'_, PyQ1>>().is_ok() {
        SpatialPolicy::Q1
    } else if spatial.extract::<PyRef<'_, PyCellCenteredTpfa>>().is_ok() {
        SpatialPolicy::CellCenteredTpfa
    } else if spatial.extract::<PyRef<'_, PyMiniP1>>().is_ok() {
        SpatialPolicy::MiniP1
    } else {
        return Err(PyTypeError::new_err(
            "spatial must be eqiora.fem.Q1(), eqiora.fem.MiniP1(), or eqiora.fvm.CellCenteredTpfa()",
        ));
    };
    if scaling.is_some_and(|value| !value.is_none()) {
        return Err(PyTypeError::new_err(
            "typed scaling overrides are not admitted for this capability; omit scaling or pass None",
        ));
    }
    let solve_ref = solve.borrow(py);
    let model_ref = model.borrow(py);
    let mesh_ref = mesh.borrow(py);
    let owner = mesh_ref
        .authenticated_common_mesh()
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?
        .ok_or_else(|| {
            PyTypeError::new_err("mesh must be an authenticated caller-owned common Mesh")
        })?;
    let native = resolve_common_plan(
        model_ref.artifact(),
        owner,
        spatial.native(),
        solve_ref.native,
        &FaerLinearSolver,
    )
    .map(|plan| {
        plan.project(
            |plan| CommonPlanKind::Scalar(Box::new(plan)),
            |plan| CommonPlanKind::SteadyStokes(Box::new(plan)),
        )
    })
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
    match native {
        CommonPlanKind::Scalar(native) => {
            let (run, elapsed) = py
                .detach(move || {
                    let started = Instant::now();
                    native.run().map(|run| (run, started.elapsed()))
                })
                .map_err(|diagnostic| execution_error(py, &[diagnostic]))?;
            let CommonPlanKind::Scalar(native) = &plan.native else {
                unreachable!()
            };
            let identity = RunIdentity::from_common_plan(native);
            PyRunResult::from_common_scalar(
                py,
                identity,
                plan.mesh_handle(py),
                native.field_id().to_owned(),
                native.cells(),
                elapsed,
                run,
            )
        }
        CommonPlanKind::SteadyStokes(native) => {
            let (run, elapsed) = py
                .detach(move || {
                    let started = Instant::now();
                    native
                        .run(&FaerLinearSolver)
                        .map(|run| (run, started.elapsed()))
                })
                .map_err(|diagnostic| execution_error(py, &[diagnostic]))?;
            let CommonPlanKind::SteadyStokes(native) = &plan.native else {
                unreachable!()
            };
            let identity = RunIdentity::from_common_steady_stokes(native);
            PyRunResult::from_common_steady_stokes(
                py,
                identity,
                plan.mesh_handle(py),
                native.velocity_field_id().to_owned(),
                native.pressure_field_id().to_owned(),
                elapsed,
                run,
            )
        }
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyQ1>()?;
    module.add_class::<PyMiniP1>()?;
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

    const STOKES_COMPONENT: &str =
        include_str!("../../eqiora-api/src/steady_stokes/accepted_component.eqi");

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

    fn stokes_document(geometry: &CanonicalGeometryV1) -> ModelDocument {
        let fluid = geometry.entity_set("fluid").unwrap();
        let supports: [SupportBinding<'_>; 5] = [
            ("fluid", fluid, None),
            (
                "inlet",
                geometry.entity_set("inlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "outlet",
                geometry.entity_set("outlet").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "walls",
                geometry.entity_set("walls").unwrap(),
                Some(("fluid", fluid)),
            ),
            (
                "cylinder",
                geometry.entity_set("cylinder").unwrap(),
                Some(("fluid", fluid)),
            ),
        ];
        let parameters = [
            (
                "dynamic_viscosity",
                DynQuantity::new(
                    0.001,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "zero_pressure",
                DynQuantity::new(
                    0.0,
                    DimExponents {
                        mass: 1,
                        length: -1,
                        time: -2,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "inlet_speed",
                DynQuantity::new(
                    0.3,
                    DimExponents {
                        length: 1,
                        time: -1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
            (
                "channel_height",
                DynQuantity::new(
                    0.41,
                    DimExponents {
                        length: 1,
                        ..DimExponents::DIMENSIONLESS
                    },
                ),
            ),
        ];
        ModelDocument::compile_external_component(
            "steady-flow-past-cylinder.eqi",
            STOKES_COMPONENT,
            geometry,
            "SteadyFlowPastCylinderModel",
            "SteadyFlowPastCylinder",
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
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("MINI/P1 selected Stokes physics for a scalar Model")

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

    #[test]
    fn root_plan_resolves_model_owned_steady_stokes_with_automatic_scaling() -> PyResult<()> {
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
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
source = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:],
    "cylinder": circle.boundaries[0],
})
mesher = package.meshing.ReferenceMesher(maximum_boundary_error=1e-4, minimum_mean_ratio=1e-5, maximum_boundary_facets=50)
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
            let model = PyModel::from_document(py, stokes_document(&geometry))?;
            let fresh_model = PyModel::from_document(py, stokes_document(&geometry))?;
            let foreign_model = PyModel::from_document(
                py,
                scalar_document(&rectangle_geometry(1.0), 2.0 * std::f64::consts::PI.powi(2)),
            )?;
            locals.set_item("model", Py::new(py, model)?)?;
            locals.set_item("fresh_model", Py::new(py, fresh_model)?)?;
            locals.set_item("foreign_model", Py::new(py, foreign_model)?)?;
            py.run(
                c_str!(r#"
linear = package.solve.Linear(relative_tolerance=1e-6, absolute_tolerance=1e-13, maximum_iterations=10000)
plan = package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
explicit_none = package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear, scaling=None)
replayed = package.replay(model.to_json())
replayed_plan = package.resolve(replayed, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
fresh_plan = package.resolve(fresh_model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear)
assert plan.identity == explicit_none.identity == replayed_plan.identity == fresh_plan.identity
assert plan.model is model
assert plan.mesh is mesh
assert plan.mesh_digest == mesh.digest
assert plan.discretization == "mini-p1"
assert plan.mesh_kind == "imported-affine-simplicial"
assert plan.operator_properties == "symmetric-indefinite"
assert plan.solver_algorithm == "sparse-lu"
assert plan.preconditioner == "identity"
assert plan.reduction == "fast"
assert plan.solver_backend == "eqiora.faer"
assert plan.solve is linear
result = package.run(plan)
assert isinstance(result, package.Result)
assert result.model_digest == plan.model_digest
assert result.plan_key == plan.identity
assert len(plan.fields) == 2
assert plan.fields == (plan.velocity_field, plan.pressure_field)
assert plan.field is None
assert not hasattr(result, "outputs")
velocity = result.output(plan.velocity_field)
pressure = result.output(plan.pressure_field)
assert result.mesh(plan.velocity_field) is mesh
assert result.mesh(plan.pressure_field) is mesh
assert velocity.field == plan.velocity_field
assert pressure.field == plan.pressure_field
assert velocity.mesh is pressure.mesh is mesh
assert velocity.dimension == (0, 1, -1, 0, 0, 0, 0)
assert velocity.components == 2
assert velocity.vertex_count == mesh.vertex_count
assert len(velocity.vertex_values) == mesh.vertex_count * 2
assert velocity.cell_bubble_count == mesh.cell_count
assert len(velocity.cell_bubble_values) == mesh.cell_count * 2
assert pressure.dimension == (1, -1, -2, 0, 0, 0, 0)
assert pressure.components == 1
assert pressure.vertex_count == mesh.vertex_count
assert len(pressure.vertex_values) == mesh.vertex_count
assert pressure.cell_bubble_count == 0
assert pressure.cell_bubble_values is None
assert result.solve.algorithm == "sparse-lu"
foreign_field = foreign_model.field(foreign_model.field_ids[0])
try:
    result.output(foreign_field)
except ValueError:
    pass
else:
    raise AssertionError("foreign exact FieldRef selected a common output")
try:
    result.output("pressure")
except TypeError:
    pass
else:
    raise AssertionError("string/name lookup selected a common output")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("Stokes Model selected scalar physics through a spatial policy")
try:
    package.resolve(model, mesh=mesh, spatial=package.fem.MiniP1(), solve=linear, scaling=object())
except TypeError:
    pass
else:
    raise AssertionError("untyped scaling reached Plan publication")
"#),
                None,
                Some(&locals),
            )
        })
    }
}
