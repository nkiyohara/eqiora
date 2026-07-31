//! One accepted exact-cylinder steady-Stokes application result.
//!
//! This module owns the complete composition shared by Studio and Python. It
//! deliberately exposes one narrow application value instead of a generic
//! solver service: the accepted Model, scale profile, numerical policy, and
//! Realization revision remain one indivisible reference configuration.

use std::num::NonZeroUsize;

use eqiora_artifact::{
    CircularHoleChordalRealizationEnvelopeV1, DiscreteFieldEnvelopeV1, ExecutionProvenanceV1,
    ExecutionTopologyV1, FieldSnapshotEnvelopeV1, GeometryDefinitionV1,
    GeometryMeshCorrespondenceEnvelopeV1, LayoutArtifacts, ModelEnvelopeV7, RealizationEnvelopeV2,
    RunManifestV2, SimplicialMeshEnvelopeV1,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_geometry::{CanonicalCircularHoleGeometryV1, CircularHoleChordalMeshV1};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_meshing::{DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape};
use eqiora_numerics::fluid::{
    IncompressibleFlowScaleProfile2d, SteadyStokesGeometryBinding2d, SteadyStokesMiniSolution2d,
    solve_resolved_steady_stokes_geometry_mini_2d,
};
use eqiora_realization::{
    DiscretizationMethod, FieldwiseRealizationRequest, MeshKind, RealizationCapabilities,
    RealizationRevision, SemanticRevision, SpatialDimensionSupport, TargetCapabilities,
    VectorLayoutKind, resolve_fieldwise,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, LinearSolverBackend, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, ScalarType, SolverCapabilities, SolverCapability, SolverPlan,
};

use crate::UnstructuredP1ScalarFieldProjection2d;

const ACCEPTED_MODEL_DIGEST: &str =
    "668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de";
const ACCEPTED_SOURCE_DIGEST: &str =
    "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";
const ACCEPTED_MESH_DIGEST: &str =
    "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a";
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

/// Complete accepted lineage for the exact-cylinder steady MINI/P1 Stokes case.
///
/// This is an immutable in-process application value, not a durable Result
/// artifact and not a general fluid solver. It retains every artifact required
/// to replay the accepted Model → Realization → Run path and projects pressure
/// coefficients in the exact canonical mesh-vertex order.
#[derive(Debug, Clone, PartialEq)]
pub struct CircularHoleSteadyStokesResult2d {
    model: ModelEnvelopeV7,
    source: CanonicalCircularHoleGeometryV1,
    owner: CircularHoleChordalMeshV1,
    chordal_realization: CircularHoleChordalRealizationEnvelopeV1,
    realized_geometry: GeometryDefinitionV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
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
    /// Execute the single accepted exact-cylinder reference configuration.
    ///
    /// The caller supplies the canonical Model explicitly; this operation
    /// accepts only the independently frozen Model, exact source, and chordal
    /// mesh identities. Scale, solver, reduction, execution topology, and
    /// Realization revision are owned here and cannot diverge between clients.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign inputs, replay or binding
    /// drift, unsupported backend policy, solve failure, incomplete balance
    /// evidence, or any invalid artifact in the resulting lineage.
    pub fn solve_reference(
        model: &ModelEnvelopeV7,
        source: &CanonicalCircularHoleGeometryV1,
        owner: &CircularHoleChordalMeshV1,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        require_accepted_inputs(model, source, owner)?;
        let program = replay_program(model, source)?;
        if program.revision().0 != ACCEPTED_SEMANTIC_REVISION {
            return Err(invalid_reference_input(format!(
                "exact-cylinder reference Model must replay semantic revision \
                 {ACCEPTED_SEMANTIC_REVISION}"
            )));
        }

        let realized_geometry = GeometryDefinitionV1::from_region(owner.region());
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())?;
        let correspondence =
            GeometryMeshCorrespondenceEnvelopeV1::from_region(&realized_geometry, &mesh)?;
        let chordal_realization = CircularHoleChordalRealizationEnvelopeV1::capture(
            source,
            owner,
            &realized_geometry,
            &mesh,
            &correspondence,
        )?;
        chordal_realization.replay_against(source, &realized_geometry, &mesh, &correspondence)?;

        let binding = SteadyStokesGeometryBinding2d::new(
            &program,
            source.clone(),
            owner.clone(),
            realized_geometry.clone(),
            mesh.clone(),
            correspondence.clone(),
        )?;
        let solver_plan = reference_solver_plan()?;
        let plan = binding.mini_plan(
            mesh.artifact_reference()?,
            reference_scale_profile()?,
            solver_plan,
        )?;

        let backend_provider = backend.provider();
        let backend_capabilities = backend.capabilities();
        backend_capabilities.require_problem(
            solver_plan,
            ScalarType::F64,
            LinearOperatorProperties::SymmetricIndefinite,
        )?;
        let selected_solver = SolverCapabilities::exact([SolverCapability {
            algorithm: solver_plan.algorithm(),
            operator_properties: LinearOperatorProperties::SymmetricIndefinite,
            preconditioner: solver_plan.preconditioner(),
            reduction: solver_plan.reduction(),
            scalar_type: ScalarType::F64,
        }])?;
        let capabilities = reference_capabilities(selected_solver)?;
        let resolved = resolve_fieldwise(
            &FieldwiseRealizationRequest::explicit(
                program.model(),
                SemanticRevision::new(program.revision().0),
                RealizationRevision::new(APPLICATION_REALIZATION_REVISION),
                plan,
            ),
            binding.fieldwise_requirements(),
            &capabilities,
        )?;
        let realization =
            RealizationEnvelopeV2::from_resolved(model, &resolved, LayoutArtifacts::Replicated)?;
        if realization.realization_revision().get() != APPLICATION_REALIZATION_REVISION {
            return Err(internal_failure(
                "exact-cylinder application Realization revision changed during resolution",
            ));
        }

        let solution =
            solve_resolved_steady_stokes_geometry_mini_2d(&program, &resolved, &binding, backend)?;
        if backend.provider() != backend_provider || backend.capabilities() != backend_capabilities
        {
            return Err(internal_failure(
                "linear solver provider identity or capabilities changed during execution",
            ));
        }

        let pressure_payload = DiscreteFieldPayload::new(
            mesh.mesh(),
            DiscreteFieldAssociation::Vertex,
            DiscreteFieldShape::Scalar,
            solution.pressure().vertex_values().to_vec(),
        )?;
        let pressure_block = DiscreteFieldEnvelopeV1::from_payload(&mesh, &pressure_payload)?;
        let snapshot = FieldSnapshotEnvelopeV1::new_authored_fieldwise(
            model,
            &realization,
            source,
            owner,
            &realized_geometry,
            &correspondence,
            &mesh,
            solution.pressure_field(),
            std::slice::from_ref(&pressure_block),
        )?;
        let execution = ExecutionProvenanceV1::from_provider_releases(
            backend_provider,
            SERIAL_EXECUTION_PROVIDER,
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            solver_plan.reduction(),
            std::iter::empty::<(&str, &str)>(),
        )?;
        let run = RunManifestV2::new(&realization, execution)?.with_output(snapshot.digest()?);
        let pressure_projection =
            UnstructuredP1ScalarFieldProjection2d::from_authored_fieldwise_snapshot(
                model,
                &realization,
                source,
                owner,
                &realized_geometry,
                &correspondence,
                &mesh,
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
            source: source.clone(),
            owner: owner.clone(),
            chordal_realization,
            realized_geometry,
            mesh,
            correspondence,
            realization,
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
    pub const fn model(&self) -> &ModelEnvelopeV7 {
        &self.model
    }

    /// Exact circular-hole source.
    #[must_use]
    pub const fn source(&self) -> &CanonicalCircularHoleGeometryV1 {
        &self.source
    }

    /// Source-owned chordal realization.
    #[must_use]
    pub const fn owner(&self) -> &CircularHoleChordalMeshV1 {
        &self.owner
    }

    /// Durable exact-source-to-chordal-resource binding.
    #[must_use]
    pub const fn chordal_realization(&self) -> &CircularHoleChordalRealizationEnvelopeV1 {
        &self.chordal_realization
    }

    /// Realized straight-edged geometry artifact.
    #[must_use]
    pub const fn realized_geometry(&self) -> &GeometryDefinitionV1 {
        &self.realized_geometry
    }

    /// Accepted affine-triangle mesh artifact.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }

    /// Authored geometry-to-mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
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
    model: &ModelEnvelopeV7,
    source: &CanonicalCircularHoleGeometryV1,
    owner: &CircularHoleChordalMeshV1,
) -> Result<(), Diagnostic> {
    if model.source_revision() != ACCEPTED_SEMANTIC_REVISION {
        return Err(invalid_reference_input(format!(
            "exact-cylinder reference Model must retain source revision \
             {ACCEPTED_SEMANTIC_REVISION}"
        )));
    }
    if model.digest()?.to_string() != ACCEPTED_MODEL_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted canonical Model v7 artifact",
        ));
    }
    if encode_digest(&source.digest_bytes()) != ACCEPTED_SOURCE_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted exact geometry source",
        ));
    }
    if owner.source().digest_bytes() != source.digest_bytes() {
        return Err(invalid_reference_input(
            "exact-cylinder chordal mesh belongs to another exact geometry source",
        ));
    }
    if owner.requested_max_boundary_error_m().to_bits() != ACCEPTED_MAX_BOUNDARY_ERROR_M.to_bits()
        || owner.mesh().quality_gate().minimum_mean_ratio().to_bits()
            != ACCEPTED_MINIMUM_MEAN_RATIO.to_bits()
    {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted chordal realization policy",
        ));
    }
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(owner.mesh())?;
    if mesh.digest()?.to_string() != ACCEPTED_MESH_DIGEST {
        return Err(invalid_reference_input(
            "exact-cylinder reference operation requires the accepted chordal mesh policy",
        ));
    }
    Ok(())
}

fn replay_program(
    model: &ModelEnvelopeV7,
    source: &CanonicalCircularHoleGeometryV1,
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

fn reference_scale_profile() -> Result<IncompressibleFlowScaleProfile2d, Diagnostic> {
    IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(0.41, LENGTH),
        DynQuantity::new(0.3, VELOCITY),
        DynQuantity::new(0.001 * 0.3 / 0.41, PRESSURE),
    )
}

fn reference_solver_plan() -> Result<SolverPlan, Diagnostic> {
    SolverPlan::new(
        LinearSolver::SparseLu,
        1.0e-6,
        1.0e-13,
        NonZeroUsize::new(10_000).expect("nonzero reference constant"),
    )
    .map(|plan| plan.with_reduction(ReductionPolicy::Fast))
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
