//! Typed inspection of the effective solver selected by root resolution.

use eqiora::realization::NonlinearSolvePlan;
use eqiora::solver::{
    LinearOperatorProperties, LinearSolver, PreconditionerPolicy, ReductionPolicy, SolverPlan,
};
use pyo3::prelude::*;

#[pyclass(
    name = "ResolvedLinear",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyResolvedLinear {
    plan: SolverPlan,
    operator: LinearOperatorProperties,
    backend: &'static str,
    backend_version: &'static str,
}

impl PyResolvedLinear {
    pub(super) const fn new(
        plan: SolverPlan,
        operator: LinearOperatorProperties,
        backend: &'static str,
        backend_version: &'static str,
    ) -> Self {
        Self {
            plan,
            operator,
            backend,
            backend_version,
        }
    }
}

#[pymethods]
impl PyResolvedLinear {
    #[getter]
    fn algorithm(&self) -> &'static str {
        match self.plan.algorithm() {
            LinearSolver::ConjugateGradient => "conjugate-gradient",
            LinearSolver::MinimumResidual => "minimum-residual",
            LinearSolver::BiConjugateGradientStabilized => "bicgstab",
            LinearSolver::SparseLu => "sparse-lu",
        }
    }

    #[getter]
    fn preconditioner(&self) -> &'static str {
        match self.plan.preconditioner() {
            PreconditionerPolicy::Identity => "identity",
            PreconditionerPolicy::Jacobi => "jacobi",
        }
    }

    #[getter]
    fn reduction(&self) -> &'static str {
        match self.plan.reduction() {
            ReductionPolicy::Reproducible => "reproducible",
            ReductionPolicy::Fast => "fast",
        }
    }

    #[getter]
    fn relative_tolerance(&self) -> f64 {
        self.plan.relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.plan.absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.plan.maximum_iterations().get()
    }

    #[getter]
    fn operator(&self) -> &'static str {
        match self.operator {
            LinearOperatorProperties::General => "general",
            LinearOperatorProperties::SymmetricPositiveDefinite => "symmetric-positive-definite",
            LinearOperatorProperties::SymmetricIndefinite => "symmetric-indefinite",
        }
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
        format!(
            "ResolvedLinear(algorithm={:?}, preconditioner={:?}, reduction={:?}, backend={:?})",
            self.algorithm(),
            self.preconditioner(),
            self.reduction(),
            self.backend,
        )
    }
}

#[pyclass(
    name = "ResolvedNewton",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug)]
pub(crate) struct PyResolvedNewton {
    linear: Py<PyResolvedLinear>,
    nonlinear: NonlinearSolvePlan,
}

impl PyResolvedNewton {
    pub(super) const fn new(linear: Py<PyResolvedLinear>, nonlinear: NonlinearSolvePlan) -> Self {
        Self { linear, nonlinear }
    }
}

#[pymethods]
impl PyResolvedNewton {
    #[getter]
    fn linear(&self, py: Python<'_>) -> Py<PyResolvedLinear> {
        self.linear.clone_ref(py)
    }

    #[getter]
    fn relative_tolerance(&self) -> f64 {
        self.nonlinear.relative_tolerance()
    }

    #[getter]
    fn absolute_tolerance(&self) -> f64 {
        self.nonlinear.absolute_tolerance()
    }

    #[getter]
    fn maximum_iterations(&self) -> usize {
        self.nonlinear.maximum_iterations().get()
    }

    #[getter]
    fn maximum_line_search_steps(&self) -> usize {
        self.nonlinear.maximum_line_search_steps()
    }

    fn __repr__(&self) -> String {
        format!(
            "ResolvedNewton(linear=<ResolvedLinear>, relative_tolerance={}, absolute_tolerance={}, maximum_iterations={}, maximum_line_search_steps={})",
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
            self.maximum_line_search_steps(),
        )
    }
}
