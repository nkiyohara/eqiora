//! Fixed-reference fluid-structure interaction realizations.

pub use crate::canonical_fsi::{
    AcceptedDistributedFixedReferenceFsiStep2d, FinalizedResolvedFixedReferenceFsiStep2d,
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiFieldIdentities2d,
    FixedReferenceFsiInterface2d, FixedReferenceFsiInterfaceSide2d,
    FixedReferenceFsiScaleProfile2d, FsiInterface, FsiInterface2d, FsiInterface3d,
    FsiInterfaceSide, FsiInterfaceSide2d, FsiInterfaceSide3d,
    PreparedDistributedFixedReferenceFsiStep2d, ResolvedFixedReferenceFsiSolution2d,
    finalize_resolved_fixed_reference_fsi_step_2d,
    finalize_resolved_fixed_reference_fsi_step_2d_with_assembly, fixed_reference_fsi_cuda_plan_2d,
    fixed_reference_fsi_distributed_cuda_plan_2d, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d, fixed_reference_fsi_requirements_2d_for_layout,
    lower_fixed_reference_fsi_cartesian_2d,
};
pub use crate::simplicial_fsi::{
    FinalizedFixedReferenceFsiStep, FinalizedFixedReferenceFsiStep2d,
    FinalizedFixedReferenceFsiStep3d, FixedReferenceFsiBoundary, FixedReferenceFsiBoundary2d,
    FixedReferenceFsiBoundary3d, FixedReferenceFsiEnergyBalance, FixedReferenceFsiEnergyBalance2d,
    FixedReferenceFsiEnergyBalance3d, FixedReferenceFsiInterfaceAction,
    FixedReferenceFsiInterfaceAction2d, FixedReferenceFsiInterfaceAction3d,
    FixedReferenceFsiInterfaceFacet, FixedReferenceFsiInterfaceFacet2d,
    FixedReferenceFsiInterfaceFacet3d, FixedReferenceFsiLoad, FixedReferenceFsiLoad2d,
    FixedReferenceFsiLoad3d, FixedReferenceFsiMaterial, FixedReferenceFsiMaterial2d,
    FixedReferenceFsiMaterial3d, FixedReferenceFsiPartition, FixedReferenceFsiPartition2d,
    FixedReferenceFsiPartition3d, FixedReferenceFsiScale, FixedReferenceFsiScale2d,
    FixedReferenceFsiScale3d, FixedReferenceFsiSolution, FixedReferenceFsiSolution2d,
    FixedReferenceFsiSolution3d, FixedReferenceFsiState, FixedReferenceFsiState2d,
    FixedReferenceFsiState3d, FixedReferenceFsiStepConfig, FixedReferenceFsiStepConfig2d,
    FixedReferenceFsiStepConfig3d, finalize_fixed_reference_fsi_step_2d,
    finalize_fixed_reference_fsi_step_2d_with_assembly, finalize_fixed_reference_fsi_step_3d,
    solve_fixed_reference_fsi_step_2d, solve_fixed_reference_fsi_step_3d,
};
