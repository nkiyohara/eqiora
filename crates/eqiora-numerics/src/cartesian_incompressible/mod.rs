//! Collocated cell-centered incompressible-flow numerical realization.

mod api;
mod newton;
mod operator;
mod pressure_coupling;
mod replay;

pub use api::{CellCenteredPressureField2d, CellCenteredVelocityField2d};

pub(crate) use newton::{CollocatedNewtonEvidence2d, solve_collocated_step_2d};
pub(crate) use operator::{
    CartesianIncompressibleOperator2d, CollocatedPoint2d, CollocatedResidual2d,
    PreparedCartesianIncompressibleOperator2d,
};
