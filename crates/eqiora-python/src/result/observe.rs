//! Typed Python projections delegate evaluation and tangent admission to CommonResult.
use super::*;
use crate::model::PyObservableRef;
use crate::modeling::{PyDimension, PyValueType};
use eqiora::graph::Op;
use eqiora::kernel::{KernelNode, ObservableMeasure, ObservableReduction};
use eqiora::meshing::{MeshTopology, QuadratureRule};
use eqiora::{DynQuantity, Id, ValueLiteral, kinds};
use pyo3::types::PyDict;

/// Typed Result-owned value with its effective numerical quadrature.
#[pyclass(
    name = "Observation",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyObservation {
    value: ValueLiteral,
    #[pyo3(get)]
    result_identity: String,
    #[pyo3(get)]
    observable_id: String,
    #[pyo3(get)]
    evaluation_kind: &'static str,
    #[pyo3(get)]
    quadrature_points: Option<usize>,
    #[pyo3(get)]
    quadrature_dimension: Option<usize>,
}

#[pymethods]
impl PyObservation {
    fn __repr__(&self) -> String {
        format!(
            "Observation(observable_id={:?}, evaluation_kind={:?}, result_identity={:?})",
            self.observable_id, self.evaluation_kind, self.result_identity
        )
    }

    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        crate::modeling::value_literal::to_python(py, &self.value)
    }
    #[getter]
    fn value_type(&self) -> PyValueType {
        PyValueType {
            value: self.value.value_type().clone(),
        }
    }
    #[getter]
    fn quadrature(&self) -> Option<&'static str> {
        self.quadrature_dimension.map(|dimension| {
            if dimension == 0 {
                "Point"
            } else {
                "GaussLegendre"
            }
        })
    }
}

/// Dimensioned Field coefficient variation bound to one exact accepted Result.
#[pyclass(
    name = "ObservableStateTangent",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyObservableStateTangent {
    native: eqiora_numerics::CommonObservableStateTangent,
    #[pyo3(get)]
    result_identity: String,
}

#[pymethods]
impl PyObservableStateTangent {
    fn __repr__(&self) -> String {
        format!(
            "ObservableStateTangent(result_identity={:?})",
            self.result_identity
        )
    }
}

impl PyRunResult {
    fn observation_rule(
        &self,
        py: Python<'_>,
        observable: &PyObservableRef,
        points: Option<usize>,
    ) -> PyResult<Option<QuadratureRule>> {
        if observable.model_digest != self.identity.model_digest() {
            return Err(PyValueError::new_err(
                "ObservableRef belongs to a different exact Model artifact",
            ));
        }
        let (transaction, _) = self
            .native
            .plan()
            .model_artifact()
            .to_transaction()
            .map_err(|errors| diagnostic_error(py, &errors))?;
        let definition = transaction
            .ops()
            .iter()
            .find_map(|operation| match operation {
                Op::DefineKernelNode {
                    node: KernelNode::Observable(definition),
                } if definition.id() == observable.id => Some(definition),
                _ => None,
            })
            .ok_or_else(|| PyKeyError::new_err("Observable is outside this exact Result Model"))?;
        let Some(points) = points else {
            return Ok(None);
        };
        let ObservableReduction::SpatialIntegral { measure, .. } = definition.reduction() else {
            return Err(PyValueError::new_err(
                "finite Observable does not accept spatial quadrature",
            ));
        };
        let owner = self.native.plan().authenticated_mesh().ok_or_else(|| {
            PyValueError::new_err("Observable requires an authenticated Result mesh")
        })?;
        let mesh = owner.cartesian_mesh().ok_or_else(|| {
            PyValueError::new_err(
                "Observable quadrature requires an admitted Cartesian Result mesh",
            )
        })?;
        let dimension = mesh
            .mesh()
            .topological_dimension()
            .checked_sub(usize::from(measure == ObservableMeasure::Boundary))
            .ok_or_else(|| PyValueError::new_err("Observable boundary has no ambient dimension"))?;
        if dimension == 0 {
            if points != 1 {
                return Err(PyValueError::new_err(
                    "point measure requires quadrature_points=1",
                ));
            }
            return Ok(Some(QuadratureRule::point()));
        }
        QuadratureRule::tensor_product_gauss_legendre(dimension, points)
            .map(Some)
            .map_err(|error| diagnostic_error(py, &[error]))
    }

    pub(super) fn observe_value(
        &self,
        py: Python<'_>,
        observable: &PyObservableRef,
        points: Option<usize>,
    ) -> PyResult<PyObservation> {
        let rule = self.observation_rule(py, observable, points)?;
        let value = self
            .native
            .observe(
                self.native.plan().model_artifact(),
                observable.id,
                rule.as_ref(),
            )
            .map_err(|error| diagnostic_error(py, &[error]))?;
        Ok(PyObservation {
            value: value.value().clone(),
            result_identity: value.result_identity().to_owned(),
            observable_id: value.observable().ulid().to_string(),
            evaluation_kind: "value",
            quadrature_points: points,
            quadrature_dimension: value
                .quadrature()
                .map(|rule| rule.reference_cell().dimension()),
        })
    }

    pub(super) fn bind_observable_tangent(
        &self,
        py: Python<'_>,
        directions: &Bound<'_, PyDict>,
    ) -> PyResult<PyObservableStateTangent> {
        let mut fields = Vec::with_capacity(directions.len());
        for (field, direction) in directions.iter() {
            let field = field.extract::<PyRef<'_, PyModelFieldRef>>()?;
            if field.exact_model_digest() != self.identity.model_digest() {
                return Err(PyValueError::new_err(
                    "FieldRef belongs to a different exact Model artifact",
                ));
            }
            let (dimension, values) = direction.extract::<(PyRef<'_, PyDimension>, Vec<f64>)>()?;
            let id = Id::<kinds::Field>::from_ulid(
                field
                    .exact_id()
                    .parse()
                    .map_err(|_| PyValueError::new_err("FieldRef has an invalid exact ID"))?,
            );
            fields.push((
                id,
                values
                    .into_iter()
                    .map(|value| DynQuantity::new(value, dimension.native()))
                    .collect(),
            ));
        }
        let native = self
            .native
            .observable_state_tangent(fields)
            .map_err(|error| diagnostic_error(py, &[error]))?;
        Ok(PyObservableStateTangent {
            native,
            result_identity: self.native.identity().to_owned(),
        })
    }

    pub(super) fn observe_jvp(
        &self,
        py: Python<'_>,
        observable: &PyObservableRef,
        tangent: &PyObservableStateTangent,
        points: usize,
    ) -> PyResult<PyObservation> {
        let rule = self
            .observation_rule(py, observable, Some(points))?
            .expect("explicit spatial quadrature");
        let value = self
            .native
            .observe_state_jvp(
                self.native.plan().model_artifact(),
                observable.id,
                &rule,
                &tangent.native,
            )
            .map_err(|error| diagnostic_error(py, &[error]))?;
        Ok(PyObservation {
            value,
            result_identity: self.native.identity().to_owned(),
            observable_id: observable.id.ulid().to_string(),
            evaluation_kind: "state-jvp",
            quadrature_points: Some(points),
            quadrature_dimension: Some(rule.reference_cell().dimension()),
        })
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyObservation>()?;
    module.add_class::<PyObservableStateTangent>()?;
    Ok(())
}
