//! Immutable Python declarations over the client-neutral Rust model draft.

use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::language::{
    BoundarySideSyntax, DraftConservingConnection, DraftConservingPort, DraftDeclaration,
    DraftExpression, DraftField, DraftParameter, DraftPhysicalDomain, DraftRelation,
    DraftSpatialDomain, FieldRoleSyntax, ModelDraft,
};
pub(crate) mod dimension;
mod nominal;
pub(crate) mod value_literal;
mod value_type;
pub(crate) use value_type::PyValueType;

use pyo3::exceptions::{PyAttributeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyComplex, PyInt, PyModule, PyTuple};

use crate::diagnostic_error;

/// SI base-dimension exponents in M,L,T,I,Theta,N,J order.
#[pyclass(
    name = "Dimension",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct PyDimension {
    value: DimExponents,
}

/// Closed orientation of one Cartesian boundary Domain.
#[pyclass(
    name = "BoundarySide",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyBoundarySide {
    /// Lower coordinate side.
    Lower,
    /// Upper coordinate side.
    Upper,
}

impl From<PyBoundarySide> for BoundarySideSyntax {
    fn from(value: PyBoundarySide) -> Self {
        match value {
            PyBoundarySide::Lower => Self::Lower,
            PyBoundarySide::Upper => Self::Upper,
        }
    }
}

/// Immutable draft-local Cartesian volume or oriented boundary Domain.
#[pyclass(
    name = "Domain",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyDomain {
    value: DraftSpatialDomain,
}

#[pymethods]
impl PyDomain {
    /// Construct one Cartesian box with one `(lower, upper)` pair per axis.
    #[staticmethod]
    #[pyo3(name = "box", signature = (name, *bounds))]
    fn box_(name: String, bounds: &Bound<'_, PyTuple>) -> PyResult<Self> {
        let bounds = bounds
            .iter()
            .map(|bound| {
                bound.extract::<(f64, f64)>().map_err(|_| {
                    PyTypeError::new_err(
                        "Domain.box bounds must be (lower, upper) real-number pairs",
                    )
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok(Self {
            value: DraftSpatialDomain::cartesian_box(name, bounds),
        })
    }

    /// Construct one oriented side of this exact draft-local Domain.
    #[pyo3(signature = (name, *, axis, side))]
    fn boundary(&self, name: String, axis: usize, side: PyBoundarySide) -> Self {
        Self {
            value: DraftSpatialDomain::boundary(name, &self.value, axis, side.into()),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    fn bounds(&self) -> Option<Vec<(f64, f64)>> {
        self.value.bounds().map(<[_]>::to_vec)
    }

    #[getter]
    fn parent(&self) -> Option<Self> {
        self.value.parent().map(|parent| Self {
            value: parent.clone(),
        })
    }

    #[getter]
    fn axis(&self) -> Option<usize> {
        self.value.boundary_axis()
    }

    #[getter]
    fn side(&self) -> Option<PyBoundarySide> {
        self.value.boundary_side().map(|side| match side {
            BoundarySideSyntax::Lower => PyBoundarySide::Lower,
            BoundarySideSyntax::Upper => PyBoundarySide::Upper,
        })
    }

    fn __repr__(&self) -> String {
        if let Some(bounds) = self.value.bounds() {
            format!("Domain.box({:?}, bounds={bounds:?})", self.name())
        } else {
            format!(
                "Domain.boundary({:?}, parent={:?}, axis={}, side={:?})",
                self.name(),
                self.value.parent().map(DraftSpatialDomain::name),
                self.value.boundary_axis().unwrap_or_default(),
                self.side()
            )
        }
    }
}

/// Author-declared evolution role independent of spatial support.
#[pyclass(
    name = "FieldRole",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyFieldRole {
    Variable,
    State,
}

impl From<PyFieldRole> for FieldRoleSyntax {
    fn from(role: PyFieldRole) -> Self {
        match role {
            PyFieldRole::Variable => Self::Variable,
            PyFieldRole::State => Self::State,
        }
    }
}

/// Simultaneous fresh-initialization residuals, each equal to zero.
#[pyclass(
    name = "Initial",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyInitial {
    residuals: Vec<DraftExpression>,
}

#[pymethods]
impl PyInitial {
    #[new]
    #[pyo3(signature = (*residuals))]
    fn new(residuals: &Bound<'_, PyTuple>) -> PyResult<Self> {
        Ok(Self {
            residuals: residuals
                .iter()
                .map(|value| expression_from_python(&value))
                .collect::<PyResult<_>>()?,
        })
    }

    #[getter]
    fn residuals(&self) -> Vec<PyExpression> {
        self.residuals
            .iter()
            .cloned()
            .map(PyExpression::new)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!("Initial(residuals={})", self.residuals.len())
    }
}

/// Immutable typed Field declaration.
#[pyclass(name = "Field", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub(crate) struct PyField {
    value: DraftField,
}

#[pymethods]
impl PyField {
    #[new]
    #[pyo3(signature = (name, *, role, domain=None, value_type=None))]
    fn new(
        name: String,
        role: PyFieldRole,
        domain: Option<&PyDomain>,
        value_type: Option<&PyValueType>,
    ) -> Self {
        let value_type = value_type.map_or_else(
            || eqiora::ValueType::scalar(eqiora::ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            |value| value.value.clone(),
        );
        let value = match domain {
            None => DraftField::new(name, value_type, role.into()),
            Some(domain) => DraftField::spatial(name, &domain.value, value_type, role.into()),
        };
        Self { value }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    const fn dimension(&self) -> PyDimension {
        PyDimension {
            value: self.value.dimension(),
        }
    }

    #[getter]
    const fn role(&self) -> PyFieldRole {
        match self.value.role() {
            FieldRoleSyntax::Variable => PyFieldRole::Variable,
            FieldRoleSyntax::State => PyFieldRole::State,
        }
    }

    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type().clone(),
        }
    }

    #[getter]
    fn domain(&self) -> Option<PyDomain> {
        self.value.domain().map(|domain| PyDomain {
            value: domain.clone(),
        })
    }

    fn __getitem__(&self, index: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        if index.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("index must be a nonnegative integer"));
        }
        Ok(PyExpression::new(
            self.value.expression().index(index.extract::<u32>()?),
        ))
    }

    fn __neg__(&self) -> PyExpression {
        PyExpression::new(-self.value.expression())
    }

    fn __add__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Add, false)
    }

    fn __radd__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Add, true)
    }

    fn __sub__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Subtract, false)
    }

    fn __rsub__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Subtract, true)
    }

    fn __mul__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Multiply, false)
    }

    fn __rmul__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Multiply, true)
    }

    fn __truediv__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Divide, false)
    }

    fn __rtruediv__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Divide, true)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(symbolic_truth_error())
    }

    fn __repr__(&self) -> String {
        format!(
            "Field({:?}, role=FieldRole.{:?}, domain={:?}, value_type={:?})",
            self.name(),
            self.role(),
            self.value.domain().map(DraftSpatialDomain::name),
            self.value.value_type()
        )
    }
}

/// Immutable Parameter declaration owning a complete typed value.
#[pyclass(
    name = "Parameter",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyParameter {
    value: DraftParameter,
}

#[pymethods]
impl PyParameter {
    #[new]
    #[pyo3(signature = (name, *, value_type=None, value))]
    fn new(
        name: String,
        value_type: Option<&PyValueType>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let kind = value_type.map_or_else(
            || {
                eqiora::ValueType::scalar(
                    if value.is_instance_of::<PyComplex>() {
                        eqiora::ScalarDomain::Complex
                    } else {
                        eqiora::ScalarDomain::Real
                    },
                    DimExponents::DIMENSIONLESS,
                )
            },
            |value| value.value.clone(),
        );
        let literal = value_literal::from_python(value, kind)?;
        Ok(Self {
            value: DraftParameter::new(name, literal),
        })
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    const fn dimension(&self) -> PyDimension {
        PyDimension {
            value: self.value.dimension(),
        }
    }

    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        value_literal::to_python(py, self.value.value())
    }

    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type().clone(),
        }
    }

    fn __getitem__(&self, index: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        if index.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("index must be a nonnegative integer"));
        }
        Ok(PyExpression::new(
            self.value.expression().index(index.extract::<u32>()?),
        ))
    }

    fn __neg__(&self) -> PyExpression {
        PyExpression::new(-self.value.expression())
    }

    fn __add__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Add, false)
    }

    fn __radd__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Add, true)
    }

    fn __sub__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Subtract, false)
    }

    fn __rsub__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Subtract, true)
    }

    fn __mul__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Multiply, false)
    }

    fn __rmul__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Multiply, true)
    }

    fn __truediv__(&self, right: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), right, Binary::Divide, false)
    }

    fn __rtruediv__(&self, left: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
        binary(self.value.expression(), left, Binary::Divide, true)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(symbolic_truth_error())
    }

    fn __repr__(&self) -> String {
        format!(
            "Parameter({:?}, value_type={:?}, value={:?})",
            self.name(),
            self.value.value_type(),
            self.value.value()
        )
    }
}

/// Immutable nominal scalar physical Domain declaration.
#[pyclass(
    name = "PhysicalDomain",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyPhysicalDomain {
    value: DraftPhysicalDomain,
}

#[pymethods]
impl PyPhysicalDomain {
    #[new]
    #[pyo3(signature = (name, *, across_type, through_type))]
    fn new(name: String, across_type: &PyValueType, through_type: &PyValueType) -> Self {
        Self {
            value: DraftPhysicalDomain::new(
                name,
                across_type.value.clone(),
                through_type.value.clone(),
            ),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    fn across_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.across_type().clone(),
        }
    }

    #[getter]
    fn through_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.through_type().clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PhysicalDomain({:?}, across_type={:?}, through_type={:?})",
            self.name(),
            self.value.across_type(),
            self.value.through_type()
        )
    }
}

/// Immutable scalar conserving Port declaration.
#[pyclass(
    name = "ConservingPort",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyConservingPort {
    value: DraftConservingPort,
}

#[pymethods]
impl PyConservingPort {
    #[new]
    #[pyo3(signature = (name, *, domain))]
    fn new(name: String, domain: &PyPhysicalDomain) -> Self {
        Self {
            value: DraftConservingPort::new(name, &domain.value),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    fn domain(&self) -> PyPhysicalDomain {
        PyPhysicalDomain {
            value: self.value.domain().clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ConservingPort({:?}, domain={:?})",
            self.name(),
            self.value.domain().name()
        )
    }
}

/// Immutable anonymous N-ary conserving connection declaration.
#[pyclass(
    name = "Connection",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyConnection {
    value: DraftConservingConnection,
}

#[pymethods]
impl PyConnection {
    fn __repr__(&self) -> String {
        format!("Connection(ports={})", self.value.ports().len())
    }
}

/// Immutable symbolic expression with shape/support inferred by Rust.
#[pyclass(
    name = "Expression",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyExpression {
    value: DraftExpression,
}

impl PyExpression {
    const fn new(value: DraftExpression) -> Self {
        Self { value }
    }
}

#[pymethods]
impl PyExpression {
    fn __getitem__(&self, index: &Bound<'_, PyAny>) -> PyResult<Self> {
        if index.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("index must be a nonnegative integer"));
        }
        Ok(Self::new(self.value.clone().index(index.extract::<u32>()?)))
    }

    fn __neg__(&self) -> Self {
        Self::new(-self.value.clone())
    }

    fn __add__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), right, Binary::Add, false)
    }

    fn __radd__(&self, left: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), left, Binary::Add, true)
    }

    fn __sub__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), right, Binary::Subtract, false)
    }

    fn __rsub__(&self, left: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), left, Binary::Subtract, true)
    }

    fn __mul__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), right, Binary::Multiply, false)
    }

    fn __rmul__(&self, left: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), left, Binary::Multiply, true)
    }

    fn __truediv__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), right, Binary::Divide, false)
    }

    fn __rtruediv__(&self, left: &Bound<'_, PyAny>) -> PyResult<Self> {
        binary(self.value.clone(), left, Binary::Divide, true)
    }

    fn __bool__(&self) -> PyResult<bool> {
        Err(symbolic_truth_error())
    }

    fn __repr__(&self) -> &'static str {
        "Expression(<symbolic>)"
    }
}

/// Immutable continuous implicit Relation declaration.
#[pyclass(
    name = "Relation",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyRelation {
    value: DraftRelation,
    residuals: Vec<PyExpression>,
}

#[pymethods]
impl PyRelation {
    #[new]
    #[pyo3(signature = (name, *, domain=None, residual=None, residuals=None))]
    fn new(
        name: String,
        domain: Option<&PyDomain>,
        residual: Option<&Bound<'_, PyAny>>,
        residuals: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let residuals = match (residual, residuals) {
            (Some(_), Some(_)) => {
                return Err(PyTypeError::new_err(
                    "Relation accepts exactly one of residual= or residuals=",
                ));
            }
            (None, None) => {
                return Err(PyTypeError::new_err(
                    "Relation requires exactly one of residual= or residuals=",
                ));
            }
            (Some(residual), None) => vec![expression_from_python(residual)?],
            (None, Some(residuals)) => residuals
                .try_iter()
                .map_err(|_| {
                    PyTypeError::new_err("Relation residuals must be an iterable of expressions")
                })?
                .map(|residual| expression_from_python(&residual?))
                .collect::<PyResult<Vec<_>>>()?,
        };
        let value = match domain {
            Some(domain) => {
                DraftRelation::continuous_on(name, &domain.value, residuals.iter().cloned())
            }
            None => DraftRelation::continuous(name, residuals.iter().cloned()),
        };
        Ok(Self {
            value,
            residuals: residuals.into_iter().map(PyExpression::new).collect(),
        })
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    fn residual(&self) -> PyResult<PyExpression> {
        if self.residuals.len() == 1 {
            Ok(self.residuals[0].clone())
        } else {
            Err(PyAttributeError::new_err(
                "multi-residual Relation has no unique residual; use residuals",
            ))
        }
    }

    #[getter]
    fn residuals(&self) -> Vec<PyExpression> {
        self.residuals.clone()
    }

    #[getter]
    fn domain(&self) -> Option<PyDomain> {
        self.value.domain().map(|domain| PyDomain {
            value: domain.clone(),
        })
    }

    fn __repr__(&self) -> String {
        match self.value.domain() {
            Some(domain) => format!(
                "Relation({:?}, activation='continuous', domain={:?})",
                self.name(),
                domain.name()
            ),
            None => format!("Relation({:?}, activation='continuous')", self.name()),
        }
    }
}

/// Time derivative of one Field.
#[pyfunction]
pub(crate) fn derivative(field: &PyField) -> PyExpression {
    PyExpression::new(DraftExpression::derivative(&field.value))
}

/// Across variable of one scalar conserving Port.
#[pyfunction]
pub(crate) fn across(port: &PyConservingPort) -> PyExpression {
    PyExpression::new(DraftExpression::across(&port.value))
}

/// Through variable of one scalar conserving Port.
#[pyfunction]
pub(crate) fn through(port: &PyConservingPort) -> PyExpression {
    PyExpression::new(DraftExpression::through(&port.value))
}

/// Spatial gradient of one symbolic expression.
#[pyfunction]
pub(crate) fn grad(value: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    expression_from_python(value)
        .map(DraftExpression::gradient)
        .map(PyExpression::new)
}

/// Spatial divergence of one symbolic expression.
#[pyfunction]
pub(crate) fn div(value: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    expression_from_python(value)
        .map(DraftExpression::divergence)
        .map(PyExpression::new)
}

/// Boundary trace of one symbolic expression.
#[pyfunction]
pub(crate) fn trace(value: &Bound<'_, PyAny>) -> PyResult<PyExpression> {
    expression_from_python(value)
        .map(DraftExpression::trace)
        .map(PyExpression::new)
}

/// Build one anonymous N-ary conserving connection declaration.
#[pyfunction]
#[pyo3(signature = (*ports))]
pub(crate) fn connect(ports: &Bound<'_, PyTuple>) -> PyResult<PyConnection> {
    let ports = ports
        .iter()
        .map(|port| {
            port.extract::<PyRef<'_, PyConservingPort>>()
                .map(|port| port.value.clone())
                .map_err(|_| {
                    PyTypeError::new_err("connect arguments must be ConservingPort objects")
                })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyConnection {
        value: DraftConservingConnection::new(&ports),
    })
}

pub(crate) fn define_model(
    py: Python<'_>,
    name: String,
    declarations: &Bound<'_, PyTuple>,
) -> PyResult<ModelDocument> {
    let draft = model_draft(py, name, declarations)?;
    py.detach(move || ModelDocument::define(&draft))
        .map_err(|diagnostics| diagnostic_error(py, &diagnostics))
}

fn model_draft(
    py: Python<'_>,
    name: String,
    declarations: &Bound<'_, PyTuple>,
) -> PyResult<ModelDraft> {
    let mut draft_declarations = Vec::with_capacity(declarations.len());
    for declaration in declarations.iter() {
        draft_declarations.push(declaration_from_python(&declaration)?);
    }
    ModelDraft::new(name, draft_declarations)
        .map_err(|diagnostics| diagnostic_error(py, &diagnostics))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDimension>()?;
    module.add_class::<PyValueType>()?;
    module.add_class::<nominal::PyFiniteSpace>()?;
    module.add_class::<nominal::PyIndexSet>()?;
    module.add_function(wrap_pyfunction!(nominal::_nominal_type_source, module)?)?;
    module.add_class::<PyBoundarySide>()?;
    module.add_class::<PyDomain>()?;
    module.add_class::<PyFieldRole>()?;
    module.add_class::<PyInitial>()?;
    module.add_class::<PyField>()?;
    module.add_class::<PyParameter>()?;
    module.add_class::<PyPhysicalDomain>()?;
    module.add_class::<PyConservingPort>()?;
    module.add_class::<PyConnection>()?;
    module.add_class::<PyExpression>()?;
    module.add_class::<PyRelation>()?;
    module.add_function(wrap_pyfunction!(derivative, module)?)?;
    module.add_function(wrap_pyfunction!(across, module)?)?;
    module.add_function(wrap_pyfunction!(through, module)?)?;
    module.add_function(wrap_pyfunction!(grad, module)?)?;
    module.add_function(wrap_pyfunction!(div, module)?)?;
    module.add_function(wrap_pyfunction!(trace, module)?)?;
    module.add_function(wrap_pyfunction!(connect, module)?)?;
    Ok(())
}

fn declaration_from_python(value: &Bound<'_, PyAny>) -> PyResult<DraftDeclaration> {
    if let Ok(space) = value.extract::<PyRef<'_, nominal::PyFiniteSpace>>() {
        return Ok(DraftDeclaration::FiniteSpace {
            name: space.name.clone(),
            definition: space.value.clone(),
        });
    }
    if let Ok(set) = value.extract::<PyRef<'_, nominal::PyIndexSet>>() {
        return Ok(DraftDeclaration::IndexSet {
            name: set.name.clone(),
            definition: set.value.clone(),
        });
    }
    if let Ok(domain) = value.extract::<PyRef<'_, PyDomain>>() {
        return Ok(domain.value.clone().into());
    }
    if let Ok(initial) = value.extract::<PyRef<'_, PyInitial>>() {
        return Ok(DraftDeclaration::Initial(initial.residuals.clone()));
    }
    if let Ok(field) = value.extract::<PyRef<'_, PyField>>() {
        return Ok(field.value.clone().into());
    }
    if let Ok(parameter) = value.extract::<PyRef<'_, PyParameter>>() {
        return Ok(parameter.value.clone().into());
    }
    if let Ok(domain) = value.extract::<PyRef<'_, PyPhysicalDomain>>() {
        return Ok(domain.value.clone().into());
    }
    if let Ok(port) = value.extract::<PyRef<'_, PyConservingPort>>() {
        return Ok(port.value.clone().into());
    }
    if let Ok(relation) = value.extract::<PyRef<'_, PyRelation>>() {
        return Ok(relation.value.clone().into());
    }
    if let Ok(connection) = value.extract::<PyRef<'_, PyConnection>>() {
        return Ok(connection.value.clone().into());
    }
    Err(PyTypeError::new_err(
        "Model.define arguments must be model declaration objects",
    ))
}

fn expression_from_python(value: &Bound<'_, PyAny>) -> PyResult<DraftExpression> {
    if let Ok(expression) = value.extract::<PyRef<'_, PyExpression>>() {
        return Ok(expression.value.clone());
    }
    if let Ok(field) = value.extract::<PyRef<'_, PyField>>() {
        return Ok(field.value.expression());
    }
    if let Ok(parameter) = value.extract::<PyRef<'_, PyParameter>>() {
        return Ok(parameter.value.expression());
    }
    if let Ok(value) = value.cast::<PyComplex>() {
        return Ok(DraftExpression::complex(value.real(), value.imag()));
    }
    if value.is_instance_of::<PyBool>() {
        return Err(expression_type_error());
    }
    if value.is_instance_of::<PyInt>() {
        return value_literal::expression(value);
    }
    value
        .extract::<f64>()
        .map_err(|_| expression_type_error())
        .and_then(|value| {
            eqiora::language::DecimalLiteral::from_f64(value)
                .map_err(|error| PyValueError::new_err(error.to_string()))
        })
        .map(DraftExpression::constant)
}

#[derive(Debug, Clone, Copy)]
enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
}

fn binary(
    own: DraftExpression,
    other: &Bound<'_, PyAny>,
    operator: Binary,
    reverse: bool,
) -> PyResult<PyExpression> {
    let other = expression_from_python(other)?;
    let (left, right) = if reverse { (other, own) } else { (own, other) };
    let value = match operator {
        Binary::Add => left + right,
        Binary::Subtract => left - right,
        Binary::Multiply => left * right,
        Binary::Divide => left / right,
    };
    Ok(PyExpression::new(value))
}

fn expression_type_error() -> PyErr {
    PyTypeError::new_err("expected an Expression, Field, Parameter, or real/complex number")
}

fn symbolic_truth_error() -> PyErr {
    PyTypeError::new_err(
        "symbolic Eqiora values have no truth value; construct a Relation explicitly",
    )
}
