//! Thin finite-affine policy projection.
use super::*;

#[pyclass(name = "AlgebraicPlanView", module = "eqiora._eqiora", frozen)]
pub(super) struct PyAlgebraicPlanView {
    #[pyo3(get)]
    pub(super) unknown_count: usize,
}

#[pymethods]
impl PyAlgebraicPlanView {
    #[getter]
    fn kind(&self) -> &'static str {
        "finite-affine"
    }
}

pub(super) fn resolve(
    py: Python<'_>,
    model: Py<PyModel>,
    solve: Option<&Bound<'_, PyAny>>,
    formulation: Option<&Bound<'_, PyAny>>,
    scaling: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPlan> {
    if formulation.is_some_and(|v| !v.is_none()) || scaling.is_some_and(|v| !v.is_none()) {
        return Err(PyTypeError::new_err(
            "finite affine resolve accepts Model and linear solve controls",
        ));
    }
    let linear = solve
        .ok_or_else(|| PyTypeError::new_err("finite affine resolve requires solve=Linear(...)"))?
        .extract::<Py<PyLinear>>()?;
    let request = CommonSolvePolicy::Linear(linear.borrow(py).native);
    let native = eqiora_numerics::CommonAlgebraicPlan::resolve(
        model.borrow(py).artifact(),
        request,
        &FaerLinearSolver,
    )
    .map_err(|d| validation_error(py, &[d]))?;
    PyPlan::from_native_artifact(py, ResolvedCommonPlan::Algebraic(Box::new(native)))
}

pub(super) fn view(
    py: Python<'_>,
    plan: &eqiora_numerics::CommonAlgebraicPlan,
) -> PyResult<Py<PyAny>> {
    Py::new(
        py,
        PyAlgebraicPlanView {
            unknown_count: plan.symbols().len(),
        },
    )
    .map(Py::into_any)
}
