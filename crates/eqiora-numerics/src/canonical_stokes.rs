//! Method-neutral lowering of exact incompressible Newtonian fluid subsets.

mod api;
mod block;
mod boundary;
mod expression;
mod geometry_realization;
mod inertial;
mod navier_stokes;
mod navier_stokes_fvm_realization;
mod navier_stokes_geometry_realization;
mod navier_stokes_realization;
mod physical;
mod realization;
mod recognize;
mod support;

pub use api::{SteadyIncompressibleStokesCartesianModel2d, SteadyStokesNormalPressure2d};
pub(crate) use boundary::LoweredStokesBoundary;
pub use geometry_realization::{
    SteadyStokesGeometryBinding2d, solve_resolved_steady_stokes_geometry_mini_2d,
};
pub use inertial::{
    InertialIncompressibleNewtonianCartesianModel2d,
    lower_inertial_incompressible_newtonian_cartesian_2d,
};
pub(crate) use inertial::{
    LoweredInertialIncompressibleNewtonianSubdomain2d,
    lower_inertial_incompressible_newtonian_subdomain_2d,
};
pub(crate) use navier_stokes::lower_transient_incompressible_navier_stokes_subdomain;
pub use navier_stokes::{
    TransientIncompressibleNavierStokesCartesianModel,
    TransientIncompressibleNavierStokesCartesianModel2d,
    TransientIncompressibleNavierStokesCartesianModel3d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
    lower_transient_incompressible_navier_stokes_cartesian_3d,
};
pub use navier_stokes_fvm_realization::{
    CellCenteredNavierStokesInitialState2d, CellCenteredNavierStokesStepEvidence2d,
    ResolvedCellCenteredNavierStokesState2d, ResolvedCellCenteredNavierStokesTrajectory2d,
    advance_resolved_transient_navier_stokes_cell_centered_2d,
    transient_navier_stokes_cell_centered_plan_2d,
    transient_navier_stokes_cell_centered_requirements_2d,
};
pub use navier_stokes_realization::{
    ResolvedTransientNavierStokesState2d, ResolvedTransientNavierStokesTrajectory2d,
    TransientNavierStokesInitialState2d, TransientNavierStokesRun2d,
    advance_resolved_transient_navier_stokes_mini_2d,
    advance_resolved_transient_navier_stokes_mini_2d_with_assembly,
    transient_navier_stokes_fieldwise_requirements_2d, transient_navier_stokes_mini_plan_2d,
};
pub use physical::{
    FinalizedSteadyStokesMini2dProblem, SteadyStokesMiniSolution2d, SteadyStokesPressureReference2d,
};
pub use realization::{
    IncompressibleFlowScaleProfile2d, SteadyStokesScaleProfile2d,
    finalize_resolved_steady_stokes_mini_2d, solve_resolved_steady_stokes_mini_2d,
    steady_stokes_fieldwise_requirements_2d, steady_stokes_mini_plan_2d,
};
pub use recognize::lower_steady_incompressible_stokes_cartesian_2d;

#[cfg(test)]
mod tests;
