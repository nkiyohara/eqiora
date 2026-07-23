//! **eqiora-time** — backend-neutral contracts for time execution.
//!
//! Canonical model meaning remains an activation-controlled network of
//! implicit Relations. A compiler may classify a continuous subsystem into
//! the narrower first-order form `M(t) y_dot = f(t, y)`. General
//! `F(t, y, y_dot) = 0` systems use a distinct residual/JVP problem and must
//! never be disguised as mass-matrix problems to satisfy an adapter.

mod diagnostic;
mod event;
mod lowering;
mod plan;
mod problem;
mod reference_implicit;
mod solution;
mod system;

#[cfg(test)]
mod tests;

pub use event::{
    EventFlowLinearization, EventForwardSensitivity, EventGuardLinearization,
    EventResetLinearization, RegisteredRootProblem, RootActivationGroup, RootProposal,
    RootRegistrationId, RootRegistrationProof, TransversalEventLinearization,
};
pub use lowering::{
    ConstantDerivativeMatrixProof, DaeVariableKind, GeneralImplicitLoweringProof,
    GeneralImplicitReason, MassMatrixRank, MonomialDerivativeRow, TimeEquationClass,
    TimeLoweringProof,
};
pub use plan::{ForwardSensitivityPlan, TimeMethod, TimePlan};
pub use problem::{
    ForwardSensitivityProblem, ImplicitDaeInitialization, ImplicitDaeProblem,
    InitialConditionPolicy, TimeProblem,
};
pub use reference_implicit::{REFERENCE_IMPLICIT_TIME_BACKEND, ReferenceImplicitTimeBackend};
pub use solution::{
    ForwardSensitivitySolution, TimeBackendId, TimeBackendIdentity, TimeBackendVersion,
    TimeExecutionReport, TimeSolution,
};
pub use system::{
    ImplicitTimeSystem, MassParameterDependence, ParametricTimeSystem, RootFunctions, TimeSystem,
};
