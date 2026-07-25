//! **eqiora** — the public facade.
//!
//! This is the only crate downstream users and physics packages depend on.
//! Its stable and transitional exports are enumerated in
//! `api/eqiora-facade-v1.json`; internal crates may refactor without silently
//! expanding the curated surface. Physics packages must not depend on
//! `eqiora-*` crates directly; that rule is enforced socially now and by the
//! package toolchain later.
//! The repository's `docs/architecture.md#dependency-layers` section is the
//! human-readable contract; `cargo xtask check-layers` and
//! `cargo xtask check-facade` enforce the two boundaries.
//!
//! Each underlying crate is also independently usable: depending on
//! `eqiora-core` alone for its unit system is a legitimate use of Eqiora.

mod release_identity {
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}

pub use eqiora_core::diagnostic::{self, Code, Diagnostic, GraphPath, Patch, Severity, Span};
pub use eqiora_core::entity::{self, Entity, EntityKind, GraphClass, GraphKind, kinds};
pub use eqiora_core::id::{Id, RawId};
pub use eqiora_core::quantity::{
    self, DimExponents, Dimension, DynQuantity, Quantity, Scalar, aliases, dim,
};
pub use eqiora_core::{InvalidValueShape, ValueShape};
/// Exact Cargo SemVer release identity of the public Eqiora facade.
pub use release_identity::VERSION;

/// Shared application operations used by thin language and Studio clients.
pub mod api {
    pub use eqiora_api::package;
    pub use eqiora_api::{
        CadBoxIntentV1, CadBoxPlanV1, CadRegenerationPlanV1, CadRenderProjectionV1,
        CadRenderTriangleV1, CadSelectionRequestV1, CadSemanticEntityKindV1, CadSemanticEntityV1,
        CadSemanticSelectionV1, CartesianFieldOrder, CartesianScalarFieldProjection,
        DerivativeContract, DerivativeImplementation, DifferentiableDevice,
        DifferentiableEvaluation, DifferentiableJvp, DifferentiableParameterPoint,
        DifferentiablePrimal, DifferentiableProgram, DifferentiableProgramIdentity,
        DifferentiableScalarType, DifferentiableVjp, DifferentiationEvidence, DifferentiationMode,
        FixedReferenceFsiSnapshotSetV1, LinearizationState, MAX_SCALAR_ELLIPTIC_ENTITY_COUNT,
        MlDatasetArtifactsV1, MlDatasetBlockArrayV1, MlDatasetDerivationPlanV1,
        MlDatasetDescriptorRoleV1, MlDatasetFieldSelectionV1, MlDatasetMaterializationLimitsV1,
        MlDatasetMaterializationV1, MlDatasetSampleArraysV1, MlDatasetSampleSelectionV1,
        MlDatasetSampleSplitV1, ModelDocument, ModelFieldRef, ModelParameterRef,
        REFERENCE_EXECUTION_ADAPTER, ReferenceAcceptance, ReferenceExecutionPlacement,
        ReferenceIntegrationMethod, ReferenceNonlinearMethod, ReferenceRunCancellation,
        ReferenceRunDirective, ReferenceRunEvidence, ReferenceRunObserver, ReferenceRunOutcome,
        ReferenceRunPlan, ReferenceRunProgress, ReferenceRunResult, ReferenceSeries,
        RemeshingTrajectoryReplayInputV1, ScalarEllipticBalanceEvidence,
        ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
        ScalarEllipticRunCancellation, ScalarEllipticRunDirective, ScalarEllipticRunObserver,
        ScalarEllipticRunOutcome, ScalarEllipticRunPlan, ScalarEllipticRunProgress,
        ScalarEllipticRunResult, ScalarFieldLocation, ScalarFieldSummary,
        SemanticFingerprintGeneration, StructuralSemanticFingerprint,
        TransientNavierStokesInitialCondition2d, TransientNavierStokesReference2d, ValueEditPlan,
        ValueEditResult, VerifiedMlDatasetV1, derive_ml_dataset_v1,
        snapshot_fixed_reference_fsi_solution_v1, verify_ml_dataset_v1,
    };

    /// Fresh XDMF artifact derivation and verified persisted replay.
    #[cfg(feature = "xdmf")]
    pub use eqiora_api::{
        VerifiedXdmfImportV1, XdmfImportArtifactsV1, import_xdmf_v1, verify_xdmf_import_v1,
    };

    /// Fresh VTU artifact derivation and verified persisted replay.
    #[cfg(feature = "vtu")]
    pub use eqiora_api::{
        VerifiedVtuImportV1, VtuImportArtifactsV1, import_vtu_v1, verify_vtu_import_v1,
    };

    /// Native HDF5-backed XDMF import and remeshing-trajectory export.
    #[cfg(feature = "hdf5")]
    pub use eqiora_api::{
        VerifiedXdmfHdf5TrajectoryExportV1, XdmfHdf5TrajectoryExportArtifactsV1,
        XdmfHdf5TrajectoryExportLimits, export_xdmf_hdf5_trajectory_v1, import_xdmf_hdf5_v1,
        verify_xdmf_hdf5_import_v1, verify_xdmf_hdf5_trajectory_storage_v1,
    };
}

/// Exact historical artifact codecs and replay selection.
///
/// Ordinary authoring belongs to [`api`] and never requires this namespace.
pub mod compatibility {
    pub use eqiora_api::ExactModelCodec;
}

/// Versioned, transport-neutral commands and diagnostics for thin clients.
pub mod control {
    pub use eqiora_api::control::{
        COMPILE_COMMAND_V1, COMPILE_FEATURE_V1, COMPILE_V1_SCHEMA_JSON, CONTROL_PROTOCOL_V1,
        CompileControlExecutionV1, CompileFeatureV1, CompileModelDescriptorV1, CompileOutcomeV1,
        CompileRequestV1, CompileResponseV1, ControlDiagnosticSourceV1, ControlDiagnosticV1,
        ControlPatchV1, ControlSeverityV1, ControlSourceSpanV1, MAX_COMPILE_FILENAME_BYTES_V1,
        MAX_COMPILE_REQUEST_BYTES_V1, MAX_COMPILE_REQUIRED_FEATURES_V1,
        MAX_COMPILE_RESPONSE_BYTES_V1, MAX_COMPILE_SOURCE_BYTES_V1,
        MAX_CONTROL_REQUEST_ID_BYTES_V1, execute_compile_v1, generated_compile_v1_schema_json,
    };
}

/// Closed Semantic Kernel node definitions and residual expressions.
pub mod kernel {
    pub use eqiora_schema::kernel::*;
}

/// Standard Ontology: typed named subgraphs, never graph nodes.
pub mod ontology {
    pub use eqiora_core::{NamedSubgraph, OntologyId, OntologySchema, OntologyView, RawOntologyId};
    pub use eqiora_schema::{
        Coupling, CouplingView, EvidenceSet, EvidenceSetView, Model, ModelView, Objective,
        ObjectiveView, Scale, ScaleView, Solver, SolverView,
    };
}

/// Graph Federation: stores, typed transactions, semantic diff.
pub mod graph {
    pub use eqiora_graph::{
        CommitRecord, Committed, Edge, EdgeKind, GraphStore, InMemoryGraphStore, Node, Op,
        Precondition, Revision, Snapshot, Transaction,
    };
}

/// Eqiora Language syntax, lossless tokens, parser, and canonical formatter.
pub mod language {
    pub use eqiora_lang::*;
}

/// Typed Eqiora Language lowering to Graph Federation transactions.
pub mod compiler {
    pub use eqiora_compiler::*;
}

/// Exact, offline model-package contracts and the locked compilation facade.
pub mod package {
    pub use eqiora_api::package::*;
    pub use eqiora_package::*;
}

/// Backend-independent lowered representations.
pub mod ir {
    pub use eqiora_ir::{
        BoundAffineFailure, BoundAffineScalarIr, ComponentScalarRow, ComponentScalarization,
        ConstantSymbolJacobian, DifferentiationRole, DiscreteStepLinearization, LinearizedOutput,
        LinearizedRelation, LocalLinearActionIr, RelationCotangent, RelationTangent,
        ScalarInputOperatorIr, ScalarInputSlot, ScalarLinearization, ScalarObjectiveLinearization,
        ScalarOperatorIr, ScalarSymbolCoordinate, SymbolicLinearityFailure,
    };
}

/// Implicit forward and adjoint differentiation over lowered relations.
pub mod differentiation {
    pub use eqiora_differentiation::*;
}

/// Backend-neutral local contributions, assembly maps, and sparse algebra.
pub mod assembly {
    pub use eqiora_assembly::*;
}

/// Backend-neutral device identity, capability, residency, and evidence.
pub mod device {
    pub use eqiora_device::*;
}

/// Numerical realizations kept separate from canonical model meaning.
pub mod numerics {
    pub use eqiora_numerics::ale::{
        AleFsiCartesianModel2d, FinalizedResolvedFixedTopologyAleFsi2d,
        finalize_resolved_fixed_topology_ale_fsi_2d, lower_ale_fsi_cartesian_2d,
    };
    pub use eqiora_numerics::fluid::{
        FinalizedSteadyStokesMini2dProblem, SteadyIncompressibleStokesCartesianModel2d,
        SteadyStokesMiniSolution2d, finalize_resolved_steady_stokes_mini_2d,
        lower_steady_incompressible_stokes_cartesian_2d, solve_resolved_steady_stokes_mini_2d,
    };
    pub use eqiora_numerics::fsi::{
        FinalizedResolvedFixedReferenceFsiStep2d, FixedReferenceFsiCartesianModel2d,
        ResolvedFixedReferenceFsiSolution2d, finalize_resolved_fixed_reference_fsi_step_2d,
        lower_fixed_reference_fsi_cartesian_2d,
    };
    pub use eqiora_numerics::scalar::{
        FinalizedScalarEllipticCartesianProblem, ResolvedScalarEllipticCartesianSolution,
        ScalarEllipticCartesianModel, finalize_resolved_scalar_elliptic_cartesian,
        lower_scalar_elliptic_cartesian, solve_resolved_scalar_elliptic_cartesian,
    };
    pub use eqiora_numerics::solid::{
        CartesianLinearElasticity2dSolution, FinalizedIsotropicElasticityCartesian2dProblem,
        IsotropicElasticityCartesianModel2d, finalize_resolved_isotropic_elasticity_cartesian_2d,
        lower_isotropic_elasticity_cartesian_2d, solve_resolved_isotropic_elasticity_cartesian_2d,
    };
}

/// Backend-neutral mesh topology, affine geometry, and quality contracts.
pub mod meshing {
    pub use eqiora_meshing::*;
}

/// Geometry identity, geometry-to-mesh correspondence, and kernel-neutral CAD
/// contracts.
pub mod geometry {
    pub use eqiora_geometry::*;

    /// Concrete bounded Truck STEP/B-rep implementation of the CAD contracts.
    #[cfg(feature = "cad-truck")]
    pub mod truck {
        pub use eqiora_cad_truck::*;
    }
}

/// Optional external-format adapters. Imported data is always reconstructed
/// through the backend-neutral contracts exposed by [`meshing`].
pub mod io {
    /// Bounded ASCII and binary Gmsh MSH 4.1 simplex import.
    #[cfg(feature = "gmsh")]
    pub mod gmsh {
        pub use eqiora_io_gmsh::*;
    }

    /// Pure, bounded XDMF 3 metadata planning and caller-owned array replay.
    #[cfg(feature = "xdmf")]
    pub mod xdmf {
        pub use eqiora_io_xdmf::*;
    }

    /// Pure, bounded VTK XML UnstructuredGrid import.
    #[cfg(feature = "vtu")]
    pub mod vtu {
        pub use eqiora_io_vtu::*;
    }

    /// Native HDF5 file-image resolution with no caller path authority.
    #[cfg(feature = "hdf5")]
    pub mod hdf5 {
        pub use eqiora_io_hdf5::*;
    }
}

/// Backend-neutral solver plans, operators, capabilities, and evidence.
pub mod solver {
    pub use eqiora_solver::*;
}

/// Backend-neutral lowered time problems, adaptive plans, and evidence.
pub mod time {
    pub use eqiora_time::*;
}

/// Transport-neutral distributed ownership, halo, and collective contracts.
pub mod distributed {
    pub use eqiora_distributed::*;
}

/// Optional production execution adapters.
pub mod backends {
    /// faer host linear-algebra adapter.
    #[cfg(feature = "faer")]
    pub mod faer {
        pub use eqiora_backend_faer::*;
    }

    /// MPI distributed-transport adapter.
    #[cfg(feature = "mpi")]
    pub mod mpi {
        pub use eqiora_backend_mpi::*;
    }

    /// Diffsol adaptive ODE and mass-matrix DAE adapter.
    #[cfg(feature = "diffsol")]
    pub mod diffsol {
        pub use eqiora_backend_diffsol::*;
    }

    /// Dynamically loaded single-device CUDA adapter.
    #[cfg(feature = "cuda")]
    pub mod cuda {
        pub use eqiora_backend_cuda::{
            CUDA_ADAPTER_VERSION, CUDA_BINDING_TOOLKIT, CUDA_LINEAR_EXECUTION,
            CUDA_LINEAR_EXECUTION_PROVIDER, CUDA_LINEAR_SOLVER_BACKEND,
            CUDA_LINEAR_SOLVER_PROVIDER, CUDA_RUNTIME_ID, CUDARC_VERSION, CudaComputeCapability,
            CudaCsrActionEvidence, CudaCsrActionResult, CudaCsrTransferEvidence,
            CudaLibraryVersions, CudaLinearSolveEvidence, CudaLinearSolveResult, CudaLinearSolver,
            CudaLinearTransferEvidence, CudaRuntime, verify_csr_action, verify_csr_action_against,
        };
    }

    /// Explicit host-staged MPI plus rank-local CUDA composition adapter.
    #[cfg(feature = "mpi-cuda")]
    pub mod mpi_cuda {
        pub use eqiora_backend_mpi_cuda::*;
    }

    /// Run-owned Rayon CPU adapter.
    #[cfg(feature = "rayon")]
    pub mod rayon {
        pub use eqiora_backend_rayon::*;
    }
}

/// Versioned model/run artifacts and deterministic content identity.
pub mod artifact {
    pub use eqiora_artifact::*;
}

/// Typed Realization Graph policies kept separate from model meaning.
pub mod realization {
    pub use eqiora_realization::*;
}

/// Lowered Rust CPU execution and backend conformance.
pub mod runtime {
    pub use eqiora_runtime::*;
}

/// Reference semantics: the interpreter that defines what programs mean.
pub mod sem {
    pub use eqiora_sem::{
        ComposedResidualSystem, Interpreter, JunctionResidual, KernelProgram, PhysicalUnknown,
        ReferenceConfig, RelationResidual, Sample, ScalarPhysicalSubsystemId, Trajectory,
    };
}
