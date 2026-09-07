//! Thin Python adapter to reference sampled sessions and in-memory checkpoints.

use eqiora::api::ModelDocument;
use eqiora::kernel::{KernelNode, SignalDirection};
use eqiora::sem::{ReferenceConfig, SampledSession};
use eqiora::{EntityKind, RawId};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use crate::{diagnostic_error, modeling::value_literal};

#[pyclass(
    name = "SampledSession",
    module = "eqiora._eqiora",
    skip_from_py_object
)]
pub(crate) struct PySampledSession {
    document: ModelDocument,
    value: SampledSession,
}

#[pyclass(
    name = "SampledCheckpoint",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PySampledCheckpoint {
    value: SampledSession,
}

fn resolve(document: &ModelDocument, name: &str, kind: EntityKind) -> PyResult<RawId> {
    document
        .aliases()
        .get(name)
        .copied()
        .filter(|id| id.kind() == kind)
        .ok_or_else(|| {
            PyValueError::new_err(format!("{name:?} is not an exact {kind:?} in this Model"))
        })
}

pub(crate) fn start(
    py: Python<'_>,
    document: &ModelDocument,
    end_time_s: f64,
    max_step_s: f64,
    inputs: &Bound<'_, PyDict>,
) -> PyResult<PySampledSession> {
    let config = ReferenceConfig::new(end_time_s, max_step_s)
        .map_err(|diagnostic| diagnostic_error(py, &[diagnostic]))?;
    let mut tables = Vec::new();
    let mut total = 0usize;
    for (name, table) in inputs.iter() {
        let name: String = name.extract()?;
        let input = resolve(document, &name, EntityKind::Port)?;
        let table = table.cast::<PyTuple>()?;
        if table.len() != 2 {
            return Err(PyTypeError::new_err(
                "input tables must be (clock name, ordered samples)",
            ));
        }
        let clock_name: String = table.get_item(0)?.extract()?;
        let clock = resolve(document, &clock_name, EntityKind::ClockDomain)?;
        let samples = table.get_item(1)?;
        if !(samples.is_instance_of::<PyTuple>() || samples.is_instance_of::<PyList>()) {
            return Err(PyTypeError::new_err(
                "input samples must be an ordered tuple or list",
            ));
        }
        total = total
            .checked_add(samples.len()?)
            .ok_or_else(|| PyValueError::new_err("sample count overflow"))?;
        if total > 1_000_000 {
            return Err(PyValueError::new_err(
                "input samples exceed the million-value bound",
            ));
        }
        let Some(KernelNode::Port(port)) = document.program().node(input) else {
            unreachable!()
        };
        let Some((SignalDirection::Input, value_type)) = port.signal_contract() else {
            return Err(PyValueError::new_err(
                "sample target must be a causal input Port",
            ));
        };
        let values = samples
            .try_iter()?
            .map(|value| value_literal::from_python(&value?, value_type.clone()))
            .collect::<PyResult<Vec<_>>>()?;
        tables.push((input, clock, values));
    }
    let value = py
        .detach(|| document.sampled_session(config, tables))
        .map_err(|diagnostics| diagnostic_error(py, &diagnostics))?;
    Ok(PySampledSession {
        document: document.clone(),
        value,
    })
}

pub(crate) fn resume(
    py: Python<'_>,
    document: &ModelDocument,
    checkpoint: &PySampledCheckpoint,
) -> PyResult<PySampledSession> {
    let value = py
        .detach(|| document.resume_sampled(&checkpoint.value))
        .map_err(|diagnostics| diagnostic_error(py, &diagnostics))?;
    Ok(PySampledSession {
        document: document.clone(),
        value,
    })
}

#[pymethods]
impl PySampledSession {
    fn advance_ticks(&mut self, py: Python<'_>, count: usize) -> PyResult<usize> {
        py.detach(|| self.value.advance_ticks(count))
            .map_err(|diagnostics| diagnostic_error(py, &diagnostics))
    }

    #[getter]
    fn next_tick(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.value
            .next_tick()
            .map(|time| {
                Ok(py
                    .import("fractions")?
                    .getattr("Fraction")?
                    .call1((time.numerator(), time.denominator()))?
                    .unbind())
            })
            .transpose()
    }

    fn checkpoint(&self) -> PySampledCheckpoint {
        PySampledCheckpoint {
            value: self.value.checkpoint(),
        }
    }

    fn field(&self, name: &str) -> PyResult<Option<f64>> {
        let id = resolve(&self.document, name, EntityKind::Field)?;
        Ok(self.value.field(id).map(|value| value.value()))
    }

    fn output(&self, py: Python<'_>, name: &str, tick_index: u64) -> PyResult<Option<Py<PyTuple>>> {
        let id = resolve(&self.document, name, EntityKind::Port)?;
        self.value
            .output(id, tick_index)
            .map(|(time, value)| {
                let exact = py
                    .import("fractions")?
                    .getattr("Fraction")?
                    .call1((time.numerator(), time.denominator()))?
                    .unbind();
                Ok(PyTuple::new(py, [exact, value_literal::to_python(py, value)?])?.unbind())
            })
            .transpose()
    }
}
