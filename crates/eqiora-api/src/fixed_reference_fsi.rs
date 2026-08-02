//! One accepted fixed-reference two-step FSI application result.
//!
//! This module owns the complete Model-to-trajectory composition shared by
//! Studio and installed Python. It deliberately adds one bounded application
//! value, not a general coupling graph, mutable step builder, or durable Result
//! schema. Its durable leaves are retained so the general fixed-mesh replay
//! boundary can independently close every content-addressed edge.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

use eqiora_artifact::{
    ArtifactDigest, DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1,
    FieldSnapshotEnvelopeV1, GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    LayoutArtifacts, ModelEnvelope, RealizationEnvelopeV3, RunManifestV2, SimplicialMeshEnvelopeV1,
    SpatialStateEnvelopeV1, SpatialTrajectoryEnvelopeV1, SpatialTrajectorySegmentEnvelopeV1,
    ValidatedFixedSpatialContextV1,
};
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_meshing::{CellId, FacetId, MeshQualityGate, MeshTopology, SimplicialMesh};
use eqiora_numerics::fsi::{
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiPartition2d,
    FixedReferenceFsiScaleProfile2d, FixedReferenceFsiState2d, ResolvedFixedReferenceFsiSolution2d,
    finalize_resolved_fixed_reference_fsi_step_2d, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d, lower_fixed_reference_fsi_cartesian_2d,
};
use eqiora_realization::{
    CoupledFieldwiseRealizationRequest, MeshArtifactReference, RealizationCapabilities,
    RealizationRevision, ResolvedCoupledFieldwiseRealization, SemanticRevision,
    resolve_coupled_fieldwise,
};
use eqiora_solver::{
    LinearOperatorProperties, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    ReductionPolicy, SERIAL_EXECUTION_PROVIDER, ScalarType, SolverPlan,
};

use crate::{
    FixedMeshFieldTrajectoryReplay2dV1, ModelDocument, snapshot_fixed_reference_fsi_solution_v1,
};

const REFERENCE_SOURCE: &str =
    include_str!("../../../verify/fsi/fixed-reference-monolithic-step-2d/models/direct.eqi");
const STEP_CASE: &str =
    include_str!("../../../verify/fsi/fixed-reference-monolithic-step-2d/case.toml");
const TRAJECTORY_CASE: &str =
    include_str!("../../../verify/artifacts/fixed-reference-fsi-spatial-trajectory/case.toml");
const STEP_CASE_ID: &str = "fsi.fixed-reference-monolithic-step-2d";
const TRAJECTORY_CASE_ID: &str = "artifacts.fixed-reference-fsi-spatial-trajectory";
const REALIZATION_REVISION: u64 = 1;
const TIME_STEP_S: f64 = 0.05;
const LENGTH_SCALE_M: f64 = 2.0;
const VELOCITY_SCALE_M_PER_S: f64 = 0.5;
const PRESSURE_SCALE_PA: f64 = 4.0;
const RELATIVE_TOLERANCE: f64 = 1.0e-11;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-13;
const MAXIMUM_ITERATIONS: usize = 20_000;
const LENGTH: DimExponents = DimExponents {
    length: 1,
    ..DimExponents::DIMENSIONLESS
};
const TIME: DimExponents = DimExponents {
    time: 1,
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

struct SpatialContext {
    model: ModelEnvelope,
    mesh: SimplicialMesh,
    mesh_artifact: SimplicialMeshEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    partition: FixedReferenceFsiPartition2d,
}

struct ExecutionContext {
    mesh_reference: MeshArtifactReference,
    resolved: ResolvedCoupledFieldwiseRealization,
    realization: RealizationEnvelopeV3,
    run: RunManifestV2,
}

/// Complete accepted lineage for the bounded fixed-reference FSI application.
///
/// The value owns two consecutive accepted monolithic steps and their exact
/// fixed-spatial trajectory. Construction details such as the reference mesh,
/// initial prestrain, plan resolution, and state transition remain private so
/// clients cannot create a second composition authority.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedReferenceFsiResult2d {
    model: ModelEnvelope,
    mesh: SimplicialMesh,
    mesh_artifact: SimplicialMeshEnvelopeV1,
    geometry: GeometryIdentityEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    partition: FixedReferenceFsiPartition2d,
    realization: RealizationEnvelopeV3,
    solutions: [ResolvedFixedReferenceFsiSolution2d; 2],
    states: [SpatialStateEnvelopeV1; 2],
    snapshots: Vec<FieldSnapshotEnvelopeV1>,
    blocks: Vec<DiscreteFieldEnvelopeV1>,
    segments: [SpatialTrajectorySegmentEnvelopeV1; 2],
    trajectory: SpatialTrajectoryEnvelopeV1,
    run: RunManifestV2,
}

impl FixedReferenceFsiResult2d {
    /// Execute the accepted two-step fixed-reference FSI configuration.
    ///
    /// Admission compares the caller's current Model structurally with the
    /// registered direct source while retaining the caller's exact artifact
    /// identity. Structural admission does not add a digest-equality claim.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign Model meaning, stale
    /// scientific evidence, realization or backend drift, solve rejection,
    /// malformed projection data, or invalid exact lineage.
    pub fn solve_reference(
        document: &ModelDocument,
        backend: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        validate_scientific_case(STEP_CASE, STEP_CASE_ID)?;
        validate_scientific_case(TRAJECTORY_CASE, TRAJECTORY_CASE_ID)?;
        require_accepted_model(document)?;

        let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program())?;
        let spatial = spatial_context(document.program(), &canonical)?;
        let backend_provider = backend.provider();
        let backend_capabilities = backend.capabilities();
        let solver_plan = reference_solver()?;
        backend_capabilities.require_problem(
            solver_plan,
            ScalarType::F64,
            LinearOperatorProperties::SymmetricIndefinite,
        )?;
        let execution = execution_context(
            document.program(),
            &canonical,
            &spatial,
            solver_plan,
            backend,
        )?;

        let first = solve_step(
            &canonical,
            &spatial,
            &execution,
            &prestrained_state(&spatial)?,
            backend,
        )?;
        require_unchanged_backend(backend, backend_provider, &backend_capabilities)?;
        let second = solve_step(
            &canonical,
            &spatial,
            &execution,
            &state_from_solution(&spatial, &first)?,
            backend,
        )?;
        require_unchanged_backend(backend, backend_provider, &backend_capabilities)?;

        let fixed = ValidatedFixedSpatialContextV1::new(
            &spatial.model,
            &execution.realization,
            &spatial.geometry,
            &spatial.correspondence,
            &spatial.mesh_artifact,
        )?;
        let first_snapshots = snapshot_fixed_reference_fsi_solution_v1(&fixed, &first)?;
        let second_snapshots = snapshot_fixed_reference_fsi_solution_v1(&fixed, &second)?;
        let first_state =
            SpatialStateEnvelopeV1::new(&fixed, 1, TIME_STEP_S, first_snapshots.snapshots())?;
        let second_state = SpatialStateEnvelopeV1::new(
            &fixed,
            2,
            2.0 * TIME_STEP_S,
            second_snapshots.snapshots(),
        )?;
        let first_segment =
            SpatialTrajectorySegmentEnvelopeV1::new(&fixed, std::slice::from_ref(&first_state))?;
        let second_segment =
            SpatialTrajectorySegmentEnvelopeV1::new(&fixed, std::slice::from_ref(&second_state))?;
        let first_root = SpatialTrajectoryEnvelopeV1::start(&fixed, &first_segment)?;
        let trajectory = SpatialTrajectoryEnvelopeV1::extend(&fixed, &first_root, &second_segment)?;
        let run = execution.run.with_output(trajectory.digest()?);
        run.validate_against(&execution.realization)?;
        let snapshots = unique_catalog(
            first_snapshots
                .snapshots()
                .iter()
                .chain(second_snapshots.snapshots())
                .cloned(),
            FieldSnapshotEnvelopeV1::digest,
        )?;
        let blocks = unique_catalog(
            [&first_snapshots, &second_snapshots]
                .into_iter()
                .flat_map(|set| {
                    set.snapshots().iter().flat_map(|snapshot| {
                        set.blocks(snapshot.field())
                            .expect("accepted snapshot set retains every exact block")
                            .iter()
                    })
                })
                .cloned(),
            DiscreteFieldEnvelopeV1::digest,
        )?;

        let result = Self {
            model: spatial.model,
            mesh: spatial.mesh,
            mesh_artifact: spatial.mesh_artifact,
            geometry: spatial.geometry,
            correspondence: spatial.correspondence,
            partition: spatial.partition,
            realization: execution.realization,
            solutions: [first, second],
            states: [first_state, second_state],
            snapshots,
            blocks,
            segments: [first_segment, second_segment],
            trajectory,
            run,
        };
        result.validate(backend_provider)?;
        Ok(result)
    }

    /// Exact caller-owned current Model used by this execution.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Exact fixed affine-triangle mesh used by both accepted steps.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }

    /// Durable mesh identity bound into the accepted Realization.
    #[must_use]
    pub const fn mesh_artifact(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh_artifact
    }

    /// Exact two-body geometry identity.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact Geometry-to-Mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Exhaustive fluid, solid, and interface partition.
    #[must_use]
    pub const fn partition(&self) -> &FixedReferenceFsiPartition2d {
        &self.partition
    }

    /// Exact multi-Domain Realization shared by both steps.
    #[must_use]
    pub const fn realization(&self) -> &RealizationEnvelopeV3 {
        &self.realization
    }

    /// Two consecutive accepted solutions in trajectory order.
    #[must_use]
    pub const fn solutions(&self) -> &[ResolvedFixedReferenceFsiSolution2d; 2] {
        &self.solutions
    }

    /// Two complete accepted spatial states in trajectory order.
    #[must_use]
    pub const fn states(&self) -> &[SpatialStateEnvelopeV1; 2] {
        &self.states
    }

    /// Fully replay the durable fixed-mesh Field trajectory dependency DAG.
    ///
    /// # Errors
    /// Returns an artifact diagnostic if any retained resource, Field leaf,
    /// state, immutable prefix, or exact Run output has drifted.
    pub fn trajectory_replay(&self) -> Result<FixedMeshFieldTrajectoryReplay2dV1<'_>, Diagnostic> {
        FixedMeshFieldTrajectoryReplay2dV1::new(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh_artifact,
            &self.trajectory,
            &self.segments,
            &self.states,
            &self.snapshots,
            &self.blocks,
            &self.run,
        )
    }

    /// Immutable two-segment trajectory whose final digest is the Run output.
    #[must_use]
    pub const fn trajectory(&self) -> &SpatialTrajectoryEnvelopeV1 {
        &self.trajectory
    }

    /// Final Run manifest with exactly one trajectory output.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Accepted state coordinates in seconds, in solution order.
    #[must_use]
    pub fn time_coordinates_s(&self) -> [f64; 2] {
        [self.states[0].time_s(), self.states[1].time_s()]
    }

    /// Scientific cases that own the step and trajectory claims.
    #[must_use]
    pub const fn scientific_case_ids(&self) -> [&'static str; 2] {
        [STEP_CASE_ID, TRAJECTORY_CASE_ID]
    }

    /// Exact semantic revision retained by the caller Model.
    #[must_use]
    pub const fn semantic_revision(&self) -> u64 {
        self.model.source_revision()
    }

    /// Explicit revision retained by the accepted Realization request.
    #[must_use]
    pub const fn realization_revision(&self) -> u64 {
        REALIZATION_REVISION
    }

    fn validate(&self, backend_provider: eqiora_solver::SolverProvider) -> Result<(), Diagnostic> {
        self.trajectory_replay()?;
        let vertex_count = self
            .mesh
            .entity_count(0)
            .ok_or_else(|| internal("fixed-reference FSI mesh omitted vertices"))?;
        let cell_count = self
            .mesh
            .entity_count(2)
            .ok_or_else(|| internal("fixed-reference FSI mesh omitted cells"))?;
        if vertex_count != 9
            || cell_count != 8
            || self.partition.fluid_cells().len() != 4
            || self.partition.solid_cells().len() != 4
            || self.partition.interface_facets().len() != 2
        {
            return Err(internal(
                "fixed-reference FSI result differs from the frozen two-body partition",
            ));
        }
        if self.time_coordinates_s() != [TIME_STEP_S, 2.0 * TIME_STEP_S]
            || self.states[0].step() != 1
            || self.states[1].step() != 2
            || self.solutions[0].solid_displacement_coefficients()
                == self.solutions[1].solid_displacement_coefficients()
        {
            return Err(internal(
                "fixed-reference FSI result omitted two distinct consecutive accepted steps",
            ));
        }
        for solution in &self.solutions {
            let evidence = solution.numerical_evidence();
            let report = evidence.solve_report();
            if solution.vertex_velocity_coefficients().len() != 9
                || solution.fluid_velocity_bubble_coefficients().len() != 4
                || solution.fluid_pressure_vertices().len() != 6
                || solution.fluid_pressure_coefficients().len() != 6
                || solution.solid_displacement_coefficients().len() != 9
                || evidence.interface_actions().len() != 1
                || report.solver_provider() != backend_provider
                || report.solver_plan() != reference_solver()?
                || report.true_residual_norm() > report.residual_target()
            {
                return Err(internal(
                    "fixed-reference FSI solution differs from the frozen execution shape",
                ));
            }
            if solution
                .vertex_velocity_coefficients()
                .iter()
                .chain(solution.fluid_velocity_bubble_coefficients())
                .chain(solution.solid_displacement_coefficients())
                .flatten()
                .chain(solution.fluid_pressure_coefficients())
                .any(|value| !value.is_finite())
            {
                return Err(internal(
                    "fixed-reference FSI result contains a non-finite physical value",
                ));
            }
            for vertex in 0..vertex_count {
                let supported = solution
                    .solid_displacement_vertices()
                    .iter()
                    .any(|candidate| candidate.index() == vertex);
                if !supported && solution.solid_displacement_coefficients()[vertex] != [0.0; 2] {
                    return Err(internal(
                        "fixed-reference FSI displacement escaped the solid closure",
                    ));
                }
            }
        }
        let trajectory_digest = self.trajectory.digest()?;
        if self.run.outputs() != vec![trajectory_digest]
            || self.run.model() != self.model.digest()?
            || self.run.realization() != self.realization.digest()?
        {
            return Err(internal(
                "fixed-reference FSI Run does not close the accepted trajectory lineage",
            ));
        }
        self.run.validate_against(&self.realization)
    }
}

fn unique_catalog<T: Clone>(
    items: impl IntoIterator<Item = T>,
    digest: impl Fn(&T) -> Result<ArtifactDigest, Diagnostic>,
) -> Result<Vec<T>, Diagnostic> {
    let mut catalog = BTreeMap::new();
    for item in items {
        catalog.entry(digest(&item)?).or_insert(item);
    }
    Ok(catalog.into_values().collect())
}

fn require_accepted_model(document: &ModelDocument) -> Result<(), Diagnostic> {
    if document.program().revision().0 != 1 {
        return Err(invalid(
            "fixed-reference FSI requires the accepted Model at semantic revision 1",
        ));
    }
    let reference = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
        .map_err(first_diagnostic)?;
    if !document.structurally_equivalent(&reference)? {
        return Err(invalid(
            "fixed-reference FSI requires the accepted reference Model meaning",
        ));
    }
    Ok(())
}

fn spatial_context(
    program: &eqiora_sem::KernelProgram,
    canonical: &FixedReferenceFsiCartesianModel2d,
) -> Result<SpatialContext, Diagnostic> {
    let model = ModelEnvelope::from_program(program)?;
    let mesh = physical_mesh()?;
    let mesh_artifact = SimplicialMeshEnvelopeV1::from_mesh(&mesh)?;
    let fluid = canonical
        .fluid()
        .domain()
        .downcast::<eqiora_core::entity::kinds::Domain>()
        .ok_or_else(|| internal("fixed-reference fluid Domain identity changed kind"))?;
    let solid = canonical
        .solid()
        .domain()
        .downcast::<eqiora_core::entity::kinds::Domain>()
        .ok_or_else(|| internal("fixed-reference solid Domain identity changed kind"))?;
    let geometry = GeometryIdentityEnvelopeV1::new(&model, [solid, fluid], 1.0e-12)?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh_artifact)?;
    let connection = canonical
        .interface()
        .connection()
        .downcast::<eqiora_core::entity::kinds::Connection>()
        .ok_or_else(|| internal("fixed-reference interface Connection identity changed kind"))?;
    let interface = correspondence.derive_conserving_interface(
        &geometry,
        &model,
        &mesh_artifact,
        connection,
    )?;
    let fluid_cells = correspondence
        .body_cells(fluid)
        .ok_or_else(|| internal("geometry correspondence omitted the fluid body cells"))?
        .into_iter()
        .map(CellId::new)
        .collect();
    let solid_cells = correspondence
        .body_cells(solid)
        .ok_or_else(|| internal("geometry correspondence omitted the solid body cells"))?
        .into_iter()
        .map(CellId::new)
        .collect();
    let interface_facets = interface
        .facet_indices()
        .iter()
        .copied()
        .map(FacetId::new)
        .collect();
    let partition =
        FixedReferenceFsiPartition2d::new(&mesh, fluid_cells, solid_cells, interface_facets)?;
    Ok(SpatialContext {
        model,
        mesh,
        mesh_artifact,
        geometry,
        correspondence,
        partition,
    })
}

fn execution_context(
    program: &eqiora_sem::KernelProgram,
    canonical: &FixedReferenceFsiCartesianModel2d,
    spatial: &SpatialContext,
    solver_plan: SolverPlan,
    backend: &dyn LinearSolverBackend,
) -> Result<ExecutionContext, Diagnostic> {
    let mesh_reference =
        MeshArtifactReference::from_sha256(spatial.mesh_artifact.digest()?.sha256_bytes());
    let plan = fixed_reference_fsi_plan_2d(
        canonical,
        mesh_reference,
        DynQuantity::new(TIME_STEP_S, TIME),
        FixedReferenceFsiScaleProfile2d::new(
            DynQuantity::new(LENGTH_SCALE_M, LENGTH),
            DynQuantity::new(VELOCITY_SCALE_M_PER_S, VELOCITY),
            DynQuantity::new(PRESSURE_SCALE_PA, PRESSURE),
        )?,
        solver_plan,
    )?;
    let resolved = resolve_coupled_fieldwise(
        &CoupledFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(canonical.semantic_revision()),
            RealizationRevision::new(REALIZATION_REVISION),
            plan,
        ),
        fixed_reference_fsi_requirements_2d(canonical),
        &RealizationCapabilities::symmetric_mixed_simplicial_2d_reference(),
    )?;
    let realization = RealizationEnvelopeV3::from_resolved(
        &spatial.model,
        &resolved,
        LayoutArtifacts::Replicated,
    )?;
    realization.validate_model_artifact(&spatial.model)?;
    realization.validate_mesh_artifact(&spatial.mesh_artifact)?;
    let run = RunManifestV2::new(
        &realization,
        ExecutionProvenanceV1::from_provider_releases(
            backend.provider(),
            SERIAL_EXECUTION_PROVIDER,
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            ReductionPolicy::Reproducible,
            std::iter::empty::<(&str, &str)>(),
        )?,
    )?;
    run.validate_against(&realization)?;
    Ok(ExecutionContext {
        mesh_reference,
        resolved,
        realization,
        run,
    })
}

fn prestrained_state(spatial: &SpatialContext) -> Result<FixedReferenceFsiState2d, Diagnostic> {
    let mut displacement = vec![[0.0; 2]; spatial.mesh.vertices().len()];
    let interface_midpoint = spatial
        .mesh
        .vertices()
        .iter()
        .position(|point| point.as_slice() == [1.0, 0.5])
        .ok_or_else(|| internal("fixed-reference mesh omitted the free interface midpoint"))?;
    displacement[interface_midpoint] = [0.02, 0.0];
    FixedReferenceFsiState2d::new(
        &spatial.mesh,
        &spatial.partition,
        vec![[0.0; 2]; spatial.mesh.vertices().len()],
        vec![[0.0; 2]; spatial.partition.fluid_cells().len()],
        displacement,
    )
}

fn state_from_solution(
    spatial: &SpatialContext,
    solution: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<FixedReferenceFsiState2d, Diagnostic> {
    FixedReferenceFsiState2d::new(
        &spatial.mesh,
        &spatial.partition,
        solution.vertex_velocity_coefficients().to_vec(),
        solution.fluid_velocity_bubble_coefficients().to_vec(),
        solution.solid_displacement_coefficients().to_vec(),
    )
}

fn solve_step(
    canonical: &FixedReferenceFsiCartesianModel2d,
    spatial: &SpatialContext,
    execution: &ExecutionContext,
    previous: &FixedReferenceFsiState2d,
    backend: &dyn LinearSolverBackend,
) -> Result<ResolvedFixedReferenceFsiSolution2d, Diagnostic> {
    finalize_resolved_fixed_reference_fsi_step_2d(
        canonical,
        &execution.resolved,
        execution.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        previous,
    )?
    .solve(backend)
}

fn reference_solver() -> Result<SolverPlan, Diagnostic> {
    SolverPlan::new(
        LinearSolver::MinimumResidual,
        RELATIVE_TOLERANCE,
        ABSOLUTE_TOLERANCE,
        NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("positive frozen iteration limit"),
    )
    .map(|plan| {
        plan.with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Reproducible)
    })
}

fn physical_mesh() -> Result<SimplicialMesh, Diagnostic> {
    SimplicialMesh::new(
        2,
        vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 0.5],
            vec![1.0, 0.5],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![2.0, 0.0],
            vec![2.0, 0.5],
            vec![2.0, 1.0],
        ],
        vec![
            vec![0, 1, 3],
            vec![0, 3, 2],
            vec![2, 3, 5],
            vec![2, 5, 4],
            vec![1, 6, 7],
            vec![1, 7, 3],
            vec![3, 7, 8],
            vec![3, 8, 5],
        ],
        MeshQualityGate::new(0.05)?,
    )
}

fn require_unchanged_backend(
    backend: &dyn LinearSolverBackend,
    provider: eqiora_solver::SolverProvider,
    capabilities: &eqiora_solver::SolverCapabilities,
) -> Result<(), Diagnostic> {
    if backend.provider() != provider || backend.capabilities() != *capabilities {
        return Err(internal(
            "linear solver provider identity or capabilities changed during FSI execution",
        ));
    }
    Ok(())
}

fn first_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .next()
        .unwrap_or_else(|| internal("reference FSI Model compilation failed without a diagnostic"))
}

fn validate_scientific_case(manifest: &str, case_id: &str) -> Result<(), Diagnostic> {
    let expected_id = format!("id = \"{case_id}\"");
    let exact_line = |key: &str| {
        manifest
            .lines()
            .find(|line| line.starts_with(key))
            .map(str::trim)
    };
    if exact_line("id") != Some(expected_id.as_str())
        || exact_line("status") != Some("status = \"verified\"")
    {
        return Err(invalid(format!(
            "registered scientific case `{case_id}` is missing or no longer verified"
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
    use eqiora_meshing::DiscreteFieldAssociation;
    use eqiora_solver::REFERENCE_LINEAR_SOLVER;

    use super::*;

    #[test]
    fn scientific_case_references_fail_closed_when_stale() {
        for (manifest, case_id) in [
            (STEP_CASE, STEP_CASE_ID),
            (TRAJECTORY_CASE, TRAJECTORY_CASE_ID),
        ] {
            assert!(validate_scientific_case(manifest, case_id).is_ok());
            assert!(
                validate_scientific_case(
                    &manifest.replace("status = \"verified\"", "status = \"candidate\""),
                    case_id,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn shared_result_closes_two_step_trajectory_and_rejects_foreign_meaning() {
        let document = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let result =
            FixedReferenceFsiResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
                .expect("accepted shared application result");
        assert_eq!(result.mesh().vertices().len(), 9);
        assert_eq!(result.solutions().len(), 2);
        assert_eq!(
            [result.states()[0].step(), result.states()[1].step()],
            [1, 2]
        );
        assert_eq!(
            result.run().outputs(),
            vec![result.trajectory().digest().unwrap()]
        );

        let foreign_source = REFERENCE_SOURCE.replacen(
            "parameter fluid_density: kg / m ^ 3 = 2;",
            "parameter fluid_density: kg / m ^ 3 = 4;",
            1,
        );
        let foreign = ModelDocument::compile("foreign-fsi.eqi", &foreign_source)
            .expect("shape-compatible foreign source");
        let error = FixedReferenceFsiResult2d::solve_reference(&foreign, &REFERENCE_LINEAR_SOLVER)
            .expect_err("foreign physical meaning must fail before realization");
        assert_eq!(error.code(), codes::INVALID_REALIZATION);
    }

    #[test]
    fn fixed_mesh_replay_exposes_only_revalidated_snapshot_supports() {
        let document = ModelDocument::compile("fixed-reference-fsi.eqi", REFERENCE_SOURCE)
            .expect("accepted reference source");
        let result =
            FixedReferenceFsiResult2d::solve_reference(&document, &REFERENCE_LINEAR_SOLVER)
                .expect("accepted shared application result");
        let replay = result
            .trajectory_replay()
            .expect("complete trajectory replay");
        let context = ValidatedFixedSpatialContextV1::new(
            result.model(),
            result.realization(),
            result.geometry(),
            result.correspondence(),
            result.mesh_artifact(),
        )
        .expect("exact fixed-spatial context");
        let fields = replay
            .fields(0)
            .expect("first state Field inventory")
            .collect::<Vec<_>>();

        for (field_index, snapshot) in fields.iter().enumerate() {
            for (association, _) in snapshot.block_artifacts() {
                let blocks = replay
                    .blocks(0, field_index)
                    .expect("exact Field block inventory");
                let expected = snapshot
                    .active_entities_against(&context, blocks, association)
                    .expect("revalidated snapshot support");
                assert_eq!(
                    replay.support_indices(0, field_index, association),
                    Some(expected.as_slice())
                );
                assert_eq!(
                    replay.support_indices(1, field_index, association),
                    Some(expected.as_slice())
                );
            }
        }

        let vertex_only = fields
            .iter()
            .position(|snapshot| {
                snapshot
                    .block_artifacts()
                    .iter()
                    .all(|(association, _)| *association != DiscreteFieldAssociation::Cell)
            })
            .expect("accepted trajectory contains a vertex-only Field");
        assert_eq!(
            replay.support_indices(0, vertex_only, DiscreteFieldAssociation::Cell),
            None
        );
        assert_eq!(
            replay.support_indices(replay.states().len(), 0, DiscreteFieldAssociation::Vertex),
            None
        );
        assert_eq!(
            replay.support_indices(0, fields.len(), DiscreteFieldAssociation::Vertex),
            None
        );
    }
}
