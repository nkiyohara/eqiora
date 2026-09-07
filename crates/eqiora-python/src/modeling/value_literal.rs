//! Python projections of the shared complete mathematical value owner.

use eqiora::{ScalarDomain, ValueLiteral, ValueType};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyComplex, PyFloat, PyInt, PyList, PyTuple};

pub(crate) fn scalar(value: &Bound<'_, PyAny>) -> PyResult<(f64, f64)> {
    if let Ok(value) = value.cast::<PyComplex>() {
        return Ok((value.real(), value.imag()));
    }
    if value.is_instance_of::<PyBool>()
        || !(value.is_instance_of::<PyFloat>() || value.is_instance_of::<PyInt>())
    {
        return Err(PyTypeError::new_err(
            "value components must be real or complex numbers, not bool",
        ));
    }
    Ok((value.extract()?, 0.0))
}

pub(crate) fn from_python(
    value: &Bound<'_, PyAny>,
    value_type: ValueType,
) -> PyResult<ValueLiteral> {
    eqiora::language::ValueTypeSyntax::from_checked(&value_type)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    if !value.is_instance_of::<PyList>() && !value.is_instance_of::<PyTuple>() {
        let (real, imaginary) = scalar(value)?;
        let result = if imaginary == 0.0 {
            ValueLiteral::from_real(value_type, real)
        } else {
            ValueLiteral::new(value_type, [(real, imaginary)])
        };
        return result.map_err(|error| PyValueError::new_err(error.to_string()));
    }
    fn collect(
        value: &Bound<'_, PyAny>,
        extents: &[std::num::NonZeroU32],
        output: &mut Vec<(f64, f64)>,
    ) -> PyResult<()> {
        let Some((extent, tail)) = extents.split_first() else {
            output.push(scalar(value)?);
            return Ok(());
        };
        if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
            return Err(PyValueError::new_err(
                "value must match every declared shape axis; scalar broadcasting is not admitted",
            ));
        }
        if value.len()? != extent.get() as usize {
            return Err(PyValueError::new_err(
                "value shape differs from the complete declared type",
            ));
        }
        for item in value.try_iter()? {
            collect(&item?, tail, output)?;
        }
        Ok(())
    }
    let mut components = Vec::new();
    collect(value, value_type.shape().extents(), &mut components)?;
    ValueLiteral::new(value_type, components)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

pub(crate) fn to_python(py: Python<'_>, value: &ValueLiteral) -> PyResult<Py<PyAny>> {
    eqiora::language::ValueTypeSyntax::from_checked(value.value_type())
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    fn nested(
        py: Python<'_>,
        value: &ValueLiteral,
        axis: usize,
        offset: &mut usize,
    ) -> PyResult<Py<PyAny>> {
        if let Some(extent) = value.value_type().shape().extents().get(axis) {
            let items = (0..extent.get())
                .map(|_| nested(py, value, axis + 1, offset))
                .collect::<PyResult<Vec<_>>>()?;
            return Ok(PyTuple::new(py, items)?.into_any().unbind());
        }
        let (real, imaginary) = value.component(*offset).expect("bounded component");
        *offset += 1;
        if value.value_type().scalar_domain() == ScalarDomain::Complex {
            Ok(PyComplex::from_doubles(py, real, imaginary)
                .into_any()
                .unbind())
        } else {
            real.into_py_any(py)
        }
    }
    nested(py, value, 0, &mut 0)
}

pub(crate) fn expression(value: &Bound<'_, PyAny>) -> PyResult<eqiora::language::DraftExpression> {
    fn build(
        value: &Bound<'_, PyAny>,
        depth: usize,
        nodes: &mut usize,
    ) -> PyResult<eqiora::language::DraftExpression> {
        use eqiora::language::DraftExpression;
        *nodes += 1;
        if depth > 64 || *nodes > 4096 {
            return Err(PyValueError::new_err(
                "typed input exceeds expression depth or node bounds",
            ));
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            if value.len()? == 0 {
                return Err(PyValueError::new_err("channel arrays must be nonempty"));
            }
            let items = value
                .try_iter()?
                .map(|item| build(&item?, depth + 1, nodes))
                .collect::<PyResult<Vec<_>>>()?;
            return Ok(DraftExpression::array(items));
        }
        let (real, imaginary) = scalar(value)?;
        if !real.is_finite() || !imaginary.is_finite() {
            return Err(PyValueError::new_err(
                "typed input components must be finite",
            ));
        }
        if value.is_instance_of::<PyComplex>() {
            Ok(DraftExpression::complex(real, imaginary))
        } else {
            Ok(DraftExpression::constant(real))
        }
    }
    build(value, 1, &mut 0)
}
