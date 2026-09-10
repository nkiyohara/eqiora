//! Owned native stencils from accepted steps, independent of output sampling.
use crate::diagnostic::time_solve_failed;
use eqiora_core::Diagnostic;

/// One accepted interval with its solver-native midpoint value.
///
/// These three states are a quadrature stencil, not a general dense interpolant.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeHistoryStep {
    start_time: f64,
    end_time: f64,
    start_state: Vec<f64>,
    midpoint_state: Vec<f64>,
    end_state: Vec<f64>,
}

impl TimeHistoryStep {
    /// Accept a finite, consistently shaped native step stencil.
    /// # Errors
    /// Rejects empty or inconsistent state shapes and non-increasing times.
    pub fn accepted(
        start_time: f64,
        end_time: f64,
        start_state: Vec<f64>,
        midpoint_state: Vec<f64>,
        end_state: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if !start_time.is_finite()
            || !end_time.is_finite()
            || start_time >= end_time
            || start_state.is_empty()
            || start_state.len() != midpoint_state.len()
            || start_state.len() != end_state.len()
            || start_state
                .iter()
                .chain(&midpoint_state)
                .chain(&end_state)
                .any(|value| !value.is_finite())
        {
            return Err(time_solve_failed(
                "accepted time step has invalid interval, state shape, or values",
            ));
        }
        Ok(Self {
            start_time,
            end_time,
            start_state,
            midpoint_state,
            end_state,
        })
    }
    /// Exact beginning of the accepted step.
    #[must_use]
    pub const fn start_time(&self) -> f64 {
        self.start_time
    }
    /// Exact end of the accepted step.
    #[must_use]
    pub const fn end_time(&self) -> f64 {
        self.end_time
    }
    /// State at the start of the smooth step.
    #[must_use]
    pub fn start_state(&self) -> &[f64] {
        &self.start_state
    }
    /// State evaluated by native dense output at the interval midpoint.
    #[must_use]
    pub fn midpoint_state(&self) -> &[f64] {
        &self.midpoint_state
    }
    /// State at the end of the smooth step.
    #[must_use]
    pub fn end_state(&self) -> &[f64] {
        &self.end_state
    }
}

/// Cadence-independent accepted intervals for a smooth trajectory.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedTimeHistory {
    dimension: usize,
    steps: Vec<TimeHistoryStep>,
}
impl AcceptedTimeHistory {
    /// Accept nonempty contiguous intervals with exact continuous state joins.
    /// # Errors
    /// Rejects inconsistent dimensions, gaps, overlaps, and reset discontinuities.
    pub fn accepted(dimension: usize, steps: Vec<TimeHistoryStep>) -> Result<Self, Diagnostic> {
        if dimension == 0
            || steps.is_empty()
            || steps.iter().any(|step| step.start_state.len() != dimension)
        {
            return Err(time_solve_failed("accepted history has invalid shape"));
        }
        for pair in steps.windows(2) {
            if pair[0].end_time != pair[1].start_time {
                return Err(time_solve_failed(
                    "accepted history intervals are not contiguous",
                ));
            }
            if pair[0].end_state != pair[1].start_state {
                return Err(time_solve_failed(
                    "smooth accepted history cannot contain a reset discontinuity",
                ));
            }
        }
        Ok(Self { dimension, steps })
    }
    /// Number of scalar coordinates in every stencil state.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }
    /// Native accepted intervals in execution order.
    #[must_use]
    pub fn steps(&self) -> &[TimeHistoryStep] {
        &self.steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(start: f64, end: f64, left: f64, right: f64) -> TimeHistoryStep {
        TimeHistoryStep::accepted(
            start,
            end,
            vec![left],
            vec![(left + right) * 0.5],
            vec![right],
        )
        .unwrap()
    }

    #[test]
    fn smooth_history_requires_exact_continuous_joins() {
        let smooth = vec![step(0.0, 1.0, 0.0, 1.0), step(1.0, 1.5, 1.0, 1.5)];
        assert!(AcceptedTimeHistory::accepted(1, smooth).is_ok());
        // x'=1 with a reset to zero at t=1 is outside smooth history.
        let reset = vec![step(0.0, 1.0, 0.0, 1.0), step(1.0, 1.5, 0.0, 0.5)];
        assert!(AcceptedTimeHistory::accepted(1, reset).is_err());
        let gap = vec![step(0.0, 0.5, 0.0, 0.5), step(0.6, 1.0, 0.5, 1.0)];
        assert!(AcceptedTimeHistory::accepted(1, gap).is_err());
        assert!(AcceptedTimeHistory::accepted(2, vec![step(0.0, 1.0, 0.0, 1.0)]).is_err());
        assert!(AcceptedTimeHistory::accepted(1, vec![]).is_err());
    }
}
