//! Typed capability-specific views projected from one resolved root Plan.

use pyo3::prelude::*;

use eqiora_numerics::{CommonFormulationDescription, FormulationKind, FormulationSelectionMode};

use crate::model::PyModelFieldRef;

use super::policy::PyPressureGauge2d;
use super::scaling::{PyIncompressibleScales, PyIncompressibleScalingReceipt2d};

/// Inspectable effective mathematical Formulation selected before Realization.
///
/// Authority: `crates/eqiora-python/src/common_plan/capability_view.rs::PyFormulationView`.
#[pyclass(
    name = "FormulationView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyFormulationView {
    requested: &'static str,
    effective: &'static str,
    boundary_treatment: &'static str,
    rule_ids: Vec<&'static str>,
    selection_reason_codes: Vec<&'static str>,
}

impl PyFormulationView {
    pub(crate) fn from_native(description: CommonFormulationDescription) -> Self {
        let requested = match description.requested() {
            FormulationSelectionMode::Automatic => "automatic",
        };
        let effective = match description.effective() {
            FormulationKind::PrimalGalerkin => "primal-galerkin",
            FormulationKind::MixedGalerkin => "mixed-galerkin",
            FormulationKind::IntegralConservative => "integral-conservative",
        };
        Self {
            requested,
            effective,
            boundary_treatment: description.boundary_treatment(),
            rule_ids: description.rule_ids().to_vec(),
            selection_reason_codes: description.selection_reason_codes().to_vec(),
        }
    }
}

#[pymethods]
impl PyFormulationView {
    #[getter]
    const fn requested(&self) -> &'static str {
        self.requested
    }
    #[getter]
    const fn effective(&self) -> &'static str {
        self.effective
    }
    #[getter]
    const fn boundary_treatment(&self) -> &'static str {
        self.boundary_treatment
    }
    #[getter]
    fn rule_ids(&self) -> Vec<&'static str> {
        self.rule_ids.clone()
    }
    #[getter]
    fn selection_reason_codes(&self) -> Vec<&'static str> {
        self.selection_reason_codes.clone()
    }
    fn __repr__(&self) -> String {
        format!(
            "FormulationView(requested={:?}, effective={:?})",
            self.requested, self.effective
        )
    }
}

/// Resolved no-Mesh ODE capability.
#[pyclass(
    name = "OdePlanView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyOdePlanView {
    pub(super) backend: &'static str,
    pub(super) backend_version: &'static str,
}

#[pymethods]
impl PyOdePlanView {
    #[getter]
    const fn kind(&self) -> &'static str {
        "ode"
    }
    #[getter]
    const fn backend(&self) -> &'static str {
        self.backend
    }
    #[getter]
    const fn backend_version(&self) -> &'static str {
        self.backend_version
    }
    fn __repr__(&self) -> String {
        format!("OdePlanView(backend={:?})", self.backend)
    }
}

/// Scalar-elliptic field roles resolved from one Model.
#[pyclass(
    name = "ScalarPlanView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyScalarPlanView {
    pub(super) field: PyModelFieldRef,
}

#[pymethods]
impl PyScalarPlanView {
    #[getter]
    const fn kind(&self) -> &'static str {
        "scalar-elliptic"
    }

    #[getter]
    fn field(&self) -> PyModelFieldRef {
        self.field.clone()
    }
    fn __repr__(&self) -> String {
        format!("ScalarPlanView(field={:?})", self.field.exact_id())
    }
}

/// Resolved linear-elasticity field roles.
#[pyclass(
    name = "ElasticityPlanView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyElasticityPlanView {
    pub(super) displacement: PyModelFieldRef,
}

#[pymethods]
impl PyElasticityPlanView {
    #[getter]
    const fn kind(&self) -> &'static str {
        "linear-elasticity"
    }

    #[getter]
    fn displacement(&self) -> PyModelFieldRef {
        self.displacement.clone()
    }
    fn __repr__(&self) -> String {
        format!(
            "ElasticityPlanView(displacement={:?})",
            self.displacement.exact_id()
        )
    }
}

/// Resolved incompressible-flow roles, spaces, gauge, and scales.
#[pyclass(
    name = "IncompressibleFlowPlanView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyIncompressibleFlowPlanView {
    pub(super) kind: &'static str,
    pub(super) velocity: PyModelFieldRef,
    pub(super) pressure: PyModelFieldRef,
    pub(super) velocity_space: &'static str,
    pub(super) pressure_space: &'static str,
    pub(super) pressure_gauge: Option<PyPressureGauge2d>,
    pub(super) scaling: Py<PyIncompressibleScales>,
    pub(super) scaling_receipt: Py<PyIncompressibleScalingReceipt2d>,
}

#[pymethods]
impl PyIncompressibleFlowPlanView {
    #[getter]
    const fn kind(&self) -> &'static str {
        self.kind
    }
    #[getter]
    fn velocity(&self) -> PyModelFieldRef {
        self.velocity.clone()
    }
    #[getter]
    fn pressure(&self) -> PyModelFieldRef {
        self.pressure.clone()
    }
    #[getter]
    const fn velocity_space(&self) -> &'static str {
        self.velocity_space
    }
    #[getter]
    const fn pressure_space(&self) -> &'static str {
        self.pressure_space
    }
    #[getter]
    const fn pressure_gauge(&self) -> Option<PyPressureGauge2d> {
        self.pressure_gauge
    }
    #[getter]
    fn scaling(&self, py: Python<'_>) -> Py<PyIncompressibleScales> {
        self.scaling.clone_ref(py)
    }
    #[getter]
    fn scaling_receipt(&self, py: Python<'_>) -> Py<PyIncompressibleScalingReceipt2d> {
        self.scaling_receipt.clone_ref(py)
    }
    fn __repr__(&self) -> String {
        format!("IncompressibleFlowPlanView(kind={:?})", self.kind)
    }
}

/// Resolved field roles and scales for fixed-reference FSI.
#[pyclass(
    name = "FixedReferenceFsiPlanView",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyFixedReferenceFsiPlanView {
    pub(super) fluid_velocity: PyModelFieldRef,
    pub(super) pressure: PyModelFieldRef,
    pub(super) solid_velocity: PyModelFieldRef,
    pub(super) displacement: PyModelFieldRef,
    pub(super) scaling: Py<PyIncompressibleScales>,
    pub(super) scaling_receipt: Py<PyIncompressibleScalingReceipt2d>,
}

#[pymethods]
impl PyFixedReferenceFsiPlanView {
    #[getter]
    const fn kind(&self) -> &'static str {
        "fixed-reference-fsi"
    }
    #[getter]
    fn fluid_velocity(&self) -> PyModelFieldRef {
        self.fluid_velocity.clone()
    }
    #[getter]
    fn pressure(&self) -> PyModelFieldRef {
        self.pressure.clone()
    }
    #[getter]
    fn solid_velocity(&self) -> PyModelFieldRef {
        self.solid_velocity.clone()
    }
    #[getter]
    fn displacement(&self) -> PyModelFieldRef {
        self.displacement.clone()
    }
    #[getter]
    fn scaling(&self, py: Python<'_>) -> Py<PyIncompressibleScales> {
        self.scaling.clone_ref(py)
    }
    #[getter]
    fn scaling_receipt(&self, py: Python<'_>) -> Py<PyIncompressibleScalingReceipt2d> {
        self.scaling_receipt.clone_ref(py)
    }
    fn __repr__(&self) -> &'static str {
        "FixedReferenceFsiPlanView()"
    }
}
