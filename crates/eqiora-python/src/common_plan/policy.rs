//! Closed Python numerical-policy requests consumed by the root resolver.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::realization::NonlinearSolvePlan;
use eqiora::solver::{LinearSolver, SolverPlan};
use eqiora_numerics::{CommonBackwardEuler, CommonPressureGauge2d};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyFloat, PyInt};

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

/// Collocated cell-centred incompressible-flow spatial policy.
#[pyclass(
    name = "CellCentered",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PyCellCentered;

#[pymethods]
impl PyCellCentered {
    #[new]
    const fn new() -> Self {
        Self
    }

    fn __repr__(&self) -> &'static str {
        "CellCentered()"
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

/// Backward-Euler temporal operator policy.
#[pyclass(
    name = "BackwardEuler",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PyBackwardEuler {
    pub(super) native: CommonBackwardEuler,
}

#[pymethods]
impl PyBackwardEuler {
    #[new]
    #[pyo3(signature = (step_s))]
    fn new(py: Python<'_>, step_s: &Bound<'_, PyAny>) -> PyResult<Self> {
        let step_s = step_s
            .cast::<PyFloat>()
            .map_err(|_| PyTypeError::new_err("step_s must be a float"))?
            .value();
        CommonBackwardEuler::from_seconds(step_s)
            .map(|native| Self { native })
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    #[getter]
    fn step_s(&self) -> f64 {
        self.native.step().value()
    }

    fn __repr__(&self) -> String {
        format!("BackwardEuler(step_s={})", self.step_s())
    }
}

/// Bounded Newton policy owning exact nested linear controls.
#[pyclass(
    name = "Newton",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyNewton {
    pub(super) linear: Py<PyLinear>,
    pub(super) native: NonlinearSolvePlan,
}

impl PyNewton {
    pub(super) fn from_native(linear: Py<PyLinear>, native: NonlinearSolvePlan) -> Self {
        Self { linear, native }
    }
}

#[pymethods]
impl PyNewton {
    #[new]
    #[pyo3(signature = (*, linear, relative_tolerance=1.0e-9, absolute_tolerance=1.0e-11, maximum_iterations=16, maximum_line_search_steps=12))]
    fn new(
        py: Python<'_>,
        linear: Py<PyLinear>,
        #[pyo3(from_py_with = exact_float_extract)] relative_tolerance: f64,
        #[pyo3(from_py_with = exact_float_extract)] absolute_tolerance: f64,
        #[pyo3(from_py_with = exact_usize_extract)] maximum_iterations: usize,
        #[pyo3(from_py_with = exact_usize_extract)] maximum_line_search_steps: usize,
    ) -> PyResult<Self> {
        let maximum_iterations = NonZeroUsize::new(maximum_iterations)
            .ok_or_else(|| PyTypeError::new_err("maximum_iterations must be positive"))?;
        NonlinearSolvePlan::new(
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
            maximum_line_search_steps,
        )
        .map(|native| Self { linear, native })
        .map_err(|diagnostic| validation_error(py, &[diagnostic]))
    }

    #[getter]
    fn linear(&self, py: Python<'_>) -> Py<PyLinear> {
        self.linear.clone_ref(py)
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

    #[getter]
    fn maximum_line_search_steps(&self) -> usize {
        self.native.maximum_line_search_steps()
    }

    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            self.native == other.native
                && self.linear.borrow(py).native == other.linear.borrow(py).native
        })
    }

    fn __hash__(&self, py: Python<'_>) -> isize {
        let mut hasher = DefaultHasher::new();
        self.relative_tolerance().to_bits().hash(&mut hasher);
        self.absolute_tolerance().to_bits().hash(&mut hasher);
        self.maximum_iterations().hash(&mut hasher);
        self.maximum_line_search_steps().hash(&mut hasher);
        self.linear.borrow(py).__hash__().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "Newton(linear=<Linear>, relative_tolerance={}, absolute_tolerance={}, maximum_iterations={}, maximum_line_search_steps={})",
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
            self.maximum_line_search_steps()
        )
    }
}

/// Closed pressure representative selected by transient resolution.
#[pyclass(
    name = "PressureGauge2d",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyPressureGauge2d {
    ZeroIntegral,
    BoundaryTraction,
}

impl From<CommonPressureGauge2d> for PyPressureGauge2d {
    fn from(value: CommonPressureGauge2d) -> Self {
        match value {
            CommonPressureGauge2d::ZeroIntegral => Self::ZeroIntegral,
            CommonPressureGauge2d::BoundaryTraction => Self::BoundaryTraction,
        }
    }
}

fn exact_float_extract(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    Ok(value
        .cast::<PyFloat>()
        .map_err(|_| PyTypeError::new_err("nonlinear tolerance must be a float"))?
        .value())
}

fn exact_usize_extract(value: &Bound<'_, PyAny>) -> PyResult<usize> {
    if value.is_instance_of::<PyBool>() {
        return Err(PyTypeError::new_err(
            "nonlinear iteration bound must be an integer",
        ));
    }
    value
        .cast::<PyInt>()
        .map_err(|_| PyTypeError::new_err("nonlinear iteration bound must be an integer"))?
        .extract::<usize>()
        .map_err(|_| {
            PyTypeError::new_err("nonlinear iteration bound must be a non-negative integer")
        })
}
