//! Fixed-reference fluid-structure interaction realizations.

pub use crate::canonical_fsi::{
    AcceptedDistributedFixedReferenceFsiStep2d, FinalizedResolvedFixedReferenceFsiStep2d,
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiFieldIdentities2d,
    FixedReferenceFsiScaleProfile2d, FsiInterface, FsiInterfaceSide,
    PreparedDistributedFixedReferenceFsiStep2d, ResolvedFixedReferenceFsiSolution2d,
    finalize_resolved_fixed_reference_fsi_step_2d,
    finalize_resolved_fixed_reference_fsi_step_2d_with_assembly, fixed_reference_fsi_cuda_plan_2d,
    fixed_reference_fsi_distributed_cuda_plan_2d, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d, fixed_reference_fsi_requirements_2d_for_layout,
    lower_fixed_reference_fsi_cartesian_2d,
};
pub use crate::simplicial_fsi::{
    FinalizedFixedReferenceFsiStep, FinalizedFixedReferenceFsiStep3d, FixedReferenceFsiBoundary3d,
    FixedReferenceFsiEnergyBalance, FixedReferenceFsiEnergyBalance2d,
    FixedReferenceFsiEnergyBalance3d, FixedReferenceFsiInterfaceAction,
    FixedReferenceFsiInterfaceAction2d, FixedReferenceFsiInterfaceAction3d,
    FixedReferenceFsiInterfaceFacet, FixedReferenceFsiInterfaceFacet2d,
    FixedReferenceFsiInterfaceFacet3d, FixedReferenceFsiLoad3d, FixedReferenceFsiMaterial3d,
    FixedReferenceFsiPartition2d, FixedReferenceFsiPartition3d, FixedReferenceFsiScale2d,
    FixedReferenceFsiScale3d, FixedReferenceFsiSolution, FixedReferenceFsiSolution3d,
    FixedReferenceFsiState2d, FixedReferenceFsiState3d, FixedReferenceFsiStepConfig3d,
    finalize_fixed_reference_fsi_step_2d, finalize_fixed_reference_fsi_step_2d_with_assembly,
    finalize_fixed_reference_fsi_step_3d, solve_fixed_reference_fsi_step_2d,
    solve_fixed_reference_fsi_step_3d,
};
