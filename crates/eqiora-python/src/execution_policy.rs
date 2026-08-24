//! Composable Python policies for capability resolution.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

use eqiora::diagnostic::codes;
use eqiora::numerics::IncompressibleFlowScaleProfile2d;
use eqiora::solver::{LinearSolver, PreconditionerPolicy, ReductionPolicy, SolverPlan};
use eqiora::{Diagnostic, DimExponents, DynQuantity};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::error::diagnostic_error;

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

/// Characteristic coherent-SI scales for incompressible flow realization.
#[pyclass(
    name = "IncompressibleFlowScales",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyIncompressibleFlowScales {
    native: IncompressibleFlowScaleProfile2d,
}

#[pymethods]
impl PyIncompressibleFlowScales {
    #[new]
    #[pyo3(signature = (*, length_m, velocity_m_per_s, pressure_pa))]
    fn new(
        py: Python<'_>,
        length_m: f64,
        velocity_m_per_s: f64,
        pressure_pa: f64,
    ) -> PyResult<Self> {
        IncompressibleFlowScaleProfile2d::new(
            DynQuantity::new(length_m, LENGTH),
            DynQuantity::new(velocity_m_per_s, VELOCITY),
            DynQuantity::new(pressure_pa, PRESSURE),
        )
        .map(|native| Self { native })
        .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))
    }

    #[getter]
    fn length_m(&self) -> f64 {
        self.native.length().value()
    }

    #[getter]
    fn velocity_m_per_s(&self) -> f64 {
        self.native.velocity().value()
    }

    #[getter]
    fn pressure_pa(&self) -> f64 {
        self.native.pressure().value()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.native == other.native)
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.length_m().to_bits().hash(&mut hasher);
        self.velocity_m_per_s().to_bits().hash(&mut hasher);
        self.pressure_pa().to_bits().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "IncompressibleFlowScales(length_m={}, velocity_m_per_s={}, pressure_pa={})",
            self.length_m(),
            self.velocity_m_per_s(),
            self.pressure_pa(),
        )
    }
}

impl PyIncompressibleFlowScales {
    pub(crate) const fn native(&self) -> IncompressibleFlowScaleProfile2d {
        self.native
    }
}

/// Complete backend-neutral policy for one linear solve.
#[pyclass(
    name = "LinearSolve",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyLinearSolve {
    native: SolverPlan,
}

#[pymethods]
impl PyLinearSolve {
    #[new]
    #[pyo3(signature = (*, algorithm, preconditioner, reduction, relative_tolerance, absolute_tolerance, maximum_iterations))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        algorithm: &str,
        preconditioner: &str,
        reduction: &str,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: i64,
    ) -> PyResult<Self> {
        let maximum_iterations = usize::try_from(maximum_iterations)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                diagnostic_error(
                    py,
                    &[Diagnostic::error(
                        codes::INVALID_REALIZATION,
                        "linear solve maximum_iterations must be strictly positive",
                    )],
                )
            })?;
        let algorithm = parse_linear_solver(py, algorithm)?;
        let preconditioner = parse_preconditioner(py, preconditioner)?;
        let reduction = parse_reduction(py, reduction)?;
        SolverPlan::new(
            algorithm,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )
        .map(|native| {
            native
                .with_preconditioner(preconditioner)
                .with_reduction(reduction)
        })
        .map(|native| Self { native })
        .map_err(|diagnostic| diagnostic_error(py, std::slice::from_ref(&diagnostic)))
    }

    #[getter]
    fn algorithm(&self) -> &'static str {
        linear_solver_name(self.native.algorithm())
    }

    #[getter]
    fn preconditioner(&self) -> &'static str {
        preconditioner_name(self.native.preconditioner())
    }

    #[getter]
    fn reduction(&self) -> &'static str {
        reduction_name(self.native.reduction())
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

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __hash__(&self) -> isize {
        let mut hasher = DefaultHasher::new();
        self.algorithm().hash(&mut hasher);
        self.preconditioner().hash(&mut hasher);
        self.reduction().hash(&mut hasher);
        self.relative_tolerance().to_bits().hash(&mut hasher);
        self.absolute_tolerance().to_bits().hash(&mut hasher);
        self.maximum_iterations().hash(&mut hasher);
        hasher.finish() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "LinearSolve(algorithm={:?}, preconditioner={:?}, reduction={:?}, relative_tolerance={:e}, absolute_tolerance={:e}, maximum_iterations={})",
            self.algorithm(),
            self.preconditioner(),
            self.reduction(),
            self.relative_tolerance(),
            self.absolute_tolerance(),
            self.maximum_iterations(),
        )
    }
}

impl PyLinearSolve {
    pub(crate) const fn native(&self) -> SolverPlan {
        self.native
    }
}

pub(crate) const fn linear_solver_name(value: LinearSolver) -> &'static str {
    match value {
        LinearSolver::ConjugateGradient => "conjugate-gradient",
        LinearSolver::MinimumResidual => "minimum-residual",
        LinearSolver::BiConjugateGradientStabilized => "bicgstab",
        LinearSolver::SparseLu => "sparse-lu",
    }
}

pub(crate) const fn preconditioner_name(value: PreconditionerPolicy) -> &'static str {
    match value {
        PreconditionerPolicy::Identity => "identity",
        PreconditionerPolicy::Jacobi => "jacobi",
    }
}

pub(crate) const fn reduction_name(value: ReductionPolicy) -> &'static str {
    match value {
        ReductionPolicy::Reproducible => "reproducible",
        ReductionPolicy::Fast => "fast",
    }
}

fn parse_linear_solver(py: Python<'_>, value: &str) -> PyResult<LinearSolver> {
    match value {
        "conjugate-gradient" => Ok(LinearSolver::ConjugateGradient),
        "minimum-residual" => Ok(LinearSolver::MinimumResidual),
        "bicgstab" => Ok(LinearSolver::BiConjugateGradientStabilized),
        "sparse-lu" => Ok(LinearSolver::SparseLu),
        _ => Err(unknown_policy(
            py,
            "linear solve algorithm",
            value,
            "conjugate-gradient, minimum-residual, bicgstab, or sparse-lu",
        )),
    }
}

fn parse_preconditioner(py: Python<'_>, value: &str) -> PyResult<PreconditionerPolicy> {
    match value {
        "identity" => Ok(PreconditionerPolicy::Identity),
        "jacobi" => Ok(PreconditionerPolicy::Jacobi),
        _ => Err(unknown_policy(
            py,
            "linear solve preconditioner",
            value,
            "identity or jacobi",
        )),
    }
}

fn parse_reduction(py: Python<'_>, value: &str) -> PyResult<ReductionPolicy> {
    match value {
        "reproducible" => Ok(ReductionPolicy::Reproducible),
        "fast" => Ok(ReductionPolicy::Fast),
        _ => Err(unknown_policy(
            py,
            "linear solve reduction",
            value,
            "reproducible or fast",
        )),
    }
}

fn unknown_policy(py: Python<'_>, name: &str, value: &str, accepted: &str) -> PyErr {
    diagnostic_error(
        py,
        &[Diagnostic::error(
            codes::INVALID_REALIZATION,
            format!("unknown {name} {value:?}; expected {accepted}"),
        )],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyIncompressibleFlowScales>()?;
    module.add_class::<PyLinearSolve>()?;
    Ok(())
}
