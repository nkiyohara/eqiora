//! Root common Plan resolution over an exact Model and applicable caller resources.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::artifact::CanonicalModelArtifact;
use eqiora::backends::faer::{FAER_SOLVER_PROVIDER, FaerLinearSolver};
use eqiora::realization::{Space, SpaceFamily};
use eqiora::solver::{LinearSolver, REFERENCE_SOLVER_PROVIDER, ReductionPolicy, SolverPlan};
use eqiora_numerics::{
    CommonElasticityPlan, CommonFsiPlan, CommonOdePlan, CommonScalarPlan,
    CommonScopedSpatialPolicy, CommonSolvePolicy, CommonSpatialPolicy, CommonSpatialRequest,
    CommonSteadyStokesPlan, CommonTransientFlowPlan, resolve_common_ode_plan, resolve_common_plan,
};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule, PyTuple};

use crate::error::validation_error;
use crate::meshing::PyMesh;
use crate::model::{PyModel, PyModelFieldRef};

mod policy;
use policy::{
    PyBackwardEuler, PyCellCentered, PyCellCenteredTpfa, PyLinear, PyMiniP1, PyNewton, PyP1,
    PyPressureGauge2d, PyQ1, PyScopedSpatialBinding, PyTsitouras45, ScopedSpatialKind,
};
mod scaling;
use scaling::{PyIncompressibleScales, PyIncompressibleScaling, PyIncompressibleScalingReceipt2d};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpatialPolicy {
    Q1,
    CellCenteredTpfa,
    MiniP1,
    CellCentered,
}

#[derive(Debug)]
enum SpatialHandle {
    Uniform(SpatialPolicy),
    Scoped(Vec<Py<PyScopedSpatialBinding>>),
}

impl SpatialPolicy {
    const fn discretization(self) -> &'static str {
        match self {
            Self::Q1 => "q1",
            Self::CellCenteredTpfa => "cell-centered-tpfa",
            Self::MiniP1 => "mini-p1",
            Self::CellCentered => "cell-centered",
        }
    }
    const fn space(self) -> &'static str {
        match self {
            Self::Q1 => "continuous-lagrange-q1",
            Self::CellCenteredTpfa => "cell-constant",
            Self::MiniP1 => "(simplex-p1-bubble)^2/continuous-lagrange-p1",
            Self::CellCentered => "cell-constant-velocity/cell-constant-pressure",
        }
    }
    const fn quadrature(self) -> &'static str {
        match self {
            Self::Q1 => "gauss-legendre-2-per-axis",
            Self::CellCenteredTpfa => "cell-centroid/facet-midpoint",
            Self::MiniP1 => "triangle-duffy-gauss-legendre-3-per-axis",
            Self::CellCentered => "cell-centroid/facet-midpoint",
        }
    }
}

fn space_name(space: Space) -> &'static str {
    match space.family() {
        SpaceFamily::SimplexP1Bubble => "simplex-p1-bubble",
        SpaceFamily::ContinuousLagrange { order } if order.get() == 1 => "continuous-lagrange-p1",
        SpaceFamily::CellConstant => "cell-constant",
        SpaceFamily::ContinuousLagrange { .. } => {
            unreachable!("common Plan only publishes the admitted P1 continuous space")
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CommonPlanKind {
    Ode(Box<CommonOdePlan>),
    Scalar(Box<CommonScalarPlan>),
    Elasticity(Box<CommonElasticityPlan>),
    SteadyStokes(Box<CommonSteadyStokesPlan>),
    TransientFlow(Box<CommonTransientFlowPlan>),
    Fsi(Box<CommonFsiPlan>),
}

impl CommonPlanKind {
    fn identity(&self) -> &str {
        match self {
            Self::Ode(plan) => plan.identity(),
            Self::Scalar(plan) => plan.identity(),
            Self::Elasticity(plan) => plan.identity(),
            Self::SteadyStokes(plan) => plan.identity(),
            Self::TransientFlow(plan) => plan.identity(),
            Self::Fsi(plan) => plan.identity(),
        }
    }
    fn model_id(&self) -> &str {
        match self {
            Self::Ode(plan) => plan.model_id(),
            Self::Scalar(plan) => plan.model_id(),
            Self::Elasticity(plan) => plan.model_id(),
            Self::SteadyStokes(plan) => plan.model_id(),
            Self::TransientFlow(plan) => plan.model_id(),
            Self::Fsi(plan) => plan.model_id(),
        }
    }
    fn model_digest(&self) -> &str {
        match self {
            Self::Ode(plan) => plan.model_digest(),
            Self::Scalar(plan) => plan.model_digest(),
            Self::Elasticity(plan) => plan.model_digest(),
            Self::SteadyStokes(plan) => plan.model_digest(),
            Self::TransientFlow(plan) => plan.model_digest(),
            Self::Fsi(plan) => plan.model_digest(),
        }
    }
    const fn model_revision(&self) -> u64 {
        match self {
            Self::Ode(plan) => plan.model_revision(),
            Self::Scalar(plan) => plan.model_revision(),
            Self::Elasticity(plan) => plan.model_revision(),
            Self::SteadyStokes(plan) => plan.model_revision(),
            Self::TransientFlow(plan) => plan.model_revision(),
            Self::Fsi(plan) => plan.model_revision(),
        }
    }
    fn geometry_digest(&self) -> Option<&str> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(plan) => Some(plan.geometry_digest()),
            Self::Elasticity(plan) => Some(plan.geometry_digest()),
            Self::SteadyStokes(plan) => Some(plan.geometry_digest()),
            Self::TransientFlow(plan) => Some(plan.geometry_digest()),
            Self::Fsi(plan) => Some(plan.geometry_digest()),
        }
    }
    fn mesh_digest(&self) -> Option<&str> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(plan) => Some(plan.mesh_digest()),
            Self::Elasticity(plan) => Some(plan.mesh_digest()),
            Self::SteadyStokes(plan) => Some(plan.mesh_digest()),
            Self::TransientFlow(plan) => Some(plan.mesh_digest()),
            Self::Fsi(plan) => Some(plan.mesh_digest()),
        }
    }
    fn correspondence_digest(&self) -> Option<&str> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(plan) => Some(plan.correspondence_digest()),
            Self::Elasticity(plan) => Some(plan.correspondence_digest()),
            Self::SteadyStokes(plan) => Some(plan.correspondence_digest()),
            Self::TransientFlow(plan) => Some(plan.correspondence_digest()),
            Self::Fsi(plan) => Some(plan.correspondence_digest()),
        }
    }
    fn production_digest(&self) -> Option<&str> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(plan) => Some(plan.production_digest()),
            Self::Elasticity(plan) => Some(plan.production_digest()),
            Self::SteadyStokes(plan) => Some(plan.production_digest()),
            Self::TransientFlow(plan) => Some(plan.production_digest()),
            Self::Fsi(plan) => Some(plan.production_digest()),
        }
    }
    const fn effective_solver(&self) -> Option<SolverPlan> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(plan) => Some(plan.linear()),
            Self::Elasticity(plan) => Some(plan.linear()),
            Self::SteadyStokes(plan) => Some(plan.linear()),
            Self::TransientFlow(plan) => Some(plan.linear()),
            Self::Fsi(plan) => Some(plan.linear()),
        }
    }
    fn realization_digest(&self) -> Option<&str> {
        match self {
            Self::Ode(_) => None,
            Self::Scalar(_) => None,
            Self::Elasticity(_) => None,
            Self::SteadyStokes(plan) => Some(plan.realization_digest()),
            Self::TransientFlow(_) => None,
            Self::Fsi(plan) => Some(plan.realization_digest()),
        }
    }
}

#[derive(Debug)]
enum SolveHandle {
    Linear(Py<PyLinear>),
    Newton(Py<PyNewton>),
}

#[derive(Debug)]
enum TemporalHandle {
    BackwardEuler(Py<PyBackwardEuler>),
    Tsitouras45(Py<PyTsitouras45>),
}

/// Immutable common Plan owning one exact Model, Mesh, and effective policy set.
#[pyclass(name = "Plan", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug)]
pub(crate) struct PyPlan {
    native: CommonPlanKind,
    model: Py<PyModel>,
    mesh: Option<Py<PyMesh>>,
    spatial: Option<SpatialHandle>,
    solve: Option<SolveHandle>,
    temporal: Option<TemporalHandle>,
}

impl PyPlan {
    pub(crate) fn model_handle(&self, py: Python<'_>) -> Py<PyModel> {
        self.model.clone_ref(py)
    }

    pub(crate) fn mesh_handle(&self, py: Python<'_>) -> Py<PyMesh> {
        self.mesh
            .as_ref()
            .expect("spatial common Plan owns an exact Mesh")
            .clone_ref(py)
    }

    pub(crate) fn ode_native(&self) -> Option<&CommonOdePlan> {
        match &self.native {
            CommonPlanKind::Ode(plan) => Some(plan),
            _ => None,
        }
    }

    pub(crate) fn transient_native(&self) -> Option<&CommonTransientFlowPlan> {
        match &self.native {
            CommonPlanKind::TransientFlow(plan) => Some(plan),
            CommonPlanKind::Ode(_)
            | CommonPlanKind::Scalar(_)
            | CommonPlanKind::Elasticity(_)
            | CommonPlanKind::SteadyStokes(_)
            | CommonPlanKind::Fsi(_) => None,
        }
    }

    pub(crate) fn fsi_native(&self) -> Option<&CommonFsiPlan> {
        match &self.native {
            CommonPlanKind::Fsi(plan) => Some(plan),
            _ => None,
        }
    }

    pub(crate) fn scalar_native(&self) -> Option<&CommonScalarPlan> {
        match &self.native {
            CommonPlanKind::Scalar(plan) => Some(plan),
            CommonPlanKind::Ode(_)
            | CommonPlanKind::Elasticity(_)
            | CommonPlanKind::SteadyStokes(_)
            | CommonPlanKind::TransientFlow(_)
            | CommonPlanKind::Fsi(_) => None,
        }
    }

    pub(crate) fn native(&self) -> &CommonPlanKind {
        &self.native
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
    fn geometry_digest(&self) -> Option<&str> {
        self.native.geometry_digest()
    }
    #[getter]
    fn mesh_digest(&self) -> Option<&str> {
        self.native.mesh_digest()
    }
    #[getter]
    fn correspondence_digest(&self) -> Option<&str> {
        self.native.correspondence_digest()
    }
    #[getter]
    fn production_digest(&self) -> Option<&str> {
        self.native.production_digest()
    }
    #[getter]
    fn realization_digest(&self) -> Option<&str> {
        self.native.realization_digest()
    }
    #[getter]
    fn model(&self, py: Python<'_>) -> Py<PyModel> {
        self.model.clone_ref(py)
    }
    #[getter]
    fn mesh(&self, py: Python<'_>) -> Option<Py<PyMesh>> {
        self.mesh.as_ref().map(|mesh| mesh.clone_ref(py))
    }
    #[getter]
    fn field(&self) -> Option<PyModelFieldRef> {
        match &self.native {
            CommonPlanKind::Scalar(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.field_id().to_owned(),
            )),
            CommonPlanKind::Elasticity(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.displacement_field_id().to_owned(),
            )),
            CommonPlanKind::Ode(_)
            | CommonPlanKind::SteadyStokes(_)
            | CommonPlanKind::TransientFlow(_)
            | CommonPlanKind::Fsi(_) => None,
        }
    }
    #[getter]
    fn fields(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let model_digest = self.native.model_digest().to_owned();
        let fields = match &self.native {
            CommonPlanKind::Ode(plan) => plan
                .field_ids()
                .map(|field| PyModelFieldRef::from_exact(model_digest.clone(), field.to_string()))
                .collect(),
            CommonPlanKind::Scalar(plan) => vec![PyModelFieldRef::from_exact(
                model_digest,
                plan.field_id().to_owned(),
            )],
            CommonPlanKind::Elasticity(plan) => vec![PyModelFieldRef::from_exact(
                model_digest,
                plan.displacement_field_id().to_owned(),
            )],
            CommonPlanKind::SteadyStokes(plan) => vec![
                PyModelFieldRef::from_exact(
                    model_digest.clone(),
                    plan.velocity_field_id().to_owned(),
                ),
                PyModelFieldRef::from_exact(model_digest, plan.pressure_field_id().to_owned()),
            ],
            CommonPlanKind::TransientFlow(plan) => vec![
                PyModelFieldRef::from_exact(
                    model_digest.clone(),
                    plan.velocity_field_id().to_owned(),
                ),
                PyModelFieldRef::from_exact(model_digest, plan.pressure_field_id().to_owned()),
            ],
            CommonPlanKind::Fsi(plan) => plan
                .field_ids()
                .iter()
                .map(|field| PyModelFieldRef::from_exact(model_digest.clone(), field.clone()))
                .collect(),
        };
        Ok(PyTuple::new(py, fields)?.unbind())
    }
    #[getter]
    fn velocity_field(&self) -> Option<PyModelFieldRef> {
        match &self.native {
            CommonPlanKind::SteadyStokes(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.velocity_field_id().to_owned(),
            )),
            CommonPlanKind::TransientFlow(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.velocity_field_id().to_owned(),
            )),
            CommonPlanKind::Fsi(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.field_ids()[0].clone(),
            )),
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                None
            }
        }
    }
    #[getter]
    fn pressure_field(&self) -> Option<PyModelFieldRef> {
        match &self.native {
            CommonPlanKind::SteadyStokes(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.pressure_field_id().to_owned(),
            )),
            CommonPlanKind::TransientFlow(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.pressure_field_id().to_owned(),
            )),
            CommonPlanKind::Fsi(plan) => Some(PyModelFieldRef::from_exact(
                plan.model_digest().to_owned(),
                plan.field_ids()[1].clone(),
            )),
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                None
            }
        }
    }
    #[getter]
    fn spatial(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.spatial
            .as_ref()
            .map(|spatial| match spatial {
                SpatialHandle::Uniform(SpatialPolicy::Q1) => Py::new(py, PyQ1).map(Py::into_any),
                SpatialHandle::Uniform(SpatialPolicy::CellCenteredTpfa) => {
                    Py::new(py, PyCellCenteredTpfa).map(Py::into_any)
                }
                SpatialHandle::Uniform(SpatialPolicy::MiniP1) => {
                    Py::new(py, PyMiniP1).map(Py::into_any)
                }
                SpatialHandle::Uniform(SpatialPolicy::CellCentered) => {
                    Py::new(py, PyCellCentered).map(Py::into_any)
                }
                SpatialHandle::Scoped(values) => {
                    PyTuple::new(py, values.iter().map(|value| value.clone_ref(py)))
                        .map(|value| value.unbind().into_any())
                }
            })
            .transpose()
    }
    #[getter]
    fn solve(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.solve.as_ref().map(|solve| match solve {
            SolveHandle::Linear(value) => value.clone_ref(py).into_any(),
            SolveHandle::Newton(value) => value.clone_ref(py).into_any(),
        })
    }
    #[getter]
    fn temporal(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.temporal.as_ref().map(|value| match value {
            TemporalHandle::BackwardEuler(value) => value.clone_ref(py).into_any(),
            TemporalHandle::Tsitouras45(value) => value.clone_ref(py).into_any(),
        })
    }
    #[getter]
    fn discretization(&self) -> Option<&'static str> {
        self.spatial.as_ref().map(|spatial| match spatial {
            SpatialHandle::Uniform(value) => value.discretization(),
            SpatialHandle::Scoped(_) => "mini-p1@fluid+p1@solid",
        })
    }
    #[getter]
    fn space(&self) -> Option<&'static str> {
        match self.native {
            CommonPlanKind::Ode(_) | CommonPlanKind::TransientFlow(_) | CommonPlanKind::Fsi(_) => {
                None
            }
            CommonPlanKind::Scalar(_)
            | CommonPlanKind::Elasticity(_)
            | CommonPlanKind::SteadyStokes(_) => {
                self.spatial.as_ref().and_then(|spatial| match spatial {
                    SpatialHandle::Uniform(value) => Some(value.space()),
                    SpatialHandle::Scoped(_) => None,
                })
            }
        }
    }
    #[getter]
    fn velocity_space(&self) -> Option<&'static str> {
        match &self.native {
            CommonPlanKind::SteadyStokes(plan) => Some(space_name(plan.velocity_space())),
            CommonPlanKind::TransientFlow(plan) => Some(space_name(plan.velocity_space())),
            CommonPlanKind::Fsi(_) => Some("shared-vertex-p1 + fluid-cell-bubble"),
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                None
            }
        }
    }
    #[getter]
    fn pressure_space(&self) -> Option<&'static str> {
        match &self.native {
            CommonPlanKind::SteadyStokes(plan) => Some(space_name(plan.pressure_space())),
            CommonPlanKind::TransientFlow(plan) => Some(space_name(plan.pressure_space())),
            CommonPlanKind::Fsi(_) => Some("continuous-lagrange-p1@fluid"),
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                None
            }
        }
    }
    #[getter]
    fn pressure_gauge(&self) -> Option<PyPressureGauge2d> {
        match &self.native {
            CommonPlanKind::TransientFlow(plan) => Some(plan.gauge().into()),
            CommonPlanKind::Ode(_)
            | CommonPlanKind::Scalar(_)
            | CommonPlanKind::Elasticity(_)
            | CommonPlanKind::SteadyStokes(_) => None,
            CommonPlanKind::Fsi(_) => None,
        }
    }
    #[getter]
    fn quadrature(&self) -> Option<&'static str> {
        self.spatial.as_ref().map(|spatial| match spatial {
            SpatialHandle::Uniform(value) => value.quadrature(),
            SpatialHandle::Scoped(_) => "triangle-duffy-gauss-legendre-4-per-axis",
        })
    }
    #[getter]
    fn mesh_kind(&self) -> Option<&'static str> {
        match &self.native {
            CommonPlanKind::Ode(_) => None,
            CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                Some("structured-cartesian")
            }
            CommonPlanKind::SteadyStokes(_) => Some("imported-affine-simplicial"),
            CommonPlanKind::TransientFlow(plan) => match plan.spatial() {
                CommonSpatialPolicy::MiniP1 => Some("imported-affine-simplicial"),
                CommonSpatialPolicy::CellCentered => Some("supplied-cartesian"),
                _ => unreachable!("closed transient spatial policy"),
            },
            CommonPlanKind::Fsi(_) => Some("imported-adjacent-partition-affine-simplicial"),
        }
    }
    #[getter]
    fn spatial_dimension(&self) -> Option<usize> {
        self.spatial.as_ref().map(|_| 2)
    }
    #[getter]
    fn cells(&self) -> Option<(usize, usize)> {
        match &self.native {
            CommonPlanKind::Scalar(plan) => {
                let [x, y] = plan.cells();
                Some((x, y))
            }
            CommonPlanKind::Elasticity(plan) => {
                let [x, y] = plan.cells();
                Some((x, y))
            }
            CommonPlanKind::Ode(_) | CommonPlanKind::SteadyStokes(_) => None,
            CommonPlanKind::TransientFlow(_) => None,
            CommonPlanKind::Fsi(_) => None,
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
    fn operator_properties(&self) -> Option<&'static str> {
        match &self.native {
            CommonPlanKind::Ode(_) => None,
            CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                Some("symmetric-positive-definite")
            }
            CommonPlanKind::SteadyStokes(_) => Some("symmetric-indefinite"),
            CommonPlanKind::TransientFlow(_) => Some("general"),
            CommonPlanKind::Fsi(_) => Some("symmetric-indefinite"),
        }
    }
    #[getter]
    const fn schedule(&self) -> &'static str {
        "offline"
    }
    #[getter]
    fn solver_algorithm(&self) -> Option<&'static str> {
        self.native
            .effective_solver()
            .map(|solver| match solver.algorithm() {
                LinearSolver::ConjugateGradient => "conjugate-gradient",
                LinearSolver::MinimumResidual => "minimum-residual",
                LinearSolver::BiConjugateGradientStabilized => "bicgstab",
                LinearSolver::SparseLu => "sparse-lu",
            })
    }
    #[getter]
    fn preconditioner(&self) -> Option<&'static str> {
        match self.native {
            CommonPlanKind::Ode(_) => None,
            _ => Some("identity"),
        }
    }
    #[getter]
    fn reduction(&self) -> Option<&'static str> {
        self.native
            .effective_solver()
            .map(|solver| match solver.reduction() {
                ReductionPolicy::Reproducible => "reproducible",
                ReductionPolicy::Fast => "fast",
            })
    }
    #[getter]
    fn solver_backend(&self) -> &'static str {
        match &self.native {
            CommonPlanKind::Ode(plan) => plan.backend().id().as_str(),
            CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                REFERENCE_SOLVER_PROVIDER.id().as_str()
            }
            CommonPlanKind::SteadyStokes(_) => FAER_SOLVER_PROVIDER.id().as_str(),
            CommonPlanKind::TransientFlow(plan) => match plan.spatial() {
                CommonSpatialPolicy::MiniP1 => FAER_SOLVER_PROVIDER.id().as_str(),
                CommonSpatialPolicy::CellCentered => REFERENCE_SOLVER_PROVIDER.id().as_str(),
                _ => unreachable!("closed transient spatial policy"),
            },
            CommonPlanKind::Fsi(_) => REFERENCE_SOLVER_PROVIDER.id().as_str(),
        }
    }
    #[getter]
    fn solver_backend_version(&self) -> &'static str {
        match &self.native {
            CommonPlanKind::Ode(plan) => plan.backend().version().as_str(),
            CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                REFERENCE_SOLVER_PROVIDER.implementation_version()
            }
            CommonPlanKind::SteadyStokes(_) => FAER_SOLVER_PROVIDER.implementation_version(),
            CommonPlanKind::TransientFlow(plan) => match plan.spatial() {
                CommonSpatialPolicy::MiniP1 => FAER_SOLVER_PROVIDER.implementation_version(),
                CommonSpatialPolicy::CellCentered => {
                    REFERENCE_SOLVER_PROVIDER.implementation_version()
                }
                _ => unreachable!("closed transient spatial policy"),
            },
            CommonPlanKind::Fsi(_) => REFERENCE_SOLVER_PROVIDER.implementation_version(),
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
    fn scaling(&self, py: Python<'_>) -> PyResult<Option<Py<PyIncompressibleScales>>> {
        match &self.native {
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                Ok(None)
            }
            CommonPlanKind::SteadyStokes(plan) => {
                Py::new(py, PyIncompressibleScales::from_native(plan.scales())).map(Some)
            }
            CommonPlanKind::TransientFlow(plan) => {
                Py::new(py, PyIncompressibleScales::from_native(plan.scales())).map(Some)
            }
            CommonPlanKind::Fsi(plan) => {
                Py::new(py, PyIncompressibleScales::from_fsi(plan.scaling())).map(Some)
            }
        }
    }
    #[getter]
    fn scaling_receipt(
        &self,
        py: Python<'_>,
    ) -> PyResult<Option<Py<PyIncompressibleScalingReceipt2d>>> {
        match &self.native {
            CommonPlanKind::Ode(_) | CommonPlanKind::Scalar(_) | CommonPlanKind::Elasticity(_) => {
                Ok(None)
            }
            CommonPlanKind::SteadyStokes(plan) => Py::new(
                py,
                PyIncompressibleScalingReceipt2d::from_native(plan.scaling_receipt().clone()),
            )
            .map(Some),
            CommonPlanKind::TransientFlow(plan) => Py::new(
                py,
                PyIncompressibleScalingReceipt2d::from_native(plan.scaling_receipt().clone()),
            )
            .map(Some),
            CommonPlanKind::Fsi(plan) => Py::new(
                py,
                PyIncompressibleScalingReceipt2d::from_native(plan.scaling_receipt().clone()),
            )
            .map(Some),
        }
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
#[pyo3(signature = (model, /, *, mesh=None, spatial=None, solve=None, scaling=None, temporal=None))]
fn resolve_plan(
    py: Python<'_>,
    model: Py<PyModel>,
    mesh: Option<Py<PyMesh>>,
    spatial: Option<&Bound<'_, PyAny>>,
    solve: Option<&Bound<'_, PyAny>>,
    scaling: Option<&Bound<'_, PyAny>>,
    temporal: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPlan> {
    let ode_temporal = temporal.and_then(|value| value.extract::<Py<PyTsitouras45>>().ok());
    if let Some(temporal_handle) = ode_temporal {
        if mesh.is_some()
            || spatial.is_some()
            || solve.is_some()
            || scaling.is_some_and(|value| !value.is_none())
        {
            return Err(PyTypeError::new_err(
                "no-Mesh explicit ODE resolve accepts only model and temporal=eqiora.time.Tsitouras45(...)",
            ));
        }
        let model_ref = model.borrow(py);
        let artifact = model_ref.artifact();
        let reference = artifact
            .artifact_reference()
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        let temporal_ref = temporal_handle.borrow(py);
        if !temporal_ref.belongs_to_model(py, &reference.artifact().to_string()) {
            return Err(PyTypeError::new_err(
                "Tsitouras45 absolute_tolerances must use exact FieldRefs from this Model",
            ));
        }
        let program = artifact
            .to_program()
            .map_err(|diagnostics| validation_error(py, &diagnostics))?;
        let native = resolve_common_ode_plan(
            artifact,
            &program,
            temporal_ref.native.clone(),
            eqiora::backends::diffsol::DIFFSOL_TIME_BACKEND,
        )
        .map(|plan| {
            plan.project(
                |plan| CommonPlanKind::Ode(Box::new(plan)),
                |plan| CommonPlanKind::Scalar(Box::new(plan)),
                |plan| CommonPlanKind::Elasticity(Box::new(plan)),
                |plan| CommonPlanKind::SteadyStokes(Box::new(plan)),
                |plan| CommonPlanKind::TransientFlow(Box::new(plan)),
                |plan| CommonPlanKind::Fsi(Box::new(plan)),
            )
        })
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        drop(temporal_ref);
        drop(model_ref);
        return Ok(PyPlan {
            native,
            model,
            mesh: None,
            spatial: None,
            solve: None,
            temporal: Some(TemporalHandle::Tsitouras45(temporal_handle)),
        });
    }

    let mesh = mesh.ok_or_else(|| PyTypeError::new_err("spatial resolve requires mesh=Mesh"))?;
    let spatial_value =
        spatial.ok_or_else(|| PyTypeError::new_err("spatial resolve requires spatial policy"))?;
    let solve =
        solve.ok_or_else(|| PyTypeError::new_err("spatial resolve requires solve policy"))?;
    let (spatial_request, spatial_handle) = if spatial_value.extract::<PyRef<'_, PyQ1>>().is_ok() {
        (
            CommonSpatialRequest::Uniform(CommonSpatialPolicy::Q1),
            SpatialHandle::Uniform(SpatialPolicy::Q1),
        )
    } else if spatial_value
        .extract::<PyRef<'_, PyCellCenteredTpfa>>()
        .is_ok()
    {
        (
            CommonSpatialRequest::Uniform(CommonSpatialPolicy::CellCenteredTpfa),
            SpatialHandle::Uniform(SpatialPolicy::CellCenteredTpfa),
        )
    } else if spatial_value.extract::<PyRef<'_, PyMiniP1>>().is_ok() {
        (
            CommonSpatialRequest::Uniform(CommonSpatialPolicy::MiniP1),
            SpatialHandle::Uniform(SpatialPolicy::MiniP1),
        )
    } else if spatial_value.extract::<PyRef<'_, PyCellCentered>>().is_ok() {
        (
            CommonSpatialRequest::Uniform(CommonSpatialPolicy::CellCentered),
            SpatialHandle::Uniform(SpatialPolicy::CellCentered),
        )
    } else if let Ok(tuple) = spatial_value.cast::<PyTuple>() {
        if tuple.is_empty() {
            return Err(PyTypeError::new_err(
                "scoped spatial policy tuple must be nonempty",
            ));
        }
        let mut native = Vec::with_capacity(tuple.len());
        let mut handles = Vec::with_capacity(tuple.len());
        for value in tuple.iter() {
            let handle = value.extract::<Py<PyScopedSpatialBinding>>().map_err(|_| {
                PyTypeError::new_err("scoped spatial tuples must contain only MiniP1.at(DomainRef) or P1.at(DomainRef)")
            })?;
            let binding = handle.borrow(py);
            let model_digest = eqiora::artifact::ArtifactDigest::from_hex(
                binding.domain.exact_model_digest().to_owned(),
            )
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
            let domain = ulid::Ulid::from_string(binding.domain.exact_id())
                .map(eqiora::Id::<eqiora::kinds::Domain>::from_ulid)
                .map_err(|_| {
                    PyTypeError::new_err("DomainRef contains an invalid exact Domain ULID")
                })?;
            let policy = match binding.policy {
                ScopedSpatialKind::MiniP1 => CommonSpatialPolicy::MiniP1,
                ScopedSpatialKind::P1 => CommonSpatialPolicy::P1,
            };
            native.push(CommonScopedSpatialPolicy::new(model_digest, domain, policy));
            drop(binding);
            handles.push(handle);
        }
        (
            CommonSpatialRequest::Scoped(native),
            SpatialHandle::Scoped(handles),
        )
    } else {
        return Err(PyTypeError::new_err(
            "spatial must be a supported uniform policy or an exact tuple of Domain-scoped policies",
        ));
    };
    let scaling = match scaling {
        None => None,
        Some(value) if value.is_none() => None,
        Some(value) => Some(
            value
                .extract::<PyRef<'_, PyIncompressibleScaling>>()
                .map_err(|_| {
                    PyTypeError::new_err(
                        "scaling must be eqiora.fluid.IncompressibleScaling or None",
                    )
                })?
                .native(),
        ),
    };
    let (solve_native, requested_solve_handle) = if let Ok(linear) = solve.extract::<Py<PyLinear>>()
    {
        let native = linear.borrow(py).native;
        (
            CommonSolvePolicy::Linear(native),
            SolveHandle::Linear(linear),
        )
    } else if let Ok(newton) = solve.extract::<Py<PyNewton>>() {
        let newton_ref = newton.borrow(py);
        let linear = newton_ref.linear.clone_ref(py);
        let native = CommonSolvePolicy::newton(linear.borrow(py).native, newton_ref.native);
        drop(newton_ref);
        (native, SolveHandle::Newton(newton))
    } else {
        return Err(PyTypeError::new_err(
            "solve must be eqiora.solve.Linear or eqiora.solve.Newton",
        ));
    };
    let (temporal_native, temporal_handle) = match temporal {
        None => (None, None),
        Some(value) if value.is_none() => (None, None),
        Some(value) => {
            let handle = value.extract::<Py<PyBackwardEuler>>().map_err(|_| {
                PyTypeError::new_err("temporal must be eqiora.time.BackwardEuler or None")
            })?;
            let native = handle.borrow(py).native;
            (Some(native), Some(TemporalHandle::BackwardEuler(handle)))
        }
    };
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
        spatial_request,
        solve_native,
        scaling,
        temporal_native,
        &FaerLinearSolver,
    )
    .map(|plan| {
        plan.project(
            |plan| CommonPlanKind::Ode(Box::new(plan)),
            |plan| CommonPlanKind::Scalar(Box::new(plan)),
            |plan| CommonPlanKind::Elasticity(Box::new(plan)),
            |plan| CommonPlanKind::SteadyStokes(Box::new(plan)),
            |plan| CommonPlanKind::TransientFlow(Box::new(plan)),
            |plan| CommonPlanKind::Fsi(Box::new(plan)),
        )
    })
    .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
    if native.mesh_digest() != Some(mesh_ref.exact_mesh_digest()) {
        return Err(PyTypeError::new_err(
            "resolved Plan did not retain the exact caller Mesh occurrence",
        ));
    }
    let solve_handle = match &native {
        CommonPlanKind::TransientFlow(plan) => {
            let linear = Py::new(
                py,
                PyLinear {
                    native: plan.linear(),
                },
            )?;
            SolveHandle::Newton(Py::new(
                py,
                PyNewton::from_native(linear, plan.nonlinear()),
            )?)
        }
        CommonPlanKind::Fsi(plan) => SolveHandle::Linear(Py::new(
            py,
            PyLinear {
                native: plan.linear(),
            },
        )?),
        CommonPlanKind::Scalar(_)
        | CommonPlanKind::Ode(_)
        | CommonPlanKind::Elasticity(_)
        | CommonPlanKind::SteadyStokes(_) => requested_solve_handle,
    };
    drop(mesh_ref);
    drop(model_ref);
    Ok(PyPlan {
        native,
        model,
        mesh: Some(mesh),
        spatial: Some(spatial_handle),
        solve: Some(solve_handle),
        temporal: temporal_handle,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyQ1>()?;
    module.add_class::<PyMiniP1>()?;
    module.add_class::<PyP1>()?;
    module.add_class::<PyScopedSpatialBinding>()?;
    module.add_class::<PyCellCenteredTpfa>()?;
    module.add_class::<PyCellCentered>()?;
    module.add_class::<PyLinear>()?;
    module.add_class::<PyNewton>()?;
    module.add_class::<PyPressureGauge2d>()?;
    module.add_class::<PyBackwardEuler>()?;
    module.add_class::<PyTsitouras45>()?;
    module.add_class::<PyPlan>()?;
    scaling::register(module)?;
    module.add_function(wrap_pyfunction!(resolve_plan, module)?)?;
    Ok(())
}

#[cfg(test)]
#[path = "common_plan/tests.rs"]
mod tests;
