//! One accepted steady-Stokes intent, resolved plan, and application result.
//!
//! This module owns the complete composition shared by Studio and Python. It
//! separates inspectable intent and resolution from execution while retaining
//! the narrow accepted Model, mesh, and scientific reference configuration.

use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_artifact::{
    AcceptedCircularHoleChordalRealizationV1, CircularHoleChordalRealizationEnvelopeV1,
    DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, FieldSnapshotEnvelopeV1,
    GeometryDefinitionV1, GeometryMeshCorrespondenceEnvelopeV1, LayoutArtifacts, ModelEnvelope,
    RealizationEnvelopeV2, RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape};
use eqiora_numerics::fluid::{
    IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d, SteadyStokesMiniSolution2d,
    solve_resolved_steady_stokes_geometry_mini_2d,
};
use eqiora_realization::{
    DiscretizationMethod, FieldwiseRealizationRequest, MeshKind, RealizationCapabilities,
    RealizationRevision, ResolvedFieldwiseRealization, SemanticRevision, Space, SpaceFamily,
    SpatialDimensionSupport, TargetCapabilities, VectorLayoutKind, resolve_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, LinearSolver, LinearSolverBackend,
    ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability,
    SolverPlan, SolverProvider,
};

use crate::UnstructuredP1ScalarFieldProjection2d;

const ACCEPTED_MODEL_DIGEST: &str =
    "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146";
const ACCEPTED_SOURCE_DIGEST: &str =
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";
const ACCEPTED_REFERENCE_MESH_DIGEST: &str =
    "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a";
const ACCEPTED_GMSH_MESH_DIGEST: &str =
    "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b";
const ACCEPTED_SEMANTIC_REVISION: u64 = 1;
const APPLICATION_REALIZATION_REVISION: u64 = 133;
const ACCEPTED_MAX_BOUNDARY_ERROR_M: f64 = 1.0e-4;
const ACCEPTED_MINIMUM_MEAN_RATIO: f64 = 1.0e-5;

const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const VELOCITY: DimExponents = DimExponents {
    length: 1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};
const PRESSURE: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -2,
    ..DimExponents::DIMENSIONLESS
};

/// Typed, inspectable request for a steady two-dimensional Stokes solve.
///
/// Construction validates physical scales and numerical controls. Resolution
/// remains responsible for deciding whether a concrete application path can
/// implement the requested tuple without fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteadyStokesIntent2d {
    scales: IncompressibleFlowScaleProfile2d,
    solver: SolverPlan,
}

impl SteadyStokesIntent2d {
    /// Compose already validated physical scales and linear-solve policy.
    #[must_use]
    pub const fn from_parts(scales: IncompressibleFlowScaleProfile2d, solver: SolverPlan) -> Self {
        Self { scales, solver }
    }

    /// Construct a complete request with no hidden numerical defaults.
    ///
    /// # Errors
    /// Returns `EQ0807` for non-positive/non-finite scales or tolerances.
    pub fn new(
        length_scale_m: f64,
        velocity_scale_m_per_s: f64,
        pressure_scale_pa: f64,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: NonZeroUsize,
    ) -> Result<Self, Diagnostic> {
        if !relative_tolerance.is_finite()
            || !absolute_tolerance.is_finite()
            || relative_tolerance <= 0.0
            || absolute_tolerance <= 0.0
        {
            return Err(invalid_reference_input(
                "steady-Stokes tolerances must be finite and strictly positive",
            ));
        }
        let scales = IncompressibleFlowScaleProfile2d::new(
            DynQuantity::new(length_scale_m, LENGTH),
            DynQuantity::new(velocity_scale_m_per_s, VELOCITY),
            DynQuantity::new(pressure_scale_pa, PRESSURE),
        )?;
        let solver = SolverPlan::new(
            LinearSolver::SparseLu,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )?
        .with_reduction(ReductionPolicy::Fast);
        Ok(Self { scales, solver })
    }

    /// Characteristic physical scales used by realization and execution.
    #[must_use]
    pub const fn scales(self) -> IncompressibleFlowScaleProfile2d {
        self.scales
    }

    /// Complete linear-solver policy requested by the caller.
    #[must_use]
    pub const fn solver(self) -> SolverPlan {
        self.solver
    }
}

/// Immutable result of resolving a steady-Stokes intent before execution.
///
/// This is an owned in-process plan, not a new durable wire format. Its
/// canonical bytes and digest are those of the existing field-wise
/// [`RealizationEnvelopeV2`]. Execution replays the retained inputs and
/// revalidates the exact backend release and capability inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSteadyStokesPlan2d {
    model: ModelEnvelope,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    intent: SteadyStokesIntent2d,
    resolved: ResolvedFieldwiseRealization,
    realization: RealizationEnvelopeV2,
    velocity_space: Space,
    pressure_space: Space,
    solver_provider: SolverProvider,
    solver_capabilities: SolverCapabilities,
}

impl ResolvedSteadyStokesPlan2d {
    /// Resolve one typed request against exact Model, mesh, and backend inputs.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign lineage, unsupported intent
    /// or backend policy, or invalid field-wise realization.
    pub fn resolve(
        model: &ModelEnvelope,
        intent: SteadyStokesIntent2d,
        accepted: &AcceptedCircularHoleChordalRealizationV1,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        require_supported_intent(intent)?;
        let solver_provider = backend.provider();
        let solver_capabilities = backend.capabilities();
        let (_, _, resolved, realization) = resolve_application(model, accepted, intent, backend)?;
        if backend.provider() != solver_provider || backend.capabilities() != solver_capabilities {
            return Err(internal_failure(
                "linear solver provider identity or capabilities changed during resolution",
            ));
        }
        let (velocity_space, pressure_space) = resolved_mini_spaces(&resolved)?;
        Ok(Self {
            model: model.clone(),
            accepted: accepted.clone(),
            intent,
            resolved,
            realization,
            velocity_space,
            pressure_space,
            solver_provider,
            solver_capabilities,
        })
    }

    /// Execute exactly this resolved occurrence through the admitted backend.
    ///
    /// # Errors
    /// Revalidates every retained input and returns a structured diagnostic for
    /// lineage, provider, realization, solve, or output-evidence drift.
    pub fn execute(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<CircularHoleSteadyStokesResult2d, Diagnostic> {
        if backend.provider() != self.solver_provider
            || backend.capabilities() != self.solver_capabilities
        {
            return Err(invalid_reference_input(
                "resolved steady-Stokes Plan requires the admitted solver provider release and capabilities",
            ));
        }
        let (program, binding, resolved, realization) =
            resolve_application(&self.model, &self.accepted, self.intent, backend)?;
        if resolved != self.resolved || realization != self.realization {
            return Err(internal_failure(
                "steady-Stokes Realization changed between resolution and execution",
            ));
        }
        let solution =
            solve_resolved_steady_stokes_geometry_mini_2d(&program, &resolved, &binding, backend)?;
        if backend.provider() != self.solver_provider
            || backend.capabilities() != self.solver_capabilities
        {
            return Err(internal_failure(
                "linear solver provider identity or capabilities changed during execution",
            ));
        }
        CircularHoleSteadyStokesResult2d::from_execution(self, solution)
    }

    /// Accepted canonical Model.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Complete caller intent consumed by this Plan.
    #[must_use]
    pub const fn intent(&self) -> SteadyStokesIntent2d {
        self.intent
    }

    /// Existing durable field-wise Realization artifact.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV2 {
        &self.realization
    }

    /// Resolved velocity basis.
    #[must_use]
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }

    /// Resolved pressure basis.
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
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

/// Complete accepted lineage for the exact-cylinder steady MINI/P1 Stokes case.
///
/// This is an immutable in-process application value, not a durable Result
/// artifact and not a general fluid solver. It retains every artifact required
/// to replay the accepted Model → Realization → Run path and projects pressure
/// coefficients in the exact canonical mesh-vertex order.
#[derive(Debug, Clone, PartialEq)]
pub struct CircularHoleSteadyStokesResult2d {
    model: ModelEnvelope,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    realization: RealizationEnvelopeV2,
    pressure_block: DiscreteFieldEnvelopeV1,
    snapshot: FieldSnapshotEnvelopeV1,
    run: RunManifestV2,
    pressure_projection: UnstructuredP1ScalarFieldProjection2d,
    solution: SteadyStokesMiniSolution2d,
    cylinder_force_on_fluid: [f64; 2],
    inlet_flux: f64,
    outlet_flux: f64,
    net_flux: f64,
    momentum_closure: [f64; 2],
}

impl CircularHoleSteadyStokesResult2d {
    fn from_execution(
        plan: &ResolvedSteadyStokesPlan2d,
        solution: SteadyStokesMiniSolution2d,
    ) -> Result<Self, Diagnostic> {
        let model = &plan.model;
        let accepted = &plan.accepted;
        let realization = &plan.realization;
        let mesh = accepted.mesh();
        let pressure_payload = DiscreteFieldPayload::new(
            mesh.mesh(),
            DiscreteFieldAssociation::Vertex,
            DiscreteFieldShape::Scalar,
            solution.pressure().vertex_values().to_vec(),
        )?;
        let pressure_block = DiscreteFieldEnvelopeV1::from_payload(mesh, &pressure_payload)?;
        let snapshot = FieldSnapshotEnvelopeV1::new_authored_fieldwise(
            model,
            realization,
            accepted,
            solution.pressure_field(),
            std::slice::from_ref(&pressure_block),
        )?;
        let execution = ExecutionProvenanceV1::from_provider_releases(
            plan.solver_provider,
            plan.execution_provider(),
            ExecutionTopologyV1::Host {
                workers: plan.workers(),
            },
            plan.intent.solver().reduction(),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let run = RunManifestV2::new(realization, execution)?.with_output(snapshot.digest()?);
        let pressure_projection =
            UnstructuredP1ScalarFieldProjection2d::from_authored_fieldwise_snapshot(
                model,
                realization,
                accepted,
                &run,
                &snapshot,
                &pressure_block,
            )?;
        if pressure_projection.vertices_m().len() != solution.pressure().vertex_values().len()
            || pressure_projection.values() != solution.pressure().vertex_values()
        {
            return Err(internal_failure(
                "pressure projection does not preserve canonical mesh-vertex coefficient order",
            ));
        }

        let cylinder_force_on_fluid = required_reaction(&solution, "cylinder")?;
        let inlet_flux = required_flux(&solution, "inlet")?;
        let outlet_flux = required_flux(&solution, "outlet")?;
        let net_flux = inlet_flux + outlet_flux;
        let constrained = solution.boundary_reaction();
        let body = solution.integrated_body_force();
        let traction = solution.integrated_boundary_traction();
        let momentum_closure = std::array::from_fn(|component| {
            constrained[component] + body[component] + traction[component]
        });
        if !net_flux.is_finite() || momentum_closure.iter().any(|value| !value.is_finite()) {
            return Err(internal_failure(
                "exact-cylinder balance evidence contains a non-finite value",
            ));
        }

        Ok(Self {
            model: model.clone(),
            accepted: accepted.clone(),
            realization: realization.clone(),
            pressure_block,
            snapshot,
            run,
            pressure_projection,
            solution,
            cylinder_force_on_fluid,
            inlet_flux,
            outlet_flux,
            net_flux,
            momentum_closure,
        })
    }

    /// Accepted canonical Model.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Exact circular-hole source.
    #[must_use]
    pub const fn source(&self) -> &CanonicalGeometryV1 {
        self.accepted.source()
    }

    /// Durable exact-source-to-chordal-resource binding.
    #[must_use]
    pub const fn chordal_realization(&self) -> &CircularHoleChordalRealizationEnvelopeV1 {
        self.accepted.envelope()
    }

    /// Realized straight-edged geometry artifact.
    #[must_use]
    pub const fn realized_geometry(&self) -> &GeometryDefinitionV1 {
        self.accepted.realized_geometry()
    }

    /// Accepted affine-triangle mesh artifact.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        self.accepted.mesh()
    }

    /// Authored geometry-to-mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        self.accepted.correspondence()
    }

    /// Exact field-wise Realization.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV2 {
        &self.realization
    }

    /// Pressure coefficient block retained by the snapshot.
    #[must_use]
    pub const fn pressure_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.pressure_block
    }

    /// Logical pressure Field snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.snapshot
    }

    /// Exact Run manifest.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Renderer- and adapter-ready pressure projection.
    #[must_use]
    pub const fn pressure_projection(&self) -> &UnstructuredP1ScalarFieldProjection2d {
        &self.pressure_projection
    }

    /// Transfer the pressure buffers to one client adapter without cloning.
    #[must_use]
    pub fn into_pressure_projection(self) -> UnstructuredP1ScalarFieldProjection2d {
        self.pressure_projection
    }

    /// Complete coherent-SI solution and solver evidence.
    #[must_use]
    pub const fn solution(&self) -> &SteadyStokesMiniSolution2d {
        &self.solution
    }

    /// Constraint force exerted by the cylinder on the fluid, in N/m.
    #[must_use]
    pub const fn cylinder_force_on_fluid(&self) -> [f64; 2] {
        self.cylinder_force_on_fluid
    }

    /// Parent-outward inlet flux in m²/s.
    #[must_use]
    pub const fn inlet_flux(&self) -> f64 {
        self.inlet_flux
    }

    /// Parent-outward outlet flux in m²/s.
    #[must_use]
    pub const fn outlet_flux(&self) -> f64 {
        self.outlet_flux
    }

    /// Sum of parent-outward inlet and outlet fluxes in m²/s.
    #[must_use]
    pub const fn net_flux(&self) -> f64 {
        self.net_flux
    }

    /// Constrained reaction + body force + boundary traction, in N/m.
    #[must_use]
    pub const fn momentum_closure(&self) -> [f64; 2] {
        self.momentum_closure
    }
}

fn require_accepted_inputs(
    model: &ModelEnvelope,
    accepted: &AcceptedCircularHoleChordalRealizationV1,
) -> Result<(), Diagnostic> {
    accepted.revalidate()?;
    let source = accepted.source();
    if model.source_revision() != ACCEPTED_SEMANTIC_REVISION {
        return Err(invalid_reference_input(format!(
            "exact-cylinder reference Model must retain source revision \
             {ACCEPTED_SEMANTIC_REVISION}"
        )));
    }
    if model.digest()?.to_string() != ACCEPTED_MODEL_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted canonical current Model artifact",
        ));
    }
    if encode_digest(&source.digest_bytes()) != ACCEPTED_SOURCE_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted exact geometry source",
        ));
    }
    if accepted.requested_max_boundary_error_m().to_bits()
        != ACCEPTED_MAX_BOUNDARY_ERROR_M.to_bits()
        || accepted.envelope().required_minimum_mean_ratio().to_bits()
            != ACCEPTED_MINIMUM_MEAN_RATIO.to_bits()
    {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted chordal realization policy",
        ));
    }
    let mesh_digest = accepted.mesh().digest()?.to_string();
    if mesh_digest != ACCEPTED_REFERENCE_MESH_DIGEST && mesh_digest != ACCEPTED_GMSH_MESH_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires an accepted exact mesh policy",
        ));
    }
    Ok(())
}

fn require_supported_intent(intent: SteadyStokesIntent2d) -> Result<(), Diagnostic> {
    if intent == reference_intent()? {
        Ok(())
    } else {
        Err(Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "the accepted steady-Stokes application does not implement this intent without fallback",
        ))
    }
}

fn resolve_application(
    model: &ModelEnvelope,
    accepted: &AcceptedCircularHoleChordalRealizationV1,
    intent: SteadyStokesIntent2d,
    backend: &dyn LinearSolverBackend,
) -> Result<
    (
        KernelProgram,
        SteadyStokesGeometryBinding2d,
        ResolvedFieldwiseRealization,
        RealizationEnvelopeV2,
    ),
    Diagnostic,
> {
    require_supported_intent(intent)?;
    require_accepted_inputs(model, accepted)?;
    let program = replay_program(model, accepted.source())?;
    if program.revision().0 != ACCEPTED_SEMANTIC_REVISION {
        return Err(invalid_reference_input(format!(
            "exact-cylinder reference Model must replay semantic revision \
             {ACCEPTED_SEMANTIC_REVISION}"
        )));
    }

    let binding = SteadyStokesGeometryBinding2d::new(&program, accepted.clone())?;
    let solver = intent.solver();
    let fieldwise = binding.mini_plan(
        accepted.mesh().artifact_reference()?,
        intent.scales(),
        solver,
    )?;
    backend.capabilities().require_problem(
        solver,
        ScalarType::F64,
        LinearOperatorProperties::SymmetricIndefinite,
    )?;
    let selected_solver = SolverCapabilities::exact([SolverCapability {
        algorithm: solver.algorithm(),
        operator_properties: LinearOperatorProperties::SymmetricIndefinite,
        preconditioner: solver.preconditioner(),
        reduction: solver.reduction(),
        scalar_type: ScalarType::F64,
    }])?;
    let capabilities = reference_capabilities(selected_solver)?;
    let resolved = resolve_fieldwise(
        &FieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            RealizationRevision::new(APPLICATION_REALIZATION_REVISION),
            fieldwise,
        ),
        binding.fieldwise_requirements(),
        &capabilities,
    )?;
    let realization =
        RealizationEnvelopeV2::from_resolved(model, &resolved, LayoutArtifacts::Replicated)?;
    if realization.realization_revision().get() != APPLICATION_REALIZATION_REVISION {
        return Err(internal_failure(
            "steady-Stokes application Realization revision changed during resolution",
        ));
    }
    Ok((program, binding, resolved, realization))
}

fn resolved_mini_spaces(
    resolved: &ResolvedFieldwiseRealization,
) -> Result<(Space, Space), Diagnostic> {
    let mut velocity = None;
    let mut pressure = None;
    for binding in resolved.plan().spatial().field_spaces() {
        match binding.space().family() {
            SpaceFamily::SimplexP1Bubble if velocity.replace(binding.space()).is_none() => {}
            SpaceFamily::ContinuousLagrange { order }
                if order == NonZeroU16::MIN && pressure.replace(binding.space()).is_none() => {}
            _ => {
                return Err(internal_failure(
                    "resolved steady-Stokes Plan does not contain exactly one MINI velocity and one P1 pressure space",
                ));
            }
        }
    }
    velocity.zip(pressure).ok_or_else(|| {
        internal_failure(
            "resolved steady-Stokes Plan is missing its MINI velocity or P1 pressure space",
        )
    })
}

fn replay_program(
    model: &ModelEnvelope,
    source: &CanonicalGeometryV1,
) -> Result<KernelProgram, Diagnostic> {
    let (transaction, model_id) = model.to_transaction().map_err(first_diagnostic)?;
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).map_err(first_diagnostic)?;
    KernelProgram::from_snapshot_with_geometry(&store.snapshot(), model_id, &[source.into()])
        .map_err(first_diagnostic)
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics.into_iter().next().unwrap_or_else(|| {
        internal_failure("exact-cylinder Model replay failed without a diagnostic")
    })
}

fn reference_intent() -> Result<SteadyStokesIntent2d, Diagnostic> {
    SteadyStokesIntent2d::new(
        0.41,
        0.3,
        0.001 * 0.3 / 0.41,
        1.0e-6,
        1.0e-13,
        NonZeroUsize::new(10_000).expect("nonzero reference constant"),
    )
}

fn reference_capabilities(
    solver: SolverCapabilities,
) -> Result<RealizationCapabilities, Diagnostic> {
    RealizationCapabilities::cartesian_product(
        [DiscretizationMethod::ContinuousGalerkin],
        [(
            MeshKind::ImportedAffineSimplicial,
            SpatialDimensionSupport::exact(
                NonZeroUsize::new(2).expect("nonzero reference dimension"),
            ),
        )],
        [VectorLayoutKind::Replicated],
        solver,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
}

fn required_reaction(
    solution: &SteadyStokesMiniSolution2d,
    name: &str,
) -> Result<[f64; 2], Diagnostic> {
    solution.named_boundary_reaction(name).ok_or_else(|| {
        internal_failure(format!(
            "exact-cylinder result has no `{name}` reaction evidence"
        ))
    })
}

fn required_flux(solution: &SteadyStokesMiniSolution2d, name: &str) -> Result<f64, Diagnostic> {
    solution.named_boundary_flux(name).ok_or_else(|| {
        internal_failure(format!(
            "exact-cylinder result has no `{name}` flux evidence"
        ))
    })
}

fn encode_digest(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn internal_failure(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INTERNAL_FAILURE, message)
}

fn invalid_reference_input(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
#[path = "steady_stokes/tests.rs"]
mod tests;
