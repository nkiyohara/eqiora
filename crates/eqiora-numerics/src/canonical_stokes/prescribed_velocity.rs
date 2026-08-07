//! Private Model-owned prescribed velocity traces for steady Stokes.

use eqiora_core::{Diagnostic, RawId};

use crate::spatial_expression::ScalarSpatialExpression;

/// The complete prescribed-velocity meaning retained from one exact Model.
///
/// `Normal` preserves the established scalar-normal subset.  The complete
/// variant is deliberately restricted to the gradient of one retained scalar
/// affine potential; it is not a general vector expression or callback law.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SteadyStokesPrescribedVelocityTrace2d {
    Normal {
        coefficient_field: RawId,
        definition_relation: RawId,
        expression: ScalarSpatialExpression,
    },
    CompleteAffinePotential {
        potential_field: RawId,
        definition_relation: RawId,
        speed_parameter: RawId,
        expression: ScalarSpatialExpression,
    },
}

impl SteadyStokesPrescribedVelocityTrace2d {
    pub(super) fn normal(
        coefficient_field: RawId,
        definition_relation: RawId,
        expression: ScalarSpatialExpression,
    ) -> Self {
        Self::Normal {
            coefficient_field,
            definition_relation,
            expression,
        }
    }

    pub(super) fn complete_affine_potential(
        potential_field: RawId,
        definition_relation: RawId,
        speed_parameter: RawId,
        expression: ScalarSpatialExpression,
    ) -> Result<Self, Diagnostic> {
        let gradient = exact_complete_gradient(&expression)?;
        let speed = expression.parameter_values()[0];
        if gradient != [speed, 0.0] || !speed.is_finite() || speed <= 0.0 {
            return Err(invalid(
                "complete Stokes trace requires the exact positive affine gradient `U e_x`",
            ));
        }
        Ok(Self::CompleteAffinePotential {
            potential_field,
            definition_relation,
            speed_parameter,
            expression,
        })
    }

    pub(super) const fn coefficient_field(&self) -> RawId {
        match self {
            Self::Normal {
                coefficient_field, ..
            } => *coefficient_field,
            Self::CompleteAffinePotential {
                potential_field, ..
            } => *potential_field,
        }
    }

    pub(super) const fn definition_relation(&self) -> RawId {
        match self {
            Self::Normal {
                definition_relation,
                ..
            }
            | Self::CompleteAffinePotential {
                definition_relation,
                ..
            } => *definition_relation,
        }
    }

    pub(super) const fn expression(&self) -> &ScalarSpatialExpression {
        match self {
            Self::Normal { expression, .. } | Self::CompleteAffinePotential { expression, .. } => {
                expression
            }
        }
    }

    pub(super) const fn speed_parameter(&self) -> Option<RawId> {
        match self {
            Self::Normal { .. } => None,
            Self::CompleteAffinePotential {
                speed_parameter, ..
            } => Some(*speed_parameter),
        }
    }

    pub(super) const fn is_complete(&self) -> bool {
        matches!(self, Self::CompleteAffinePotential { .. })
    }

    /// Replay the complete vector from retained Model meaning.
    pub(super) fn value(
        &self,
        outward_normal: Option<[f64; 2]>,
        coordinates: &[f64],
    ) -> Result<[f64; 2], Diagnostic> {
        match self {
            Self::Normal { expression, .. } => {
                let outward = outward_normal.ok_or_else(|| {
                    invalid("normal Stokes trace requires one parent-outward normal")
                })?;
                let speed = expression.evaluate(coordinates)?;
                Ok(outward.map(|component| component * speed))
            }
            Self::CompleteAffinePotential { expression, .. } => exact_complete_gradient(expression),
        }
    }
}

fn exact_complete_gradient(expression: &ScalarSpatialExpression) -> Result<[f64; 2], Diagnostic> {
    if expression.coordinate_dimension() != 2
        || expression.parameter_fields().len() != 1
        || expression.parameter_values().len() != 1
        || expression.evaluate(&[0.0, 0.0])? != 0.0
    {
        return Err(invalid(
            "complete Stokes trace requires one zero-intercept two-dimensional affine potential",
        ));
    }
    expression
        .affine_gradient()
        .and_then(|gradient| gradient.try_into().ok())
        .ok_or_else(|| invalid("complete Stokes trace potential is not exactly affine in 2D"))
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(
        eqiora_core::diagnostic::codes::INVALID_DISCRETIZATION,
        message,
    )
}
