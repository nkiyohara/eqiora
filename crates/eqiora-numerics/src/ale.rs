//! Moving-domain ALE and remeshing realizations.

pub use crate::canonical_fsi::{
    AcceptedResolvedAleFsiRemesh2d, AleFsiCartesianModel, AleFsiFieldIdentities,
    AleFsiInitialPhysicalState, FinalizedResolvedFixedTopologyAleFsi,
    finalize_resolved_fixed_topology_ale_fsi_2d, finalize_resolved_fixed_topology_ale_fsi_3d,
    fixed_topology_ale_fsi_requirements_2d, fixed_topology_ale_fsi_requirements_3d,
    lower_ale_fsi_cartesian_2d, lower_ale_fsi_cartesian_3d,
    remesh_resolved_fixed_topology_ale_fsi_2d, solve_resolved_fixed_topology_ale_fsi_2d,
    solve_resolved_fixed_topology_ale_fsi_2d_with_assembly,
    solve_resolved_fixed_topology_ale_fsi_3d,
    solve_resolved_fixed_topology_ale_fsi_3d_with_assembly,
};
pub use crate::simplicial_ale_fsi::{
    AleFsiBoundary, AleFsiBoundary2d, AleFsiBoundary3d, AleFsiInterfaceAction,
    AleFsiInterfaceAction2d, AleFsiInterfaceAction3d, AleFsiState, AleFsiState2d, AleFsiState3d,
    AleFsiStepEvidence, AleFsiStepEvidence2d, AleFsiStepEvidence3d, AleFsiStepPlan,
    AleFsiStepPlan2d, AleFsiStepPlan3d, AleFsiTrajectory, AleFsiTrajectory2d, AleFsiTrajectory3d,
    P1HarmonicMeshMotionAction, P1HarmonicMeshMotionAction2d, P1HarmonicMeshMotionAction3d,
    advance_simplicial_ale_fsi_2d, advance_simplicial_ale_fsi_2d_with_assembly,
    advance_simplicial_ale_fsi_3d, advance_simplicial_ale_fsi_3d_with_assembly,
};
pub use crate::simplicial_ale_remesh::{
    AcceptedAleFsiRemeshProjection2d, AleFsiRemeshProjectionEvidence2d,
    project_simplicial_ale_fsi_remesh_2d,
};
pub use crate::simplicial_motion::SimplicialMeshVelocity;
