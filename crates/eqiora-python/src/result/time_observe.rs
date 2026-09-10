//! Typed time functionals delegate to the accepted native integration history.
use super::*;
use crate::model::PyObservableRef;
use crate::modeling::PyValueType;
use eqiora::ValueLiteral;
use eqiora_numerics::TimeFunctionalQuadrature;

#[pyclass(
    name = "TimeFunctionalQuadrature",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyTimeFunctionalQuadrature {
    AcceptedStepSimpson,
}

impl From<PyTimeFunctionalQuadrature> for TimeFunctionalQuadrature {
    fn from(value: PyTimeFunctionalQuadrature) -> Self {
        match value {
            PyTimeFunctionalQuadrature::AcceptedStepSimpson => Self::AcceptedStepSimpson,
        }
    }
}

#[pyclass(name = "TrajectoryObservation", module = "eqiora._eqiora", frozen)]
pub(crate) struct PyTrajectoryObservation {
    value: ValueLiteral,
    #[pyo3(get)]
    result_identity: String,
    #[pyo3(get)]
    trajectory_identity: String,
    #[pyo3(get)]
    observable_id: String,
    #[pyo3(get)]
    evaluation_kind: &'static str,
    #[pyo3(get)]
    quadrature: Option<PyTimeFunctionalQuadrature>,
    #[pyo3(get)]
    interval_s: (f64, f64),
    #[pyo3(get)]
    endpoint_convention: &'static str,
}

#[pymethods]
impl PyTrajectoryObservation {
    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        crate::modeling::value_literal::to_python(py, &self.value)
    }
    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type().clone(),
        }
    }
}

impl PyRunResult {
    pub(super) fn observe_trajectory(
        &self,
        py: Python<'_>,
        observable: &PyObservableRef,
        quadrature: Option<PyTimeFunctionalQuadrature>,
    ) -> PyResult<PyTrajectoryObservation> {
        if observable.model_digest != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "ObservableRef belongs to a different exact Model artifact",
            ));
        }
        let trajectory = self.native.trajectory().ok_or_else(|| {
            PyValueError::new_err("time observation requires an accepted Trajectory")
        })?;
        let model = self.native.plan().model_artifact();
        let value = match quadrature {
            None => trajectory.observe_terminal(model, observable.id),
            Some(quadrature) => {
                trajectory.observe_time_integral(model, observable.id, quadrature.into())
            }
        }
        .map_err(|error| diagnostic_error(py, &[error]))?;
        let quadrature = value.quadrature().map(|rule| match rule {
            TimeFunctionalQuadrature::AcceptedStepSimpson => {
                PyTimeFunctionalQuadrature::AcceptedStepSimpson
            }
        });
        Ok(PyTrajectoryObservation {
            value: value.value().clone(),
            result_identity: self.native.identity().to_owned(),
            trajectory_identity: value.trajectory_identity().to_owned(),
            observable_id: value.observable().ulid().to_string(),
            evaluation_kind: if quadrature.is_some() {
                "time-integral"
            } else {
                "terminal"
            },
            quadrature,
            interval_s: (value.interval_s()[0], value.interval_s()[1]),
            endpoint_convention: value.endpoint_convention(),
        })
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyTimeFunctionalQuadrature>()?;
    module.add_class::<PyTrajectoryObservation>()?;
    Ok(())
}
