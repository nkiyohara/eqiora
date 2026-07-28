//! Least-privilege native adapter for Eqiora Studio.
//!
//! This crate contains transport projection only. Canonical compilation,
//! transaction replay, graph commit, artifact reconstruction, and reference
//! execution remain in the public `eqiora` facade.

mod cad;
mod compile;
mod lifecycle;
mod scalar_field;

use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;

use eqiora::api::{
    MAX_SCALAR_ELLIPTIC_ENTITY_COUNT, ModelDocument, ReferenceAcceptance,
    ReferenceExecutionPlacement, ReferenceIntegrationMethod, ReferenceNonlinearMethod,
    ReferenceRunCancellation, ReferenceRunEvidence, ReferenceRunOutcome, ReferenceRunPlan,
    ReferenceRunProgress, ReferenceRunResult, ScalarEllipticExecutionEnvironment,
    ScalarEllipticIntent, ScalarEllipticMethod, ScalarEllipticRunPlan, ScalarEllipticRunResult,
    ScalarFieldLocation, ValueEditPlan,
};
use eqiora::graph::EdgeKind;
use eqiora::kernel::{
    ActivationKind, ClockKind, ConnectionSemantics, DomainKind, KernelNode, PortPayload,
    RepresentationKind, SignalDirection,
};
use eqiora::numerics::lower_scalar_elliptic_cartesian;
use eqiora::realization::{
    DiscretizationMethod, QuadraturePolicy, RealizationRevision, SpaceFamily, Target,
};
use eqiora::solver::{
    ConvergenceReason, ExecutionReport, ExecutionTopology, LinearSolver, PreconditionerPolicy,
    ReductionPolicy,
};
use eqiora::{Diagnostic, RawId, Severity};
use serde::{Deserialize, Serialize};
use tauri::{State, ipc::Channel};

use lifecycle::{CancellationStatus, CoalescingObserver, RunRegistry};
use scalar_field::{ScalarFieldCache, open_scalar_field_view, read_scalar_field_chunk};

const PROTOCOL: &str = "eqiora.studio.bridge/v5";
const MAX_DOCUMENTS: usize = 32;
const MAX_REQUESTED_STEPS: f64 = 5_000_000.0;
const MAX_STUDIO_HOST_WORKERS: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunRequest {
    protocol: String,
    digest: String,
    end_time: f64,
    max_step: f64,
    run_id: String,
    plan_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelRunRequest {
    protocol: String,
    run_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunPreviewRequest {
    protocol: String,
    digest: String,
    end_time: f64,
    max_step: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueEditPreviewRequest {
    protocol: String,
    digest: String,
    target_id: String,
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueEditCommitRequest {
    protocol: String,
    digest: String,
    target_id: String,
    value: f64,
    plan_key: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SpatialMethodRequest {
    FiniteElement,
    FiniteVolume,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpatialRealizationPreviewRequest {
    protocol: String,
    digest: String,
    realization_revision: u64,
    method: SpatialMethodRequest,
    cells_per_axis: usize,
    workers: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpatialRealizationRunRequest {
    protocol: String,
    digest: String,
    run_id: String,
    plan_key: String,
    realization_revision: u64,
    method: SpatialMethodRequest,
    cells_per_axis: usize,
    workers: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CadProjectionRequest {
    protocol: String,
    model_digest: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeEnvelope<T> {
    protocol: &'static str,
    result: Option<T>,
    diagnostics: Vec<DiagnosticDto>,
}

impl<T> BridgeEnvelope<T> {
    fn success(result: T) -> Self {
        Self {
            protocol: PROTOCOL,
            result: Some(result),
            diagnostics: Vec::new(),
        }
    }

    fn failure(diagnostics: Vec<DiagnosticDto>) -> Self {
        Self {
            protocol: PROTOCOL,
            result: None,
            diagnostics,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDto {
    source: &'static str,
    severity: &'static str,
    code: String,
    message: String,
    graph_path: Option<String>,
    span: Option<SourceSpanDto>,
}

type ProjectionError = Box<DiagnosticDto>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSpanDto {
    file: String,
    start: u32,
    end: u32,
}

impl From<Diagnostic> for DiagnosticDto {
    fn from(diagnostic: Diagnostic) -> Self {
        let severity = match diagnostic.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        Self {
            source: "kernel",
            severity,
            code: diagnostic.code().to_string(),
            message: diagnostic.message().to_owned(),
            graph_path: diagnostic.graph_path().map(ToString::to_string),
            span: diagnostic.source_span().map(|span| SourceSpanDto {
                file: span.file.clone(),
                start: span.start,
                end: span.end,
            }),
        }
    }
}

fn studio_error(code: &str, message: impl Into<String>) -> DiagnosticDto {
    DiagnosticDto {
        source: "studio",
        severity: "error",
        code: code.to_owned(),
        message: message.into(),
        graph_path: None,
        span: None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentProjection {
    protocol: &'static str,
    digest: String,
    revision: u64,
    model_id: String,
    nodes: Vec<NodeDto>,
    edges: Vec<EdgeDto>,
    workflows: WorkflowProjectionDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowProjectionDto {
    scalar_elliptic: Option<ScalarEllipticWorkflowDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScalarEllipticWorkflowDto {
    spatial_dimension: usize,
    scalar_type: &'static str,
    vector_layout: &'static str,
    maximum_host_workers: usize,
    worker_budget_source: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeDto {
    id: String,
    name: String,
    kind: &'static str,
    summary: String,
    dimension: Option<String>,
    value: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EdgeDto {
    id: String,
    source: String,
    target: String,
    kind: &'static str,
    label: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunResultDto {
    protocol: &'static str,
    digest: String,
    evidence: RunEvidenceDto,
    series: Vec<ResultSeriesDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunProgressDto {
    protocol: &'static str,
    run_id: String,
    model_time: f64,
    end_time: f64,
    accepted_steps: usize,
    maximum_steps: usize,
    elapsed_seconds: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunCancellationDto {
    protocol: &'static str,
    run_id: String,
    plan: RunPlanDto,
    elapsed_seconds: f64,
    progress: RunProgressDto,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum RunOutcomeDto {
    Completed { result: RunResultDto },
    Cancelled { cancellation: RunCancellationDto },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CancelRunResultDto {
    protocol: &'static str,
    run_id: String,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunPlanDto {
    protocol: &'static str,
    key: String,
    adapter: AdapterDto,
    placement: PlacementDto,
    integration: IntegrationDto,
    nonlinear: NonlinearDto,
    events: EventControlsDto,
    limits: RunLimitsDto,
    acceptance: AcceptanceDto,
}

#[derive(Debug, Serialize)]
struct AdapterDto {
    id: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct PlacementDto {
    kind: &'static str,
    workers: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationDto {
    method: &'static str,
    end_time: f64,
    max_step: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NonlinearDto {
    method: &'static str,
    absolute_tolerance: f64,
    relative_tolerance: f64,
    maximum_iterations: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventControlsDto {
    time_tolerance: f64,
    guard_tolerance: f64,
    maximum_localization_iterations: usize,
    maximum_zero_time_events: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunLimitsDto {
    maximum_steps: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptanceDto {
    kind: &'static str,
    independent_verifier: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunEvidenceDto {
    plan: RunPlanDto,
    elapsed_seconds: f64,
    field_count: usize,
    sample_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultSeriesDto {
    field_id: String,
    name: String,
    dimension: String,
    time: Vec<f64>,
    values: Vec<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditPlanDto {
    protocol: &'static str,
    key: String,
    base_digest: String,
    base_revision: u64,
    target_id: String,
    before: QuantityDto,
    after: QuantityDto,
    transaction_digest: String,
}

#[derive(Debug, Serialize)]
struct QuantityDto {
    value: f64,
    dimension: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditEvidenceDto {
    plan: ValueEditPlanDto,
    result_digest: String,
    result_revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditResultDto {
    protocol: &'static str,
    document: DocumentProjection,
    evidence: ValueEditEvidenceDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialRunPlanDto {
    protocol: &'static str,
    key: String,
    model_digest: String,
    realization_revision: u64,
    requirements: SpatialRequirementsDto,
    discretization: SpatialDiscretizationDto,
    solver: SpatialSolverPlanDto,
    placement: SpatialPlacementDto,
    limits: SpatialLimitsDto,
    acceptance: SpatialAcceptanceDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialRequirementsDto {
    spatial_dimension: usize,
    scalar_type: &'static str,
    vector_layout: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialDiscretizationDto {
    method: &'static str,
    space: &'static str,
    order: Option<usize>,
    mesh: &'static str,
    cells_per_axis: usize,
    cell_count: usize,
    quadrature: &'static str,
    points_per_axis: Option<usize>,
    field_value_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialSolverPlanDto {
    adapter: &'static str,
    algorithm: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    relative_tolerance: f64,
    absolute_tolerance: f64,
    maximum_iterations: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialPlacementDto {
    kind: &'static str,
    adapter: &'static str,
    workers: usize,
    maximum_workers: usize,
    budget_source: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialLimitsDto {
    maximum_entity_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialAcceptanceDto {
    algebraic: &'static str,
    continuous: &'static str,
    independent_true_residual: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialRunResultDto {
    protocol: &'static str,
    run_id: String,
    digest: String,
    plan: SpatialRunPlanDto,
    elapsed_seconds: f64,
    field: SpatialFieldSummaryDto,
    balance: SpatialBalanceDto,
    assembly: SpatialAssemblyEvidenceDto,
    solve: SpatialSolveEvidenceDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialFieldSummaryDto {
    location: &'static str,
    value_count: usize,
    minimum: f64,
    maximum: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialBalanceDto {
    boundary_total: f64,
    integrated_source: f64,
    relative_imbalance: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialAssemblyEvidenceDto {
    execution: ExecutionEvidenceDto,
    packet_count: usize,
    target_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialSolveEvidenceDto {
    backend: String,
    execution: ExecutionEvidenceDto,
    verification: ExecutionEvidenceDto,
    algorithm: &'static str,
    preconditioner: &'static str,
    reduction: &'static str,
    reason: &'static str,
    completed_iterations: usize,
    initial_residual_norm: f64,
    reported_residual_norm: f64,
    true_residual_norm: f64,
    residual_target: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionEvidenceDto {
    adapter: String,
    topology: ExecutionTopologyDto,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ExecutionTopologyDto {
    Host { workers: usize },
    Distributed { ranks: usize },
    Cuda { device: u16 },
}

impl From<ReferenceRunPlan> for RunPlanDto {
    fn from(plan: ReferenceRunPlan) -> Self {
        let config = plan.config();
        let integration_method = match plan.integration_method() {
            ReferenceIntegrationMethod::BackwardEuler => "backward-euler",
        };
        let nonlinear_method = match plan.nonlinear_method() {
            ReferenceNonlinearMethod::DenseFiniteDifferenceNewton => {
                "dense-finite-difference-newton"
            }
        };
        let placement = match plan.placement() {
            ReferenceExecutionPlacement::HostSerial => PlacementDto {
                kind: "host",
                workers: 1,
            },
        };
        let acceptance = match plan.acceptance() {
            ReferenceAcceptance::SemanticOracle => AcceptanceDto {
                kind: "semantic-oracle",
                independent_verifier: false,
            },
        };
        Self {
            protocol: PROTOCOL,
            key: plan.key(),
            adapter: AdapterDto {
                id: plan.adapter(),
                version: plan.adapter_version(),
            },
            placement,
            integration: IntegrationDto {
                method: integration_method,
                end_time: config.end_time(),
                max_step: config.max_step(),
            },
            nonlinear: NonlinearDto {
                method: nonlinear_method,
                absolute_tolerance: config.absolute_tolerance(),
                relative_tolerance: config.relative_tolerance(),
                maximum_iterations: config.max_nonlinear_iterations(),
            },
            events: EventControlsDto {
                time_tolerance: config.event_time_tolerance(),
                guard_tolerance: config.event_guard_tolerance(),
                maximum_localization_iterations: config.max_event_localization_iterations(),
                maximum_zero_time_events: config.max_zero_time_events(),
            },
            limits: RunLimitsDto {
                maximum_steps: config.max_steps(),
            },
            acceptance,
        }
    }
}

impl From<ReferenceRunEvidence> for RunEvidenceDto {
    fn from(evidence: ReferenceRunEvidence) -> Self {
        Self {
            plan: evidence.plan().into(),
            elapsed_seconds: evidence.elapsed().as_secs_f64(),
            field_count: evidence.field_count(),
            sample_count: evidence.sample_count(),
        }
    }
}

impl RunProgressDto {
    fn new(run_id: String, progress: ReferenceRunProgress, elapsed: Duration) -> Self {
        Self {
            protocol: PROTOCOL,
            run_id,
            model_time: progress.model_time(),
            end_time: progress.end_time(),
            accepted_steps: progress.accepted_steps(),
            maximum_steps: progress.maximum_steps(),
            elapsed_seconds: elapsed.as_secs_f64(),
        }
    }
}

impl RunCancellationDto {
    fn new(run_id: String, cancellation: ReferenceRunCancellation) -> Self {
        Self {
            protocol: PROTOCOL,
            run_id: run_id.clone(),
            plan: cancellation.plan().into(),
            elapsed_seconds: cancellation.elapsed().as_secs_f64(),
            progress: RunProgressDto::new(run_id, cancellation.progress(), cancellation.elapsed()),
        }
    }
}

fn project_run_result(digest: String, result: ReferenceRunResult) -> RunResultDto {
    let evidence = (*result.evidence()).into();
    RunResultDto {
        protocol: PROTOCOL,
        digest,
        evidence,
        series: result
            .into_series()
            .into_iter()
            .map(|series| {
                let field_id = series.field().to_string();
                let name = series.name().unwrap_or(&field_id).to_owned();
                let dimension = series.dimension().to_string();
                let (time, values) = series.into_buffers();
                ResultSeriesDto {
                    field_id,
                    name,
                    dimension,
                    time,
                    values,
                }
            })
            .collect(),
    }
}

impl From<&ValueEditPlan> for ValueEditPlanDto {
    fn from(plan: &ValueEditPlan) -> Self {
        let before = plan.before();
        let after = plan.after();
        Self {
            protocol: PROTOCOL,
            key: plan.key(),
            base_digest: plan.base_digest().to_owned(),
            base_revision: plan.base_revision().0,
            target_id: plan.target().to_string(),
            before: QuantityDto {
                value: before.value(),
                dimension: before.dim().to_string(),
            },
            after: QuantityDto {
                value: after.value(),
                dimension: after.dim().to_string(),
            },
            transaction_digest: plan.transaction_digest().to_owned(),
        }
    }
}

fn project_spatial_plan(
    plan: &ScalarEllipticRunPlan,
) -> Result<SpatialRunPlanDto, ProjectionError> {
    let intent = plan.intent();
    let realization = plan.realization();
    let requirements = plan.requirements();
    let (method, space, order, quadrature, points_per_axis) = match intent.method() {
        ScalarEllipticMethod::FiniteElement => {
            if realization.discretization().method() != DiscretizationMethod::ContinuousGalerkin
                || realization.space().family()
                    != (SpaceFamily::ContinuousLagrange {
                        order: 1.try_into().expect("one is non-zero"),
                    })
            {
                return Err(Box::new(studio_error(
                    "ST0008",
                    "accepted finite-element projection contradicts its typed Realization",
                )));
            }
            let QuadraturePolicy::GaussLegendre { points_per_axis } =
                realization.discretization().quadrature()
            else {
                return Err(Box::new(studio_error(
                    "ST0008",
                    "accepted finite-element projection has no Gauss-Legendre quadrature",
                )));
            };
            (
                "finite-element",
                "continuous-lagrange",
                Some(1),
                "gauss-legendre",
                Some(points_per_axis.get()),
            )
        }
        ScalarEllipticMethod::FiniteVolume => {
            if realization.discretization().method()
                != DiscretizationMethod::CellCenteredFiniteVolume
                || realization.space().family() != SpaceFamily::CellConstant
                || realization.discretization().quadrature() != QuadraturePolicy::CellCentroid
            {
                return Err(Box::new(studio_error(
                    "ST0008",
                    "accepted finite-volume projection contradicts its typed Realization",
                )));
            }
            (
                "finite-volume",
                "cell-constant",
                None,
                "cell-centroid",
                None,
            )
        }
    };
    let Target::HostCpu { threads } = realization.target() else {
        return Err(Box::new(studio_error(
            "ST0008",
            "the local scalar-elliptic Studio workflow requires host placement",
        )));
    };
    if threads != intent.workers() {
        return Err(Box::new(studio_error(
            "ST0008",
            "accepted host placement differs from its Realization intent",
        )));
    }
    let solver = realization.solver();
    if solver.algorithm() != LinearSolver::ConjugateGradient
        || solver.preconditioner() != PreconditionerPolicy::Identity
        || solver.reduction() != ReductionPolicy::Reproducible
    {
        return Err(Box::new(studio_error(
            "ST0008",
            "the local scalar-elliptic Studio workflow received an unsupported solver policy",
        )));
    }
    Ok(SpatialRunPlanDto {
        protocol: PROTOCOL,
        key: plan.key().to_owned(),
        model_digest: plan.model_digest().to_owned(),
        realization_revision: intent.realization_revision().get(),
        requirements: SpatialRequirementsDto {
            spatial_dimension: requirements.spatial_dimension().get(),
            scalar_type: "f64",
            vector_layout: "replicated",
        },
        discretization: SpatialDiscretizationDto {
            method,
            space,
            order,
            mesh: "generated-cartesian",
            cells_per_axis: intent.cells_per_axis().get(),
            cell_count: plan.cell_count(),
            quadrature,
            points_per_axis,
            field_value_count: plan.field_value_count(),
        },
        solver: SpatialSolverPlanDto {
            adapter: "eqiora.reference",
            algorithm: "conjugate-gradient",
            preconditioner: "identity",
            reduction: "reproducible",
            relative_tolerance: solver.relative_tolerance(),
            absolute_tolerance: solver.absolute_tolerance(),
            maximum_iterations: solver.maximum_iterations().get(),
        },
        placement: SpatialPlacementDto {
            kind: "host",
            adapter: if threads == NonZeroUsize::MIN {
                "eqiora.host.serial"
            } else {
                "eqiora.rayon"
            },
            workers: threads.get(),
            maximum_workers: plan.environment().maximum_workers().get(),
            budget_source: "studio-session-budget",
        },
        limits: SpatialLimitsDto {
            maximum_entity_count: MAX_SCALAR_ELLIPTIC_ENTITY_COUNT,
        },
        acceptance: SpatialAcceptanceDto {
            algebraic: "independent-true-residual",
            continuous: "boundary-source-balance",
            independent_true_residual: true,
        },
    })
}

fn project_execution(report: ExecutionReport) -> ExecutionEvidenceDto {
    let topology = match report.topology() {
        ExecutionTopology::Host { workers } => ExecutionTopologyDto::Host {
            workers: workers.get(),
        },
        ExecutionTopology::Distributed { ranks, .. } => {
            ExecutionTopologyDto::Distributed { ranks: ranks.get() }
        }
        ExecutionTopology::Cuda { device } => ExecutionTopologyDto::Cuda { device },
    };
    ExecutionEvidenceDto {
        adapter: report.adapter().as_str().to_owned(),
        topology,
    }
}

fn project_spatial_result(
    run_id: String,
    digest: String,
    result: &ScalarEllipticRunResult,
) -> Result<SpatialRunResultDto, ProjectionError> {
    let plan = project_spatial_plan(result.plan())?;
    let field = result.field();
    let balance = result.balance();
    let assembly = result.assembly();
    let solve = result.solve();
    Ok(SpatialRunResultDto {
        protocol: PROTOCOL,
        run_id,
        digest,
        plan,
        elapsed_seconds: result.elapsed().as_secs_f64(),
        field: SpatialFieldSummaryDto {
            location: match field.location() {
                ScalarFieldLocation::Vertex => "vertex",
                ScalarFieldLocation::CellCenter => "cell-center",
            },
            value_count: field.value_count(),
            minimum: field.minimum(),
            maximum: field.maximum(),
        },
        balance: SpatialBalanceDto {
            boundary_total: balance.boundary_total(),
            integrated_source: balance.integrated_source(),
            relative_imbalance: balance.relative_imbalance(),
        },
        assembly: SpatialAssemblyEvidenceDto {
            execution: project_execution(assembly.execution()),
            packet_count: assembly.packet_count(),
            target_count: assembly.target_count(),
        },
        solve: SpatialSolveEvidenceDto {
            backend: solve.backend().as_str().to_owned(),
            execution: project_execution(solve.execution()),
            verification: project_execution(solve.verification()),
            algorithm: project_solver_algorithm(solve.algorithm())?,
            preconditioner: match solve.preconditioner() {
                PreconditionerPolicy::Identity => "identity",
                PreconditionerPolicy::Jacobi => "jacobi",
            },
            reduction: match solve.reduction() {
                ReductionPolicy::Reproducible => "reproducible",
                ReductionPolicy::Fast => "fast",
            },
            reason: match solve.reason() {
                ConvergenceReason::InitialResidualSatisfied => "initial-residual-satisfied",
                ConvergenceReason::ResidualToleranceSatisfied => "residual-tolerance-satisfied",
            },
            completed_iterations: solve.completed_iterations(),
            initial_residual_norm: solve.initial_residual_norm(),
            reported_residual_norm: solve.reported_residual_norm(),
            true_residual_norm: solve.true_residual_norm(),
            residual_target: solve.residual_target(),
        },
    })
}

fn project_solver_algorithm(algorithm: LinearSolver) -> Result<&'static str, ProjectionError> {
    match algorithm {
        LinearSolver::ConjugateGradient => Ok("conjugate-gradient"),
        LinearSolver::BiConjugateGradientStabilized => Ok("bicgstab"),
        LinearSolver::MinimumResidual => Err(Box::new(studio_error(
            "ST0008",
            "the local scalar-elliptic Studio workflow received an unsupported minimum-residual solve report",
        ))),
        LinearSolver::SparseLu => Err(Box::new(studio_error(
            "ST0008",
            "the local scalar-elliptic Studio workflow received an unsupported sparse-LU solve report",
        ))),
    }
}

#[derive(Debug, Default)]
struct DocumentCache {
    documents: BTreeMap<String, ModelDocument>,
    active_lineage: VecDeque<String>,
}

impl DocumentCache {
    fn reset(&mut self, digest: String, document: ModelDocument) {
        self.documents.clear();
        self.active_lineage.clear();
        self.active_lineage.push_back(digest.clone());
        self.documents.insert(digest, document);
    }

    fn insert_child(
        &mut self,
        base_digest: &str,
        child_digest: String,
        child: ModelDocument,
    ) -> bool {
        let Some(base_index) = self
            .active_lineage
            .iter()
            .position(|digest| digest == base_digest)
        else {
            return false;
        };
        while self.active_lineage.len() > base_index + 1 {
            if let Some(abandoned) = self.active_lineage.pop_back() {
                self.documents.remove(&abandoned);
            }
        }
        self.active_lineage.push_back(child_digest.clone());
        self.documents.insert(child_digest, child);
        while self.active_lineage.len() > MAX_DOCUMENTS {
            if let Some(oldest) = self.active_lineage.pop_front() {
                self.documents.remove(&oldest);
            }
        }
        true
    }

    fn get(&self, digest: &str) -> Option<ModelDocument> {
        self.documents.get(digest).cloned()
    }

    fn contains(&self, digest: &str) -> bool {
        self.documents.contains_key(digest)
    }
}

#[derive(Debug)]
struct AppState {
    documents: Mutex<DocumentCache>,
    runs: Mutex<RunRegistry>,
    scalar_fields: Mutex<ScalarFieldCache>,
    host_worker_budget: NonZeroUsize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            documents: Mutex::new(DocumentCache::default()),
            runs: Mutex::new(RunRegistry::default()),
            scalar_fields: Mutex::new(ScalarFieldCache::default()),
            host_worker_budget: recommended_host_worker_budget(),
        }
    }
}

#[tauri::command]
fn preview_cad_box(
    request: CadProjectionRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<cad::CadProjectionDto> {
    if request.protocol != cad::CAD_PROTOCOL {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio CAD payload protocol",
        )]);
    }
    let document = match load_document(&state, &request.model_digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match cad::project(&document) {
        Ok(projection) => BridgeEnvelope::success(projection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn select_cad_entity(
    request: cad::CadSelectionRequestDto,
    state: State<'_, AppState>,
) -> BridgeEnvelope<cad::CadSelectionDto> {
    let document = match load_document(&state, &request.model_digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match cad::select(&document, &request) {
        Ok(selection) => BridgeEnvelope::success(selection),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn preview_reference_run(
    request: RunPreviewRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<RunPlanDto> {
    if let Err(diagnostic) = validate_run_controls(
        &request.protocol,
        &request.digest,
        request.end_time,
        request.max_step,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    if let Err(diagnostic) = require_document_loaded(&state, &request.digest) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    match ReferenceRunPlan::new(request.end_time, request.max_step) {
        Ok(plan) => BridgeEnvelope::success(plan.into()),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn preview_spatial_realization(
    request: SpatialRealizationPreviewRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<SpatialRunPlanDto> {
    if let Err(diagnostic) = validate_spatial_controls(
        &request.protocol,
        &request.digest,
        request.cells_per_axis,
        request.workers,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let intent = match spatial_intent(
        request.realization_revision,
        request.method,
        request.cells_per_axis,
        request.workers,
    ) {
        Ok(intent) => intent,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let environment = ScalarEllipticExecutionEnvironment::host_threaded(state.host_worker_budget);
    match document.preview_scalar_elliptic_run(intent, environment) {
        Ok(plan) => match project_spatial_plan(&plan) {
            Ok(plan) => BridgeEnvelope::success(plan),
            Err(diagnostic) => BridgeEnvelope::failure(vec![*diagnostic]),
        },
        Err(diagnostics) => {
            BridgeEnvelope::failure(diagnostics.into_iter().map(Into::into).collect())
        }
    }
}

#[tauri::command]
fn preview_value_edit(
    request: ValueEditPreviewRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<ValueEditPlanDto> {
    if let Err(diagnostic) = validate_value_edit_controls(
        &request.protocol,
        &request.digest,
        &request.target_id,
        request.value,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let target = match resolve_target(&document, &request.target_id) {
        Ok(target) => target,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    match document.preview_value_edit(target, request.value) {
        Ok(plan) => BridgeEnvelope::success((&plan).into()),
        Err(diagnostic) => BridgeEnvelope::failure(vec![diagnostic.into()]),
    }
}

#[tauri::command]
fn commit_value_edit(
    request: ValueEditCommitRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<ValueEditResultDto> {
    if let Err(diagnostic) = validate_value_edit_controls(
        &request.protocol,
        &request.digest,
        &request.target_id,
        request.value,
    ) {
        return BridgeEnvelope::failure(vec![*diagnostic]);
    }
    if request.plan_key.is_empty() || request.plan_key.len() > 256 {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "value-edit plan key must contain 1 to 256 UTF-8 bytes",
        )]);
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let target = match resolve_target(&document, &request.target_id) {
        Ok(target) => target,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let plan = match document.preview_value_edit(target, request.value) {
        Ok(plan) => plan,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![diagnostic.into()]),
    };
    if request.plan_key != plan.key() {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0006",
            "value edit no longer matches the accepted transaction preview; preview it again",
        )]);
    }
    let result = match document.commit_value_edit(plan) {
        Ok(result) => result,
        Err(diagnostics) => {
            return BridgeEnvelope::failure(diagnostics.into_iter().map(Into::into).collect());
        }
    };
    let result_digest = result.result_digest().to_owned();
    let evidence = ValueEditEvidenceDto {
        plan: result.plan().into(),
        result_digest: result_digest.clone(),
        result_revision: result.result_revision().0,
    };
    let projection = match project_document(
        result.document(),
        result_digest.clone(),
        state.host_worker_budget,
    ) {
        Ok(projection) => projection,
        Err(diagnostic) => return BridgeEnvelope::failure(vec![*diagnostic]),
    };
    let child = result.into_document();
    match state.documents.lock() {
        Ok(mut documents) => {
            if !documents.insert_child(&request.digest, result_digest, child) {
                return BridgeEnvelope::failure(vec![studio_error(
                    "ST0004",
                    "value-edit base revision left the active Studio lineage",
                )]);
            }
        }
        Err(_) => {
            return BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native document cache is unavailable",
            )]);
        }
    }
    BridgeEnvelope::success(ValueEditResultDto {
        protocol: PROTOCOL,
        document: projection,
        evidence,
    })
}

#[tauri::command]
async fn run_reference(
    request: RunRequest,
    on_progress: Channel<RunProgressDto>,
    state: State<'_, AppState>,
) -> Result<BridgeEnvelope<RunOutcomeDto>, ()> {
    if let Err(diagnostic) = validate_run_request(&request) {
        return Ok(BridgeEnvelope::failure(vec![*diagnostic]));
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![*diagnostic])),
    };
    let plan = match ReferenceRunPlan::new(request.end_time, request.max_step) {
        Ok(plan) => plan,
        Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![diagnostic.into()])),
    };
    if request.plan_key != plan.key() {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0005",
            "run plan no longer matches the accepted capability preview; preview it again",
        )]));
    }
    let cancellation = match state.runs.lock() {
        Ok(mut runs) => match runs.begin_cancellable(request.run_id.clone()) {
            Ok(cancellation) => cancellation,
            Err(message) => {
                return Ok(BridgeEnvelope::failure(vec![studio_error(
                    "ST0007", message,
                )]));
            }
        },
        Err(_) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native run registry is unavailable",
            )]));
        }
    };
    let digest = request.digest.clone();
    let run_id = request.run_id.clone();
    let progress_run_id = run_id.clone();
    let mut observer = CoalescingObserver::new(cancellation, move |progress, elapsed| {
        let _ = on_progress.send(RunProgressDto::new(
            progress_run_id.clone(),
            progress,
            elapsed,
        ));
    });
    let execution = tauri::async_runtime::spawn_blocking(move || {
        document.run_reference_plan_controlled(plan, &mut observer)
    })
    .await;
    match state.runs.lock() {
        Ok(mut runs) => runs.finish(&request.run_id),
        Err(_) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native run registry is unavailable after execution",
            )]));
        }
    }
    let outcome = match execution {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(diagnostics)) => {
            return Ok(BridgeEnvelope::failure(
                diagnostics.into_iter().map(Into::into).collect(),
            ));
        }
        Err(error) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                format!("native execution worker failed: {error}"),
            )]));
        }
    };
    Ok(BridgeEnvelope::success(match outcome {
        ReferenceRunOutcome::Completed(result) => RunOutcomeDto::Completed {
            result: project_run_result(digest, result),
        },
        ReferenceRunOutcome::Cancelled(cancellation) => RunOutcomeDto::Cancelled {
            cancellation: RunCancellationDto::new(run_id, cancellation),
        },
    }))
}

#[tauri::command]
async fn run_spatial_realization(
    request: SpatialRealizationRunRequest,
    state: State<'_, AppState>,
) -> Result<BridgeEnvelope<SpatialRunResultDto>, ()> {
    if let Err(diagnostic) = validate_spatial_controls(
        &request.protocol,
        &request.digest,
        request.cells_per_axis,
        request.workers,
    ) {
        return Ok(BridgeEnvelope::failure(vec![*diagnostic]));
    }
    if !valid_run_id(&request.run_id) || request.plan_key.len() != 64 {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "spatial run requires a canonical UUID and complete Realization artifact digest",
        )]));
    }
    let document = match load_document(&state, &request.digest) {
        Ok(document) => document,
        Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![*diagnostic])),
    };
    let intent = match spatial_intent(
        request.realization_revision,
        request.method,
        request.cells_per_axis,
        request.workers,
    ) {
        Ok(intent) => intent,
        Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![*diagnostic])),
    };
    let environment = ScalarEllipticExecutionEnvironment::host_threaded(state.host_worker_budget);
    let accepted = match document.preview_scalar_elliptic_run(intent, environment) {
        Ok(plan) => plan,
        Err(diagnostics) => {
            return Ok(BridgeEnvelope::failure(
                diagnostics.into_iter().map(Into::into).collect(),
            ));
        }
    };
    if request.plan_key != accepted.key() {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0005",
            "spatial run no longer matches the accepted Realization artifact; preview it again",
        )]));
    }
    if state
        .scalar_fields
        .lock()
        .is_ok_and(|cache| cache.retains_run_id(&request.run_id))
    {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0007",
            "the field-view cache already retains this run ID",
        )]));
    }
    let pending_field =
        match scalar_field::prepare(&accepted, request.digest.clone(), request.run_id.clone()) {
            Ok(pending) => pending,
            Err(diagnostic) => return Ok(BridgeEnvelope::failure(vec![*diagnostic])),
        };
    match state.runs.lock() {
        Ok(mut runs) => {
            if let Err(message) = runs.begin_non_cancellable(request.run_id.clone()) {
                return Ok(BridgeEnvelope::failure(vec![studio_error(
                    "ST0007", message,
                )]));
            }
        }
        Err(_) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native run registry is unavailable",
            )]));
        }
    }
    let execution = tauri::async_runtime::spawn_blocking(move || {
        document.run_scalar_elliptic_plan(accepted, environment)
    })
    .await;
    let response = match execution {
        Ok(Ok(result)) => {
            match project_spatial_result(request.run_id.clone(), request.digest.clone(), &result) {
                Ok(projected) => {
                    let publication = if let Some(pending) = pending_field {
                        let summary = result.field();
                        let values = result.into_field_values();
                        scalar_field::accept(pending, summary, values).and_then(|field| {
                            state
                                .scalar_fields
                                .lock()
                                .map_err(|_| {
                                    Box::new(studio_error(
                                        "ST0001",
                                        "native scalar Field cache is unavailable after execution",
                                    ))
                                })?
                                .insert(field)
                        })
                    } else {
                        Ok(())
                    };
                    match publication {
                        Ok(()) => BridgeEnvelope::success(projected),
                        Err(diagnostic) => BridgeEnvelope::failure(vec![*diagnostic]),
                    }
                }
                Err(diagnostic) => BridgeEnvelope::failure(vec![*diagnostic]),
            }
        }
        Ok(Err(diagnostics)) => {
            BridgeEnvelope::failure(diagnostics.into_iter().map(Into::into).collect())
        }
        Err(error) => BridgeEnvelope::failure(vec![studio_error(
            "ST0001",
            format!("native spatial execution worker failed: {error}"),
        )]),
    };
    match state.runs.lock() {
        Ok(mut runs) => runs.finish(&request.run_id),
        Err(_) => {
            return Ok(BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native run registry is unavailable after spatial execution",
            )]));
        }
    }
    Ok(response)
}

#[tauri::command]
fn cancel_reference_run(
    request: CancelRunRequest,
    state: State<'_, AppState>,
) -> BridgeEnvelope<CancelRunResultDto> {
    if request.protocol != PROTOCOL || !valid_run_id(&request.run_id) {
        return BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "cancellation request does not satisfy the current Studio bridge protocol",
        )]);
    }
    let status = match state.runs.lock() {
        Ok(runs) => runs.cancel(&request.run_id),
        Err(_) => {
            return BridgeEnvelope::failure(vec![studio_error(
                "ST0001",
                "native run registry is unavailable",
            )]);
        }
    };
    BridgeEnvelope::success(CancelRunResultDto {
        protocol: PROTOCOL,
        run_id: request.run_id,
        status: match status {
            CancellationStatus::Requested => "requested",
            CancellationStatus::AlreadyTerminal => "already-terminal",
            CancellationStatus::NotCancellable => "not-cancellable",
        },
    })
}

fn recommended_host_worker_budget() -> NonZeroUsize {
    let detected = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    NonZeroUsize::new(detected.get().min(MAX_STUDIO_HOST_WORKERS))
        .expect("the detected worker budget is non-zero")
}

fn spatial_intent(
    realization_revision: u64,
    method: SpatialMethodRequest,
    cells_per_axis: usize,
    workers: usize,
) -> Result<ScalarEllipticIntent, ProjectionError> {
    let cells_per_axis = NonZeroUsize::new(cells_per_axis).ok_or_else(|| {
        Box::new(studio_error(
            "ST0002",
            "spatial Realization requires a non-zero cell count on every axis",
        ))
    })?;
    let workers = NonZeroUsize::new(workers).ok_or_else(|| {
        Box::new(studio_error(
            "ST0002",
            "spatial Realization requires a non-zero host worker count",
        ))
    })?;
    Ok(ScalarEllipticIntent::new(
        RealizationRevision::new(realization_revision),
        match method {
            SpatialMethodRequest::FiniteElement => ScalarEllipticMethod::FiniteElement,
            SpatialMethodRequest::FiniteVolume => ScalarEllipticMethod::FiniteVolume,
        },
        cells_per_axis,
        workers,
    ))
}

fn validate_spatial_controls(
    protocol: &str,
    digest: &str,
    cells_per_axis: usize,
    workers: usize,
) -> Result<(), ProjectionError> {
    validate_protocol_and_digest(protocol, digest)?;
    if cells_per_axis == 0 || workers == 0 {
        return Err(Box::new(studio_error(
            "ST0002",
            "spatial cell and host worker counts must be strictly positive integers",
        )));
    }
    Ok(())
}

fn load_document(
    state: &State<'_, AppState>,
    digest: &str,
) -> Result<ModelDocument, ProjectionError> {
    let document = state
        .documents
        .lock()
        .map_err(|_| {
            Box::new(studio_error(
                "ST0001",
                "native document cache is unavailable",
            ))
        })?
        .get(digest);
    document.ok_or_else(|| {
        Box::new(studio_error(
            "ST0004",
            "the requested canonical revision is not loaded; compile it again",
        ))
    })
}

fn require_document_loaded(
    state: &State<'_, AppState>,
    digest: &str,
) -> Result<(), ProjectionError> {
    let contains = state
        .documents
        .lock()
        .map_err(|_| {
            Box::new(studio_error(
                "ST0001",
                "native document cache is unavailable",
            ))
        })?
        .contains(digest);
    if contains {
        Ok(())
    } else {
        Err(Box::new(studio_error(
            "ST0004",
            "the requested canonical revision is not loaded; compile it again",
        )))
    }
}

fn validate_run_controls(
    protocol: &str,
    digest: &str,
    end_time: f64,
    max_step: f64,
) -> Result<(), ProjectionError> {
    validate_protocol_and_digest(protocol, digest)?;
    if !end_time.is_finite() || end_time <= 0.0 || !max_step.is_finite() || max_step <= 0.0 {
        return Err(Box::new(studio_error(
            "ST0002",
            "end time and maximum step must be finite and strictly positive",
        )));
    }
    if (end_time / max_step).ceil() > MAX_REQUESTED_STEPS {
        return Err(Box::new(studio_error(
            "ST0002",
            "run request exceeds the 5,000,000-step Studio bridge limit",
        )));
    }
    Ok(())
}

fn validate_value_edit_controls(
    protocol: &str,
    digest: &str,
    target_id: &str,
    value: f64,
) -> Result<(), ProjectionError> {
    validate_protocol_and_digest(protocol, digest)?;
    if target_id.is_empty() || target_id.len() > 128 {
        return Err(Box::new(studio_error(
            "ST0002",
            "value-edit target ID must contain 1 to 128 UTF-8 bytes",
        )));
    }
    if !value.is_finite() {
        return Err(Box::new(studio_error(
            "ST0002",
            "value edit requires one finite coherent-SI scalar",
        )));
    }
    Ok(())
}

fn validate_protocol_and_digest(protocol: &str, digest: &str) -> Result<(), ProjectionError> {
    if protocol != PROTOCOL {
        return Err(Box::new(studio_error(
            "ST0002",
            "unsupported Studio bridge protocol",
        )));
    }
    if digest.len() < 16 || digest.len() > 128 {
        return Err(Box::new(studio_error(
            "ST0002",
            "model digest must contain 16 to 128 UTF-8 bytes",
        )));
    }
    Ok(())
}

fn resolve_target(document: &ModelDocument, target_id: &str) -> Result<RawId, ProjectionError> {
    document
        .program()
        .nodes()
        .map(KernelNode::id)
        .find(|id| id.to_string() == target_id)
        .ok_or_else(|| {
            Box::new(studio_error(
                "ST0004",
                "value-edit target is outside the requested canonical revision",
            ))
        })
}

fn validate_run_request(request: &RunRequest) -> Result<(), ProjectionError> {
    validate_run_controls(
        &request.protocol,
        &request.digest,
        request.end_time,
        request.max_step,
    )?;
    if !valid_run_id(&request.run_id) {
        return Err(Box::new(studio_error(
            "ST0002",
            "run ID must be a canonical UUID",
        )));
    }
    if request.plan_key.is_empty() || request.plan_key.len() > 256 {
        return Err(Box::new(studio_error(
            "ST0002",
            "run plan key must contain 1 to 256 UTF-8 bytes",
        )));
    }
    Ok(())
}

fn valid_run_id(value: &str) -> bool {
    value.len() == 36
        && value.as_bytes()[14] == b'4'
        && matches!(value.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
            }
        })
}

fn project_document(
    document: &ModelDocument,
    digest: String,
    host_worker_budget: NonZeroUsize,
) -> Result<DocumentProjection, ProjectionError> {
    let mut preferred_names = BTreeMap::<RawId, String>::new();
    for (name, &id) in document.aliases() {
        preferred_names.entry(id).or_insert_with(|| name.clone());
    }
    let nodes = document
        .program()
        .nodes()
        .map(|node| project_node(document, node, &preferred_names))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = document
        .program()
        .edges()
        .iter()
        .map(|edge| {
            let (kind, label) = edge_contract(edge.kind())?;
            let source = edge.from().to_string();
            let target = edge.to().to_string();
            Ok(EdgeDto {
                id: format!("{source}→{target}:{kind}"),
                source,
                target,
                kind,
                label,
            })
        })
        .collect::<Result<Vec<_>, ProjectionError>>()?;
    let scalar_elliptic = lower_scalar_elliptic_cartesian(document.program())
        .ok()
        .map(|model| ScalarEllipticWorkflowDto {
            spatial_dimension: model.dimension(),
            scalar_type: "f64",
            vector_layout: "replicated",
            maximum_host_workers: host_worker_budget.get(),
            worker_budget_source: "studio-session-budget",
        });
    Ok(DocumentProjection {
        protocol: PROTOCOL,
        digest,
        revision: document.program().revision().0,
        model_id: document.program().model().erase().to_string(),
        nodes,
        edges,
        workflows: WorkflowProjectionDto { scalar_elliptic },
    })
}

fn project_node(
    document: &ModelDocument,
    node: &KernelNode,
    names: &BTreeMap<RawId, String>,
) -> Result<NodeDto, ProjectionError> {
    let id = node.id();
    let name = names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("{} {}", kind_title(node), id.ulid()));
    let (kind, summary, dimension, value) = match node {
        KernelNode::Domain(definition) => (
            "domain",
            match definition.kind() {
                DomainKind::Abstract => "Abstract continuous domain".to_owned(),
                DomainKind::CartesianBox { bounds } => {
                    format!("{}D Cartesian continuous domain", bounds.len())
                }
                DomainKind::CartesianBoundary { axis, side } => {
                    format!("Cartesian boundary · axis {axis} · {side:?}")
                }
                DomainKind::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                } => format!(
                    "Scalar physical domain · across {across_dimension} · through {through_dimension}"
                ),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Representation(definition) => (
            "representation",
            match definition.kind() {
                RepresentationKind::Abstract => "Abstract field representation".to_owned(),
                RepresentationKind::Continuum => "Continuous field representation".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Field(definition) => (
            "field",
            if definition.initial().is_some() {
                "Scalar field with an initial value".to_owned()
            } else {
                "Scalar field requiring execution input".to_owned()
            },
            Some(definition.dimension().to_string()),
            document
                .program()
                .value(id)
                .map(|quantity| quantity.value()),
        ),
        KernelNode::Parameter(definition) => (
            "parameter",
            "Canonical model parameter".to_owned(),
            Some(definition.value().dim().to_string()),
            document.program().value(id).map_or_else(
                || Some(definition.value().value()),
                |quantity| Some(quantity.value()),
            ),
        ),
        KernelNode::Port(definition) => {
            let (summary, dimension) = match definition.payload() {
                PortPayload::Signal {
                    direction: SignalDirection::Input,
                    dimension,
                } => (
                    "Causal signal input".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::Signal {
                    direction: SignalDirection::Output,
                    dimension,
                } => (
                    "Causal signal output".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::ConservingMarker { dimension } => (
                    "Structural conserving marker".to_owned(),
                    Some(dimension.to_string()),
                ),
                PortPayload::ScalarPhysical { domain } => {
                    let domain = domain.erase();
                    let domain_name = names
                        .get(&domain)
                        .cloned()
                        .unwrap_or_else(|| format!("domain {}", domain.ulid()));
                    (
                        format!("Scalar physical conserving port · {domain_name}"),
                        None,
                    )
                }
                _ => return Err(unsupported_node_contract()),
            };
            ("port", summary, dimension, None)
        }
        KernelNode::Relation(definition) => (
            "relation",
            format!(
                "{} implicit residual{} · {} expression operations",
                definition.residuals().roots().len(),
                if definition.residuals().roots().len() == 1 {
                    ""
                } else {
                    "s"
                },
                definition.residuals().nodes().len()
            ),
            None,
            None,
        ),
        KernelNode::Activation(definition) => (
            "activation",
            match definition.kind() {
                ActivationKind::Continuous => "Continuous activation".to_owned(),
                ActivationKind::Periodic => "Periodic activation".to_owned(),
                ActivationKind::Event { direction, .. } => {
                    format!("Zero-crossing event · {direction:?}")
                }
                ActivationKind::Guard { .. } => "Guarded activation".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::Connection(definition) => (
            "connection",
            match definition.semantics() {
                ConnectionSemantics::Signal => "Causal signal connection".to_owned(),
                ConnectionSemantics::Conserving => "Acausal conserving connection".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        KernelNode::ClockDomain(definition) => (
            "clock-domain",
            match definition.kind() {
                ClockKind::Continuous => "Continuous model time".to_owned(),
                ClockKind::Periodic { period, phase } => format!(
                    "Periodic model time · {}/{} s · phase {}/{} s",
                    period.numerator(),
                    period.denominator(),
                    phase.numerator(),
                    phase.denominator()
                ),
                ClockKind::Aperiodic => "Aperiodic semantic clock".to_owned(),
                ClockKind::Inherited => "Inherited semantic clock".to_owned(),
                _ => return Err(unsupported_node_contract()),
            },
            None,
            None,
        ),
        _ => {
            return Err(unsupported_node_contract());
        }
    };
    Ok(NodeDto {
        id: id.to_string(),
        name,
        kind,
        summary,
        dimension,
        value,
    })
}

fn unsupported_node_contract() -> ProjectionError {
    Box::new(studio_error(
        "ST0003",
        "the native adapter does not support a new Semantic Kernel node contract",
    ))
}

fn kind_title(node: &KernelNode) -> &'static str {
    match node {
        KernelNode::Domain(_) => "Domain",
        KernelNode::Representation(_) => "Representation",
        KernelNode::Field(_) => "Field",
        KernelNode::Parameter(_) => "Parameter",
        KernelNode::Port(_) => "Port",
        KernelNode::Relation(_) => "Relation",
        KernelNode::Activation(_) => "Activation",
        KernelNode::Connection(_) => "Connection",
        KernelNode::ClockDomain(_) => "Clock domain",
        _ => "Entity",
    }
}

fn edge_contract(kind: EdgeKind) -> Result<(&'static str, &'static str), ProjectionError> {
    match kind {
        EdgeKind::DefinedOn => Ok(("defined-on", "defined on")),
        EdgeKind::AppliesOn => Ok(("applies-on", "applies on")),
        EdgeKind::BoundaryOf => Ok(("boundary-of", "boundary of")),
        EdgeKind::DependsOn => Ok(("depends-on", "depends on")),
        EdgeKind::HasPort => Ok(("has-port", "has port")),
        EdgeKind::Activates => Ok(("activates", "activates")),
        EdgeKind::Connects => Ok(("connects", "connects")),
        EdgeKind::ClockedBy => Ok(("clocked-by", "clocked by")),
        _ => Err(Box::new(studio_error(
            "ST0003",
            "the native adapter received an unsupported model edge kind",
        ))),
    }
}

/// Launch the native Studio shell.
pub fn run() {
    use compile::compile_model;

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            compile_model,
            preview_cad_box,
            select_cad_entity,
            preview_value_edit,
            commit_value_edit,
            preview_reference_run,
            preview_spatial_realization,
            run_reference,
            run_spatial_realization,
            open_scalar_field_view,
            read_scalar_field_chunk,
            cancel_reference_run
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch Eqiora Studio");
}

#[cfg(test)]
mod tests {
    use super::{
        DocumentCache, MAX_DOCUMENTS, ModelDocument, ReferenceRunPlan, RunPlanDto,
        ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
        ValueEditPlanDto, project_document, project_node, project_solver_algorithm,
        project_spatial_plan, project_spatial_result, valid_run_id,
    };
    use eqiora::entity::kinds;
    use eqiora::kernel::{DomainDef, KernelNode, PortDef};
    use eqiora::realization::RealizationRevision;
    use eqiora::solver::LinearSolver;
    use eqiora::{DimExponents, Id};
    use std::collections::BTreeMap;
    use std::num::NonZeroUsize;

    const SOURCE: &str = r#"
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"#;

    const POISSON_2D: &str =
        include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");

    #[test]
    fn projection_is_deterministic_and_semantically_read_only() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let digest = document.digest().unwrap();
        let projection = project_document(&document, digest.clone(), NonZeroUsize::MIN).unwrap();
        assert_eq!(projection.digest, digest);
        assert_eq!(projection.nodes.len(), 4);
        assert_eq!(projection.edges.len(), 3);
        assert!(projection.nodes.iter().any(|node| node.name == "x"));
        assert_eq!(document.digest().unwrap(), projection.digest);
    }

    #[test]
    fn projection_preserves_nominal_scalar_physical_port_meaning() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let domain = Id::<kinds::Domain>::new();
        let port = Id::<kinds::Port>::new();
        let across_dimension = DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -1,
            ..DimExponents::DIMENSIONLESS
        };
        let through_dimension = DimExponents {
            current: 1,
            ..DimExponents::DIMENSIONLESS
        };
        let domain_node = KernelNode::from(DomainDef::scalar_physical(
            domain,
            across_dimension,
            through_dimension,
        ));
        let port_node = KernelNode::from(PortDef::scalar_physical(port, domain));
        let names = BTreeMap::from([
            (domain.erase(), "electrical".to_owned()),
            (port.erase(), "positive".to_owned()),
        ]);

        let domain_dto = project_node(&document, &domain_node, &names).unwrap();
        assert!(domain_dto.summary.contains(&across_dimension.to_string()));
        assert!(domain_dto.summary.contains(&through_dimension.to_string()));
        let port_dto = project_node(&document, &port_node, &names).unwrap();
        assert_eq!(port_dto.name, "positive");
        assert_eq!(
            port_dto.summary,
            "Scalar physical conserving port · electrical"
        );
        assert_eq!(port_dto.dimension, None);
    }

    #[test]
    fn document_cache_is_a_bounded_active_lineage() {
        let root = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let mut cache = DocumentCache::default();
        cache.reset("root".to_owned(), root.clone());

        let mut base = "root".to_owned();
        for index in 0..MAX_DOCUMENTS + 4 {
            let child = format!("child-{index}");
            assert!(cache.insert_child(&base, child.clone(), root.clone()));
            base = child;
        }
        assert_eq!(cache.documents.len(), MAX_DOCUMENTS);
        assert!(!cache.contains("root"));
        assert!(cache.contains(&base));

        assert!(cache.insert_child("child-4", "branch".to_owned(), root));
        assert!(cache.contains("child-4"));
        assert!(cache.contains("branch"));
        assert!(!cache.contains(&base));
        assert_eq!(
            cache.active_lineage.back().map(String::as_str),
            Some("branch")
        );
    }

    #[test]
    fn run_plan_projection_retains_the_complete_native_contract() {
        let plan = ReferenceRunPlan::new(4.0, 0.1).unwrap();
        let dto = RunPlanDto::from(plan);

        assert_eq!(dto.key, plan.key());
        assert_eq!(dto.adapter.id, "eqiora.reference");
        assert_eq!(dto.placement.kind, "host");
        assert_eq!(dto.placement.workers, 1);
        assert_eq!(dto.integration.method, "backward-euler");
        assert_eq!(dto.nonlinear.method, "dense-finite-difference-newton");
        assert_eq!(dto.nonlinear.absolute_tolerance, 1.0e-10);
        assert_eq!(dto.events.maximum_localization_iterations, 80);
        assert_eq!(dto.limits.maximum_steps, 1_000_000);
        assert_eq!(dto.acceptance.kind, "semantic-oracle");
        assert!(!dto.acceptance.independent_verifier);
    }

    #[test]
    fn run_ids_are_canonical_uuid_v4_values() {
        assert!(valid_run_id("f8c0c89e-64a8-4f20-b623-d167499bb97d"));
        assert!(!valid_run_id("F8C0C89E-64A8-4F20-B623-D167499BB97D"));
        assert!(!valid_run_id("f8c0c89e-64a8-1f20-b623-d167499bb97d"));
        assert!(!valid_run_id("f8c0c89e-64a8-4f20-7623-d167499bb97d"));
        assert!(!valid_run_id("f8c0c89e64a84f20b623d167499bb97d"));
    }

    #[test]
    fn spatial_projection_rejects_unadmitted_linear_algorithms() {
        assert_eq!(
            project_solver_algorithm(LinearSolver::ConjugateGradient).unwrap(),
            "conjugate-gradient"
        );
        for algorithm in [LinearSolver::MinimumResidual, LinearSolver::SparseLu] {
            let error = project_solver_algorithm(algorithm).unwrap_err();
            assert_eq!(error.code, "ST0008");
        }
    }

    #[test]
    fn spatial_projection_retains_requirements_realization_and_execution_evidence() {
        let document = ModelDocument::compile("poisson.eqi", POISSON_2D).unwrap();
        let digest = document.digest().unwrap();
        let workers = NonZeroUsize::new(2).unwrap();
        let projection = project_document(&document, digest.clone(), workers).unwrap();
        let workflow = projection.workflows.scalar_elliptic.unwrap();
        assert_eq!(workflow.spatial_dimension, 2);
        assert_eq!(workflow.scalar_type, "f64");
        assert_eq!(workflow.vector_layout, "replicated");
        assert_eq!(workflow.maximum_host_workers, 2);

        let environment = ScalarEllipticExecutionEnvironment::host_threaded(workers);
        let accepted = document
            .preview_scalar_elliptic_run(
                ScalarEllipticIntent::new(
                    RealizationRevision::new(3),
                    ScalarEllipticMethod::FiniteVolume,
                    NonZeroUsize::new(8).unwrap(),
                    workers,
                ),
                environment,
            )
            .unwrap();
        let plan = project_spatial_plan(&accepted).unwrap();
        assert_eq!(plan.key.len(), 64);
        assert_eq!(plan.model_digest, digest);
        assert_eq!(plan.requirements.spatial_dimension, 2);
        assert_eq!(plan.discretization.method, "finite-volume");
        assert_eq!(plan.discretization.space, "cell-constant");
        assert_eq!(plan.discretization.cell_count, 64);
        assert_eq!(plan.placement.adapter, "eqiora.rayon");
        assert_eq!(plan.placement.workers, 2);
        assert!(plan.acceptance.independent_true_residual);

        let result = document
            .run_scalar_elliptic_plan(accepted, environment)
            .unwrap();
        let result = project_spatial_result(
            "f8c0c89e-64a8-4f20-b623-d167499bb97d".to_owned(),
            digest,
            &result,
        )
        .unwrap();
        assert_eq!(result.field.location, "cell-center");
        assert_eq!(result.field.value_count, 64);
        assert!(result.balance.relative_imbalance < 1.0e-12);
        assert_eq!(result.assembly.execution.adapter, "eqiora.rayon");
        assert_eq!(result.solve.execution.adapter, "eqiora.rayon");
        assert_eq!(result.solve.verification.adapter, "eqiora.rayon");
        assert!(result.solve.true_residual_norm <= result.solve.residual_target);
    }

    #[test]
    fn value_edit_projection_retains_transaction_identity_and_revision_lineage() {
        let document = ModelDocument::compile("decay.eqi", SOURCE).unwrap();
        let rate = document.aliases()["rate"];
        let plan = document.preview_value_edit(rate, 2.0).unwrap();
        let dto = ValueEditPlanDto::from(&plan);

        assert_eq!(dto.key, plan.key());
        assert_eq!(dto.base_digest, document.digest().unwrap());
        assert_eq!(dto.base_revision, 1);
        assert_eq!(dto.target_id, rate.to_string());
        assert_eq!(dto.before.value, 1.0);
        assert_eq!(dto.after.value, 2.0);
        assert_eq!(dto.before.dimension, dto.after.dimension);
        assert_eq!(dto.transaction_digest, plan.transaction_digest());

        let result = document.commit_value_edit(plan).unwrap();
        let child = project_document(
            result.document(),
            result.result_digest().to_owned(),
            NonZeroUsize::MIN,
        )
        .unwrap();
        assert_eq!(child.revision, 2);
        assert_eq!(
            child
                .nodes
                .iter()
                .find(|node| node.name == "rate")
                .and_then(|node| node.value),
            Some(2.0)
        );
        assert_eq!(document.program().value(rate).unwrap().value(), 1.0);
    }
}
