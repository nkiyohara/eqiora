//! Notebook presentation lifecycle for an immutable trajectory projection.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBool, PyDict, PyTuple};

use super::PyTrajectory;
use crate::notebook_mime::{TEXT_MIME, WIDGET_MIME, select_mime_types};

const TRAJECTORY_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: this viewer supports only a fixed-mesh 2D Trajectory with one consistent invariant scalar vertex Field.";
const CORRUPT_NOTEBOOK_MESSAGE: &str = "Notebook view unavailable: the installed Eqiora Notebook presentation runtime or assets are incomplete. Reinstall eqiora[notebook].";

pub(super) struct TrajectoryPresentation {
    state: Mutex<PresentationState>,
}

impl Default for TrajectoryPresentation {
    fn default() -> Self {
        Self {
            state: Mutex::new(PresentationState::Empty),
        }
    }
}

enum PresentationState {
    Empty,
    Creating,
    Ready(Py<PyAny>),
}

enum AdapterOutcome {
    Absent,
    Unsupported,
    Rich {
        delegate: Py<PyAny>,
        widget_view: Py<PyAny>,
    },
}

pub(super) fn mimebundle(
    slf: Py<PyTrajectory>,
    py: Python<'_>,
    include: Option<&Bound<'_, PyAny>>,
    exclude: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyDict>> {
    let selected = select_mime_types(py, include, exclude)?;
    let output = PyDict::new(py);
    if selected.is_empty() {
        return Ok(output.unbind());
    }

    let trajectory = slf.get();
    let representation = trajectory.__repr__();
    if !selected.contains(WIDGET_MIME) {
        if selected.contains(TEXT_MIME) {
            output.set_item(TEXT_MIME, representation)?;
        }
        return Ok(output.unbind());
    }

    let coordinates = trajectory.coordinates.numpy(py)?;
    let cells = trajectory.cells.numpy(py)?;
    let states = PyTuple::new(
        py,
        trajectory.states.iter().map(|state| state.clone_ref(py)),
    )?;
    let token = PyDict::new(py);
    token.set_item("geometry_digest", &trajectory.geometry_digest)?;
    token.set_item("correspondence_digest", &trajectory.correspondence_digest)?;
    token.set_item("mesh_digest", &trajectory.mesh_digest)?;
    token.set_item("realization_digest", &trajectory.realization_digest)?;
    token.set_item("run_digest", &trajectory.run_digest)?;
    token.set_item("trajectory_digest", &trajectory.trajectory_digest)?;
    token.set_item("coordinates", coordinates.bind(py))?;
    token.set_item("cells", cells.bind(py))?;
    token.set_item("states", &states)?;

    let current = {
        let mut state = trajectory
            .presentation
            .state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("Trajectory presentation lock is poisoned"))?;
        match std::mem::replace(&mut *state, PresentationState::Creating) {
            PresentationState::Empty => None,
            PresentationState::Ready(delegate) => Some(delegate),
            PresentationState::Creating => {
                if selected.contains(TEXT_MIME) {
                    output.set_item(
                        TEXT_MIME,
                        format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                    )?;
                }
                return Ok(output.unbind());
            }
        }
    };

    match call_adapter(py, slf.bind(py), &token, current.as_ref()) {
        Ok(AdapterOutcome::Absent) => {
            trajectory.presentation.set(PresentationState::Empty)?;
            if selected.contains(TEXT_MIME) {
                output.set_item(TEXT_MIME, representation)?;
            }
        }
        Ok(AdapterOutcome::Unsupported) => {
            if let Some(delegate) = current {
                close_delegate(py, &delegate);
            }
            trajectory.presentation.set(PresentationState::Empty)?;
            if selected.contains(TEXT_MIME) {
                output.set_item(
                    TEXT_MIME,
                    format!("{representation}\n{TRAJECTORY_NOTEBOOK_MESSAGE}"),
                )?;
            }
        }
        Ok(AdapterOutcome::Rich {
            delegate,
            widget_view,
        }) => {
            trajectory
                .presentation
                .set(PresentationState::Ready(delegate))?;
            if selected.contains(TEXT_MIME) {
                output.set_item(TEXT_MIME, representation)?;
            }
            output.set_item(WIDGET_MIME, widget_view)?;
        }
        Err(delegate) => {
            if let Some(delegate) = delegate.or(current) {
                close_delegate(py, &delegate);
            }
            trajectory.presentation.set(PresentationState::Empty)?;
            if selected.contains(TEXT_MIME) {
                output.set_item(
                    TEXT_MIME,
                    format!("{representation}\n{CORRUPT_NOTEBOOK_MESSAGE}"),
                )?;
            }
        }
    }
    Ok(output.unbind())
}

impl TrajectoryPresentation {
    fn set(&self, next: PresentationState) -> PyResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("Trajectory presentation lock is poisoned"))?;
        *state = next;
        Ok(())
    }
}

fn call_adapter(
    py: Python<'_>,
    trajectory: &Bound<'_, PyTrajectory>,
    token: &Bound<'_, PyDict>,
    current: Option<&Py<PyAny>>,
) -> Result<AdapterOutcome, Option<Py<PyAny>>> {
    let module = py.import("eqiora._presentation").map_err(|_| None)?;
    let adapter = module.getattr("trajectory_mimebundle").map_err(|_| None)?;
    let current = current.map_or_else(|| py.None(), |value| value.clone_ref(py));
    let result = adapter
        .call1((trajectory, token, current))
        .map_err(|_| None)?;
    let tuple = result.cast::<PyTuple>().map_err(|_| None)?;
    if tuple.len() != 3 {
        return Err(tuple.get_item(1).ok().map(Bound::unbind));
    }
    let status = tuple
        .get_item(0)
        .and_then(|value| value.extract::<String>())
        .map_err(|_| tuple.get_item(1).ok().map(Bound::unbind))?;
    if status == "absent"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Absent);
    }
    if status == "unsupported"
        && tuple.get_item(1).is_ok_and(|value| value.is_none())
        && tuple.get_item(2).is_ok_and(|value| value.is_none())
    {
        return Ok(AdapterOutcome::Unsupported);
    }
    if status != "rich" {
        return Err(tuple
            .get_item(1)
            .ok()
            .and_then(|value| (!value.is_none()).then(|| value.unbind())));
    }
    let delegate = tuple.get_item(1).map_err(|_| None)?;
    if delegate.is_none() {
        return Err(None);
    }
    let delegate = delegate.unbind();
    let hook_result = tuple
        .get_item(2)
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let hook_tuple = hook_result
        .cast::<PyTuple>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if hook_tuple.len() != 2
        || !hook_tuple
            .get_item(1)
            .is_ok_and(|value| value.is_instance_of::<PyDict>())
    {
        return Err(Some(delegate));
    }
    let data = hook_tuple
        .get_item(0)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .cast_into::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    let widget_view = data
        .get_item(WIDGET_MIME)
        .map_err(|_| Some(delegate.clone_ref(py)))?
        .ok_or_else(|| Some(delegate.clone_ref(py)))?;
    let widget = widget_view
        .cast::<PyDict>()
        .map_err(|_| Some(delegate.clone_ref(py)))?;
    if widget.len() != 3
        || widget
            .get_item("version_major")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(2)
        || widget
            .get_item("version_minor")
            .ok()
            .flatten()
            .and_then(exact_u8)
            != Some(0)
        || widget
            .get_item("model_id")
            .ok()
            .flatten()
            .and_then(|value| value.extract::<String>().ok())
            .is_none_or(|model_id| model_id.is_empty())
    {
        return Err(Some(delegate));
    }
    Ok(AdapterOutcome::Rich {
        delegate,
        widget_view: widget_view.unbind(),
    })
}

fn close_delegate(py: Python<'_>, delegate: &Py<PyAny>) {
    let _ = delegate.bind(py).call_method0("close");
}

fn exact_u8(value: Bound<'_, PyAny>) -> Option<u8> {
    if value.is_instance_of::<PyBool>() {
        None
    } else {
        value.extract::<u8>().ok()
    }
}
