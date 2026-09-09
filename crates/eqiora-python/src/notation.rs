//! Python declaration metadata delegates admission to the language AST owner.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[pyclass(name = "Notation", module = "eqiora.lang", frozen)]
pub(crate) struct PyNotation(pub(crate) eqiora::language::Notation);

#[pymethods]
impl PyNotation {
    #[new]
    fn new(island: &str) -> PyResult<Self> {
        eqiora::language::Notation::parse(island)
            .map(Self)
            .map_err(|error| {
                PyValueError::new_err(format!(
                    "{} at notation bytes {}..{}",
                    error.message(),
                    error.range().start(),
                    error.range().end()
                ))
            })
    }

    #[getter]
    fn canonical(&self) -> String {
        self.0.canonical()
    }

    fn __str__(&self) -> String {
        self.0.canonical()
    }
}
