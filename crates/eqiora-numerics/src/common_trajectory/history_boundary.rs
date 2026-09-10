//! Check only retained native states; a quadrature stencil is not an interpolant.
use super::*;

pub(super) fn validate(
    history: &AcceptedTimeHistory,
    time: f64,
    values: &[f64],
) -> Result<(), Diagnostic> {
    let known = history
        .steps()
        .get(
            history
                .steps()
                .partition_point(|step| step.end_time() < time),
        )
        .and_then(|step| {
            if time == step.start_time() {
                Some(step.start_state())
            } else if time == step.end_time() {
                Some(step.end_state())
            } else if time == step.start_time() + (step.end_time() - step.start_time()) * 0.5 {
                Some(step.midpoint_state())
            } else {
                None
            }
        });
    if known.is_some_and(|known| known != values) {
        return Err(invalid(
            "ODE output State differs from the retained native history state at the same exact time",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_time::TimeHistoryStep;

    #[test]
    fn known_native_stencils_reject_inconsistent_outputs_without_inventing_interpolation() {
        let history = AcceptedTimeHistory::accepted(
            1,
            vec![TimeHistoryStep::accepted(0.0, 1.0, vec![3.0], vec![2.0], vec![1.0]).unwrap()],
        )
        .unwrap();
        for (time, value) in [(0.0, 3.0), (0.5, 2.0), (1.0, 1.0)] {
            assert!(validate(&history, time, &[value]).is_ok());
            assert!(validate(&history, time, &[value + 1.0]).is_err());
        }
        // The midpoint stencil owns quadrature, not arbitrary dense interpolation.
        assert!(validate(&history, 0.25, &[8.0]).is_ok());
    }
}
