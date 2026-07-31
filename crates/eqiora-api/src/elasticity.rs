//! One accepted mixed-boundary linear-elasticity application result.
//!
//! This module owns the complete Model-to-Run composition shared by Studio
//! and Python. It is intentionally one bounded application value rather than
//! a general structural-result hierarchy.

use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, LayoutArtifacts, ModelEnvelopeV4,
    RealizationEnvelopeV1, RunManifestV2,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents};
use eqiora_meshing::{MeshEntity, MeshTopology};
use eqiora_numerics::solid::{
    CartesianLinearElasticity2dSolution, solve_resolved_isotropic_elasticity_cartesian_2d,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, ResolutionSource, SemanticRevision, Space, Target, VectorLayoutKind,
    resolve,
};
use eqiora_schema::kernel::KernelNode;
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType, SolverPlan,
};

use crate::{ExactModelCodec, ModelDocument};

const REFERENCE_SOURCE: &str =
    include_str!("../../../verify/solid/mixed-boundary-elasticity-2d/models/direct.eqi");
const SCIENTIFIC_CASE: &str =
    include_str!("../../../verify/solid/mixed-boundary-elasticity-2d/case.toml");
const SCIENTIFIC_CASE_ID: &str = "solid.mixed-boundary-elasticity-2d";
const CELLS_PER_AXIS: usize = 16;
const REALIZATION_REVISION: u64 = 1;
const RELATIVE_TOLERANCE: f64 = 1.0e-12;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-14;
const MAXIMUM_ITERATIONS: usize = 10_000;
const DISPLACEMENT_DIMENSION: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};

/// Complete accepted lineage for the mixed-boundary Cartesian Q1 case.
///
/// This immutable in-process value is neither a durable Result artifact nor a
/// general elasticity API. It retains the accepted Model, Realization, Run,
/// native solution evidence, and canonical Q1 projection consumed by clients.
#[derive(Debug, Clone, PartialEq)]
pub struct MixedBoundaryElasticityResult2d {
    model: ModelEnvelopeV4,
    realization: RealizationEnvelopeV1,
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
    /// The caller supplies an exact V4 Model explicitly. Admission compares
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
        validate_scientific_case(SCIENTIFIC_CASE)?;
        require_accepted_model(document)?;
        let plan = reference_plan()?;
        let backend_provider = backend.provider();
        let backend_capabilities = backend.capabilities();
        backend_capabilities.require_problem(
            plan.solver(),
            ScalarType::F64,
            LinearOperatorProperties::SymmetricPositiveDefinite,
        )?;

        let resolved = resolve(
            &RealizationRequest::explicit(
                document.program().model(),
                SemanticRevision::new(document.program().revision().0),
                RealizationRevision::new(REALIZATION_REVISION),
                plan,
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

        let (lowered, solution) = solve_resolved_isotropic_elasticity_cartesian_2d(
            document.program(),
            &resolved,
            backend,
        )?;
        if backend.provider() != backend_provider || backend.capabilities() != backend_capabilities
        {
            return Err(internal(
                "linear solver provider identity or capabilities changed during execution",
            ));
        }
        validate_solve_report(&solution, backend_provider)?;

        let (vertices_m, cells, displacements_m) = project_solution(&solution)?;
        let model = ModelEnvelopeV4::from_program(document.program())?;
        let realization =
            RealizationEnvelopeV1::from_resolved(&model, &resolved, LayoutArtifacts::Replicated)?;
        let execution = ExecutionProvenanceV1::from_provider_releases(
            backend_provider,
            SERIAL_EXECUTION_PROVIDER,
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            ReductionPolicy::Reproducible,
            std::iter::empty::<(&str, &str)>(),
        )?;
        let run = RunManifestV2::new(&realization, execution)?;
        run.validate_against(&realization)?;

        Ok(Self {
            model,
            realization,
            run,
            vertices_m,
            cells,
            displacements_m,
            bounds_m: *lowered.bounds(),
            solution,
        })
    }

    /// Exact V4 Model used by this execution.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelopeV4 {
        &self.model
    }

    /// Exact field-wise Realization.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV1 {
        &self.realization
    }

    /// Output-less Run manifest for this bounded solve.
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

fn require_accepted_model(document: &ModelDocument) -> Result<(), Diagnostic> {
    if document.exact_codec() != ExactModelCodec::V4 || document.program().revision().0 != 1 {
        return Err(invalid(
            "mixed-boundary elasticity requires the accepted exact V4 Model at semantic revision 1",
        ));
    }
    let reference = ExactModelCodec::V4
        .compile("mixed-boundary-elasticity.eqi", REFERENCE_SOURCE)
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
    Ok(())
}

fn reference_plan() -> Result<RealizationPlan, Diagnostic> {
    RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(CELLS_PER_AXIS)
                    .expect("positive frozen refinement"),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("positive frozen quadrature"),
            },
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            RELATIVE_TOLERANCE,
            ABSOLUTE_TOLERANCE,
            NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("positive frozen iteration limit"),
        )?,
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
}

fn validate_solve_report(
    solution: &CartesianLinearElasticity2dSolution,
    backend_provider: eqiora_solver::SolverProvider,
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
        || report.solver_plan() != reference_plan()?.solver()
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
        let displacement = solution
            .displacement()
            .vertex_values(index)
            .and_then(|value| <[f64; 2]>::try_from(value).ok())
            .ok_or_else(|| {
                internal(format!(
                    "mixed-boundary result omitted displacement vertex {index}"
                ))
            })?;
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
        let document = ExactModelCodec::V4
            .compile("mixed-boundary-elasticity.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let result =
            MixedBoundaryElasticityResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
                .expect("accepted shared application result");
        assert_eq!(result.vertices_m().len(), 289);
        assert_eq!(result.cells().len(), 256);
        assert_eq!(result.displacements_m().len(), 289);
        assert!(result.run().outputs().is_empty());
        assert!(
            result.solution().solve_report().true_residual_norm()
                <= result.solution().solve_report().residual_target()
        );

        let replay = ExactModelCodec::V4
            .replay(&document.canonical_json().expect("canonical exact-v4 Model"))
            .expect("exact-v4 replay");
        let error =
            MixedBoundaryElasticityResult2d::solve_reference(&replay, &REFERENCE_LINEAR_SOLVER)
                .expect_err(
                    "artifact replay without source aliases is outside this source workflow",
                );
        assert_eq!(error.code(), codes::INVALID_REALIZATION);
    }
}
