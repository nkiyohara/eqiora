//! Pure event-calendar primitives for the reference interpreter.

use std::collections::BTreeSet;

use eqiora_core::RawId;
use eqiora_schema::kernel::{EventDirection, ExprDag};

#[derive(Debug, Clone)]
pub(crate) struct EventTask {
    pub(crate) activation: RawId,
    pub(crate) relations: BTreeSet<RawId>,
    pub(crate) guard: ExprDag,
    pub(crate) direction: EventDirection,
}

/// A crossing is a transition through zero from a strictly armed side.
///
/// Requiring the pre-step guard to lie outside the zero band prevents an
/// event reset onto its own guard surface from firing again without first
/// departing and returning.
pub(crate) fn crosses(
    direction: EventDirection,
    before: f64,
    after: f64,
    guard_tolerance: f64,
) -> bool {
    match direction {
        EventDirection::Any => {
            (before > guard_tolerance && after <= guard_tolerance)
                || (before < -guard_tolerance && after >= -guard_tolerance)
        }
        EventDirection::Rising => before < -guard_tolerance && after >= -guard_tolerance,
        EventDirection::Falling => before > guard_tolerance && after <= guard_tolerance,
    }
}

pub(crate) fn root_is_left_of(
    direction: EventDirection,
    left_guard: f64,
    midpoint_guard: f64,
    guard_tolerance: f64,
) -> bool {
    crosses(direction, left_guard, midpoint_guard, guard_tolerance)
}

pub(crate) fn same_instant(left: f64, right: f64, tolerance: f64) -> bool {
    (left - right).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_filter_requires_an_armed_side() {
        assert!(crosses(EventDirection::Falling, 1.0, -1.0, 1.0e-9));
        assert!(!crosses(EventDirection::Rising, 1.0, -1.0, 1.0e-9));
        assert!(crosses(EventDirection::Rising, -1.0, 0.0, 1.0e-9));
        assert!(crosses(EventDirection::Any, 1.0, -1.0, 1.0e-9));
        assert!(crosses(EventDirection::Any, -1.0, 1.0, 1.0e-9));
        assert!(!crosses(EventDirection::Any, 0.0, 1.0, 1.0e-9));
        assert!(!crosses(EventDirection::Any, 0.0, 0.0, 1.0e-9));
    }
}
