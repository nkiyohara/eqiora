//! Exact enum declarations and values shared by Source, native drafts, and compiled Models.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eqiora::ValueLiteral;
use eqiora::kernel::EnumDef;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyString, PyTuple};

use super::PyValueType;

/// Immutable checked enum declaration with exact nominal identity and ordered members.
#[pyclass(
    name = "Enum",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyEnum {
    pub(crate) name: Option<String>,
    pub(crate) value: EnumDef,
}

impl PartialEq for PyEnum {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for PyEnum {}

#[pymethods]
impl PyEnum {
    #[new]
    #[pyo3(signature = (name, *, members))]
    fn new(name: String, members: &Bound<'_, PyAny>) -> PyResult<Self> {
        if members.is_instance_of::<PyString>() {
            return Err(PyTypeError::new_err(
                "enum members must be an ordered sequence of names",
            ));
        }
        if members.len()? > eqiora::ValueType::MAX_ENUM_MEMBERS as usize {
            return Err(PyValueError::new_err(
                "enum members exceed the declaration bound",
            ));
        }
        let members: Vec<String> = members.extract()?;
        super::nominal::name_path(&name)?;
        for member in &members {
            super::nominal::name_path(member)?;
        }
        let value = EnumDef::new(eqiora::Id::new(), members)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(Self {
            name: Some(name),
            value,
        })
    }
    #[getter]
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    #[getter]
    fn id(&self) -> String {
        self.value.id().ulid().to_string()
    }
    #[getter]
    fn members(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, self.value.members())?.unbind())
    }
    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type(),
        }
    }
    fn member(&self, name: &str) -> PyResult<PyEnumValue> {
        let tag = self
            .value
            .members()
            .iter()
            .position(|member| member == name)
            .ok_or_else(|| PyValueError::new_err("enum member is not declared"))?;
        let value = self
            .value
            .value(tag as u32)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(PyEnumValue { value })
    }
    fn __hash__(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        self.value.id().hash(&mut hash);
        hash.finish()
    }
    fn __repr__(&self) -> String {
        format!(
            "Enum(name={:?}, id={:?}, members={:?})",
            self.name,
            self.id(),
            self.value.members()
        )
    }
}

/// Immutable exact enum value, constructed only through an admitted declaration or Model value.
#[pyclass(
    name = "EnumValue",
    module = "eqiora._eqiora",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PyEnumValue {
    pub(crate) value: ValueLiteral,
}

#[pymethods]
impl PyEnumValue {
    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type().clone(),
        }
    }
    #[getter]
    fn enum_id(&self) -> String {
        self.value
            .value_type()
            .enum_definition()
            .expect("checked enum value")
            .ulid()
            .to_string()
    }
    fn __hash__(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        self.value.value_type().hash(&mut hash);
        self.value.enum_tag().hash(&mut hash);
        hash.finish()
    }
    fn __bool__(&self) -> PyResult<bool> {
        Err(PyTypeError::new_err(
            "enum values have no Boolean or numeric coercion",
        ))
    }
    fn __repr__(&self) -> String {
        format!(
            "EnumValue(enum_id={:?}, tag={})",
            self.enum_id(),
            self.value.enum_tag().expect("checked enum value")
        )
    }
}
