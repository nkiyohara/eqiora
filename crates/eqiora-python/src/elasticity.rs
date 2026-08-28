//! Python observation projection for common linear elasticity.

use eqiora_numerics::CommonElasticityObservation;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::realization::PyLinearSolveSummary;
use crate::result::PyRunResult;

#[pyclass(
    name = "LinearElasticityEvidence",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyLinearElasticityEvidence {
    plan_key: String,
    constrained_reaction: (f64, f64),
    integrated_body_force: (f64, f64),
    assembly_packets: usize,
    assembly_targets: usize,
    solve: Py<PyLinearSolveSummary>,
    exact_bounds: ((f64, f64), (f64, f64)),
}

impl PyLinearElasticityEvidence {
    pub(crate) fn from_common(
        py: Python<'_>,
        plan_key: &str,
        observation: &CommonElasticityObservation,
    ) -> PyResult<Self> {
        let [[x_lower, x_upper], [y_lower, y_upper]] = observation.exact_bounds();
        Ok(Self {
            plan_key: plan_key.to_owned(),
            constrained_reaction: tuple2(observation.constrained_reaction()),
            integrated_body_force: tuple2(observation.integrated_body_force()),
            assembly_packets: observation.assembly_packets(),
            assembly_targets: observation.assembly_targets(),
            solve: Py::new(py, PyLinearSolveSummary::from_report(observation.solve()))?,
            exact_bounds: ((x_lower, x_upper), (y_lower, y_upper)),
        })
    }
}

#[pymethods]
impl PyLinearElasticityEvidence {
    #[getter]
    fn plan_key(&self) -> &str {
        &self.plan_key
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
    const fn assembly_packets(&self) -> usize {
        self.assembly_packets
    }
    #[getter]
    const fn assembly_targets(&self) -> usize {
        self.assembly_targets
    }
    #[getter]
    fn solve(&self, py: Python<'_>) -> Py<PyLinearSolveSummary> {
        self.solve.clone_ref(py)
    }
    #[getter]
    const fn exact_bounds(&self) -> ((f64, f64), (f64, f64)) {
        self.exact_bounds
    }
    fn __repr__(&self) -> String {
        format!("LinearElasticityEvidence(plan_key={:?})", self.plan_key)
    }
}

#[pyfunction]
#[pyo3(signature = (result, /))]
fn linear_elasticity_evidence(
    py: Python<'_>,
    result: &PyRunResult,
) -> PyResult<Py<PyLinearElasticityEvidence>> {
    result.linear_elasticity_evidence(py)
}

const fn tuple2(value: [f64; 2]) -> (f64, f64) {
    (value[0], value[1])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLinearElasticityEvidence>()?;
    module.add_function(wrap_pyfunction!(linear_elasticity_evidence, module)?)?;
    Ok(())
}
