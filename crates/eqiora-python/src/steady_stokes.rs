//! Python observation projection for common steady Stokes.

use eqiora_numerics::CommonSteadyStokesObservation;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::realization::PyLinearSolveSummary;
use crate::result::PyRunResult;

#[pyclass(
    name = "SteadyStokesEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PySteadyStokesEvidence {
    plan_key: String,
    pressure_minimum: f64,
    pressure_maximum: f64,
    exact_bounds: ((f64, f64), (f64, f64)),
    cylinder_force_on_fluid: (f64, f64),
    inlet_flux: f64,
    outlet_flux: f64,
    net_flux: f64,
    constrained_reaction: (f64, f64),
    integrated_body_force: (f64, f64),
    integrated_boundary_traction: (f64, f64),
    momentum_closure: (f64, f64),
    solve: Py<PyLinearSolveSummary>,
    continuity_residual_norm: f64,
}

impl PySteadyStokesEvidence {
    pub(crate) fn from_common(
        py: Python<'_>,
        plan_key: &str,
        observation: &CommonSteadyStokesObservation,
    ) -> PyResult<Self> {
        let [[x_lower, x_upper], [y_lower, y_upper]] = observation.exact_bounds();
        Ok(Self {
            plan_key: plan_key.to_owned(),
            pressure_minimum: observation.pressure_minimum(),
            pressure_maximum: observation.pressure_maximum(),
            exact_bounds: ((x_lower, x_upper), (y_lower, y_upper)),
            cylinder_force_on_fluid: tuple2(observation.cylinder_force_on_fluid()),
            inlet_flux: observation.inlet_flux(),
            outlet_flux: observation.outlet_flux(),
            net_flux: observation.net_flux(),
            constrained_reaction: tuple2(observation.constrained_reaction()),
            integrated_body_force: tuple2(observation.integrated_body_force()),
            integrated_boundary_traction: tuple2(observation.integrated_boundary_traction()),
            momentum_closure: tuple2(observation.momentum_closure()),
            solve: Py::new(py, PyLinearSolveSummary::from_report(observation.solve()))?,
            continuity_residual_norm: observation.continuity_residual_norm(),
        })
    }
}

#[pymethods]
impl PySteadyStokesEvidence {
    #[getter]
    fn plan_key(&self) -> &str {
        &self.plan_key
    }
    #[getter]
    const fn pressure_minimum(&self) -> f64 {
        self.pressure_minimum
    }
    #[getter]
    const fn pressure_maximum(&self) -> f64 {
        self.pressure_maximum
    }
    #[getter]
    const fn exact_bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.exact_bounds
    }
    #[getter]
    const fn cylinder_force_on_fluid(&self) -> (f64, f64) {
        self.cylinder_force_on_fluid
    }
    #[getter]
    const fn inlet_flux(&self) -> f64 {
        self.inlet_flux
    }
    #[getter]
    const fn outlet_flux(&self) -> f64 {
        self.outlet_flux
    }
    #[getter]
    const fn net_flux(&self) -> f64 {
        self.net_flux
    }
    #[getter]
    const fn constrained_reaction(&self) -> (f64, f64) {
        self.constrained_reaction
    }
    #[getter]
    const fn integrated_body_force(&self) -> (f64, f64) {
        self.integrated_body_force
    }
    #[getter]
    const fn integrated_boundary_traction(&self) -> (f64, f64) {
        self.integrated_boundary_traction
    }
    #[getter]
    const fn momentum_closure(&self) -> (f64, f64) {
        self.momentum_closure
    }
    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinearSolveSummary> {
        self.solve.clone_ref(py)
    }
    #[getter]
    const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }
    fn __repr__(&self) -> String {
        format!("SteadyStokesEvidence(plan_key={:?})", self.plan_key)
    }
}

#[pyfunction]
#[pyo3(signature = (result, /))]
fn steady_stokes_evidence(
    py: Python<'_>,
    result: &PyRunResult,
) -> PyResult<Py<PySteadyStokesEvidence>> {
    result.steady_stokes_evidence(py)
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySteadyStokesEvidence>()?;
    module.add_function(wrap_pyfunction!(steady_stokes_evidence, module)?)?;
    Ok(())
}
