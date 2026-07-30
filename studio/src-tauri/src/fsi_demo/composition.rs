//! Exact public-facade composition behind the bounded FSI presentation.

use std::num::NonZeroUsize;

use eqiora::api::{ModelDocument, snapshot_fixed_reference_fsi_solution_v1};
use eqiora::artifact::{
    ExecutionProvenanceV1, ExecutionTopologyV1, GeometryIdentityEnvelopeV1,
    GeometryMeshCorrespondenceEnvelopeV1, LayoutArtifacts, ModelEnvelopeV4, RealizationEnvelopeV3,
    RunManifestV2, SimplicialMeshEnvelopeV1, SpatialStateEnvelopeV1, SpatialTrajectoryEnvelopeV1,
    SpatialTrajectorySegmentEnvelopeV1, ValidatedFixedSpatialContextV1,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::meshing::{CellId, FacetId, MeshQualityGate, SimplicialMesh};
use eqiora::numerics::{
    FixedReferenceFsiCartesianModel2d, FixedReferenceFsiPartition2d,
    FixedReferenceFsiScaleProfile2d, FixedReferenceFsiState2d, ResolvedFixedReferenceFsiSolution2d,
    finalize_resolved_fixed_reference_fsi_step_2d, fixed_reference_fsi_plan_2d,
    fixed_reference_fsi_requirements_2d, lower_fixed_reference_fsi_cartesian_2d,
};
use eqiora::realization::{
    CoupledFieldwiseRealizationRequest, MeshArtifactReference, RealizationCapabilities,
    RealizationRevision, ResolvedCoupledFieldwiseRealization, SemanticRevision,
    resolve_coupled_fieldwise,
};
use eqiora::solver::{
    LinearSolver, LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
    ReductionPolicy, SERIAL_EXECUTION_PROVIDER, SolverPlan,
};
use eqiora::{DimExponents, DynQuantity};

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

pub(super) struct SpatialContext {
    pub(super) model: ModelEnvelopeV4,
    pub(super) mesh: SimplicialMesh,
    pub(super) mesh_artifact: SimplicialMeshEnvelopeV1,
    pub(super) geometry: GeometryIdentityEnvelopeV1,
    pub(super) correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    pub(super) partition: FixedReferenceFsiPartition2d,
}

struct ExecutionContext {
    mesh_reference: MeshArtifactReference,
    resolved: ResolvedCoupledFieldwiseRealization,
    realization: RealizationEnvelopeV3,
    run: RunManifestV2,
}

pub(super) struct AcceptedComposition {
    pub(super) document: ModelDocument,
    pub(super) spatial: SpatialContext,
    pub(super) realization: RealizationEnvelopeV3,
    pub(super) first: ResolvedFixedReferenceFsiSolution2d,
    pub(super) second: ResolvedFixedReferenceFsiSolution2d,
    pub(super) first_state: SpatialStateEnvelopeV1,
    pub(super) second_state: SpatialStateEnvelopeV1,
    pub(super) trajectory: SpatialTrajectoryEnvelopeV1,
    pub(super) run: RunManifestV2,
}

pub(super) fn compose(model_source: &str) -> Result<AcceptedComposition, String> {
    let document = ExactModelCodec::V4
        .compile("fixed-reference-fsi.eqi", model_source)
        .map_err(diagnostics)?;
    let canonical = lower_fixed_reference_fsi_cartesian_2d(document.program()).map_err(error)?;
    let spatial = spatial_context(document.program(), &canonical)?;
    let execution = execution_context(document.program(), &canonical, &spatial)?;
    let first = solve_step(
        &canonical,
        &spatial,
        &execution,
        &prestrained_state(&spatial)?,
    )?;
    let second = solve_step(
        &canonical,
        &spatial,
        &execution,
        &state_from_solution(&spatial, &first)?,
    )?;
    let fixed = ValidatedFixedSpatialContextV1::new(
        &spatial.model,
        &execution.realization,
        &spatial.geometry,
        &spatial.correspondence,
        &spatial.mesh_artifact,
    )
    .map_err(error)?;
    let first_snapshots =
        snapshot_fixed_reference_fsi_solution_v1(&fixed, &first).map_err(error)?;
    let second_snapshots =
        snapshot_fixed_reference_fsi_solution_v1(&fixed, &second).map_err(error)?;
    let dt = execution
        .realization
        .plan()
        .map_err(error)?
        .time_step()
        .duration()
        .value();
    let first_state =
        SpatialStateEnvelopeV1::new(&fixed, 1, dt, first_snapshots.snapshots()).map_err(error)?;
    let second_state =
        SpatialStateEnvelopeV1::new(&fixed, 2, 2.0 * dt, second_snapshots.snapshots())
            .map_err(error)?;
    let first_segment =
        SpatialTrajectorySegmentEnvelopeV1::new(&fixed, std::slice::from_ref(&first_state))
            .map_err(error)?;
    let second_segment =
        SpatialTrajectorySegmentEnvelopeV1::new(&fixed, std::slice::from_ref(&second_state))
            .map_err(error)?;
    let first_root = SpatialTrajectoryEnvelopeV1::start(&fixed, &first_segment).map_err(error)?;
    let trajectory =
        SpatialTrajectoryEnvelopeV1::extend(&fixed, &first_root, &second_segment).map_err(error)?;
    let run = execution
        .run
        .clone()
        .with_output(trajectory.digest().map_err(error)?);
    run.validate_against(&execution.realization)
        .map_err(error)?;

    Ok(AcceptedComposition {
        document,
        spatial,
        realization: execution.realization,
        first,
        second,
        first_state,
        second_state,
        trajectory,
        run,
    })
}

fn spatial_context(
    program: &eqiora::sem::KernelProgram,
    canonical: &FixedReferenceFsiCartesianModel2d,
) -> Result<SpatialContext, String> {
    let model = ModelEnvelopeV4::from_program(program).map_err(error)?;
    let mesh = physical_mesh()?;
    let mesh_artifact = SimplicialMeshEnvelopeV1::from_mesh(&mesh).map_err(error)?;
    let fluid = canonical
        .fluid()
        .domain()
        .downcast::<eqiora::kinds::Domain>()
        .ok_or_else(|| "fixed-reference fluid Domain identity changed kind".to_owned())?;
    let solid = canonical
        .solid()
        .domain()
        .downcast::<eqiora::kinds::Domain>()
        .ok_or_else(|| "fixed-reference solid Domain identity changed kind".to_owned())?;
    let geometry =
        GeometryIdentityEnvelopeV1::new(&model, [solid, fluid], 1.0e-12).map_err(error)?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh_artifact)
            .map_err(error)?;
    let connection = canonical
        .interface()
        .connection()
        .downcast::<eqiora::kinds::Connection>()
        .ok_or_else(|| "fixed-reference interface Connection identity changed kind".to_owned())?;
    let interface = correspondence
        .derive_conserving_interface(&geometry, &model, &mesh_artifact, connection)
        .map_err(error)?;
    let fluid_cells = correspondence
        .body_cells(fluid)
        .ok_or_else(|| "geometry correspondence omitted the fluid body cells".to_owned())?
        .into_iter()
        .map(CellId::new)
        .collect();
    let solid_cells = correspondence
        .body_cells(solid)
        .ok_or_else(|| "geometry correspondence omitted the solid body cells".to_owned())?
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
        FixedReferenceFsiPartition2d::new(&mesh, fluid_cells, solid_cells, interface_facets)
            .map_err(error)?;
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
    program: &eqiora::sem::KernelProgram,
    canonical: &FixedReferenceFsiCartesianModel2d,
    spatial: &SpatialContext,
) -> Result<ExecutionContext, String> {
    let mesh_reference = MeshArtifactReference::from_sha256(
        spatial
            .mesh_artifact
            .digest()
            .map_err(error)?
            .sha256_bytes(),
    );
    let plan = fixed_reference_fsi_plan_2d(
        canonical,
        mesh_reference,
        DynQuantity::new(0.05, TIME),
        FixedReferenceFsiScaleProfile2d::new(
            DynQuantity::new(2.0, LENGTH),
            DynQuantity::new(0.5, VELOCITY),
            DynQuantity::new(4.0, PRESSURE),
        )
        .map_err(error)?,
        reference_solver()?,
    )
    .map_err(error)?;
    let resolved = resolve_coupled_fieldwise(
        &CoupledFieldwiseRealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(canonical.semantic_revision()),
            RealizationRevision::new(1),
            plan,
        ),
        fixed_reference_fsi_requirements_2d(canonical),
        &RealizationCapabilities::symmetric_mixed_simplicial_2d_reference(),
    )
    .map_err(error)?;
    let realization = RealizationEnvelopeV3::from_resolved(
        &spatial.model,
        &resolved,
        LayoutArtifacts::Replicated,
    )
    .map_err(error)?;
    realization
        .validate_model_artifact(&spatial.model)
        .map_err(error)?;
    realization
        .validate_mesh_artifact(&spatial.mesh_artifact)
        .map_err(error)?;
    let run = RunManifestV2::new(&realization, execution_provenance()?).map_err(error)?;
    run.validate_against(&realization).map_err(error)?;
    Ok(ExecutionContext {
        mesh_reference,
        resolved,
        realization,
        run,
    })
}

fn prestrained_state(spatial: &SpatialContext) -> Result<FixedReferenceFsiState2d, String> {
    let mut displacement = vec![[0.0; 2]; spatial.mesh.vertices().len()];
    let interface_midpoint = spatial
        .mesh
        .vertices()
        .iter()
        .position(|point| point.as_slice() == [1.0, 0.5])
        .ok_or_else(|| "fixed FSI mesh omitted the free interface midpoint".to_owned())?;
    displacement[interface_midpoint] = [0.02, 0.0];
    FixedReferenceFsiState2d::new(
        &spatial.mesh,
        &spatial.partition,
        vec![[0.0; 2]; spatial.mesh.vertices().len()],
        vec![[0.0; 2]; spatial.partition.fluid_cells().len()],
        displacement,
    )
    .map_err(error)
}

fn state_from_solution(
    spatial: &SpatialContext,
    solution: &ResolvedFixedReferenceFsiSolution2d,
) -> Result<FixedReferenceFsiState2d, String> {
    FixedReferenceFsiState2d::new(
        &spatial.mesh,
        &spatial.partition,
        solution.vertex_velocity_coefficients().to_vec(),
        solution.fluid_velocity_bubble_coefficients().to_vec(),
        solution.solid_displacement_coefficients().to_vec(),
    )
    .map_err(error)
}

fn solve_step(
    canonical: &FixedReferenceFsiCartesianModel2d,
    spatial: &SpatialContext,
    execution: &ExecutionContext,
    previous: &FixedReferenceFsiState2d,
) -> Result<ResolvedFixedReferenceFsiSolution2d, String> {
    finalize_resolved_fixed_reference_fsi_step_2d(
        canonical,
        &execution.resolved,
        execution.mesh_reference,
        &spatial.mesh,
        &spatial.partition,
        previous,
    )
    .map_err(error)?
    .solve(&REFERENCE_LINEAR_SOLVER)
    .map_err(error)
}

fn reference_solver() -> Result<SolverPlan, String> {
    SolverPlan::new(
        LinearSolver::MinimumResidual,
        1.0e-11,
        1.0e-13,
        NonZeroUsize::new(20_000).expect("positive frozen iteration limit"),
    )
    .map_err(error)
    .map(|plan| {
        plan.with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Reproducible)
    })
}

fn execution_provenance() -> Result<ExecutionProvenanceV1, String> {
    ExecutionProvenanceV1::from_provider_releases(
        REFERENCE_LINEAR_SOLVER.provider(),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
        std::iter::empty::<(&str, &str)>(),
    )
    .map_err(error)
}

fn physical_mesh() -> Result<SimplicialMesh, String> {
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
        MeshQualityGate::new(0.05).map_err(error)?,
    )
    .map_err(error)
}

fn diagnostics(diagnostics: Vec<eqiora::Diagnostic>) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn error(error: eqiora::Diagnostic) -> String {
    error.to_string()
}
