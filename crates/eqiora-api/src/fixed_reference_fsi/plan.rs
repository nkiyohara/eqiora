//! Explicit intent and pre-execution Plan for the accepted FSI application.

use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, REFERENCE_LINEAR_SOLVER,
    REFERENCE_SOLVER_PROVIDER, ScalarType, SolverCapabilities, SolverProvider,
};

use super::*;

/// Typed request for the accepted fixed-mesh monolithic FSI execution.
///
/// Every numerical and initial-state input is explicit. Construction validates
/// their basic domains; resolution admits only the one tuple already owned by
/// the registered fixed-reference scientific evidence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedMeshMonolithicFsiIntent2d {
    time_step_s: f64,
    steps: NonZeroUsize,
    initial_velocity_m_per_s: [f64; 2],
    initial_free_interface_displacement_m: [f64; 2],
    length_scale_m: f64,
    velocity_scale_m_per_s: f64,
    pressure_scale_pa: f64,
    solver: SolverPlan,
}

impl FixedMeshMonolithicFsiIntent2d {
    /// Construct a complete request with no hidden numerical defaults.
    ///
    /// # Errors
    /// Returns `EQ0807` when a physical value is non-finite, a time or scale
    /// is not strictly positive, or a solver tolerance is invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_step_s: f64,
        steps: NonZeroUsize,
        initial_velocity_m_per_s: [f64; 2],
        initial_free_interface_displacement_m: [f64; 2],
        length_scale_m: f64,
        velocity_scale_m_per_s: f64,
        pressure_scale_pa: f64,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: NonZeroUsize,
    ) -> Result<Self, Diagnostic> {
        if !time_step_s.is_finite() || time_step_s <= 0.0 {
            return Err(invalid(
                "fixed-mesh monolithic FSI time step must be finite and strictly positive",
            ));
        }
        if initial_velocity_m_per_s
            .into_iter()
            .chain(initial_free_interface_displacement_m)
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "fixed-mesh monolithic FSI initial state must be finite",
            ));
        }
        if [length_scale_m, velocity_scale_m_per_s, pressure_scale_pa]
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(invalid(
                "fixed-mesh monolithic FSI scales must be finite and strictly positive",
            ));
        }
        if !relative_tolerance.is_finite()
            || !absolute_tolerance.is_finite()
            || relative_tolerance <= 0.0
            || absolute_tolerance <= 0.0
        {
            return Err(invalid(
                "fixed-mesh monolithic FSI tolerances must be finite and strictly positive",
            ));
        }
        let solver = SolverPlan::new(
            LinearSolver::MinimumResidual,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )?
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Reproducible);
        Ok(Self {
            time_step_s,
            steps,
            initial_velocity_m_per_s,
            initial_free_interface_displacement_m,
            length_scale_m,
            velocity_scale_m_per_s,
            pressure_scale_pa,
            solver,
        })
    }

    /// Physical time increment between consecutive states.
    #[must_use]
    pub const fn time_step_s(self) -> f64 {
        self.time_step_s
    }

    /// Number of requested monolithic state transitions.
    #[must_use]
    pub const fn steps(self) -> NonZeroUsize {
        self.steps
    }

    /// Initial velocity applied to every supported velocity coefficient.
    #[must_use]
    pub const fn initial_velocity_m_per_s(self) -> [f64; 2] {
        self.initial_velocity_m_per_s
    }

    /// Initial displacement applied to every unconstrained interface vertex.
    #[must_use]
    pub const fn initial_free_interface_displacement_m(self) -> [f64; 2] {
        self.initial_free_interface_displacement_m
    }

    /// Characteristic length scale.
    #[must_use]
    pub const fn length_scale_m(self) -> f64 {
        self.length_scale_m
    }

    /// Characteristic velocity scale.
    #[must_use]
    pub const fn velocity_scale_m_per_s(self) -> f64 {
        self.velocity_scale_m_per_s
    }

    /// Characteristic pressure scale.
    #[must_use]
    pub const fn pressure_scale_pa(self) -> f64 {
        self.pressure_scale_pa
    }

    /// Requested relative residual tolerance.
    #[must_use]
    pub const fn relative_tolerance(self) -> f64 {
        self.solver.relative_tolerance()
    }

    /// Requested absolute residual tolerance.
    #[must_use]
    pub const fn absolute_tolerance(self) -> f64 {
        self.solver.absolute_tolerance()
    }

    /// Requested maximum linear-solver iterations per step.
    #[must_use]
    pub const fn maximum_iterations(self) -> NonZeroUsize {
        self.solver.maximum_iterations()
    }

    /// Complete requested linear-solver policy.
    #[must_use]
    pub const fn solver(self) -> SolverPlan {
        self.solver
    }
}

/// Immutable resolution of one accepted fixed-mesh monolithic FSI request.
///
/// This in-process Plan retains the exact caller Model, fixed spatial
/// realization, initial-state support, and pre-output Run manifest. It is not
/// a durable wire or a general coupling-plan hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFixedMeshMonolithicFsiPlan2d {
    document: ModelDocument,
    intent: FixedMeshMonolithicFsiIntent2d,
    canonical: FixedReferenceFsiCartesianModel2d,
    spatial: SpatialContext,
    execution: ExecutionContext,
    solver_provider: SolverProvider,
    solver_capabilities: SolverCapabilities,
}

impl ResolvedFixedMeshMonolithicFsiPlan2d {
    /// Resolve one typed request against the exact accepted Model and backend.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign Model meaning, unsupported
    /// intent, backend drift, or invalid realization/artifact lineage.
    pub fn resolve(
        document: &ModelDocument,
        intent: FixedMeshMonolithicFsiIntent2d,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        validate_scientific_case(STEP_CASE, STEP_CASE_ID)?;
        validate_scientific_case(TRAJECTORY_CASE, TRAJECTORY_CASE_ID)?;
        require_supported_intent(intent)?;
        require_accepted_model(document)?;
        let solver_provider = backend.provider();
        let solver_capabilities = backend.capabilities();
        require_reference_backend(solver_provider, &solver_capabilities)?;
        solver_capabilities.require_problem(
            intent.solver(),
            ScalarType::F64,
            LinearOperatorProperties::SymmetricIndefinite,
        )?;

        let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program())?;
        let spatial = spatial_context(document.program(), &canonical)?;
        let execution =
            execution_context(document.program(), &canonical, &spatial, intent, backend)?;
        require_unchanged_backend(backend, solver_provider, &solver_capabilities)?;

        Ok(Self {
            document: document.clone(),
            intent,
            canonical,
            spatial,
            execution,
            solver_provider,
            solver_capabilities,
        })
    }

    /// Execute exactly this resolved occurrence through the admitted backend.
    ///
    /// # Errors
    /// Revalidates the frozen intent, retained Model, provider release,
    /// capabilities, solve evidence, and complete trajectory lineage.
    pub fn execute(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<FixedReferenceFsiResult2d, Diagnostic> {
        validate_scientific_case(STEP_CASE, STEP_CASE_ID)?;
        validate_scientific_case(TRAJECTORY_CASE, TRAJECTORY_CASE_ID)?;
        require_supported_intent(self.intent)?;
        require_accepted_model(&self.document)?;
        if backend.provider() != self.solver_provider
            || backend.capabilities() != self.solver_capabilities
        {
            return Err(invalid(
                "resolved fixed-mesh monolithic FSI Plan requires the admitted reference solver release and capabilities",
            ));
        }
        let canonical = lower_fixed_reference_fsi_cartesian_2d(self.document.program())?;
        if canonical != self.canonical {
            return Err(internal(
                "fixed-mesh monolithic FSI meaning changed between resolution and execution",
            ));
        }
        self.execution
            .realization
            .validate_model_artifact(&self.spatial.model)?;
        self.execution
            .realization
            .validate_mesh_artifact(&self.spatial.mesh_artifact)?;
        self.execution
            .run
            .validate_against(&self.execution.realization)?;

        let first = solve_step(
            &self.canonical,
            &self.spatial,
            &self.execution,
            &initial_state(&self.spatial, self.intent)?,
            backend,
        )?;
        require_unchanged_backend(backend, self.solver_provider, &self.solver_capabilities)?;
        let second = solve_step(
            &self.canonical,
            &self.spatial,
            &self.execution,
            &state_from_solution(&self.spatial, &first)?,
            backend,
        )?;
        require_unchanged_backend(backend, self.solver_provider, &self.solver_capabilities)?;

        FixedReferenceFsiResult2d::from_execution(self, [first, second])
    }

    /// Complete caller intent consumed by this Plan.
    #[must_use]
    pub const fn intent(&self) -> FixedMeshMonolithicFsiIntent2d {
        self.intent
    }

    /// Exact current Model admitted by this Plan.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.spatial.model
    }

    /// Exact fixed affine-triangle mesh resolved before execution.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.spatial.mesh
    }

    /// Durable mesh identity bound into the resolved Realization.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.spatial.mesh_artifact
    }

    /// Exact two-body geometry identity resolved before execution.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.spatial.geometry
    }

    /// Exact Geometry-to-Mesh correspondence resolved before execution.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.spatial.correspondence
    }

    /// Exhaustive fluid, solid, and interface partition.
    #[must_use]
    pub const fn partition(&self) -> &FixedReferenceFsiPartition2d {
        &self.spatial.partition
    }

    /// Exact multi-Domain Realization artifact resolved before execution.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV3 {
        &self.execution.realization
    }

    /// Complete resolved field-wise plan behind the durable artifact.
    #[must_use]
    pub const fn resolved(&self) -> &ResolvedCoupledFieldwiseRealization {
        &self.execution.resolved
    }

    /// Pre-output Run manifest retaining the admitted execution provenance.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.execution.run
    }

    /// Solver provider release admitted during resolution.
    #[must_use]
    pub const fn solver_provider(&self) -> SolverProvider {
        self.solver_provider
    }

    /// Host execution adapter used by this bounded Plan.
    #[must_use]
    pub const fn execution_provider(&self) -> ExecutionProvider {
        SERIAL_EXECUTION_PROVIDER
    }

    /// Exact host worker count admitted by this bounded Plan.
    #[must_use]
    pub const fn workers(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }
}

pub(super) fn reference_intent() -> Result<FixedMeshMonolithicFsiIntent2d, Diagnostic> {
    FixedMeshMonolithicFsiIntent2d::new(
        TIME_STEP_S,
        NonZeroUsize::new(2).expect("positive frozen state count"),
        [0.0, 0.0],
        [0.02, 0.0],
        LENGTH_SCALE_M,
        VELOCITY_SCALE_M_PER_S,
        PRESSURE_SCALE_PA,
        RELATIVE_TOLERANCE,
        ABSOLUTE_TOLERANCE,
        NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("positive frozen iteration limit"),
    )
}

fn require_supported_intent(intent: FixedMeshMonolithicFsiIntent2d) -> Result<(), Diagnostic> {
    if intent == reference_intent()? {
        Ok(())
    } else {
        Err(Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "the accepted fixed-mesh monolithic FSI application does not implement this intent without fallback",
        ))
    }
}

fn require_reference_backend(
    provider: SolverProvider,
    capabilities: &SolverCapabilities,
) -> Result<(), Diagnostic> {
    if provider != REFERENCE_SOLVER_PROVIDER
        || *capabilities != REFERENCE_LINEAR_SOLVER.capabilities()
    {
        return Err(Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "the accepted fixed-mesh monolithic FSI application requires the current serial reference solver backend",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use eqiora_solver::REFERENCE_LINEAR_SOLVER;

    use super::*;

    #[test]
    fn resolved_plan_facts_are_the_exact_execution_inputs() {
        let document = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let plan = ResolvedFixedMeshMonolithicFsiPlan2d::resolve(
            &document,
            reference_intent().expect("accepted explicit intent"),
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("accepted resolved Plan");
        assert_eq!(plan.intent(), reference_intent().unwrap());
        assert_eq!(plan.solver_provider(), REFERENCE_SOLVER_PROVIDER);
        assert_eq!(plan.execution_provider(), SERIAL_EXECUTION_PROVIDER);
        assert_eq!(plan.workers(), NonZeroUsize::MIN);
        assert_eq!(plan.mesh().vertices().len(), 9);
        assert!(plan.run().outputs().is_empty());

        let result = plan
            .execute(&REFERENCE_LINEAR_SOLVER)
            .expect("resolved execution");
        assert_eq!(result.model(), plan.model());
        assert_eq!(result.mesh(), plan.mesh());
        assert_eq!(result.mesh_artifact(), plan.mesh_artifact());
        assert_eq!(result.geometry(), plan.geometry());
        assert_eq!(result.correspondence(), plan.correspondence());
        assert_eq!(result.partition(), plan.partition());
        assert_eq!(result.realization(), plan.realization());
        assert_eq!(result.time_coordinates_s(), [0.05, 0.1]);
    }

    #[test]
    fn intent_validation_and_resolution_reject_unsupported_values() {
        assert!(
            FixedMeshMonolithicFsiIntent2d::new(
                f64::NAN,
                NonZeroUsize::MIN,
                [0.0; 2],
                [0.0; 2],
                1.0,
                1.0,
                1.0,
                1.0e-8,
                1.0e-10,
                NonZeroUsize::MIN,
            )
            .is_err()
        );
        assert!(
            FixedMeshMonolithicFsiIntent2d::new(
                0.05,
                NonZeroUsize::MIN,
                [f64::INFINITY, 0.0],
                [0.0; 2],
                1.0,
                1.0,
                1.0,
                1.0e-8,
                1.0e-10,
                NonZeroUsize::MIN,
            )
            .is_err()
        );
        assert!(
            FixedMeshMonolithicFsiIntent2d::new(
                0.05,
                NonZeroUsize::MIN,
                [0.0; 2],
                [0.0; 2],
                0.0,
                1.0,
                1.0,
                1.0e-8,
                1.0e-10,
                NonZeroUsize::MIN,
            )
            .is_err()
        );

        let document = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let accepted = reference_intent().expect("accepted explicit intent");
        let unsupported = FixedMeshMonolithicFsiIntent2d::new(
            0.1,
            accepted.steps(),
            accepted.initial_velocity_m_per_s(),
            accepted.initial_free_interface_displacement_m(),
            accepted.length_scale_m(),
            accepted.velocity_scale_m_per_s(),
            accepted.pressure_scale_pa(),
            accepted.relative_tolerance(),
            accepted.absolute_tolerance(),
            accepted.maximum_iterations(),
        )
        .expect("valid but unsupported request");
        let error = ResolvedFixedMeshMonolithicFsiPlan2d::resolve(
            &document,
            unsupported,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect_err("unsupported tuple must reject before execution");
        assert_eq!(error.code(), codes::NOT_IMPLEMENTED);
    }

    #[test]
    fn one_call_reference_solve_delegates_to_the_resolved_plan() {
        let document = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let expected = ResolvedFixedMeshMonolithicFsiPlan2d::resolve(
            &document,
            reference_intent().expect("accepted explicit intent"),
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("accepted resolved Plan")
        .execute(&REFERENCE_LINEAR_SOLVER)
        .expect("resolved execution");
        let delegated =
            FixedReferenceFsiResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
                .expect("delegated reference execution");
        assert_eq!(delegated, expected);
    }
}
