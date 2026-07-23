//! **eqiora-numerics** — numerical realizations and verified solver kernels.
//!
//! This crate owns approximation choices, not model meaning. Its operators
//! consume already-resolved scalar data and can later be targeted by spatial
//! lowering without adding method-specific nodes to the Semantic Kernel.

mod affine_fem;
mod assembled_linearization;
mod canonical;
mod canonical_boundary;
mod canonical_elasticity;
mod canonical_fsi;
mod canonical_stokes;
mod canonical_transport;
mod cartesian_elasticity;
mod cartesian_elliptic;
mod cartesian_fvm_geometry;
mod cartesian_incompressible;
mod cartesian_mesh;
mod cartesian_transport;
mod diffusion;
mod discrete_block;
mod discrete_space;
mod elliptic;
mod finalized_spatial;
mod linearized_output;
mod operator;
mod physical_network;
mod poisson;
mod simplicial_ale_fsi;
mod simplicial_ale_remesh;
mod simplicial_elliptic;
mod simplicial_fsi;
mod simplicial_mini_transient;
mod simplicial_motion;
mod simplicial_navier_stokes;
mod simplicial_stokes;
mod spatial_design;
mod spatial_expression;
mod step_count;

pub use assembled_linearization::AssembledLinearizedRelation;
pub use canonical::{
    AcceptedScalarEllipticParameterPoint, DefaultScalarElliptic1dConfig,
    FinalizedScalarEllipticParameterPoint, ResolvedScalarEllipticCartesianSolution,
    ResolvedScalarEllipticSolution1d, ScalarEllipticCartesianBoundary,
    ScalarEllipticCartesianModel, ScalarEllipticModel1d, compare_canonical_scalar_elliptic_1d,
    finalize_lowered_scalar_elliptic_cartesian,
    finalize_lowered_scalar_elliptic_cartesian_with_assembly,
    finalize_resolved_scalar_elliptic_cartesian,
    finalize_resolved_scalar_elliptic_cartesian_with_assembly,
    finalize_scalar_elliptic_parameter_point, lower_scalar_elliptic_1d,
    lower_scalar_elliptic_cartesian, solve_and_linearize_resolved_scalar_elliptic_cartesian,
    solve_and_linearize_resolved_scalar_elliptic_cartesian_with_assembly,
    solve_default_scalar_elliptic_1d, solve_resolved_scalar_elliptic_1d,
    solve_resolved_scalar_elliptic_1d_with_assembly, solve_resolved_scalar_elliptic_cartesian,
    solve_resolved_scalar_elliptic_cartesian_with_assembly,
    solve_resolved_scalar_elliptic_simplicial,
    solve_resolved_scalar_elliptic_simplicial_with_assembly,
};
pub use canonical_boundary::{
    CartesianBoundaryEntry, CartesianBoundaryEntry2d, CartesianBoundaryEntry3d,
    CartesianBoundaryInventory, CartesianBoundaryInventory2d, CartesianBoundaryInventory3d,
    PhysicalBoundaryDisposition, PhysicalBoundaryQuantity, PrescribedBoundaryLaw,
};
pub use canonical_elasticity::{
    ConformingElasticityInterface2d, ConformingElasticityInterfaceSide2d,
    ConformingIsotropicElasticityCartesianPair2d, IsotropicElasticityCartesianModel2d,
    IsotropicElastodynamicsCartesianModel, IsotropicElastodynamicsCartesianModel2d,
    IsotropicElastodynamicsCartesianModel3d,
    finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
    finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
    finalize_resolved_isotropic_elasticity_cartesian_2d,
    finalize_resolved_isotropic_elasticity_cartesian_2d_with_assembly,
    lower_conforming_isotropic_elasticity_cartesian_pair_2d,
    lower_isotropic_elasticity_cartesian_2d, lower_isotropic_elastodynamics_cartesian_2d,
    lower_isotropic_elastodynamics_cartesian_3d,
    solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
    solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
    solve_resolved_isotropic_elasticity_cartesian_2d,
    solve_resolved_isotropic_elasticity_cartesian_2d_with_assembly,
};
pub use canonical_fsi::{
    AcceptedDistributedFixedReferenceFsiStep2d, AcceptedResolvedAleFsiRemesh2d,
    AleFsiCartesianModel, AleFsiCartesianModel2d, AleFsiCartesianModel3d, AleFsiFieldIdentities,
    AleFsiFieldIdentities2d, AleFsiFieldIdentities3d, AleFsiInitialPhysicalState,
    AleFsiInitialPhysicalState2d, AleFsiInitialPhysicalState3d,
    FinalizedResolvedFixedReferenceFsiStep2d, FinalizedResolvedFixedTopologyAleFsi,
    FinalizedResolvedFixedTopologyAleFsi2d, FinalizedResolvedFixedTopologyAleFsi3d,
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiFieldIdentities2d,
    FixedReferenceFsiInterface2d, FixedReferenceFsiInterfaceSide2d,
    FixedReferenceFsiScaleProfile2d, FsiInterface, FsiInterface2d, FsiInterface3d,
    FsiInterfaceSide, FsiInterfaceSide2d, FsiInterfaceSide3d,
    PreparedDistributedFixedReferenceFsiStep2d, ResolvedFixedReferenceFsiSolution2d,
    finalize_resolved_fixed_reference_fsi_step_2d,
    finalize_resolved_fixed_reference_fsi_step_2d_with_assembly,
    finalize_resolved_fixed_topology_ale_fsi_2d, finalize_resolved_fixed_topology_ale_fsi_3d,
    fixed_reference_fsi_cuda_plan_2d, fixed_reference_fsi_distributed_cuda_plan_2d,
    fixed_reference_fsi_plan_2d, fixed_reference_fsi_requirements_2d,
    fixed_reference_fsi_requirements_2d_for_layout, fixed_topology_ale_fsi_requirements_2d,
    fixed_topology_ale_fsi_requirements_3d, lower_ale_fsi_cartesian_2d, lower_ale_fsi_cartesian_3d,
    lower_fixed_reference_fsi_cartesian_2d, remesh_resolved_fixed_topology_ale_fsi_2d,
    solve_resolved_fixed_topology_ale_fsi_2d,
    solve_resolved_fixed_topology_ale_fsi_2d_with_assembly,
    solve_resolved_fixed_topology_ale_fsi_3d,
    solve_resolved_fixed_topology_ale_fsi_3d_with_assembly,
};
pub use canonical_stokes::{
    CellCenteredNavierStokesInitialState2d, CellCenteredNavierStokesStepEvidence2d,
    FinalizedSteadyStokesMini2dProblem, IncompressibleFlowScaleProfile2d,
    InertialIncompressibleNewtonianCartesianModel2d, ResolvedCellCenteredNavierStokesState2d,
    ResolvedCellCenteredNavierStokesTrajectory2d, ResolvedTransientNavierStokesState2d,
    ResolvedTransientNavierStokesTrajectory2d, SteadyIncompressibleStokesCartesianModel2d,
    SteadyStokesMiniSolution2d, SteadyStokesNormalPressure2d, SteadyStokesPressureReference2d,
    SteadyStokesScaleProfile2d, TransientIncompressibleNavierStokesCartesianModel,
    TransientIncompressibleNavierStokesCartesianModel2d,
    TransientIncompressibleNavierStokesCartesianModel3d, TransientNavierStokesInitialState2d,
    TransientNavierStokesRun2d, advance_resolved_transient_navier_stokes_cell_centered_2d,
    advance_resolved_transient_navier_stokes_mini_2d,
    advance_resolved_transient_navier_stokes_mini_2d_with_assembly,
    finalize_resolved_steady_stokes_mini_2d, finalize_resolved_steady_stokes_mini_2d_with_assembly,
    lower_inertial_incompressible_newtonian_cartesian_2d,
    lower_steady_incompressible_stokes_cartesian_2d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
    lower_transient_incompressible_navier_stokes_cartesian_3d,
    solve_resolved_steady_stokes_mini_2d, solve_resolved_steady_stokes_mini_2d_with_assembly,
    steady_stokes_fieldwise_requirements_2d, steady_stokes_mini_plan_2d,
    transient_navier_stokes_cell_centered_plan_2d,
    transient_navier_stokes_cell_centered_requirements_2d,
    transient_navier_stokes_fieldwise_requirements_2d, transient_navier_stokes_mini_plan_2d,
};
pub use canonical_transport::{
    ScalarTransportCartesianBoundary, ScalarTransportCartesianModel2d,
    lower_scalar_transport_cartesian_2d,
};
pub use cartesian_elasticity::{
    CartesianLinearElasticity2dSolution, CartesianQ1VectorField2d, CartesianVectorErrorNorms,
    ConformingCartesianInterfaceMap2d, ConformingCartesianLinearElasticityPair2dSolution,
    ConformingElasticityInterfaceAction2d, lower_cartesian_q1_linear_elasticity_local_action_2d,
    solve_cartesian_q1_linear_elasticity_2d, solve_cartesian_q1_linear_elasticity_2d_with_assembly,
};
pub use cartesian_elliptic::{
    CartesianQ1Field, ScalarEllipticCartesianFemSolution, ScalarEllipticCartesianFvmSolution,
    linearize_scalar_elliptic_cartesian_fem, linearize_scalar_elliptic_cartesian_fem_output,
    linearize_scalar_elliptic_cartesian_fvm, linearize_scalar_elliptic_cartesian_fvm_output,
    lower_cartesian_q1_diffusion_local_action, solve_scalar_elliptic_cartesian_fem,
    solve_scalar_elliptic_cartesian_fem_with_assembly, solve_scalar_elliptic_cartesian_fvm,
    solve_scalar_elliptic_cartesian_fvm_with_assembly,
};
pub use cartesian_incompressible::{CellCenteredPressureField2d, CellCenteredVelocityField2d};
pub use cartesian_mesh::CartesianMesh;
pub use cartesian_transport::{
    FinalizedScalarTransportFvmStep2d, ScalarTransportBoundaryRole, ScalarTransportCellState2d,
    ScalarTransportFvmStep2d, ScalarTransportFvmStepEvidence2d,
    finalize_resolved_scalar_transport_fvm_step_2d,
    finalize_resolved_scalar_transport_fvm_step_2d_with_assembly,
    initialize_resolved_scalar_transport_fvm_2d, solve_resolved_scalar_transport_fvm_step_2d,
    solve_resolved_scalar_transport_fvm_step_2d_with_assembly,
};
pub use diffusion::Diffusion1d;
pub use discrete_space::{
    BasisTabulation, CellConstantSpace, DiscreteSpace, HypercubeQ1Space, LocalDof,
    SimplexP1BubbleSpace, SimplexP1Space,
};
pub use elliptic::{
    ScalarBoundaryCondition1d, ScalarBoundaryPair1d, ScalarEllipticSolution1d,
    solve_scalar_elliptic_linear_fem, solve_scalar_elliptic_linear_fem_with_assembly,
};
pub use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPacketSetIdentityV1, AssemblyPlan,
    AssemblyReport, AssemblyResult, AssemblyTarget, AssemblyTargetId, AssemblyWork, CooAssembler,
    CsrMatrix, DofId, IndexedAssemblyWork, LinearSystem, LocalContribution, LocalUnknown,
    REFERENCE_ASSEMBLY_BACKEND, ReferenceAssemblyBackend, TargetAssemblyMap,
};
pub use eqiora_meshing::{
    AffineGeometryLinearization, AffineGeometryMap, AffineMapQuality, CellId, EntityIncidence,
    FacetCells1d, FacetId, FixedTopologyCellGeometryAction2d, FixedTopologyGeometryAction2d,
    FixedTopologyGeometryState2d, GeometryMap, LineGeometryMap, LineMesh, MeshEntity, MeshGeometry,
    MeshQualityGate, MeshQualityReport, MeshTopology, OrientationCode, PointGeometry1d,
    QuadraturePoint, QuadratureRule, ReferenceCell, ReferenceCellFamily, ReferenceEntity,
    ReferenceIncidence, ReferenceTopology, SegmentGeometry1d, SimplicialMesh, VertexId,
    VertexPermutation, simplex_centroid_rule, simplex_duffy_gauss_legendre,
    triangle_duffy_gauss_legendre,
};
pub use finalized_spatial::{
    FinalizedConformingIsotropicElasticityCartesianPair2dProblem,
    FinalizedIsotropicElasticityCartesian2dProblem, FinalizedScalarEllipticCartesianProblem,
    FinalizedSimplicialMiniStokes2dProblem,
};
pub use linearized_output::CartesianScalarFieldLinearization;
pub use operator::LocalOperator;
pub use physical_network::{
    ScalarPhysicalAffineProblem, ScalarPhysicalAffineSolution, lower_scalar_physical_affine,
    solve_scalar_physical_affine, solve_scalar_physical_affine_with_initial_guess,
};
pub use poisson::{
    DirichletBoundary1d, PiecewiseLinearField1d, PoissonComparisonRow, PoissonSolution1d,
    ScalarEllipticComparisonRow1d, ScalarEllipticFvmSolution1d, compare_sine_poisson_1d,
    solve_poisson_cell_fvm, solve_poisson_linear_fem, solve_scalar_elliptic_cell_fvm,
};
pub use simplicial_ale_fsi::{
    AleFsiBoundary, AleFsiBoundary2d, AleFsiBoundary3d, AleFsiInterfaceAction,
    AleFsiInterfaceAction2d, AleFsiInterfaceAction3d, AleFsiState, AleFsiState2d, AleFsiState3d,
    AleFsiStepEvidence, AleFsiStepEvidence2d, AleFsiStepEvidence3d, AleFsiStepPlan,
    AleFsiStepPlan2d, AleFsiStepPlan3d, AleFsiTrajectory, AleFsiTrajectory2d, AleFsiTrajectory3d,
    P1HarmonicMeshMotion, P1HarmonicMeshMotion2d, P1HarmonicMeshMotion3d,
    advance_simplicial_ale_fsi_2d, advance_simplicial_ale_fsi_2d_with_assembly,
    advance_simplicial_ale_fsi_3d, advance_simplicial_ale_fsi_3d_with_assembly,
};
pub use simplicial_ale_remesh::{
    AcceptedAleFsiRemeshProjection2d, AleFsiRemeshProjectionEvidence2d,
    project_simplicial_ale_fsi_remesh_2d,
};
pub use simplicial_elliptic::{
    ScalarEllipticSimplicialFemSolution, SimplicialP1Field,
    linearize_scalar_elliptic_simplicial_compliance, linearize_scalar_elliptic_simplicial_fem,
    solve_scalar_elliptic_simplicial_fem, solve_scalar_elliptic_simplicial_fem_with_assembly,
};
pub use simplicial_fsi::{
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
pub use simplicial_motion::SimplicialMeshVelocity;
pub use simplicial_navier_stokes::{
    MiniNavierStokesStepPlan2d, SimplicialMiniNavierStokesState2d,
    SimplicialMiniNavierStokesStepEvidence2d, SimplicialMiniNavierStokesTrajectory2d,
    advance_simplicial_mini_navier_stokes_2d,
    advance_simplicial_mini_navier_stokes_2d_with_assembly,
};
pub use simplicial_stokes::{
    SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
    SimplicialMiniStokesBoundaryFacet2d, SimplicialMiniStokesErrorNorms2d,
    SimplicialMiniStokesPressureReference2d, SimplicialMiniStokesSolution2d,
    SimplicialMiniVelocityField2d, finalize_simplicial_mini_stokes_2d,
    finalize_simplicial_mini_stokes_2d_with_assembly,
    finalize_simplicial_mini_stokes_2d_with_boundary,
    finalize_simplicial_mini_stokes_2d_with_boundary_and_assembly, solve_simplicial_mini_stokes_2d,
    solve_simplicial_mini_stokes_2d_with_assembly, solve_simplicial_mini_stokes_2d_with_boundary,
    solve_simplicial_mini_stokes_2d_with_boundary_and_assembly,
};
pub use spatial_design::SpatialDesignCoordinate;
pub use spatial_expression::ScalarSpatialExpression;
pub use step_count::NonZeroStepCount;

/// Curated transitional numerical surface re-exported by the `eqiora` facade.
///
/// Direct `eqiora-numerics` users may access lower-level composition bridges.
/// The facade deliberately omits bridges that accept independently supplied
/// artifact identity and mesh bytes, and it omits the coherent-SI staging
/// state used only by `eqiora-api`.
pub mod facade {
    pub use super::{
        AcceptedAleFsiRemeshProjection2d, AcceptedResolvedAleFsiRemesh2d,
        AffineGeometryLinearization, AffineGeometryMap, AffineMapQuality, AleFsiBoundary2d,
        AleFsiBoundary3d, AleFsiCartesianModel2d, AleFsiCartesianModel3d, AleFsiFieldIdentities2d,
        AleFsiFieldIdentities3d, AleFsiInitialPhysicalState2d, AleFsiInitialPhysicalState3d,
        AleFsiInterfaceAction2d, AleFsiInterfaceAction3d, AleFsiRemeshProjectionEvidence2d,
        AleFsiState2d, AleFsiState3d, AleFsiStepEvidence2d, AleFsiStepEvidence3d, AleFsiStepPlan2d,
        AleFsiStepPlan3d, AleFsiTrajectory2d, AleFsiTrajectory3d, AssembledLinearizedRelation,
        AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyResult,
        AssemblyTarget, AssemblyTargetId, AssemblyWork, BasisTabulation, CartesianBoundaryEntry2d,
        CartesianBoundaryInventory2d, CartesianLinearElasticity2dSolution, CartesianMesh,
        CartesianQ1Field, CartesianQ1VectorField2d, CartesianVectorErrorNorms,
        CellCenteredNavierStokesInitialState2d, CellCenteredNavierStokesStepEvidence2d,
        CellCenteredPressureField2d, CellCenteredVelocityField2d, CellConstantSpace, CellId,
        ConformingCartesianInterfaceMap2d, ConformingCartesianLinearElasticityPair2dSolution,
        ConformingElasticityInterface2d, ConformingElasticityInterfaceAction2d,
        ConformingElasticityInterfaceSide2d, ConformingIsotropicElasticityCartesianPair2d,
        CooAssembler, CsrMatrix, DefaultScalarElliptic1dConfig, Diffusion1d, DirichletBoundary1d,
        DiscreteSpace, DofId, EntityIncidence, FacetCells1d, FacetId,
        FinalizedConformingIsotropicElasticityCartesianPair2dProblem,
        FinalizedFixedReferenceFsiStep2d, FinalizedIsotropicElasticityCartesian2dProblem,
        FinalizedResolvedFixedReferenceFsiStep2d, FinalizedResolvedFixedTopologyAleFsi2d,
        FinalizedResolvedFixedTopologyAleFsi3d, FinalizedScalarEllipticCartesianProblem,
        FinalizedScalarTransportFvmStep2d, FinalizedSimplicialMiniStokes2dProblem,
        FinalizedSteadyStokesMini2dProblem, FixedReferenceFsiBoundary2d,
        FixedReferenceFsiCartesianModel2d, FixedReferenceFsiEnergyBalance2d,
        FixedReferenceFsiFieldIdentities2d, FixedReferenceFsiInterface2d,
        FixedReferenceFsiInterfaceAction2d, FixedReferenceFsiInterfaceSide2d,
        FixedReferenceFsiLoad2d, FixedReferenceFsiMaterial2d, FixedReferenceFsiPartition2d,
        FixedReferenceFsiPartition3d, FixedReferenceFsiScale2d, FixedReferenceFsiScaleProfile2d,
        FixedReferenceFsiSolution2d, FixedReferenceFsiState2d, FixedReferenceFsiStepConfig2d,
        GeometryMap, HypercubeQ1Space, IncompressibleFlowScaleProfile2d, IndexedAssemblyWork,
        InertialIncompressibleNewtonianCartesianModel2d, IsotropicElasticityCartesianModel2d,
        IsotropicElastodynamicsCartesianModel2d, LineGeometryMap, LineMesh, LinearSystem,
        LocalContribution, LocalDof, LocalOperator, LocalUnknown, MeshEntity, MeshGeometry,
        MeshQualityGate, MeshQualityReport, MeshTopology, MiniNavierStokesStepPlan2d,
        NonZeroStepCount, OrientationCode, P1HarmonicMeshMotion2d, P1HarmonicMeshMotion3d,
        PhysicalBoundaryDisposition, PhysicalBoundaryQuantity, PiecewiseLinearField1d,
        PointGeometry1d, PoissonComparisonRow, PoissonSolution1d, PrescribedBoundaryLaw,
        QuadraturePoint, QuadratureRule, REFERENCE_ASSEMBLY_BACKEND, ReferenceAssemblyBackend,
        ReferenceCell, ReferenceCellFamily, ReferenceEntity, ReferenceIncidence, ReferenceTopology,
        ResolvedCellCenteredNavierStokesState2d, ResolvedCellCenteredNavierStokesTrajectory2d,
        ResolvedFixedReferenceFsiSolution2d, ResolvedScalarEllipticCartesianSolution,
        ResolvedScalarEllipticSolution1d, ResolvedTransientNavierStokesState2d,
        ResolvedTransientNavierStokesTrajectory2d, ScalarBoundaryCondition1d, ScalarBoundaryPair1d,
        ScalarEllipticCartesianBoundary, ScalarEllipticCartesianFemSolution,
        ScalarEllipticCartesianFvmSolution, ScalarEllipticCartesianModel,
        ScalarEllipticComparisonRow1d, ScalarEllipticFvmSolution1d, ScalarEllipticModel1d,
        ScalarEllipticSimplicialFemSolution, ScalarEllipticSolution1d, ScalarPhysicalAffineProblem,
        ScalarPhysicalAffineSolution, ScalarSpatialExpression, ScalarTransportBoundaryRole,
        ScalarTransportCartesianBoundary, ScalarTransportCartesianModel2d,
        ScalarTransportCellState2d, ScalarTransportFvmStep2d, ScalarTransportFvmStepEvidence2d,
        SegmentGeometry1d, SimplexP1BubbleSpace, SimplexP1Space, SimplicialMesh,
        SimplicialMeshVelocity, SimplicialMiniNavierStokesState2d,
        SimplicialMiniNavierStokesStepEvidence2d, SimplicialMiniNavierStokesTrajectory2d,
        SimplicialMiniStokesBoundary2d, SimplicialMiniStokesBoundaryCondition2d,
        SimplicialMiniStokesBoundaryFacet2d, SimplicialMiniStokesErrorNorms2d,
        SimplicialMiniStokesPressureReference2d, SimplicialMiniStokesSolution2d,
        SimplicialMiniVelocityField2d, SimplicialP1Field, SpatialDesignCoordinate,
        SteadyIncompressibleStokesCartesianModel2d, SteadyStokesMiniSolution2d,
        SteadyStokesNormalPressure2d, SteadyStokesPressureReference2d, SteadyStokesScaleProfile2d,
        TargetAssemblyMap, TransientIncompressibleNavierStokesCartesianModel2d,
        TransientNavierStokesRun2d, VertexId, VertexPermutation,
        advance_resolved_transient_navier_stokes_cell_centered_2d, advance_simplicial_ale_fsi_2d,
        advance_simplicial_ale_fsi_2d_with_assembly, advance_simplicial_mini_navier_stokes_2d,
        advance_simplicial_mini_navier_stokes_2d_with_assembly,
        compare_canonical_scalar_elliptic_1d, compare_sine_poisson_1d,
        finalize_fixed_reference_fsi_step_2d, finalize_fixed_reference_fsi_step_2d_with_assembly,
        finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
        finalize_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
        finalize_resolved_fixed_reference_fsi_step_2d, finalize_resolved_fixed_topology_ale_fsi_2d,
        finalize_resolved_fixed_topology_ale_fsi_3d,
        finalize_resolved_isotropic_elasticity_cartesian_2d,
        finalize_resolved_isotropic_elasticity_cartesian_2d_with_assembly,
        finalize_resolved_scalar_elliptic_cartesian,
        finalize_resolved_scalar_elliptic_cartesian_with_assembly,
        finalize_resolved_scalar_transport_fvm_step_2d,
        finalize_resolved_scalar_transport_fvm_step_2d_with_assembly,
        finalize_resolved_steady_stokes_mini_2d,
        finalize_resolved_steady_stokes_mini_2d_with_assembly, finalize_simplicial_mini_stokes_2d,
        finalize_simplicial_mini_stokes_2d_with_assembly,
        finalize_simplicial_mini_stokes_2d_with_boundary,
        finalize_simplicial_mini_stokes_2d_with_boundary_and_assembly, fixed_reference_fsi_plan_2d,
        fixed_reference_fsi_requirements_2d, fixed_topology_ale_fsi_requirements_2d,
        fixed_topology_ale_fsi_requirements_3d, initialize_resolved_scalar_transport_fvm_2d,
        linearize_scalar_elliptic_cartesian_fem, linearize_scalar_elliptic_cartesian_fvm,
        linearize_scalar_elliptic_simplicial_compliance, linearize_scalar_elliptic_simplicial_fem,
        lower_ale_fsi_cartesian_2d, lower_ale_fsi_cartesian_3d,
        lower_cartesian_q1_diffusion_local_action,
        lower_cartesian_q1_linear_elasticity_local_action_2d,
        lower_conforming_isotropic_elasticity_cartesian_pair_2d,
        lower_fixed_reference_fsi_cartesian_2d,
        lower_inertial_incompressible_newtonian_cartesian_2d,
        lower_isotropic_elasticity_cartesian_2d, lower_isotropic_elastodynamics_cartesian_2d,
        lower_scalar_elliptic_1d, lower_scalar_elliptic_cartesian, lower_scalar_physical_affine,
        lower_scalar_transport_cartesian_2d, lower_steady_incompressible_stokes_cartesian_2d,
        lower_transient_incompressible_navier_stokes_cartesian_2d,
        project_simplicial_ale_fsi_remesh_2d, remesh_resolved_fixed_topology_ale_fsi_2d,
        simplex_centroid_rule, solve_and_linearize_resolved_scalar_elliptic_cartesian,
        solve_and_linearize_resolved_scalar_elliptic_cartesian_with_assembly,
        solve_cartesian_q1_linear_elasticity_2d,
        solve_cartesian_q1_linear_elasticity_2d_with_assembly, solve_default_scalar_elliptic_1d,
        solve_fixed_reference_fsi_step_2d, solve_poisson_cell_fvm, solve_poisson_linear_fem,
        solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d,
        solve_resolved_conforming_isotropic_elasticity_cartesian_pair_2d_with_assembly,
        solve_resolved_fixed_topology_ale_fsi_2d,
        solve_resolved_fixed_topology_ale_fsi_2d_with_assembly,
        solve_resolved_fixed_topology_ale_fsi_3d,
        solve_resolved_fixed_topology_ale_fsi_3d_with_assembly,
        solve_resolved_isotropic_elasticity_cartesian_2d,
        solve_resolved_isotropic_elasticity_cartesian_2d_with_assembly,
        solve_resolved_scalar_elliptic_1d, solve_resolved_scalar_elliptic_1d_with_assembly,
        solve_resolved_scalar_elliptic_cartesian,
        solve_resolved_scalar_elliptic_cartesian_with_assembly,
        solve_resolved_scalar_elliptic_simplicial,
        solve_resolved_scalar_elliptic_simplicial_with_assembly,
        solve_resolved_scalar_transport_fvm_step_2d,
        solve_resolved_scalar_transport_fvm_step_2d_with_assembly,
        solve_resolved_steady_stokes_mini_2d, solve_resolved_steady_stokes_mini_2d_with_assembly,
        solve_scalar_elliptic_cartesian_fem, solve_scalar_elliptic_cartesian_fem_with_assembly,
        solve_scalar_elliptic_cartesian_fvm, solve_scalar_elliptic_cartesian_fvm_with_assembly,
        solve_scalar_elliptic_cell_fvm, solve_scalar_elliptic_linear_fem,
        solve_scalar_elliptic_linear_fem_with_assembly, solve_scalar_elliptic_simplicial_fem,
        solve_scalar_elliptic_simplicial_fem_with_assembly, solve_scalar_physical_affine,
        solve_scalar_physical_affine_with_initial_guess, solve_simplicial_mini_stokes_2d,
        solve_simplicial_mini_stokes_2d_with_assembly,
        solve_simplicial_mini_stokes_2d_with_boundary,
        solve_simplicial_mini_stokes_2d_with_boundary_and_assembly,
        steady_stokes_fieldwise_requirements_2d, steady_stokes_mini_plan_2d,
        transient_navier_stokes_cell_centered_plan_2d,
        transient_navier_stokes_cell_centered_requirements_2d,
        transient_navier_stokes_fieldwise_requirements_2d, triangle_duffy_gauss_legendre,
    };
}
