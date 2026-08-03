//! One accepted mixed-boundary linear-elasticity application result.
//!
//! This module owns the complete Model-to-Run composition shared by Studio
//! and Python. It is intentionally one bounded application value rather than
//! a general structural-result hierarchy.

use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_artifact::{
    CartesianMeshEnvelopeV1, CartesianQ1FieldSnapshotEnvelopeV1, ExecutionProvenanceV1,
    ExecutionTopologyV1, GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    LayoutArtifacts, ModelEnvelope, RealizationEnvelopeV1, RunManifestV2,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id};
use eqiora_meshing::{CartesianMesh, MeshEntity, MeshTopology};
use eqiora_numerics::solid::{
    CartesianLinearElasticity2dSolution, lower_isotropic_elasticity_cartesian_2d,
    solve_resolved_isotropic_elasticity_cartesian_2d,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, ResolutionSource, ResolvedRealization, SemanticRevision, Space, Target,
    VectorLayoutKind, resolve,
};
use eqiora_schema::kernel::KernelNode;
use eqiora_solver::{
    ExecutionProvider, LinearOperatorProperties, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType,
    SolverCapabilities, SolverPlan, SolverProvider,
};

use crate::ModelDocument;

const REFERENCE_SOURCE: &str =
    include_str!("../../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi");
const SCIENTIFIC_CASE: &str =
    include_str!("../../../verify/solid/mixed-boundary-elasticity-2d/case.toml");
const SCIENTIFIC_CASE_ID: &str = "solid.mixed-boundary-elasticity-2d";
const CELLS_PER_AXIS: usize = 16;
const REALIZATION_REVISION: u64 = 1;
const GEOMETRY_CLASSIFICATION_PRECISION_M: f64 = 1.0e-12;
const RELATIVE_TOLERANCE: f64 = 1.0e-12;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-14;
const MAXIMUM_ITERATIONS: usize = 10_000;
const DISPLACEMENT_DIMENSION: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};

/// Typed, inspectable request for the accepted two-dimensional linear-elasticity solve.
///
/// Construction validates every numerical control. Resolution remains responsible
/// for admitting only the exact tuple implemented by the bounded reference path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearElasticityIntent2d {
    cells_per_axis: NonZeroUsize,
    solver: SolverPlan,
}

impl LinearElasticityIntent2d {
    /// Construct a complete request with no hidden numerical defaults.
    ///
    /// # Errors
    /// Returns `EQ0807` when either tolerance is non-finite or non-positive.
    pub fn new(
        cells_per_axis: NonZeroUsize,
        relative_tolerance: f64,
        absolute_tolerance: f64,
        maximum_iterations: NonZeroUsize,
    ) -> Result<Self, Diagnostic> {
        if !relative_tolerance.is_finite()
            || !absolute_tolerance.is_finite()
            || relative_tolerance <= 0.0
            || absolute_tolerance <= 0.0
        {
            return Err(invalid(
                "linear-elasticity tolerances must be finite and strictly positive",
            ));
        }
        let solver = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            relative_tolerance,
            absolute_tolerance,
            maximum_iterations,
        )?;
        Ok(Self {
            cells_per_axis,
            solver,
        })
    }

    /// Number of generated Cartesian cells on each axis.
    #[must_use]
    pub const fn cells_per_axis(self) -> NonZeroUsize {
        self.cells_per_axis
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

    /// Requested maximum solver iterations.
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

/// Immutable result of resolving a linear-elasticity intent before execution.
///
/// This is an owned in-process Plan, not a durable wire format. It exposes the
/// existing exact Model, Realization, geometry, generated Cartesian mesh, and
/// correspondence artifacts. Execution replays the retained inputs and
/// revalidates the admitted backend release and capability inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLinearElasticityPlan2d {
    document: ModelDocument,
    intent: LinearElasticityIntent2d,
    resolved: ResolvedRealization,
    model: ModelEnvelope,
    realization: RealizationEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    mesh_artifact: CartesianMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    solver_provider: SolverProvider,
    solver_capabilities: SolverCapabilities,
}

impl ResolvedLinearElasticityPlan2d {
    /// Resolve one typed request against the exact accepted Model and backend.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign Model meaning, unsupported
    /// intent, mutable provider identity, or invalid realization/artifact lineage.
    pub fn resolve(
        document: &ModelDocument,
        intent: LinearElasticityIntent2d,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        let solver_provider = backend.provider();
        let solver_capabilities = backend.capabilities();
        let application = resolve_application(document, intent, backend)?;
        if backend.provider() != solver_provider || backend.capabilities() != solver_capabilities {
            return Err(internal(
                "linear solver provider identity or capabilities changed during resolution",
            ));
        }
        Ok(Self {
            document: document.clone(),
            intent,
            resolved: application.resolved,
            model: application.model,
            realization: application.realization,
            geometry: application.geometry,
            mesh_artifact: application.mesh_artifact,
            correspondence: application.correspondence,
            solver_provider,
            solver_capabilities,
        })
    }

    /// Execute exactly this resolved occurrence through the admitted backend.
    ///
    /// # Errors
    /// Revalidates retained Model meaning, intent, artifacts, provider release,
    /// capabilities, solve evidence, and output lineage before publication.
    pub fn execute(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<MixedBoundaryElasticityResult2d, Diagnostic> {
        if backend.provider() != self.solver_provider
            || backend.capabilities() != self.solver_capabilities
        {
            return Err(invalid(
                "resolved linear-elasticity Plan requires the admitted solver provider release and capabilities",
            ));
        }
        let application = resolve_application(&self.document, self.intent, backend)?;
        if application.resolved != self.resolved
            || application.model != self.model
            || application.realization != self.realization
            || application.geometry != self.geometry
            || application.mesh_artifact != self.mesh_artifact
            || application.correspondence != self.correspondence
        {
            return Err(internal(
                "linear-elasticity artifacts changed between resolution and execution",
            ));
        }

        let (lowered, solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
            self.document.program(),
            &self.resolved,
            backend,
        )?;
        if backend.provider() != self.solver_provider
            || backend.capabilities() != self.solver_capabilities
        {
            return Err(internal(
                "linear solver provider identity or capabilities changed during execution",
            ));
        }
        validate_solve_report(&solution, self.solver_provider, self.intent.solver())?;
        let solved_mesh = CartesianMeshEnvelopeV1::from_mesh(solution.displacement().mesh())?;
        if solved_mesh != self.mesh_artifact {
            return Err(internal(
                "linear-elasticity execution mesh differs from the resolved exact mesh artifact",
            ));
        }
        MixedBoundaryElasticityResult2d::from_execution(self, *lowered.bounds(), solution)
    }

    /// Exact current Model admitted by this Plan.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Complete caller intent consumed by this Plan.
    #[must_use]
    pub const fn intent(&self) -> LinearElasticityIntent2d {
        self.intent
    }

    /// Exact field-wise Realization artifact produced during resolution.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV1 {
        &self.realization
    }

    /// Complete resolved realization retained behind the durable artifact.
    ///
    /// Client projections derive the admitted space, discretization, mesh,
    /// quadrature, scalar, layout, and spatial-dimension facts from this value
    /// rather than reconstructing the frozen tuple independently.
    #[must_use]
    pub const fn resolved(&self) -> &ResolvedRealization {
        &self.resolved
    }

    /// Exact Cartesian geometry identity produced during resolution.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact generated Cartesian mesh artifact produced during resolution.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &CartesianMeshEnvelopeV1 {
        &self.mesh_artifact
    }

    /// Exact geometry-to-mesh entity correspondence produced during resolution.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
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

/// Complete accepted lineage for the mixed-boundary Cartesian Q1 case.
///
/// This immutable in-process value is neither a durable Result artifact nor a
/// general elasticity API. It retains the accepted Model, Realization, Run,
/// native solution evidence, and canonical Q1 projection consumed by clients.
#[derive(Debug, Clone, PartialEq)]
pub struct MixedBoundaryElasticityResult2d {
    model: ModelEnvelope,
    realization: RealizationEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    mesh_artifact: CartesianMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    displacement_snapshot: CartesianQ1FieldSnapshotEnvelopeV1,
    run: RunManifestV2,
    vertices_m: Vec<[f64; 2]>,
    cells: Vec<[u32; 4]>,
    displacements_m: Vec<[f64; 2]>,
    bounds_m: [[f64; 2]; 2],
    solution: CartesianLinearElasticity2dSolution,
}

impl MixedBoundaryElasticityResult2d {
    /// Execute the one accepted mixed-boundary reference configuration.
    ///
    /// The caller supplies the current Model artifact explicitly. Admission compares
    /// its alpha-normalized graph with the independently registered reference
    /// source, while exact artifact identity remains attached to this run.
    ///
    /// # Errors
    /// Returns a structured diagnostic for a foreign Model, policy drift,
    /// unsupported or mutable backend identity, solve failure, invalid
    /// lineage, or malformed projection data.
    pub fn solve_reference(
        document: &ModelDocument,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        ResolvedLinearElasticityPlan2d::resolve(document, reference_intent()?, backend)?
            .execute(backend)
    }

    fn from_execution(
        plan: &ResolvedLinearElasticityPlan2d,
        bounds_m: [[f64; 2]; 2],
        solution: CartesianLinearElasticity2dSolution,
    ) -> Result<Self, Diagnostic> {
        let identities = require_accepted_model(&plan.document)?;
        let (vertices_m, cells, displacements_m) = project_solution(&solution)?;
        let displacement_snapshot = CartesianQ1FieldSnapshotEnvelopeV1::new(
            &plan.model,
            &plan.realization,
            &plan.geometry,
            &plan.correspondence,
            &plan.mesh_artifact,
            identities.displacement,
            displacements_m.iter().flatten().copied(),
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
        let run = RunManifestV2::new(&plan.realization, execution)?
            .with_output(displacement_snapshot.digest()?);
        run.validate_against(&plan.realization)?;

        Ok(Self {
            model: plan.model.clone(),
            realization: plan.realization.clone(),
            geometry: plan.geometry.clone(),
            mesh_artifact: plan.mesh_artifact.clone(),
            correspondence: plan.correspondence.clone(),
            displacement_snapshot,
            run,
            vertices_m,
            cells,
            displacements_m,
            bounds_m,
            solution,
        })
    }

    /// Exact current Model used by this execution.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Exact field-wise Realization.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV1 {
        &self.realization
    }

    /// Exact Cartesian geometry revision selected by this execution.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact generated Cartesian mesh artifact.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &CartesianMeshEnvelopeV1 {
        &self.mesh_artifact
    }

    /// Exact geometry-to-mesh entity correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Exact displacement Field snapshot emitted by this execution.
    #[must_use]
    pub const fn displacement_snapshot(&self) -> &CartesianQ1FieldSnapshotEnvelopeV1 {
        &self.displacement_snapshot
    }

    /// Run manifest whose sole output is [`Self::displacement_snapshot`].
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Canonical Q1 vertex coordinates in metres.
    #[must_use]
    pub fn vertices_m(&self) -> &[[f64; 2]] {
        &self.vertices_m
    }

    /// Canonical quadrilateral connectivity into [`Self::vertices_m`].
    #[must_use]
    pub fn cells(&self) -> &[[u32; 4]] {
        &self.cells
    }

    /// Canonical nodal displacements in metres.
    #[must_use]
    pub fn displacements_m(&self) -> &[[f64; 2]] {
        &self.displacements_m
    }

    /// Exact Cartesian bounds in metres.
    #[must_use]
    pub const fn bounds_m(&self) -> &[[f64; 2]; 2] {
        &self.bounds_m
    }

    /// Complete coherent-SI solution and execution evidence.
    #[must_use]
    pub const fn solution(&self) -> &CartesianLinearElasticity2dSolution {
        &self.solution
    }

    /// Registered scientific evidence case attributed to this application.
    #[must_use]
    pub const fn scientific_case_id(&self) -> &'static str {
        SCIENTIFIC_CASE_ID
    }

    /// Frozen number of Cartesian cells on each axis.
    #[must_use]
    pub const fn cells_per_axis(&self) -> usize {
        CELLS_PER_AXIS
    }

    /// Semantic revision retained by the exact Model.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.model.source_revision()
    }

    /// Explicit revision retained by the accepted Realization request.
    #[must_use]
    pub const fn realization_revision(&self) -> u64 {
        REALIZATION_REVISION
    }

    /// Coherent-SI dimension of every displacement component.
    #[must_use]
    pub const fn displacement_dimension(&self) -> DimExponents {
        DISPLACEMENT_DIMENSION
    }
}

struct AcceptedIdentities {
    body: Id<kinds::Domain>,
    displacement: Id<kinds::Field>,
}

struct ResolvedLinearElasticityApplication2d {
    resolved: ResolvedRealization,
    model: ModelEnvelope,
    realization: RealizationEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    mesh_artifact: CartesianMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
}

fn resolve_application(
    document: &ModelDocument,
    intent: LinearElasticityIntent2d,
    backend: &dyn LinearSolverBackend,
) -> Result<ResolvedLinearElasticityApplication2d, Diagnostic> {
    validate_scientific_case(SCIENTIFIC_CASE)?;
    require_supported_intent(intent)?;
    let identities = require_accepted_model(document)?;
    let realization_plan = realization_plan(intent)?;
    backend.capabilities().require_problem(
        realization_plan.solver(),
        ScalarType::F64,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?;

    let resolved = resolve(
        &RealizationRequest::explicit(
            document.program().model(),
            SemanticRevision::new(document.program().revision().0),
            RealizationRevision::new(REALIZATION_REVISION),
            realization_plan,
        ),
        RealizationRequirements::new(
            NonZeroUsize::new(2).expect("positive frozen dimension"),
            ScalarType::F64,
            VectorLayoutKind::Replicated,
        ),
        &RealizationCapabilities::isotropic_elasticity_2d_reference(),
    )?;
    if resolved.source()
        != ResolutionSource::Explicit(RealizationRevision::new(REALIZATION_REVISION))
    {
        return Err(internal(
            "mixed-boundary Realization revision changed during resolution",
        ));
    }

    let model = ModelEnvelope::from_program(document.program())?;
    let realization =
        RealizationEnvelopeV1::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)?;
    let geometry = GeometryIdentityEnvelopeV1::new(
        &model,
        [identities.body],
        GEOMETRY_CLASSIFICATION_PRECISION_M,
    )?;
    let lowered = lower_isotropic_elasticity_cartesian_2d(document.program())?;
    let mesh = CartesianMesh::uniform(
        lowered.bounds(),
        &[intent.cells_per_axis().get(), intent.cells_per_axis().get()],
    )?;
    let mesh_artifact = CartesianMeshEnvelopeV1::from_mesh(&mesh)?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new_cartesian(&geometry, &model, &mesh_artifact)?;

    Ok(ResolvedLinearElasticityApplication2d {
        resolved,
        model,
        realization,
        geometry,
        mesh_artifact,
        correspondence,
    })
}

fn require_accepted_model(document: &ModelDocument) -> Result<AcceptedIdentities, Diagnostic> {
    if document.program().revision().0 != 1 {
        return Err(invalid(
            "mixed-boundary elasticity requires the accepted Model at semantic revision 1",
        ));
    }
    let reference = ModelDocument::compile("mixed-boundary-elasticity.eqi", REFERENCE_SOURCE)
        .map_err(first_diagnostic)?;
    if !document.structurally_equivalent(&reference)? {
        return Err(invalid(
            "mixed-boundary elasticity requires the accepted reference Model meaning",
        ));
    }
    let displacement = document
        .aliases()
        .get("displacement")
        .copied()
        .ok_or_else(|| invalid("mixed-boundary Model omitted the displacement alias"))?;
    let Some(KernelNode::Field(field)) = document.program().node(displacement) else {
        return Err(invalid(
            "mixed-boundary displacement alias does not identify a Field",
        ));
    };
    if field.dimension() != DISPLACEMENT_DIMENSION {
        return Err(invalid(
            "mixed-boundary displacement Field is not measured in metres",
        ));
    }
    let body = document
        .aliases()
        .get("body")
        .and_then(|id| id.downcast::<kinds::Domain>())
        .ok_or_else(|| invalid("mixed-boundary Model omitted the body Domain alias"))?;
    Ok(AcceptedIdentities {
        body,
        displacement: displacement
            .downcast::<kinds::Field>()
            .expect("validated displacement node has Field identity"),
    })
}

fn require_supported_intent(intent: LinearElasticityIntent2d) -> Result<(), Diagnostic> {
    if intent == reference_intent()? {
        Ok(())
    } else {
        Err(Diagnostic::error(
            codes::NOT_IMPLEMENTED,
            "the accepted linear-elasticity application does not implement this intent without fallback",
        ))
    }
}

fn reference_intent() -> Result<LinearElasticityIntent2d, Diagnostic> {
    LinearElasticityIntent2d::new(
        NonZeroUsize::new(CELLS_PER_AXIS).expect("positive frozen refinement"),
        RELATIVE_TOLERANCE,
        ABSOLUTE_TOLERANCE,
        NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("positive frozen iteration limit"),
    )
}

fn realization_plan(intent: LinearElasticityIntent2d) -> Result<RealizationPlan, Diagnostic> {
    RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: intent.cells_per_axis(),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("positive frozen quadrature"),
            },
        ),
        intent.solver(),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
}

fn validate_solve_report(
    solution: &CartesianLinearElasticity2dSolution,
    backend_provider: SolverProvider,
    solver: SolverPlan,
) -> Result<(), Diagnostic> {
    let report = solution.solve_report();
    if report.solver_provider() != backend_provider
        || report.execution_provider() != SERIAL_EXECUTION_PROVIDER
        || report.verification_provider() != SERIAL_EXECUTION_PROVIDER
        || report.execution() != eqiora_solver::ExecutionReport::host_serial()
        || report.verification() != eqiora_solver::ExecutionReport::host_serial()
        || report.algorithm() != LinearSolver::ConjugateGradient
        || report.preconditioner() != PreconditionerPolicy::Identity
        || report.reduction() != ReductionPolicy::Reproducible
        || report.solver_plan() != solver
        || report.true_residual_norm() > report.residual_target()
    {
        return Err(internal(
            "mixed-boundary solve evidence differs from the frozen execution policy",
        ));
    }
    if solution.assembly_report().execution() != eqiora_solver::ExecutionReport::host_serial() {
        return Err(internal(
            "mixed-boundary assembly evidence differs from one-host-one-worker",
        ));
    }
    Ok(())
}

type StructuralProjection = (Vec<[f64; 2]>, Vec<[u32; 4]>, Vec<[f64; 2]>);

fn project_solution(
    solution: &CartesianLinearElasticity2dSolution,
) -> Result<StructuralProjection, Diagnostic> {
    let mesh = solution.displacement().mesh();
    let vertex_count = mesh
        .entity_count(0)
        .ok_or_else(|| internal("mixed-boundary mesh omitted its vertex count"))?;
    let cell_count = mesh
        .entity_count(2)
        .ok_or_else(|| internal("mixed-boundary mesh omitted its cell count"))?;
    if vertex_count != (CELLS_PER_AXIS + 1).pow(2) || cell_count != CELLS_PER_AXIS.pow(2) {
        return Err(internal(
            "mixed-boundary result shape differs from the frozen Q1 mesh",
        ));
    }

    let mut vertices_m = Vec::with_capacity(vertex_count);
    let mut displacements_m = Vec::with_capacity(vertex_count);
    for index in 0..vertex_count {
        let entity = MeshEntity::new(0, index);
        let coordinates = mesh
            .vertex_coordinates(entity)
            .and_then(|value| <[f64; 2]>::try_from(value).ok())
            .ok_or_else(|| internal(format!("mixed-boundary mesh omitted vertex {index}")))?;
        let mut displacement = solution
            .displacement()
            .vertex_values(index)
            .and_then(|value| <[f64; 2]>::try_from(value).ok())
            .ok_or_else(|| {
                internal(format!(
                    "mixed-boundary result omitted displacement vertex {index}"
                ))
            })?;
        for value in &mut displacement {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        if coordinates.into_iter().any(|value| !value.is_finite())
            || displacement.into_iter().any(|value| !value.is_finite())
        {
            return Err(internal(
                "mixed-boundary result contains non-finite vertex data",
            ));
        }
        vertices_m.push(coordinates);
        displacements_m.push(displacement);
    }

    let mut cells = Vec::with_capacity(cell_count);
    for index in 0..cell_count {
        let vertices = mesh
            .entity_vertices(MeshEntity::new(2, index))
            .ok_or_else(|| {
                internal(format!(
                    "mixed-boundary mesh omitted cell {index} connectivity"
                ))
            })?;
        let vertices = <[MeshEntity; 4]>::try_from(vertices).map_err(|_| {
            internal(format!(
                "mixed-boundary mesh cell {index} is not a Q1 quadrilateral"
            ))
        })?;
        let mut cell = [0_u32; 4];
        for (target, vertex) in cell.iter_mut().zip(vertices) {
            if vertex.dimension() != 0 || vertex.index() >= vertex_count {
                return Err(internal(
                    "mixed-boundary cell connectivity is not canonical vertex indexing",
                ));
            }
            *target = u32::try_from(vertex.index())
                .map_err(|_| internal("mixed-boundary vertex index exceeds u32"))?;
        }
        let mut unique = cell;
        unique.sort_unstable();
        if unique.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(internal(
                "mixed-boundary cell repeats a vertex in its connectivity",
            ));
        }
        cells.push(cell);
    }
    Ok((vertices_m, cells, displacements_m))
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| internal("reference Model compilation failed without a diagnostic"))
}

fn validate_scientific_case(manifest: &str) -> Result<(), Diagnostic> {
    let exact_line = |key: &str| {
        manifest
            .lines()
            .find(|line| line.starts_with(key))
            .map(str::trim)
    };
    if exact_line("id") != Some("id = \"solid.mixed-boundary-elasticity-2d\"")
        || exact_line("status") != Some("status = \"verified\"")
    {
        return Err(invalid(format!(
            "registered scientific case `{SCIENTIFIC_CASE_ID}` is missing or no longer verified"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

fn internal(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INTERNAL_FAILURE, message)
}

#[cfg(test)]
mod tests {
    use eqiora_solver::REFERENCE_LINEAR_SOLVER;

    use super::*;

    #[test]
    fn scientific_case_reference_fails_closed_when_stale() {
        assert!(validate_scientific_case(SCIENTIFIC_CASE).is_ok());
        assert!(
            validate_scientific_case(
                &SCIENTIFIC_CASE.replace("status = \"verified\"", "status = \"candidate\"")
            )
            .is_err()
        );
        assert!(
            validate_scientific_case(
                &SCIENTIFIC_CASE.replace(SCIENTIFIC_CASE_ID, "solid.another-case")
            )
            .is_err()
        );
    }

    #[test]
    fn shared_result_closes_lineage_and_rejects_alias_free_replay() {
        let document = ModelDocument::compile("mixed-boundary-elasticity.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let result =
            MixedBoundaryElasticityResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
                .expect("accepted shared application result");
        assert_eq!(result.vertices_m().len(), 289);
        assert_eq!(result.cells().len(), 256);
        assert_eq!(result.displacements_m().len(), 289);
        assert_eq!(
            result.run().outputs(),
            [result.displacement_snapshot().digest().unwrap()]
        );
        assert!(
            result.solution().solve_report().true_residual_norm()
                <= result.solution().solve_report().residual_target()
        );

        let replay = ModelDocument::replay(&document.canonical_json().expect("canonical Model"))
            .expect("current Model replay");
        let error =
            MixedBoundaryElasticityResult2d::solve_reference(&replay, &REFERENCE_LINEAR_SOLVER)
                .expect_err(
                    "artifact replay without source aliases is outside this source workflow",
                );
        assert_eq!(error.code(), codes::INVALID_REALIZATION);
    }
}
