use std::sync::Arc;

use eqiora_assembly::AssemblyReport;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_realization::{
    DiscretizationMethod, PortableRealizationGraph, Target, VectorLayoutKind,
};
use eqiora_solver::{
    CanonicalCsrSystemView, ExecutionTopology, FixedOrderInnerProduct, LinearOperator,
    LinearOperatorOrientation, LinearOperatorProperties, LinearProblem, LinearSolution,
    ReplicatedLinearExecution, SERIAL_LINEAR_EXECUTION, SolverPlan,
};

use crate::cartesian_elasticity::{
    FinalizedCartesianElasticity2dAssembly, FinalizedCartesianElasticity2dState,
    FinalizedConformingCartesianElasticityPair2dAssembly,
    FinalizedConformingCartesianElasticityPair2dState,
};
use crate::cartesian_elliptic::{
    FinalizedCartesianFemAssembly, FinalizedCartesianFemState, FinalizedCartesianFvmAssembly,
    FinalizedCartesianFvmState,
};
use crate::discrete_block::{BlockMaterialization, DiscreteBlockSystem};
use crate::{
    CartesianLinearElasticity2dSolution, ConformingCartesianLinearElasticityPair2dSolution,
    ResolvedScalarEllipticCartesianSolution,
};

mod stokes;

pub use stokes::FinalizedSimplicialMiniStokes2dProblem;

/// Physics-neutral ownership and acceptance boundary shared by finalized
/// linear spatial problems.
///
/// Reconstruction state deliberately remains in each public wrapper. This
/// core owns only the exact Realization-selected execution contract and the
/// single canonical algebraic source accepted by every backend.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FinalizedLinearCore {
    solver: SolverPlan,
    vector_layout: VectorLayoutKind,
    target: Target,
    canonical_system: Arc<CanonicalCsrSystemView>,
    block_materialization: Option<BlockMaterialization>,
}

impl FinalizedLinearCore {
    pub(crate) fn new(
        solver: SolverPlan,
        vector_layout: VectorLayoutKind,
        target: Target,
        canonical_system: Arc<CanonicalCsrSystemView>,
    ) -> Self {
        Self {
            solver,
            vector_layout,
            target,
            canonical_system,
            block_materialization: None,
        }
    }

    pub(crate) fn with_block_system(
        mut self,
        block_system: &DiscreteBlockSystem,
        assembly_report: &AssemblyReport,
    ) -> Result<Self, Diagnostic> {
        self.block_materialization =
            Some(block_system.bind_materialization(&self.canonical_system, assembly_report)?);
        Ok(self)
    }

    fn operator_properties(&self) -> LinearOperatorProperties {
        self.canonical_system.properties()
    }

    pub(crate) const fn solver_plan(&self) -> SolverPlan {
        self.solver
    }

    pub(crate) const fn vector_layout(&self) -> VectorLayoutKind {
        self.vector_layout
    }

    pub(crate) fn canonical_csr_system_view(&self) -> &CanonicalCsrSystemView {
        self.canonical_system.as_ref()
    }

    pub(crate) fn linear_problem(&self) -> Result<LinearProblem<'_>, Diagnostic> {
        self.canonical_system.linear_problem()
    }

    pub(crate) fn validate_solution(&self, solution: &LinearSolution) -> Result<(), Diagnostic> {
        if let Some(materialization) = self.block_materialization {
            materialization.validate(&self.canonical_system)?;
        }
        let report = solution.report();
        if solution.values().len() != self.canonical_system.rows() {
            return Err(invalid_realization(
                "accepted linear solution shape differs from the finalized spatial system",
            ));
        }
        if report.orientation() != LinearOperatorOrientation::Normal {
            return Err(invalid_realization(
                "finalized spatial problem requires a normal-orientation solution",
            ));
        }
        if report.solver_plan() != self.solver {
            return Err(invalid_realization(
                "accepted linear solution used a different SolverPlan than the finalized spatial problem",
            ));
        }
        if report.completed_iterations() > self.solver.maximum_iterations().get() {
            return Err(invalid_realization(
                "accepted linear solution exceeds the finalized SolverPlan iteration limit",
            ));
        }
        validate_producer_topology(
            self.vector_layout,
            self.target,
            report.execution().topology(),
        )?;
        validate_verifier_topology(
            self.vector_layout,
            self.target,
            report.verification().topology(),
        )?;

        let rhs_squared = SERIAL_LINEAR_EXECUTION.inner_product(FixedOrderInnerProduct::new(
            self.canonical_system.right_hand_side(),
            self.canonical_system.right_hand_side(),
        )?)?;
        let expected_target = self.solver.residual_target(rhs_squared.sqrt())?;
        if report.residual_target().to_bits() != expected_target.to_bits() {
            return Err(invalid_realization(
                "accepted linear solution tolerance evidence differs from the finalized SolverPlan",
            ));
        }
        let mut residual = fallible_residual(self.canonical_system.rows())?;
        self.canonical_system
            .apply(solution.values(), &mut residual)?;
        for (value, rhs) in residual
            .iter_mut()
            .zip(self.canonical_system.right_hand_side())
        {
            *value = rhs - *value;
        }
        let residual_squared = SERIAL_LINEAR_EXECUTION
            .inner_product(FixedOrderInnerProduct::new(&residual, &residual)?)?;
        let exact_residual = residual_squared.sqrt();
        if exact_residual > expected_target {
            return Err(Diagnostic::error(
                codes::NUMERICAL_SOLVE_FAILED,
                format!(
                    "accepted linear solution residual {exact_residual:e} exceeds this finalized spatial problem target {expected_target:e}"
                ),
            ));
        }
        Ok(())
    }

    fn into_canonical_system(self) -> Arc<CanonicalCsrSystemView> {
        self.canonical_system
    }
}

/// Finalized algebraic handoff for one resolved Cartesian scalar-elliptic
/// realization.
///
/// The public boundary is deliberately method-neutral: an execution adapter
/// sees the immutable sparse system, asserted operator properties, sole
/// [`SolverPlan`], and assembly evidence. FEM constraint recovery and TPFA
/// reconstruction remain opaque until an independently accepted
/// [`LinearSolution`] is returned.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedScalarEllipticCartesianProblem {
    portable_realization: PortableRealizationGraph,
    method: DiscretizationMethod,
    core: FinalizedLinearCore,
    state: FinalizedScalarEllipticCartesianState,
}

#[derive(Debug, Clone, PartialEq)]
enum FinalizedScalarEllipticCartesianState {
    FiniteElement(FinalizedCartesianFemState),
    FiniteVolume(FinalizedCartesianFvmState),
}

impl FinalizedScalarEllipticCartesianProblem {
    pub(crate) fn finite_element(
        portable_realization: PortableRealizationGraph,
        solver: SolverPlan,
        vector_layout: VectorLayoutKind,
        target: Target,
        assembly: FinalizedCartesianFemAssembly,
    ) -> Result<Self, Diagnostic> {
        let (canonical_system, state) = assembly.into_canonical()?;
        Ok(Self {
            portable_realization,
            method: DiscretizationMethod::ContinuousGalerkin,
            core: FinalizedLinearCore::new(solver, vector_layout, target, canonical_system),
            state: FinalizedScalarEllipticCartesianState::FiniteElement(state),
        })
    }

    pub(crate) fn finite_volume(
        portable_realization: PortableRealizationGraph,
        solver: SolverPlan,
        vector_layout: VectorLayoutKind,
        target: Target,
        assembly: FinalizedCartesianFvmAssembly,
    ) -> Result<Self, Diagnostic> {
        let (canonical_system, state) = assembly.into_canonical()?;
        Ok(Self {
            portable_realization,
            method: DiscretizationMethod::CellCenteredFiniteVolume,
            core: FinalizedLinearCore::new(solver, vector_layout, target, canonical_system),
            state: FinalizedScalarEllipticCartesianState::FiniteVolume(state),
        })
    }

    /// Numerical method whose method-native state is retained opaquely.
    #[must_use]
    pub const fn method(&self) -> DiscretizationMethod {
        self.method
    }

    /// Equation-aware portable graph independently regenerated and checked by
    /// the finalizer that owns this exact canonical system.
    #[must_use]
    pub const fn portable_realization(&self) -> &PortableRealizationGraph {
        &self.portable_realization
    }

    /// Mathematical properties asserted by this resolved realization.
    #[must_use]
    pub fn operator_properties(&self) -> LinearOperatorProperties {
        self.core.operator_properties()
    }

    /// Exact backend-neutral solver policy selected by the Realization.
    #[must_use]
    pub const fn solver_plan(&self) -> SolverPlan {
        self.core.solver_plan()
    }

    /// Replicated or explicitly distributed vector layout admitted by the
    /// resolved Realization.
    #[must_use]
    pub const fn vector_layout(&self) -> VectorLayoutKind {
        self.core.vector_layout()
    }

    /// Borrow the single captured complete-CSR mathematical source.
    ///
    /// Distributed layout derivation and its execution adapter must consume
    /// this exact view. The view also supplies the host problem used by this
    /// handoff, so raw storage, distributed shards, and residual acceptance
    /// cannot select independent operator actions.
    #[must_use]
    pub fn canonical_csr_system_view(&self) -> &CanonicalCsrSystemView {
        self.core.canonical_csr_system_view()
    }

    /// Accepted assembly placement and packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        match &self.state {
            FinalizedScalarEllipticCartesianState::FiniteElement(state) => state.assembly_report(),
            FinalizedScalarEllipticCartesianState::FiniteVolume(state) => state.assembly_report(),
        }
    }

    /// Borrow the finalized system through the common solver problem contract.
    ///
    /// # Errors
    /// Returns `EQ0802` only if the already captured canonical view
    /// contradicts its construction invariants.
    pub fn linear_problem(&self) -> Result<LinearProblem<'_>, Diagnostic> {
        self.core.linear_problem()
    }

    /// Numerically reaccept and reconstruct one solution against this
    /// finalized problem.
    ///
    /// Validation is intentionally repeated at the handoff. This proves that
    /// the supplied vector satisfies these finalized arrays under the selected
    /// plan and topology; it does not prove which system originally produced
    /// the vector. A vector that satisfies two systems is admissible to both.
    /// Durable system identity is deferred to artifact persistence.
    ///
    /// # Errors
    /// Returns `EQ0807` for cross-wired plan/topology evidence and `EQ0802`
    /// when the values do not satisfy the finalized system or its bounded
    /// verification/reconstruction storage cannot be reserved.
    pub fn finish(
        self,
        solution: LinearSolution,
    ) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.core.validate_solution(&solution)?;
        let Self { core, state, .. } = self;
        let canonical_system = core.into_canonical_system();
        match state {
            FinalizedScalarEllipticCartesianState::FiniteElement(state) => state
                .finish(solution, canonical_system)
                .map(ResolvedScalarEllipticCartesianSolution::FiniteElement),
            FinalizedScalarEllipticCartesianState::FiniteVolume(state) => state
                .finish(solution, canonical_system)
                .map(ResolvedScalarEllipticCartesianSolution::FiniteVolume),
        }
    }
}

/// Finalized algebraic handoff for one resolved Cartesian Q1 elasticity
/// realization.
///
/// Boundary recognition and package normalization are complete before this
/// value exists. It owns one immutable reduced CSR system and keeps full-
/// system constraint/reaction state opaque until an independently accepted
/// [`LinearSolution`] is returned.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedIsotropicElasticityCartesian2dProblem {
    core: FinalizedLinearCore,
    state: FinalizedCartesianElasticity2dState,
}

impl FinalizedIsotropicElasticityCartesian2dProblem {
    pub(crate) fn new(
        solver: SolverPlan,
        vector_layout: VectorLayoutKind,
        target: Target,
        assembly: FinalizedCartesianElasticity2dAssembly,
    ) -> Result<Self, Diagnostic> {
        let (canonical_system, state) = assembly.into_canonical()?;
        Ok(Self {
            core: FinalizedLinearCore::new(solver, vector_layout, target, canonical_system),
            state,
        })
    }

    /// Mathematical properties asserted by the accepted Q1 realization.
    #[must_use]
    pub fn operator_properties(&self) -> LinearOperatorProperties {
        self.core.operator_properties()
    }

    /// Exact backend-neutral solver policy selected by the Realization.
    #[must_use]
    pub const fn solver_plan(&self) -> SolverPlan {
        self.core.solver_plan()
    }

    /// Accepted vector layout at the algebraic handoff.
    #[must_use]
    pub const fn vector_layout(&self) -> VectorLayoutKind {
        self.core.vector_layout()
    }

    /// Borrow the single captured reduced CSR system and right-hand side.
    #[must_use]
    pub fn canonical_csr_system_view(&self) -> &CanonicalCsrSystemView {
        self.core.canonical_csr_system_view()
    }

    /// Accepted assembly placement and packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        self.state.assembly_report()
    }

    /// Borrow the finalized system through the common solver problem contract.
    ///
    /// # Errors
    /// Returns `EQ0802` only if the captured canonical view contradicts its
    /// construction invariants.
    pub fn linear_problem(&self) -> Result<LinearProblem<'_>, Diagnostic> {
        self.core.linear_problem()
    }

    /// Reaccept one solution against this exact finalized system and recover
    /// the constrained vector field plus full-system balance evidence.
    ///
    /// # Errors
    /// Returns `EQ0807` for cross-wired plan/topology evidence and `EQ0802`
    /// when the values do not satisfy the finalized system or reconstruction
    /// storage cannot be reserved.
    pub fn finish(
        self,
        solution: LinearSolution,
    ) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
        self.core.validate_solution(&solution)?;
        self.state
            .finish(solution, self.core.into_canonical_system())
    }
}

/// Finalized monolithic algebraic handoff for two conforming elasticity bodies.
///
/// The reduced CSR system uses a Realization-owned interface vertex quotient.
/// Body-local meshes and cut systems remain opaque until solution acceptance,
/// where they reconstruct both fields and expose weak interface equilibrium.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedConformingIsotropicElasticityCartesianPair2dProblem {
    core: FinalizedLinearCore,
    state: FinalizedConformingCartesianElasticityPair2dState,
}

impl FinalizedConformingIsotropicElasticityCartesianPair2dProblem {
    pub(crate) fn new(
        solver: SolverPlan,
        vector_layout: VectorLayoutKind,
        target: Target,
        assembly: FinalizedConformingCartesianElasticityPair2dAssembly,
        block_system: DiscreteBlockSystem,
    ) -> Result<Self, Diagnostic> {
        let (canonical_system, state) = assembly.into_canonical()?;
        let core = FinalizedLinearCore::new(solver, vector_layout, target, canonical_system)
            .with_block_system(&block_system, state.assembly_report())?;
        Ok(Self { core, state })
    }

    /// Mathematical properties asserted by the accepted monolithic Q1 realization.
    #[must_use]
    pub fn operator_properties(&self) -> LinearOperatorProperties {
        self.core.operator_properties()
    }

    /// Exact backend-neutral solver policy selected by the Realization.
    #[must_use]
    pub const fn solver_plan(&self) -> SolverPlan {
        self.core.solver_plan()
    }

    /// Accepted vector layout at the algebraic handoff.
    #[must_use]
    pub const fn vector_layout(&self) -> VectorLayoutKind {
        self.core.vector_layout()
    }

    /// Borrow the single captured reduced quotient-space CSR system.
    #[must_use]
    pub fn canonical_csr_system_view(&self) -> &CanonicalCsrSystemView {
        self.core.canonical_csr_system_view()
    }

    /// Accepted assembly placement and packet-shape evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        self.state.assembly_report()
    }

    /// Borrow the finalized system through the common solver problem contract.
    ///
    /// # Errors
    /// Returns `EQ0802` only if the captured canonical view contradicts its
    /// construction invariants.
    pub fn linear_problem(&self) -> Result<LinearProblem<'_>, Diagnostic> {
        self.core.linear_problem()
    }

    /// Reaccept one solution and recover both body fields and interface balance.
    ///
    /// # Errors
    /// Returns `EQ0807` for cross-wired plan/topology evidence and `EQ0802`
    /// when the values do not satisfy the finalized quotient system.
    pub fn finish(
        self,
        solution: LinearSolution,
    ) -> Result<ConformingCartesianLinearElasticityPair2dSolution, Diagnostic> {
        self.core.validate_solution(&solution)?;
        let Self { core, state } = self;
        state.finish(solution, core.into_canonical_system())
    }
}

fn validate_producer_topology(
    vector_layout: VectorLayoutKind,
    target: Target,
    topology: ExecutionTopology,
) -> Result<(), Diagnostic> {
    match (vector_layout, target, topology) {
        (
            VectorLayoutKind::Replicated,
            Target::HostCpu { threads },
            ExecutionTopology::Host { workers },
        ) if workers <= threads => Ok(()),
        (
            VectorLayoutKind::Replicated,
            Target::CudaGpu { device },
            ExecutionTopology::Cuda { device: produced },
        ) if produced == device => Ok(()),
        (
            VectorLayoutKind::Distributed,
            Target::HostCpu { threads },
            ExecutionTopology::Distributed {
                workers_per_partition,
                ..
            },
        ) if threads == std::num::NonZeroUsize::MIN
            && workers_per_partition == std::num::NonZeroUsize::MIN =>
        {
            Ok(())
        }
        (
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 0 },
            ExecutionTopology::Distributed {
                workers_per_partition,
                ..
            },
        ) if workers_per_partition == std::num::NonZeroUsize::MIN => Ok(()),
        (VectorLayoutKind::Replicated, Target::HostCpu { .. }, _) => Err(invalid_realization(
            "a replicated host realization requires a host producer within its admitted worker bound",
        )),
        (VectorLayoutKind::Replicated, Target::CudaGpu { .. }, _) => Err(invalid_realization(
            "a replicated CUDA realization requires a producer on the exact resolved CUDA device",
        )),
        (VectorLayoutKind::Distributed, Target::HostCpu { .. }, _) => Err(invalid_realization(
            "a distributed host realization requires a distributed producer with exactly one worker per partition",
        )),
        (VectorLayoutKind::Distributed, Target::CudaGpu { .. }, _) => Err(invalid_realization(
            "a distributed CUDA realization requires a distributed producer with one host control worker per partition and deployment-local device ordinal zero",
        )),
    }
}

fn validate_verifier_topology(
    vector_layout: VectorLayoutKind,
    target: Target,
    topology: ExecutionTopology,
) -> Result<(), Diagnostic> {
    match (vector_layout, target, topology) {
        (
            VectorLayoutKind::Replicated,
            Target::HostCpu { threads },
            ExecutionTopology::Host { workers },
        ) if workers <= threads => Ok(()),
        (
            VectorLayoutKind::Replicated,
            Target::CudaGpu { .. },
            ExecutionTopology::Host { workers },
        )
        | (
            VectorLayoutKind::Distributed,
            Target::HostCpu { .. },
            ExecutionTopology::Host { workers },
        )
        | (
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 0 },
            ExecutionTopology::Host { workers },
        ) if workers == std::num::NonZeroUsize::MIN => Ok(()),
        (VectorLayoutKind::Replicated, Target::HostCpu { .. }, _) => Err(invalid_realization(
            "a replicated host realization requires host verification within its admitted worker bound",
        )),
        (VectorLayoutKind::Replicated, Target::CudaGpu { .. }, _) => Err(invalid_realization(
            "a replicated CUDA realization requires independent one-worker host verification",
        )),
        (VectorLayoutKind::Distributed, Target::HostCpu { .. }, _) => Err(invalid_realization(
            "a distributed host realization requires independent one-worker complete-host verification",
        )),
        (VectorLayoutKind::Distributed, Target::CudaGpu { .. }, _) => Err(invalid_realization(
            "a distributed CUDA realization requires independent one-worker complete-host verification and deployment-local device ordinal zero",
        )),
    }
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn fallible_residual(length: usize) -> Result<Vec<f64>, Diagnostic> {
    let mut residual = Vec::new();
    residual.try_reserve_exact(length).map_err(|_| {
        Diagnostic::error(
            codes::NUMERICAL_SOLVE_FAILED,
            "finalized spatial residual allocation exceeds platform capacity",
        )
    })?;
    residual.resize(length, 0.0);
    Ok(residual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_capacity_failure_is_a_stable_diagnostic() {
        let diagnostic = fallible_residual(usize::MAX).unwrap_err();

        assert_eq!(diagnostic.code(), codes::NUMERICAL_SOLVE_FAILED);
        assert_eq!(
            diagnostic.message(),
            "finalized spatial residual allocation exceeds platform capacity"
        );
    }

    #[test]
    fn distributed_cuda_keeps_distributed_production_and_host_verification_distinct() {
        let distributed = ExecutionTopology::Distributed {
            ranks: std::num::NonZeroUsize::new(4).unwrap(),
            workers_per_partition: std::num::NonZeroUsize::MIN,
        };
        let host = ExecutionTopology::Host {
            workers: std::num::NonZeroUsize::MIN,
        };

        validate_producer_topology(
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 0 },
            distributed,
        )
        .unwrap();
        validate_verifier_topology(
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 0 },
            host,
        )
        .unwrap();

        let foreign_device = validate_producer_topology(
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 1 },
            distributed,
        )
        .unwrap_err();
        assert!(foreign_device.message().contains("ordinal zero"));

        let device_verifier = validate_verifier_topology(
            VectorLayoutKind::Distributed,
            Target::CudaGpu { device: 0 },
            ExecutionTopology::Cuda { device: 0 },
        )
        .unwrap_err();
        assert!(device_verifier.message().contains("complete-host"));
    }
}
