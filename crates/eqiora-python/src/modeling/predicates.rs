//! Closed predicate authoring over the shared typed Draft expression owner.
use super::{PyExpression, expression_from_python};
use pyo3::prelude::*;

/// Author the equal predicate without Python host coercion.
#[pyfunction]
fn equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.equal(expression_from_python(right)?),
    ))
}

/// Author the not equal predicate without Python host coercion.
#[pyfunction]
fn not_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.not_equal(expression_from_python(right)?),
    ))
}

/// Author the less predicate without Python host coercion.
#[pyfunction]
fn less(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.less(expression_from_python(right)?),
    ))
}

/// Author the less equal predicate without Python host coercion.
#[pyfunction]
fn less_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.less_equal(expression_from_python(right)?),
    ))
}

/// Author the greater predicate without Python host coercion.
#[pyfunction]
fn greater(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.greater(expression_from_python(right)?),
    ))
}

/// Author the greater equal predicate without Python host coercion.
#[pyfunction]
fn greater_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.greater_equal(expression_from_python(right)?),
    ))
}

/// Author the logical and predicate without Python host coercion.
#[pyfunction]
fn logical_and(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.logical_and(expression_from_python(right)?),
    ))
}

/// Author the logical or predicate without Python host coercion.
#[pyfunction]
fn logical_or(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(left)?.logical_or(expression_from_python(right)?),
    ))
}

/// Author logical negation without evaluating host truthiness.
#[pyfunction]
fn logical_not(value: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    Ok(PyExpression::new(
        expression_from_python(value)?.logical_not(),
    ))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(equal, module)?)?;
    module.add_function(wrap_pyfunction!(not_equal, module)?)?;
    module.add_function(wrap_pyfunction!(less, module)?)?;
    module.add_function(wrap_pyfunction!(less_equal, module)?)?;
    module.add_function(wrap_pyfunction!(greater, module)?)?;
    module.add_function(wrap_pyfunction!(greater_equal, module)?)?;
    module.add_function(wrap_pyfunction!(logical_and, module)?)?;
    module.add_function(wrap_pyfunction!(logical_or, module)?)?;
    module.add_function(wrap_pyfunction!(logical_not, module)?)?;
    Ok(())
}
