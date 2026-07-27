//! Immutable Python declarations over the client-neutral Rust model draft.

use eqiora::DimExponents;
use eqiora::api::ModelDocument;
use eqiora::compatibility::ExactModelCodec;
use eqiora::language::{
    DraftBoundarySide, DraftConservingConnection, DraftConservingPort, DraftDeclaration,
    DraftExpression, DraftField, DraftParameter, DraftPhysicalDomain, DraftRelation,
    DraftRepresentation, DraftSpatialDomain, ModelDraft,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pyo3::exceptions::{PyAttributeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyModule, PyTuple};

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

impl From<PyBoundarySide> for DraftBoundarySide {
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
            DraftBoundarySide::Lower => PyBoundarySide::Lower,
            DraftBoundarySide::Upper => PyBoundarySide::Upper,
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

/// Immutable continuum Representation declaration.
#[pyclass(
    name = "Representation",
    module = "eqiora._eqiora",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PyRepresentation {
    value: DraftRepresentation,
}

#[pymethods]
impl PyRepresentation {
    /// Construct one continuous pre-discretization Representation.
    #[staticmethod]
    fn continuum(name: String) -> Self {
        Self {
            value: DraftRepresentation::continuum(name),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    fn __repr__(&self) -> String {
        format!("Representation.continuum({:?})", self.name())
    }
}

#[pymethods]
impl PyDimension {
    #[new]
    #[pyo3(signature = (*, mass=0, length=0, time=0, current=0, temperature=0, amount=0, luminous_intensity=0))]
    #[allow(clippy::too_many_arguments)]
    const fn new(
        mass: i8,
        length: i8,
        time: i8,
        current: i8,
        temperature: i8,
        amount: i8,
        luminous_intensity: i8,
    ) -> Self {
        Self {
            value: DimExponents {
                mass,
                length,
                time,
                current,
                temperature,
                amount,
                luminous_intensity,
            },
        }
    }

    #[getter]
    const fn exponents(&self) -> (i8, i8, i8, i8, i8, i8, i8) {
        let value = self.value;
        (
            value.mass,
            value.length,
            value.time,
            value.current,
            value.temperature,
            value.amount,
            value.luminous_intensity,
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| self.value == other.value)
    }

    /// Consistent with `__eq__`, so equal dimensions collide as dict keys.
    ///
    /// Defining `__eq__` without this makes the type unhashable, which is a
    /// silent loss: `{Dimension.LENGTH: metres}` stops working for a value type
    /// whose whole purpose is to be compared.
    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.value.hash(&mut hasher);
        hasher.finish()
    }

    fn __ne__(&self, other: &Bound<'_, PyAny>) -> bool {
        !self.__eq__(other)
    }

    fn __repr__(&self) -> String {
        let (mass, length, time, current, temperature, amount, luminous_intensity) =
            self.exponents();
        format!(
            "Dimension(mass={mass}, length={length}, time={time}, current={current}, \
             temperature={temperature}, amount={amount}, luminous_intensity={luminous_intensity})"
        )
    }
}

/// Immutable scalar Field declaration.
#[pyclass(name = "Field", module = "eqiora._eqiora", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub(crate) struct PyField {
    value: DraftField,
}

#[pymethods]
impl PyField {
    #[new]
    #[pyo3(signature = (name, *, domain=None, representation=None, dimension=None, initial=0.0))]
    fn new(
        name: String,
        domain: Option<&PyDomain>,
        representation: Option<&PyRepresentation>,
        dimension: Option<&PyDimension>,
        initial: f64,
    ) -> PyResult<Self> {
        let dimension = dimension.map_or(DimExponents::DIMENSIONLESS, |value| value.value);
        let value = match (domain, representation) {
            (None, None) => DraftField::new(name, dimension, initial),
            (Some(domain), Some(representation)) => DraftField::spatial_scalar(
                name,
                &domain.value,
                &representation.value,
                dimension,
                initial,
            ),
            _ => {
                return Err(PyTypeError::new_err(
                    "a spatial Field requires both domain= and representation=",
                ));
            }
        };
        Ok(Self { value })
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
    const fn initial(&self) -> f64 {
        self.value.initial()
    }

    #[getter]
    fn domain(&self) -> Option<PyDomain> {
        self.value.domain().map(|domain| PyDomain {
            value: domain.clone(),
        })
    }

    #[getter]
    fn representation(&self) -> Option<PyRepresentation> {
        self.value
            .representation()
            .map(|representation| PyRepresentation {
                value: representation.clone(),
            })
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
        match (self.value.domain(), self.value.representation()) {
            (Some(domain), Some(representation)) => format!(
                "Field({:?}, domain={:?}, representation={:?}, dimension={:?}, initial={:?})",
                self.name(),
                domain.name(),
                representation.name(),
                self.dimension().exponents(),
                self.initial()
            ),
            _ => format!(
                "Field({:?}, dimension={:?}, initial={:?})",
                self.name(),
                self.dimension().exponents(),
                self.initial()
            ),
        }
    }
}

/// Immutable scalar Parameter declaration.
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
    #[pyo3(signature = (name, *, dimension=None, value))]
    fn new(name: String, dimension: Option<&PyDimension>, value: f64) -> Self {
        Self {
            value: DraftParameter::new(
                name,
                dimension.map_or(DimExponents::DIMENSIONLESS, |value| value.value),
                value,
            ),
        }
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
    const fn value(&self) -> f64 {
        self.value.value()
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
            "Parameter({:?}, dimension={:?}, value={:?})",
            self.name(),
            self.dimension().exponents(),
            self.value()
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
    #[pyo3(signature = (name, *, across_dimension, through_dimension))]
    fn new(name: String, across_dimension: &PyDimension, through_dimension: &PyDimension) -> Self {
        Self {
            value: DraftPhysicalDomain::new(name, across_dimension.value, through_dimension.value),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.value.name()
    }

    #[getter]
    const fn across_dimension(&self) -> PyDimension {
        PyDimension {
            value: self.value.across_dimension(),
        }
    }

    #[getter]
    const fn through_dimension(&self) -> PyDimension {
        PyDimension {
            value: self.value.through_dimension(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PhysicalDomain({:?}, across_dimension={:?}, through_dimension={:?})",
            self.name(),
            self.across_dimension().exponents(),
            self.through_dimension().exponents()
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

pub(crate) fn define_model_exact(
    py: Python<'_>,
    name: String,
    declarations: &Bound<'_, PyTuple>,
    codec: ExactModelCodec,
) -> PyResult<ModelDocument> {
    let draft = model_draft(py, name, declarations)?;
    py.detach(move || codec.define(&draft))
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
    module.add_class::<PyBoundarySide>()?;
    module.add_class::<PyDomain>()?;
    module.add_class::<PyRepresentation>()?;
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
    if let Ok(domain) = value.extract::<PyRef<'_, PyDomain>>() {
        return Ok(domain.value.clone().into());
    }
    if let Ok(representation) = value.extract::<PyRef<'_, PyRepresentation>>() {
        return Ok(representation.value.clone().into());
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
    if value.is_instance_of::<PyBool>() {
        return Err(expression_type_error());
    }
    value
        .extract::<f64>()
        .map(DraftExpression::constant)
        .map_err(|_| expression_type_error())
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
    PyTypeError::new_err("expected an Expression, Field, Parameter, or real number")
}

fn symbolic_truth_error() -> PyErr {
    PyTypeError::new_err(
        "symbolic Eqiora values have no truth value; construct a Relation explicitly",
    )
}
