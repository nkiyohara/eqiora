//! Typed inspection of the execution policy selected by root resolution.

use pyo3::prelude::*;

/// Exact scalar, layout, schedule, provider, and placement selected for execution.
#[pyclass(
    name = "ResolvedExecution",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyResolvedExecution;

#[pymethods]
impl PyResolvedExecution {
    #[getter]
    const fn scalar_type(&self) -> &'static str {
        "f64"
    }
    #[getter]
    const fn vector_layout(&self) -> &'static str {
        "replicated"
    }
    #[getter]
    const fn schedule(&self) -> &'static str {
        "offline"
    }
    #[getter]
    const fn provider(&self) -> &'static str {
        eqiora::solver::SERIAL_EXECUTION_PROVIDER.id().as_str()
    }
    #[getter]
    const fn provider_version(&self) -> &'static str {
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
    fn __repr__(&self) -> &'static str {
        "ResolvedExecution(provider='eqiora.host.serial', placement='host-serial', workers=1)"
    }
}
