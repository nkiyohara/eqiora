//! Python subscription preserves exact source bounds without Python slice clamping.
use super::*;
use pyo3::types::PySlice;

pub(super) fn select(value: DraftExpression, index: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    if index.is_instance_of::<PySlice>() {
        if !index.getattr("step")?.is_none() {
            return Err(PyValueError::new_err("channel slices do not accept a step"));
        }
        let lower = bound(&index.getattr("start")?)?;
        let upper = bound(&index.getattr("stop")?)?;
        Ok(PyExpression::new(value.slice(lower, upper)))
    } else {
        Ok(PyExpression::new(value.index(bound(index)?)))
    }
}

fn bound(value: &Bound<'_, PyAny>) -> PyResult<u32> {
    if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err(
            "channel bounds must be explicit nonnegative integers",
        ));
    }
    value.extract::<u32>()
}
