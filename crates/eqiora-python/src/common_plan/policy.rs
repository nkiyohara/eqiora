//! Closed Python numerical-policy requests consumed by the root resolver.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::solver::{LinearSolver, SolverPlan};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use crate::error::validation_error;

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

/// Mixed MINI velocity and continuous P1 pressure spatial policy.
#[pyclass(
    name = "MiniP1",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyMiniP1;

#[pymethods]
impl PyMiniP1 {
    #[new]
    const fn new() -> Self {
        Self
    }

    fn __repr__(&self) -> &'static str {
        "MiniP1()"
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

/// Closed linear-solve controls resolved against Model-owned operator meaning.
#[pyclass(
    name = "Linear",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PyLinear {
    pub(super) native: SolverPlan,
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
