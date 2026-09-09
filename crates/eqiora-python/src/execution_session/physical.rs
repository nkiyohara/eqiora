//! Observation of the reference executor's accepted scalar physical slots.

use eqiora::Id;
use eqiora::entity::kinds;
use eqiora::kernel::KernelNode;
use eqiora::sem::PhysicalUnknown;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use super::PyExecutionSession;

pub(super) fn read(
    session: &PyExecutionSession,
    name: &str,
    across: bool,
) -> PyResult<Option<f64>> {
    let port = session
        .document
        .aliases()
        .get(name)
        .and_then(|id| id.downcast::<kinds::Port>())
        .or_else(|| {
            name.parse::<ulid::Ulid>()
                .ok()
                .map(Id::<kinds::Port>::from_ulid)
        })
        .filter(|id| {
            matches!(session.document.program().node(id.erase()),
            Some(KernelNode::Port(port)) if port.physical_domain().is_some())
        })
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "{name:?} is not an exact scalar physical Port in this Model"
            ))
        })?;
    let unknown = if across {
        PhysicalUnknown::Across(port)
    } else {
        PhysicalUnknown::Through(port)
    };
    Ok(session.value.physical(unknown))
}
