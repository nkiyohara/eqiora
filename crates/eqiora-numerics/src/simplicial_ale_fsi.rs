//! Fixed-topology ALE fluid--structure interaction on affine simplices.

mod acceptance;
mod api;
mod assembly;
mod boundary_step;
mod contract;
mod element;
mod motion;
mod newton;
mod verification;

#[allow(unused_imports)]
pub(crate) use boundary_step::{
    AleFsiBoundaryEndpointIdentity, AleFsiExteriorFacetDisposition, PreparedAleFsiBoundaryStep,
    advance_simplicial_ale_fsi_prepared_step,
};

pub use api::{
    AleFsiInterfaceAction, AleFsiInterfaceAction2d, AleFsiInterfaceAction3d, AleFsiStepEvidence,
    AleFsiStepEvidence2d, AleFsiStepEvidence3d, AleFsiTrajectory, AleFsiTrajectory2d,
    AleFsiTrajectory3d,
};
pub use contract::{
    AleFsiBoundary, AleFsiBoundary2d, AleFsiBoundary3d, AleFsiState, AleFsiState2d, AleFsiState3d,
    AleFsiStepPlan, AleFsiStepPlan2d, AleFsiStepPlan3d,
};
pub use motion::{
    P1HarmonicMeshMotionAction, P1HarmonicMeshMotionAction2d, P1HarmonicMeshMotionAction3d,
};
pub use newton::{
    advance_simplicial_ale_fsi_2d, advance_simplicial_ale_fsi_2d_with_assembly,
    advance_simplicial_ale_fsi_3d, advance_simplicial_ale_fsi_3d_with_assembly,
};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}
