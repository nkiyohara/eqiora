//! Process-local collection and Python presentation for Eqiora tracing spans.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

pub(crate) const TELEMETRY_TARGET: &str = "eqiora::execution";

#[derive(Debug, Clone)]
pub(crate) struct ProfilePhaseData {
    pub(crate) path: Vec<String>,
    pub(crate) count: usize,
    pub(crate) duration: Duration,
}

#[derive(Debug, Clone)]
pub(crate) struct ProfileEventData {
    pub(crate) path: Vec<String>,
    pub(crate) fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProfileData {
    pub(crate) phases: Vec<ProfilePhaseData>,
    pub(crate) events: Vec<ProfileEventData>,
}

#[derive(Default)]
struct FieldVisitor(BTreeMap<String, String>);

impl Visit for FieldVisitor {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.insert(field.name().to_owned(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_owned(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name().to_owned(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}

struct OpenSpan {
    path: Vec<String>,
    entered: Option<Instant>,
    duration: Duration,
}

#[derive(Default)]
struct CollectorState {
    open: HashMap<u64, OpenSpan>,
    phases: BTreeMap<Vec<String>, (usize, Duration)>,
    phase_order: Vec<Vec<String>>,
    events: Vec<ProfileEventData>,
}

#[derive(Clone, Default)]
pub(crate) struct ProfileCollector {
    state: Arc<Mutex<CollectorState>>,
}

impl ProfileCollector {
    pub(crate) fn capture<T>(&self, operation: impl FnOnce() -> T) -> T {
        let subscriber = Registry::default().with(self.clone());
        tracing::subscriber::with_default(subscriber, operation)
    }

    pub(crate) fn finish(&self) -> ProfileData {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ProfileData {
            phases: state
                .phase_order
                .iter()
                .filter_map(|path| {
                    let (count, duration) = state.phases.get(path)?;
                    Some(ProfilePhaseData {
                        path: path.clone(),
                        count: *count,
                        duration: *duration,
                    })
                })
                .collect(),
            events: state.events.clone(),
        }
    }
}

impl<S> Layer<S> for ProfileCollector
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().target() != TELEMETRY_TARGET {
            return;
        }
        let mut visitor = FieldVisitor::default();
        attrs.record(&mut visitor);
        let phase = visitor
            .0
            .remove("phase")
            .unwrap_or_else(|| attrs.metadata().name().to_owned());
        let parent = attrs
            .parent()
            .cloned()
            .or_else(|| ctx.current_span().id().cloned());
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut path = parent
            .and_then(|parent| {
                state
                    .open
                    .get(&parent.into_u64())
                    .map(|span| span.path.clone())
            })
            .unwrap_or_default();
        path.push(phase.clone());
        if !state.phase_order.contains(&path) {
            state.phase_order.push(path.clone());
        }
        visitor.0.insert("phase".to_owned(), phase);
        visitor.0.insert("event".to_owned(), "phase".to_owned());
        state.events.push(ProfileEventData {
            path: path.clone(),
            fields: visitor.0,
        });
        state.open.insert(
            id.into_u64(),
            OpenSpan {
                path,
                entered: None,
                duration: Duration::ZERO,
            },
        );
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(span) = state.open.get_mut(&id.into_u64()) {
            span.entered = Some(Instant::now());
        }
    }

    fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(span) = state.open.get_mut(&id.into_u64())
            && let Some(entered) = span.entered.take()
        {
            span.duration += entered.elapsed();
        }
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(span) = state.open.remove(&id.into_u64()) {
            let aggregate = state.phases.entry(span.path).or_default();
            aggregate.0 += 1;
            aggregate.1 += span.duration;
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if event.metadata().target() != TELEMETRY_TARGET {
            return;
        }
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = ctx
            .current_span()
            .id()
            .and_then(|id| state.open.get(&id.into_u64()).map(|span| span.path.clone()))
            .unwrap_or_default();
        state.events.push(ProfileEventData {
            path,
            fields: visitor.0,
        });
    }
}

#[pyclass(
    name = "ProfilePhase",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
/// Aggregate timing for one hierarchical execution phase.
pub(crate) struct PyProfilePhase(ProfilePhaseData);

#[pymethods]
impl PyProfilePhase {
    #[getter]
    fn path(&self) -> Vec<String> {
        self.0.path.clone()
    }

    #[getter]
    fn name(&self) -> &str {
        self.0.path.last().map_or("", String::as_str)
    }

    #[getter]
    const fn count(&self) -> usize {
        self.0.count
    }

    #[getter]
    fn total_seconds(&self) -> f64 {
        self.0.duration.as_secs_f64()
    }

    fn __repr__(&self) -> String {
        format!(
            "ProfilePhase(name={:?}, count={}, total_seconds={:.6})",
            self.name(),
            self.count(),
            self.total_seconds()
        )
    }
}

#[pyclass(
    name = "ProfileEvent",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
/// Structured metadata for one phase or solver observation.
pub(crate) struct PyProfileEvent(ProfileEventData);

#[pymethods]
impl PyProfileEvent {
    #[getter]
    fn path(&self) -> Vec<String> {
        self.0.path.clone()
    }

    #[getter]
    fn fields<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let fields = PyDict::new(py);
        for (name, value) in &self.0.fields {
            fields.set_item(name, value)?;
        }
        Ok(fields)
    }

    fn __repr__(&self) -> String {
        format!(
            "ProfileEvent(path={:?}, fields={:?})",
            self.0.path, self.0.fields
        )
    }
}

/// Process-local phase timings and structured solver events for one Run.
#[pyclass(name = "Profile", module = "eqiora._eqiora", frozen)]
pub(crate) struct PyProfile {
    data: ProfileData,
}

impl PyProfile {
    pub(crate) fn new(data: ProfileData) -> Self {
        Self { data }
    }
}

#[pymethods]
impl PyProfile {
    #[getter]
    fn phases(&self, py: Python<'_>) -> PyResult<Vec<Py<PyProfilePhase>>> {
        self.data
            .phases
            .iter()
            .cloned()
            .map(|phase| Py::new(py, PyProfilePhase(phase)))
            .collect()
    }

    #[getter]
    fn events(&self, py: Python<'_>) -> PyResult<Vec<Py<PyProfileEvent>>> {
        self.data
            .events
            .iter()
            .cloned()
            .map(|event| Py::new(py, PyProfileEvent(event)))
            .collect()
    }

    #[getter]
    fn total_seconds(&self) -> f64 {
        self.data
            .phases
            .iter()
            .find(|phase| phase.path.as_slice() == ["run"])
            .map_or(0.0, |phase| phase.duration.as_secs_f64())
    }

    fn summary(&self) -> String {
        let mut output = String::from("Eqiora run\n");
        for phase in &self.data.phases {
            let indent = "  ".repeat(phase.path.len().saturating_sub(1));
            let name = phase.path.last().map_or("", String::as_str);
            output.push_str(&format!(
                "{indent}{name:<28} {:>10.6} s × {}\n",
                phase.duration.as_secs_f64(),
                phase.count,
            ));
        }
        output
    }

    fn __repr__(&self) -> String {
        format!(
            "Profile(phases={}, events={}, total_seconds={:.6})",
            self.data.phases.len(),
            self.data.events.len(),
            self.total_seconds(),
        )
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyProfilePhase>()?;
    module.add_class::<PyProfileEvent>()?;
    module.add_class::<PyProfile>()?;
    Ok(())
}
