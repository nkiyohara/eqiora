//! Lowered hybrid-event linearization and sensitivity contracts.

use crate::diagnostic::invalid_sensitivity;
use eqiora_core::Diagnostic;

/// Vector fields immediately before and after one canonical event reset.
#[derive(Debug, Clone, PartialEq)]
pub struct EventFlowLinearization {
    before: Vec<f64>,
    after: Vec<f64>,
}

impl EventFlowLinearization {
    /// Bind two finite vector fields in the same state-coordinate order.
    ///
    /// # Errors
    /// Returns `EQ0704` for an empty, mismatched, or non-finite pair.
    pub fn new(before: Vec<f64>, after: Vec<f64>) -> Result<Self, Diagnostic> {
        if before.is_empty()
            || before.len() != after.len()
            || before.iter().chain(&after).any(|value| !value.is_finite())
        {
            return Err(invalid_sensitivity(
                "event flow linearization requires equally shaped finite vectors",
            ));
        }
        Ok(Self { before, after })
    }

    /// Scalar state dimension.
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.before.len()
    }

    /// Pre-event vector field `f^-`.
    #[must_use]
    pub fn before(&self) -> &[f64] {
        &self.before
    }

    /// Post-event vector field `f^+`.
    #[must_use]
    pub fn after(&self) -> &[f64] {
        &self.after
    }
}

/// First derivatives of one scalar event guard `g(t, y, p)`.
#[derive(Debug, Clone, PartialEq)]
pub struct EventGuardLinearization {
    state_gradient: Vec<f64>,
    parameter_gradient: Vec<f64>,
    time_derivative: f64,
}

impl EventGuardLinearization {
    /// Bind finite guard derivatives in explicit state/Parameter order.
    ///
    /// # Errors
    /// Returns `EQ0704` for an empty state gradient or non-finite derivative.
    pub fn new(
        state_gradient: Vec<f64>,
        parameter_gradient: Vec<f64>,
        time_derivative: f64,
    ) -> Result<Self, Diagnostic> {
        if state_gradient.is_empty()
            || !time_derivative.is_finite()
            || state_gradient
                .iter()
                .chain(&parameter_gradient)
                .any(|value| !value.is_finite())
        {
            return Err(invalid_sensitivity(
                "event guard linearization requires finite derivatives and a non-empty state gradient",
            ));
        }
        Ok(Self {
            state_gradient,
            parameter_gradient,
            time_derivative,
        })
    }

    /// Guard gradient `g_y` in state-coordinate order.
    #[must_use]
    pub fn state_gradient(&self) -> &[f64] {
        &self.state_gradient
    }

    /// Direct guard derivative `g_p` in Parameter-coordinate order.
    #[must_use]
    pub fn parameter_gradient(&self) -> &[f64] {
        &self.parameter_gradient
    }

    /// Explicit model-time derivative `g_t`.
    #[must_use]
    pub const fn time_derivative(&self) -> f64 {
        self.time_derivative
    }
}

/// First derivatives of one reset map `y^+ = rho(t, y^-, p)`.
#[derive(Debug, Clone, PartialEq)]
pub struct EventResetLinearization {
    state_dimension: usize,
    parameter_dimension: usize,
    state_jacobian: Vec<f64>,
    parameter_jacobian: Vec<f64>,
    time_derivative: Vec<f64>,
}

impl EventResetLinearization {
    /// Bind complete row-major reset derivatives.
    ///
    /// # Errors
    /// Returns `EQ0704` for inconsistent shapes or non-finite values.
    pub fn new(
        state_dimension: usize,
        parameter_dimension: usize,
        state_jacobian: Vec<f64>,
        parameter_jacobian: Vec<f64>,
        time_derivative: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let state_entries = state_dimension.checked_mul(state_dimension);
        let parameter_entries = state_dimension.checked_mul(parameter_dimension);
        if state_dimension == 0
            || state_entries != Some(state_jacobian.len())
            || parameter_entries != Some(parameter_jacobian.len())
            || time_derivative.len() != state_dimension
            || state_jacobian
                .iter()
                .chain(&parameter_jacobian)
                .chain(&time_derivative)
                .any(|value| !value.is_finite())
        {
            return Err(invalid_sensitivity(
                "event reset linearization has invalid derivative shapes or values",
            ));
        }
        Ok(Self {
            state_dimension,
            parameter_dimension,
            state_jacobian,
            parameter_jacobian,
            time_derivative,
        })
    }

    /// Complete row-major reset Jacobian `rho_y`.
    #[must_use]
    pub fn state_jacobian(&self) -> &[f64] {
        &self.state_jacobian
    }

    /// Complete row-major direct Parameter derivative `rho_p`.
    #[must_use]
    pub fn parameter_jacobian(&self) -> &[f64] {
        &self.parameter_jacobian
    }

    /// Explicit model-time derivative `rho_t`.
    #[must_use]
    pub fn time_derivative(&self) -> &[f64] {
        &self.time_derivative
    }
}

/// Lowered first-order linearization of one isolated transversal event.
///
/// This object contains no root selection, event ordering, or reset-solving
/// policy. Those belong to the canonical hybrid scheduler and its lowering.
/// It only composes already-lowered `f^-`, `f^+`, guard, and reset actions into
/// event-time and saltation derivatives.
#[derive(Debug, Clone, PartialEq)]
pub struct TransversalEventLinearization {
    flow: EventFlowLinearization,
    guard: EventGuardLinearization,
    reset: EventResetLinearization,
    transversality: f64,
    saltation: Vec<f64>,
}

impl TransversalEventLinearization {
    /// Compose one mathematically transversal event linearization.
    ///
    /// The transversality denominator is `g_t + g_y f^-`. An exactly zero
    /// denominator is a grazing event and fails closed; no hidden numerical
    /// tolerance or event-selection rule is introduced here.
    ///
    /// # Errors
    /// Returns `EQ0704` for shape mismatch, non-finite arithmetic, or grazing.
    pub fn new(
        flow: EventFlowLinearization,
        guard: EventGuardLinearization,
        reset: EventResetLinearization,
    ) -> Result<Self, Diagnostic> {
        let state_dimension = flow.dimension();
        let parameter_dimension = guard.parameter_gradient.len();
        if guard.state_gradient.len() != state_dimension
            || reset.state_dimension != state_dimension
            || reset.parameter_dimension != parameter_dimension
        {
            return Err(invalid_sensitivity(
                "event flow, guard, and reset linearizations use different coordinate shapes",
            ));
        }
        let transversality = guard.time_derivative + dot(&guard.state_gradient, &flow.before);
        if !transversality.is_finite() || transversality == 0.0 {
            return Err(invalid_sensitivity(
                "event guard is grazing or has a non-finite transversality derivative",
            ));
        }

        let reset_before = matrix_vector(&reset.state_jacobian, state_dimension, &flow.before);
        let jump = (0..state_dimension)
            .map(|row| flow.after[row] - reset_before[row] - reset.time_derivative[row])
            .collect::<Vec<_>>();
        let mut saltation = reset.state_jacobian.clone();
        for row in 0..state_dimension {
            for column in 0..state_dimension {
                saltation[row * state_dimension + column] +=
                    jump[row] * guard.state_gradient[column] / transversality;
            }
        }
        if saltation.iter().any(|value| !value.is_finite()) {
            return Err(invalid_sensitivity(
                "event saltation construction produced a non-finite value",
            ));
        }
        Ok(Self {
            flow,
            guard,
            reset,
            transversality,
            saltation,
        })
    }

    /// Scalar state dimension.
    #[must_use]
    pub fn state_dimension(&self) -> usize {
        self.flow.dimension()
    }

    /// Number of selected Parameter coordinates.
    #[must_use]
    pub fn parameter_dimension(&self) -> usize {
        self.guard.parameter_gradient.len()
    }

    /// Non-zero transversality denominator `g_t + g_y f^-`.
    #[must_use]
    pub const fn transversality(&self) -> f64 {
        self.transversality
    }

    /// Flow derivatives used by the event linearization.
    #[must_use]
    pub const fn flow(&self) -> &EventFlowLinearization {
        &self.flow
    }

    /// Guard derivatives used by the event linearization.
    #[must_use]
    pub const fn guard(&self) -> &EventGuardLinearization {
        &self.guard
    }

    /// Reset derivatives used by the event linearization.
    #[must_use]
    pub const fn reset(&self) -> &EventResetLinearization {
        &self.reset
    }

    /// Complete row-major saltation matrix.
    #[must_use]
    pub fn saltation_matrix(&self) -> &[f64] {
        &self.saltation
    }

    /// Propagate fixed-time pre-event forward sensitivities across the event.
    ///
    /// Input and returned state sensitivities are row-major `(state, Parameter)`.
    /// Event-time sensitivities follow Parameter order.
    ///
    /// # Errors
    /// Returns `EQ0704` for shape mismatch or non-finite input/output.
    pub fn propagate_forward(
        &self,
        pre_state_sensitivity: &[f64],
    ) -> Result<EventForwardSensitivity, Diagnostic> {
        let state_dimension = self.state_dimension();
        let parameter_dimension = self.parameter_dimension();
        if state_dimension.checked_mul(parameter_dimension) != Some(pre_state_sensitivity.len())
            || pre_state_sensitivity.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_sensitivity(
                "pre-event sensitivity has invalid state-by-Parameter shape or values",
            ));
        }

        let mut event_time = vec![0.0; parameter_dimension];
        for parameter in 0..parameter_dimension {
            let guard_state_action = (0..state_dimension)
                .map(|state| {
                    self.guard.state_gradient[state]
                        * pre_state_sensitivity[state * parameter_dimension + parameter]
                })
                .sum::<f64>();
            event_time[parameter] = -(guard_state_action
                + self.guard.parameter_gradient[parameter])
                / self.transversality;
        }

        let reset_before = matrix_vector(
            &self.reset.state_jacobian,
            state_dimension,
            &self.flow.before,
        );
        let mut post_state = vec![0.0; pre_state_sensitivity.len()];
        for row in 0..state_dimension {
            let time_jump =
                reset_before[row] + self.reset.time_derivative[row] - self.flow.after[row];
            for parameter in 0..parameter_dimension {
                let reset_state_action = (0..state_dimension)
                    .map(|column| {
                        self.reset.state_jacobian[row * state_dimension + column]
                            * pre_state_sensitivity[column * parameter_dimension + parameter]
                    })
                    .sum::<f64>();
                post_state[row * parameter_dimension + parameter] = reset_state_action
                    + self.reset.parameter_jacobian[row * parameter_dimension + parameter]
                    + time_jump * event_time[parameter];
            }
        }
        if event_time
            .iter()
            .chain(&post_state)
            .any(|value| !value.is_finite())
        {
            return Err(invalid_sensitivity(
                "event forward propagation produced a non-finite derivative",
            ));
        }
        Ok(EventForwardSensitivity {
            event_time,
            post_state,
        })
    }
}

/// Event-time and fixed-time post-reset forward sensitivities.
#[derive(Debug, Clone, PartialEq)]
pub struct EventForwardSensitivity {
    event_time: Vec<f64>,
    post_state: Vec<f64>,
}

impl EventForwardSensitivity {
    /// Derivative of the localized event time in Parameter order.
    #[must_use]
    pub fn event_time(&self) -> &[f64] {
        &self.event_time
    }

    /// Row-major `(state, Parameter)` fixed-time post-event sensitivity.
    #[must_use]
    pub fn post_state(&self) -> &[f64] {
        &self.post_state
    }
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn matrix_vector(matrix: &[f64], dimension: usize, vector: &[f64]) -> Vec<f64> {
    matrix
        .chunks_exact(dimension)
        .map(|row| dot(row, vector))
        .collect()
}
