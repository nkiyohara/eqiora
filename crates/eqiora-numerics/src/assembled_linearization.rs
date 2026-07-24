//! Method-neutral linearized relation over assembled sparse algebra.

use std::sync::Arc;

use eqiora_assembly::CsrMatrix;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_ir::{LinearizedRelation, RelationCotangent, RelationTangent};
use eqiora_solver::{
    CanonicalCsrSystemView, CompleteCsrStorage, LinearOperator, LinearOperatorProperties,
    TransposeLinearOperator,
};

use crate::SpatialDesignCoordinate;

/// One accepted-point relation `A w - b = 0` with dense design actions.
///
/// The state action remains sparse. Parameter columns are dense because the
/// first spatial differentiation slices have few selected coordinates; changing
/// their storage later does not alter [`LinearizedRelation`] semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct AssembledLinearizedRelation {
    state_jacobian: Arc<CanonicalCsrSystemView>,
    accepted_unknowns: Vec<f64>,
    design_coordinates: Vec<SpatialDesignCoordinate>,
    design_values: Vec<f64>,
    design_jacobian: Vec<f64>,
}

/// Compare one analytic state JVP with an independently reassembled centered action.
///
/// This is verification infrastructure, not a production differentiation
/// backend. The production relation continues to expose its analytic action;
/// the closure is invoked only by bounded acceptance paths after convergence.
pub(crate) fn centered_state_jvp_error<F>(
    point: &[f64],
    direction: &[f64],
    epsilon: f64,
    analytic: &[f64],
    mut residual: F,
) -> Result<f64, Diagnostic>
where
    F: FnMut(&[f64]) -> Result<Vec<f64>, Diagnostic>,
{
    if point.len() != direction.len()
        || analytic.is_empty()
        || !epsilon.is_finite()
        || epsilon <= 0.0
    {
        return Err(invalid(
            "centered state-JVP verification has invalid shape or step",
        ));
    }
    let shifted = |sign: f64| {
        point
            .iter()
            .zip(direction)
            .map(|(point, direction)| point + sign * epsilon * direction)
            .collect::<Vec<_>>()
    };
    let plus = residual(&shifted(1.0))?;
    let minus = residual(&shifted(-1.0))?;
    if plus.len() != analytic.len() || minus.len() != analytic.len() {
        return Err(invalid(
            "centered state-JVP verification residual shape changed",
        ));
    }
    let error = plus
        .iter()
        .zip(minus)
        .zip(analytic)
        .map(|((plus, minus), analytic)| ((plus - minus) / (2.0 * epsilon) - analytic).powi(2))
        .sum::<f64>()
        .sqrt();
    if error.is_finite() {
        Ok(error)
    } else {
        Err(invalid(
            "centered state-JVP verification produced a non-finite error",
        ))
    }
}

impl AssembledLinearizedRelation {
    /// Construct one finite, shape-consistent assembled linearization.
    ///
    /// `design_jacobian` is residual-row-major with one column per
    /// `design_coordinates` entry.
    ///
    /// # Errors
    /// Returns `EQ0704` for a nonsquare state action, shape mismatch,
    /// duplicate Parameter coordinate, or non-finite data.
    pub fn new(
        state_jacobian: CsrMatrix,
        accepted_unknowns: Vec<f64>,
        right_hand_side: Vec<f64>,
        design_coordinates: Vec<SpatialDesignCoordinate>,
        design_values: Vec<f64>,
        design_jacobian: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let storage = LinearizedStorage {
            matrix: &state_jacobian,
            right_hand_side: &right_hand_side,
        };
        let state_jacobian = CanonicalCsrSystemView::new(
            &storage,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        )
        .map_err(|diagnostic| {
            invalid(format!(
                "assembled state system is not canonical: {}",
                diagnostic.message()
            ))
        })?;
        Self::from_shared_canonical(
            Arc::new(state_jacobian),
            accepted_unknowns,
            design_coordinates,
            design_values,
            design_jacobian,
        )
    }

    /// Construct an assembled linearization from an already captured state
    /// system, without recreating a second sparse action or RHS owner.
    ///
    /// # Errors
    /// Returns `EQ0704` for shape mismatch, duplicate Parameter coordinates,
    /// or non-finite point/action data.
    pub fn from_canonical(
        state_jacobian: CanonicalCsrSystemView,
        accepted_unknowns: Vec<f64>,
        design_coordinates: Vec<SpatialDesignCoordinate>,
        design_values: Vec<f64>,
        design_jacobian: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        Self::from_shared_canonical(
            Arc::new(state_jacobian),
            accepted_unknowns,
            design_coordinates,
            design_values,
            design_jacobian,
        )
    }

    pub(crate) fn from_shared_canonical(
        state_jacobian: Arc<CanonicalCsrSystemView>,
        accepted_unknowns: Vec<f64>,
        design_coordinates: Vec<SpatialDesignCoordinate>,
        design_values: Vec<f64>,
        design_jacobian: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let dimension = state_jacobian.rows();
        let design_dimension = design_coordinates.len();
        let expected_design_entries = dimension
            .checked_mul(design_dimension)
            .ok_or_else(|| invalid("assembled design Jacobian shape overflows usize"))?;
        if state_jacobian.columns() != dimension
            || accepted_unknowns.len() != dimension
            || design_values.len() != design_dimension
            || design_jacobian.len() != expected_design_entries
        {
            return Err(invalid(format!(
                "assembled linearization has state {}x{}, unknown/RHS sizes {}/{}, and design value/Jacobian sizes {}/{} for {design_dimension} coordinates",
                state_jacobian.rows(),
                state_jacobian.columns(),
                accepted_unknowns.len(),
                state_jacobian.right_hand_side().len(),
                design_values.len(),
                design_jacobian.len(),
            )));
        }
        if design_coordinates
            .iter()
            .enumerate()
            .any(|(index, coordinate)| design_coordinates[..index].contains(coordinate))
        {
            return Err(invalid(
                "assembled linearization design coordinates must be unique",
            ));
        }
        if accepted_unknowns
            .iter()
            .chain(&design_values)
            .chain(&design_jacobian)
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "assembled linearization requires finite point and action data",
            ));
        }
        Ok(Self {
            state_jacobian,
            accepted_unknowns,
            design_coordinates,
            design_values,
            design_jacobian,
        })
    }

    /// Sparse accepted-point state Jacobian `R_w`.
    #[must_use]
    pub fn state_jacobian(&self) -> &CanonicalCsrSystemView {
        self.state_jacobian.as_ref()
    }

    /// Algebraic state at which all actions were assembled.
    #[must_use]
    pub fn accepted_unknowns(&self) -> &[f64] {
        &self.accepted_unknowns
    }

    /// Accepted-point right-hand side `b` in `A w - b = 0`.
    #[must_use]
    pub fn right_hand_side(&self) -> &[f64] {
        self.state_jacobian.right_hand_side()
    }

    /// Explicit spatial design coordinates in dense action order.
    #[must_use]
    pub fn design_coordinates(&self) -> &[SpatialDesignCoordinate] {
        &self.design_coordinates
    }

    /// Revision-local design point matching [`Self::design_coordinates`].
    #[must_use]
    pub fn design_values(&self) -> &[f64] {
        &self.design_values
    }

    /// Row-major residual design Jacobian `R_p`.
    #[must_use]
    pub fn design_jacobian(&self) -> &[f64] {
        &self.design_jacobian
    }

    fn apply_parameter(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        if input.len() != self.design_coordinates.len() || output.len() != self.residual_dimension()
        {
            return Err(invalid("assembled design JVP shape mismatch"));
        }
        for (row, output) in output.iter_mut().enumerate() {
            *output = self.design_jacobian
                [row * self.design_coordinates.len()..(row + 1) * self.design_coordinates.len()]
                .iter()
                .zip(input)
                .map(|(entry, input)| entry * input)
                .sum();
        }
        finite_output(output, "assembled Parameter JVP")
    }

    fn apply_parameter_transpose(
        &self,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        if input.len() != self.residual_dimension() || output.len() != self.design_coordinates.len()
        {
            return Err(invalid("assembled design VJP shape mismatch"));
        }
        output.fill(0.0);
        for (row, input) in input.iter().enumerate() {
            for (column, output) in output.iter_mut().enumerate() {
                *output +=
                    self.design_jacobian[row * self.design_coordinates.len() + column] * input;
            }
        }
        finite_output(output, "assembled Parameter VJP")
    }
}

impl LinearizedRelation<f64> for AssembledLinearizedRelation {
    fn unknown_dimension(&self) -> usize {
        self.state_jacobian.columns()
    }

    fn parameter_dimension(&self) -> usize {
        self.design_coordinates.len()
    }

    fn residual_dimension(&self) -> usize {
        self.state_jacobian.rows()
    }

    fn primal(&self, residual: &mut [f64]) -> Result<(), Diagnostic> {
        if residual.len() != self.residual_dimension() {
            return Err(invalid("assembled primal residual shape mismatch"));
        }
        self.state_jacobian
            .apply(&self.accepted_unknowns, residual)?;
        for (residual, rhs) in residual
            .iter_mut()
            .zip(self.state_jacobian.right_hand_side())
        {
            *residual -= rhs;
        }
        finite_output(residual, "assembled primal residual")
    }

    fn jvp(
        &self,
        tangent: RelationTangent<'_, f64>,
        residual_tangent: &mut [f64],
    ) -> Result<(), Diagnostic> {
        match tangent {
            RelationTangent::Unknown(unknown) => {
                self.state_jacobian.apply(unknown, residual_tangent)
            }
            RelationTangent::Parameter(parameter) => {
                self.apply_parameter(parameter, residual_tangent)
            }
            RelationTangent::Both { unknown, parameter } => {
                let mut parameter_action = vec![0.0; self.residual_dimension()];
                self.state_jacobian.apply(unknown, residual_tangent)?;
                self.apply_parameter(parameter, &mut parameter_action)?;
                for (output, parameter) in residual_tangent.iter_mut().zip(parameter_action) {
                    *output += parameter;
                }
                finite_output(residual_tangent, "assembled combined JVP")
            }
        }
    }

    fn vjp(
        &self,
        residual_cotangent: &[f64],
        cotangent: RelationCotangent<'_, f64>,
    ) -> Result<(), Diagnostic> {
        match cotangent {
            RelationCotangent::Unknown(unknown) => self
                .state_jacobian
                .apply_transpose(residual_cotangent, unknown),
            RelationCotangent::Parameter(parameter) => {
                self.apply_parameter_transpose(residual_cotangent, parameter)
            }
            RelationCotangent::Both { unknown, parameter } => {
                self.state_jacobian
                    .apply_transpose(residual_cotangent, unknown)?;
                self.apply_parameter_transpose(residual_cotangent, parameter)
            }
        }
    }
}

struct LinearizedStorage<'a> {
    matrix: &'a CsrMatrix,
    right_hand_side: &'a [f64],
}

impl CompleteCsrStorage for LinearizedStorage<'_> {
    fn rows(&self) -> usize {
        self.matrix.rows()
    }

    fn columns(&self) -> usize {
        self.matrix.columns()
    }

    fn row_offsets(&self) -> &[usize] {
        self.matrix.row_offsets()
    }

    fn column_indices(&self) -> &[usize] {
        self.matrix.column_indices()
    }

    fn values(&self) -> &[f64] {
        self.matrix.values()
    }

    fn right_hand_side(&self) -> &[f64] {
        self.right_hand_side
    }
}

fn finite_output(output: &[f64], operation: &str) -> Result<(), Diagnostic> {
    if output.iter().any(|value| !value.is_finite()) {
        Err(invalid(format!("{operation} produced a non-finite value")))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
}
