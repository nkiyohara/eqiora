//! Artifact-authenticated application boundary for transient incompressible flow.

use std::num::NonZeroUsize;
use std::sync::Arc;

use eqiora_artifact::SimplicialMeshEnvelopeV1;
use eqiora_core::{Diagnostic, DynQuantity};
use eqiora_numerics::{
    IncompressibleFlowScaleProfile2d, ResolvedTransientNavierStokesTrajectory2d,
    SimplicialMiniVelocityField2d, SimplicialP1Field, SteadyStokesPressureReference2d,
    TransientIncompressibleNavierStokesCartesianModel2d, TransientNavierStokesInitialState2d,
    TransientNavierStokesRun2d, advance_resolved_transient_navier_stokes_mini_2d,
    lower_transient_incompressible_navier_stokes_cartesian_2d,
    transient_navier_stokes_fieldwise_requirements_2d, transient_navier_stokes_mini_plan_2d,
};
use eqiora_realization::{
    DiscretizationMethod, MeshKind, NonlinearSolvePlan, PortableRealizationGraph,
    RealizationCapabilities, RealizationRevision, ResolvedTransientFieldwiseRealization,
    ScalarType, SemanticRevision, SpatialDimensionSupport, TargetCapabilities,
    TransientFieldwiseRealizationRequest, VectorLayoutKind, resolve_transient_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    BackendId, LinearOperatorProperties, LinearSolverBackend, SolverCapabilities, SolverCapability,
    SolverPlan,
};

/// Prepared fixed-domain 2D transient-flow application service.
///
/// Preparation owns one exact Semantic Program, lowered flow projection,
/// resolved Realization, authenticated mesh envelope, and executable solver
/// adapter. Callers cannot pair a foreign Model, revision, mesh digest, mesh
/// bytes, or backend with the later run by independently wiring lower-level
/// values.
#[derive(Debug, Clone)]
pub struct TransientNavierStokesReference2d {
    program: KernelProgram,
    model: TransientIncompressibleNavierStokesCartesianModel2d,
    realization: ResolvedTransientFieldwiseRealization,
    realization_graph: PortableRealizationGraph,
    mesh: SimplicialMeshEnvelopeV1,
    solver: Arc<dyn LinearSolverBackend>,
    solver_backend: BackendId,
    solver_capabilities: SolverCapabilities,
}

impl TransientNavierStokesReference2d {
    /// Resolve the bounded serial-host MINI/P1 reference application.
    ///
    /// The owned backend contributes its typed solver capabilities. Method,
    /// mesh, scalar, layout, placement requirement, schedule, quadrature,
    /// scaling, nonlinear policy, and independent Realization revision are
    /// resolved here through the ordinary typed Realization path. The backend
    /// still validates the requested serial execution at every solve; the
    /// graph-shaped deployment binding is owned by RFC 0058.
    ///
    /// # Errors
    /// Preserves canonical lowering, mesh-artifact, plan, and capability
    /// diagnostics without fallback.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare<B>(
        program: &KernelProgram,
        mesh: SimplicialMeshEnvelopeV1,
        scales: IncompressibleFlowScaleProfile2d,
        time_step: DynQuantity,
        nonlinear: NonlinearSolvePlan,
        linear_solver: SolverPlan,
        realization_revision: RealizationRevision,
        backend: B,
    ) -> Result<Self, Diagnostic>
    where
        B: LinearSolverBackend + 'static,
    {
        let model = lower_transient_incompressible_navier_stokes_cartesian_2d(program)?;
        let plan = transient_navier_stokes_mini_plan_2d(
            &model,
            mesh.artifact_reference()?,
            scales,
            time_step,
            nonlinear,
            linear_solver,
        )?;
        let solver_backend = backend.id();
        let solver_capabilities = backend.capabilities();
        solver_capabilities.require_problem(
            linear_solver,
            ScalarType::F64,
            LinearOperatorProperties::General,
        )?;
        let selected_solver = SolverCapabilities::exact([SolverCapability {
            algorithm: linear_solver.algorithm(),
            operator_properties: LinearOperatorProperties::General,
            preconditioner: linear_solver.preconditioner(),
            reduction: linear_solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?;
        let capabilities = RealizationCapabilities::cartesian_product(
            [DiscretizationMethod::ContinuousGalerkin],
            [(
                MeshKind::ImportedAffineSimplicial,
                SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two is non-zero")),
            )],
            [VectorLayoutKind::Replicated],
            selected_solver,
            TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
        )?;
        let realization = resolve_transient_fieldwise(
            &TransientFieldwiseRealizationRequest::explicit(
                program.model(),
                SemanticRevision::new(program.revision().0),
                realization_revision,
                plan,
            ),
            transient_navier_stokes_fieldwise_requirements_2d(&model),
            &capabilities,
        )?;
        let realization_graph = realization.portable_graph()?;
        Ok(Self {
            program: program.clone(),
            model,
            realization,
            realization_graph,
            mesh,
            solver: Arc::new(backend),
            solver_backend,
            solver_capabilities,
        })
    }

    /// Exact lowered conservative flow projection.
    #[must_use]
    pub const fn model(&self) -> &TransientIncompressibleNavierStokesCartesianModel2d {
        &self.model
    }

    /// Exact typed Realization selected during preparation.
    #[must_use]
    pub const fn realization(&self) -> &ResolvedTransientFieldwiseRealization {
        &self.realization
    }

    /// Canonical portable DAG admitted before the backend is retained.
    #[must_use]
    pub const fn realization_graph(&self) -> &PortableRealizationGraph {
        &self.realization_graph
    }

    /// Exact solver adapter admitted and retained during preparation.
    #[must_use]
    pub fn solver_backend(&self) -> BackendId {
        self.solver_backend
    }

    /// Admit coherent-SI initial Fields against this exact mesh and Model.
    ///
    /// The returned opaque value cannot enter the dimensionless reference
    /// kernel directly. Continuity, pressure mean, and gauge consistency are
    /// reassembled before any nonlinear Jacobian or CSR is constructed.
    ///
    /// # Errors
    /// Returns a typed Realization diagnostic for foreign mesh data, invalid
    /// physical time, or inconsistent Field shapes.
    pub fn initial_condition(
        &self,
        time: DynQuantity,
        velocity: SimplicialMiniVelocityField2d,
        pressure: SimplicialP1Field,
        pressure_reference: SteadyStokesPressureReference2d,
    ) -> Result<TransientNavierStokesInitialCondition2d, Diagnostic> {
        if velocity.mesh() != self.mesh.mesh() || pressure.mesh() != self.mesh.mesh() {
            return Err(Diagnostic::error(
                eqiora_core::diagnostic::codes::INVALID_REALIZATION,
                "transient initial fields do not belong to the authenticated mesh revision",
            ));
        }
        let state = TransientNavierStokesInitialState2d::new(
            &self.model,
            time,
            self.mesh.artifact_reference()?,
            velocity,
            pressure,
            pressure_reference,
        )?;
        Ok(TransientNavierStokesInitialCondition2d { state })
    }

    /// Advance an admitted coherent-SI initial condition with the solver
    /// adapter owned by this prepared service.
    ///
    /// # Errors
    /// Preserves initial-consistency, Realization, block materialization,
    /// nonlinear, assembly, and solver diagnostics without fallback.
    pub fn advance(
        &self,
        initial: TransientNavierStokesInitialCondition2d,
        step_count: NonZeroUsize,
    ) -> Result<ResolvedTransientNavierStokesTrajectory2d, Diagnostic> {
        if self.realization.portable_graph()? != self.realization_graph {
            return Err(Diagnostic::error(
                eqiora_core::diagnostic::codes::INVALID_REALIZATION,
                "prepared transient portable Realization graph changed after admission",
            ));
        }
        if self.solver.id() != self.solver_backend
            || self.solver.capabilities() != self.solver_capabilities
        {
            return Err(Diagnostic::error(
                eqiora_core::diagnostic::codes::INVALID_REALIZATION,
                "prepared transient solver adapter changed its identity or capabilities",
            ));
        }
        advance_resolved_transient_navier_stokes_mini_2d(
            &self.program,
            &self.realization,
            &self.mesh,
            initial.state,
            TransientNavierStokesRun2d::new(eqiora_numerics::NonZeroStepCount::new(step_count)),
            self.solver.as_ref(),
        )
    }
}

/// Opaque coherent-SI initial condition bound to one exact Model and mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientNavierStokesInitialCondition2d {
    state: TransientNavierStokesInitialState2d,
}
