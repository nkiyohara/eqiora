//! Exact wire conversions for common Result artifacts.

use eqiora_core::{Diagnostic, DimExponents};
use eqiora_solver::{
    ConvergenceReason, LinearOperatorOrientation, LinearSolver, PreconditionerPolicy,
    ReductionPolicy,
};

use super::{
    CommonFieldAssociation, CommonResultFamily, WireAlgorithm, WireAssociation,
    WireConvergenceReason, WireOrientation, WirePreconditioner, WireReduction, WireResultFamily,
    invalid,
};

pub(super) fn encode_shape(shape: &[usize]) -> Result<Vec<u64>, Diagnostic> {
    shape
        .iter()
        .map(|extent| to_u64(*extent, "Result shape extent"))
        .collect()
}

pub(super) fn decode_shape(shape: &[u64]) -> Result<Vec<usize>, Diagnostic> {
    shape
        .iter()
        .map(|extent| positive_usize(*extent, "Result shape extent"))
        .collect()
}

pub(super) fn to_u64(value: usize, label: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| invalid(format!("{label} exceeds u64")))
}

pub(super) fn to_usize(value: u64, label: &str) -> Result<usize, Diagnostic> {
    usize::try_from(value).map_err(|_| invalid(format!("{label} exceeds usize")))
}

pub(super) fn positive_usize(value: u64, label: &str) -> Result<usize, Diagnostic> {
    let value = to_usize(value, label)?;
    if value == 0 {
        return Err(invalid(format!("{label} must be positive")));
    }
    Ok(value)
}

pub(super) fn dimension_to_wire(value: DimExponents) -> [i8; 7] {
    [
        value.length,
        value.mass,
        value.time,
        value.current,
        value.temperature,
        value.amount,
        value.luminous_intensity,
    ]
}

pub(super) fn dimension_from_wire(value: [i8; 7]) -> DimExponents {
    DimExponents {
        length: value[0],
        mass: value[1],
        time: value[2],
        current: value[3],
        temperature: value[4],
        amount: value[5],
        luminous_intensity: value[6],
    }
}

impl From<CommonResultFamily> for WireResultFamily {
    fn from(value: CommonResultFamily) -> Self {
        match value {
            CommonResultFamily::Scalar => Self::Scalar,
            CommonResultFamily::Elasticity => Self::Elasticity,
            CommonResultFamily::SteadyStokes => Self::SteadyStokes,
            CommonResultFamily::Ode => Self::Ode,
            CommonResultFamily::TransientFlow => Self::TransientFlow,
            CommonResultFamily::FixedReferenceFsi => Self::FixedReferenceFsi,
        }
    }
}

impl From<WireResultFamily> for CommonResultFamily {
    fn from(value: WireResultFamily) -> Self {
        match value {
            WireResultFamily::Scalar => Self::Scalar,
            WireResultFamily::Elasticity => Self::Elasticity,
            WireResultFamily::SteadyStokes => Self::SteadyStokes,
            WireResultFamily::Ode => Self::Ode,
            WireResultFamily::TransientFlow => Self::TransientFlow,
            WireResultFamily::FixedReferenceFsi => Self::FixedReferenceFsi,
        }
    }
}

impl From<CommonFieldAssociation> for WireAssociation {
    fn from(value: CommonFieldAssociation) -> Self {
        match value {
            CommonFieldAssociation::Vertex => Self::Vertex,
            CommonFieldAssociation::Cell => Self::Cell,
            CommonFieldAssociation::CellBubble => Self::CellBubble,
        }
    }
}

impl From<WireAssociation> for CommonFieldAssociation {
    fn from(value: WireAssociation) -> Self {
        match value {
            WireAssociation::Vertex => Self::Vertex,
            WireAssociation::Cell => Self::Cell,
            WireAssociation::CellBubble => Self::CellBubble,
        }
    }
}

impl From<LinearOperatorOrientation> for WireOrientation {
    fn from(value: LinearOperatorOrientation) -> Self {
        match value {
            LinearOperatorOrientation::Normal => Self::Normal,
            LinearOperatorOrientation::Transposed => Self::Transposed,
        }
    }
}
impl From<WireOrientation> for LinearOperatorOrientation {
    fn from(value: WireOrientation) -> Self {
        match value {
            WireOrientation::Normal => Self::Normal,
            WireOrientation::Transposed => Self::Transposed,
        }
    }
}
impl From<LinearSolver> for WireAlgorithm {
    fn from(value: LinearSolver) -> Self {
        match value {
            LinearSolver::ConjugateGradient => Self::ConjugateGradient,
            LinearSolver::MinimumResidual => Self::MinimumResidual,
            LinearSolver::BiConjugateGradientStabilized => Self::Bicgstab,
            LinearSolver::SparseLu => Self::SparseLu,
        }
    }
}
impl From<WireAlgorithm> for LinearSolver {
    fn from(value: WireAlgorithm) -> Self {
        match value {
            WireAlgorithm::ConjugateGradient => Self::ConjugateGradient,
            WireAlgorithm::MinimumResidual => Self::MinimumResidual,
            WireAlgorithm::Bicgstab => Self::BiConjugateGradientStabilized,
            WireAlgorithm::SparseLu => Self::SparseLu,
        }
    }
}
impl From<PreconditionerPolicy> for WirePreconditioner {
    fn from(value: PreconditionerPolicy) -> Self {
        match value {
            PreconditionerPolicy::Identity => Self::Identity,
            PreconditionerPolicy::Jacobi => Self::Jacobi,
        }
    }
}
impl From<WirePreconditioner> for PreconditionerPolicy {
    fn from(value: WirePreconditioner) -> Self {
        match value {
            WirePreconditioner::Identity => Self::Identity,
            WirePreconditioner::Jacobi => Self::Jacobi,
        }
    }
}
impl From<ReductionPolicy> for WireReduction {
    fn from(value: ReductionPolicy) -> Self {
        match value {
            ReductionPolicy::Reproducible => Self::Reproducible,
            ReductionPolicy::Fast => Self::Fast,
        }
    }
}
impl From<WireReduction> for ReductionPolicy {
    fn from(value: WireReduction) -> Self {
        match value {
            WireReduction::Reproducible => Self::Reproducible,
            WireReduction::Fast => Self::Fast,
        }
    }
}
impl From<ConvergenceReason> for WireConvergenceReason {
    fn from(value: ConvergenceReason) -> Self {
        match value {
            ConvergenceReason::InitialResidualSatisfied => Self::InitialResidualSatisfied,
            ConvergenceReason::ResidualToleranceSatisfied => Self::ResidualToleranceSatisfied,
        }
    }
}
impl From<WireConvergenceReason> for ConvergenceReason {
    fn from(value: WireConvergenceReason) -> Self {
        match value {
            WireConvergenceReason::InitialResidualSatisfied => Self::InitialResidualSatisfied,
            WireConvergenceReason::ResidualToleranceSatisfied => Self::ResidualToleranceSatisfied,
        }
    }
}
