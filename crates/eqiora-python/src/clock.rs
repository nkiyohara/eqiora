//! Exact nominal static clock contracts at the Python boundary.

use eqiora::kernel::{ClockDomainDef, ClockKind, RationalTime};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyInt};

#[pyclass(
    name = "ClockDomain",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PyClockDomain {
    pub(crate) value: ClockDomainDef,
}

fn exact_seconds(value: &Bound<'_, PyAny>) -> PyResult<RationalTime> {
    let fraction = value.py().import("fractions")?.getattr("Fraction")?;
    if value.is_instance_of::<PyBool>()
        || !(value.is_instance_of::<PyInt>() || value.is_instance(&fraction)?)
    {
        return Err(PyTypeError::new_err(
            "clock seconds must be Fraction or int, not float or bool",
        ));
    }
    let exact = fraction.call1((value,))?;
    let numerator: u64 = exact.getattr("numerator")?.extract()?;
    let denominator: u64 = exact.getattr("denominator")?.extract()?;
    RationalTime::new(numerator, denominator)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pymethods]
impl PyClockDomain {
    #[new]
    #[pyo3(signature = (*, period_s, phase_s=RationalTime::ZERO), text_signature = "(*, period_s, phase_s=0)")]
    fn new(
        #[pyo3(from_py_with = exact_seconds)] period_s: RationalTime,
        #[pyo3(from_py_with = exact_seconds)] phase_s: RationalTime,
    ) -> PyResult<Self> {
        Ok(Self {
            value: ClockDomainDef::periodic(eqiora::Id::new(), period_s, phase_s)
                .map_err(|error| PyValueError::new_err(error.to_string()))?,
        })
    }

    #[getter]
    fn id(&self) -> String {
        self.value.id().to_string()
    }

    #[getter]
    fn period_s(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ClockKind::Periodic { period, .. } = self.value.kind() else {
            unreachable!()
        };
        Ok(py
            .import("fractions")?
            .getattr("Fraction")?
            .call1((period.numerator(), period.denominator()))?
            .unbind())
    }

    #[getter]
    fn phase_s(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let ClockKind::Periodic { phase, .. } = self.value.kind() else {
            unreachable!()
        };
        Ok(py
            .import("fractions")?
            .getattr("Fraction")?
            .call1((phase.numerator(), phase.denominator()))?
            .unbind())
    }
}
