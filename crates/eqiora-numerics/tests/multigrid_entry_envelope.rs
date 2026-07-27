//! Admissible measurement of the protected multigrid-entry declaration.
//!
//! The declaration fixes the model, levels, solver controls, preconditioners,
//! execution placement, validity clauses, phase observations, and predicates.
//! This executor measures that exact probe without adding a solver capability.

use std::cell::Cell;
use std::io::{self, Write};
use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use eqiora_assembly::{
    AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork, REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora_compiler::compile;
use eqiora_core::Diagnostic;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_numerics::scalar::{
    finalize_lowered_scalar_elliptic_cartesian_with_assembly, lower_scalar_elliptic_cartesian,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, SemanticRevision, Space, Target, VectorLayoutKind, resolve,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    LinearSolver, LinearSolverBackend, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
    ReductionPolicy, ScalarType, SolverPlan,
};
use sha2::{Digest, Sha256};

const DECLARATION: &str =
    include_str!("../../../verify/numerics/multigrid-entry-envelope/case.toml");
const SOURCE_BYTES: &[u8] = include_bytes!(
    "../../../verify/numerics/preconditioner-scaling-envelope/models/constant-source-poisson.eqi"
);

const DIMENSION: usize = 3;
const RELATIVE_TOLERANCE: f64 = 1.0e-10;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-14;
const MAXIMUM_ITERATIONS: usize = 50_000;
const MINIMUM_COARSE_ITERATIONS: usize = 3;

#[derive(Debug)]
struct DeclaredContract {
    levels: Vec<usize>,
    adequate_maximum_ratio: f64,
    adequate_maximum_slope: f64,
    breach_terminal_ratio: f64,
    breach_slope: f64,
    breach_total_growth: f64,
    discriminator_slope_gap: f64,
    resource_budget: Duration,
}

impl DeclaredContract {
    fn read() -> Self {
        require_declared_string("equation", "-div(grad(u)) = 1");
        require_declared_string("domain", "(0, 1) x (0, 1) x (0, 1)");
        require_declared_string("boundary", "u = 0 on the complete boundary");
        require_declared_string("method", "continuous Cartesian Q1 Galerkin FEM");
        require_declared_string(
            "source_model",
            "verify/numerics/preconditioner-scaling-envelope/models/constant-source-poisson.eqi",
        );
        require_declared_string("scalar", "f64");
        require_declared_string("layout", "replicated");
        require_declared_string("target", "host serial");
        require_declared_string("schedule", "offline");
        require_declared_string("reduction", "reproducible");
        require_declared_string(
            "solver",
            "conjugate gradients, relative 1e-10, absolute 1e-14, cap 50000",
        );
        require_declared_string(
            "boundary_treatment",
            "full Dirichlet elimination on all six faces",
        );
        require_declared_string("primary_series", "fem-q1 jacobi");
        assert_eq!(
            declared_value("preconditioners"),
            "[\"identity\", \"jacobi\"]",
            "the executor admits only the two declared preconditioners"
        );
        require_declared_string(
            "relative_dominated_stopping",
            "residual target must exceed the absolute floor at every level",
        );
        require_declared_string(
            "uncensored_counts",
            "no solve may terminate on the iteration cap",
        );
        require_declared_string(
            "non_degenerate_krylov_probe",
            "identity CG must exceed three iterations at the coarsest level and must not be constant across the sequence",
        );
        require_declared_string(
            "complete_sequence",
            "every declared level must report; a partial sequence voids the run rather than shortening it",
        );
        require_declared_string(
            "assembly_seconds",
            "time to build the algebraic system, excluding model compilation",
        );
        require_declared_string(
            "finalization_seconds",
            "time to finalize the assembled system into its solver representation",
        );
        require_declared_string("solve_seconds", "time inside the Krylov iteration");
        require_declared_string(
            "peak_resident_bytes",
            "maximum resident set size of the measuring process",
        );

        let levels = declared_usize_array("cells_per_axis");
        assert_eq!(
            levels,
            [4, 8, 16, 32, 48],
            "the executor must stop if the complete declared sequence drifts"
        );

        Self {
            levels,
            adequate_maximum_ratio: declared_f64("adequate_maximum_ratio"),
            adequate_maximum_slope: declared_f64("adequate_maximum_slope"),
            breach_terminal_ratio: declared_f64("breach_terminal_ratio"),
            breach_slope: declared_f64("breach_slope"),
            breach_total_growth: declared_f64("breach_total_growth"),
            discriminator_slope_gap: declared_f64("discriminator_slope_gap"),
            resource_budget: Duration::from_secs(declared_u64("resource_budget_seconds")),
        }
    }
}

#[derive(Debug, Clone)]
struct Measurement {
    cells: usize,
    preconditioner: PreconditionerPolicy,
    unknowns: usize,
    iterations: usize,
    residual_target: f64,
    assembly_seconds: f64,
    finalization_seconds: f64,
    solve_seconds: f64,
    peak_resident_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IterationCount {
    cells: usize,
    preconditioner: PreconditionerPolicy,
    iterations: usize,
}

#[derive(Debug, Clone, Copy)]
struct Statistics {
    terminal_ratio: f64,
    maximum_ratio: f64,
    slope: f64,
    total_growth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclaredOutcome {
    Adequate,
    Breach,
    IndeterminateBand,
}

impl DeclaredOutcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Adequate => "adequate",
            Self::Breach => "breach",
            Self::IndeterminateBand => "indeterminate-band",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidityError {
    RelativeDominatedStopping,
    UncensoredCounts,
    NonDegenerateKrylovProbe,
    CompleteSequence,
}

#[derive(Debug)]
struct MeasurementRun {
    rows: Vec<Measurement>,
    repeated_counts: Vec<IterationCount>,
    elapsed: Duration,
}

fn measurement_run() -> &'static MeasurementRun {
    static RUN: OnceLock<MeasurementRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let started = Instant::now();
        let contract = DeclaredContract::read();
        validate_probe_identity(SOURCE_BYTES).expect("the probe source must match its declaration");
        let program = compile_program();
        let model =
            lower_scalar_elliptic_cartesian(&program).expect("the declared model lowers once");

        let rows = measure_once(&program, &model, &contract);
        validate_measurement(&rows, &contract)
            .expect("the first complete measurement must satisfy every validity clause");
        let repeated = measure_once(&program, &model, &contract);
        validate_measurement(&repeated, &contract)
            .expect("the repeated complete measurement must satisfy every validity clause");

        MeasurementRun {
            rows,
            repeated_counts: iteration_counts(&repeated),
            elapsed: started.elapsed(),
        }
    })
}

fn measure_once(
    program: &KernelProgram,
    model: &eqiora_numerics::scalar::ScalarEllipticCartesianModel,
    contract: &DeclaredContract,
) -> Vec<Measurement> {
    let capabilities = RealizationCapabilities::scalar_elliptic_reference();
    let requirements = RealizationRequirements::new(
        NonZeroUsize::new(DIMENSION).expect("three is non-zero"),
        ScalarType::F64,
        VectorLayoutKind::Replicated,
    );
    let semantic_revision = SemanticRevision::new(program.revision().0);
    let mut rows = Vec::with_capacity(contract.levels.len() * 2);

    for preconditioner in [PreconditionerPolicy::Identity, PreconditionerPolicy::Jacobi] {
        for (level, &cells) in contract.levels.iter().enumerate() {
            let resolved = resolve(
                &RealizationRequest::explicit(
                    program.model(),
                    semantic_revision,
                    RealizationRevision::new(revision_tag(level, preconditioner)),
                    plan(cells, preconditioner),
                ),
                requirements,
                &capabilities,
            )
            .expect("the declared tuple is inside the scalar elliptic capabilities");

            let assembly_started = Instant::now();
            let timed_assembly = TimedAssemblyBackend::new(assembly_started);
            let finalized = finalize_lowered_scalar_elliptic_cartesian_with_assembly(
                model,
                &resolved,
                &timed_assembly,
            )
            .expect("the declared algebraic system assembles and finalizes");
            let finalized_at = Instant::now();
            let assembly_finished = timed_assembly
                .assembly_finished()
                .expect("the ordinary finalization path invoked assembly exactly once");
            let assembly_seconds = assembly_finished
                .duration_since(assembly_started)
                .as_secs_f64();
            let finalization_seconds = finalized_at.duration_since(assembly_finished).as_secs_f64();

            let solver_plan = finalized.solver_plan();
            let (solved, solve_seconds) = {
                let problem = finalized
                    .linear_problem()
                    .expect("the finalized problem exposes its solver representation");
                let solve_started = Instant::now();
                let solved = REFERENCE_LINEAR_SOLVER
                    .solve(&problem, solver_plan)
                    .expect("the declared Krylov solve converges");
                (solved, solve_started.elapsed().as_secs_f64())
            };
            let unknowns = solved.values().len();
            let report = solved.report().clone();
            let _accepted = finalized
                .finish(solved)
                .expect("the accepted solve reconstructs through the ordinary path");

            assert_eq!(solver_plan.algorithm(), LinearSolver::ConjugateGradient);
            assert_eq!(report.preconditioner(), preconditioner);
            assert_eq!(report.reduction(), ReductionPolicy::Reproducible);
            assert!(
                report.true_residual_norm() <= report.residual_target(),
                "the independently recomputed true residual must be accepted"
            );
            assert_eq!(
                unknowns,
                (cells - 1).pow(DIMENSION as u32),
                "full Dirichlet elimination must retain only interior Q1 vertices"
            );

            rows.push(Measurement {
                cells,
                preconditioner,
                unknowns,
                iterations: report.completed_iterations(),
                residual_target: report.residual_target(),
                assembly_seconds,
                finalization_seconds,
                solve_seconds,
                peak_resident_bytes: peak_resident_bytes(),
            });
        }
    }
    rows
}

#[derive(Debug)]
struct TimedAssemblyBackend {
    phase_started: Instant,
    assembly_finished: Cell<Option<Instant>>,
}

impl TimedAssemblyBackend {
    fn new(phase_started: Instant) -> Self {
        Self {
            phase_started,
            assembly_finished: Cell::new(None),
        }
    }

    fn assembly_finished(&self) -> Option<Instant> {
        self.assembly_finished.get()
    }
}

impl AssemblyBackend for TimedAssemblyBackend {
    fn assemble(
        &self,
        plan: &AssemblyPlan,
        work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        assert!(
            self.assembly_finished.get().is_none(),
            "one realization must issue exactly one complete assembly operation"
        );
        let result = REFERENCE_ASSEMBLY_BACKEND.assemble(plan, work);
        let finished = Instant::now();
        assert!(
            finished >= self.phase_started,
            "the monotonic phase clock cannot run backwards"
        );
        self.assembly_finished.set(Some(finished));
        result
    }
}

fn validate_measurement(
    rows: &[Measurement],
    contract: &DeclaredContract,
) -> Result<(), ValidityError> {
    validate_complete_sequence(rows, contract)?;
    if rows
        .iter()
        .any(|row| row.residual_target <= ABSOLUTE_TOLERANCE)
    {
        return Err(ValidityError::RelativeDominatedStopping);
    }
    if rows.iter().any(|row| row.iterations >= MAXIMUM_ITERATIONS) {
        return Err(ValidityError::UncensoredCounts);
    }

    let identity = rows
        .iter()
        .filter(|row| row.preconditioner == PreconditionerPolicy::Identity)
        .collect::<Vec<_>>();
    if identity[0].iterations <= MINIMUM_COARSE_ITERATIONS
        || identity
            .iter()
            .all(|row| row.iterations == identity[0].iterations)
    {
        return Err(ValidityError::NonDegenerateKrylovProbe);
    }
    Ok(())
}

fn validate_complete_sequence(
    rows: &[Measurement],
    contract: &DeclaredContract,
) -> Result<(), ValidityError> {
    if rows.len() != contract.levels.len() * 2 {
        return Err(ValidityError::CompleteSequence);
    }
    for preconditioner in [PreconditionerPolicy::Identity, PreconditionerPolicy::Jacobi] {
        for &cells in &contract.levels {
            if rows
                .iter()
                .filter(|row| row.preconditioner == preconditioner && row.cells == cells)
                .count()
                != 1
            {
                return Err(ValidityError::CompleteSequence);
            }
        }
    }
    Ok(())
}

fn classify(counts: &[IterationCount], contract: &DeclaredContract) -> DeclaredOutcome {
    let primary = series(counts, PreconditionerPolicy::Jacobi);
    let statistics = Statistics::of(&primary);
    let adequate = statistics.maximum_ratio <= contract.adequate_maximum_ratio
        && statistics.slope <= contract.adequate_maximum_slope;
    let breached = statistics.terminal_ratio >= contract.breach_terminal_ratio
        && statistics.slope >= contract.breach_slope
        && statistics.total_growth >= contract.breach_total_growth;
    match (adequate, breached) {
        (true, false) => DeclaredOutcome::Adequate,
        (false, true) => DeclaredOutcome::Breach,
        (false, false) => DeclaredOutcome::IndeterminateBand,
        (true, true) => panic!("the disjoint declared thresholds cannot classify both outcomes"),
    }
}

impl Statistics {
    fn of(series: &[IterationCount]) -> Self {
        assert!(series.len() >= 2, "statistics require a sequence");
        let ratios = series
            .windows(2)
            .map(|pair| pair[1].iterations as f64 / pair[0].iterations as f64)
            .collect::<Vec<_>>();
        let terminal_ratio = *ratios.last().expect("the sequence has adjacent levels");
        let maximum_ratio = ratios.iter().copied().fold(f64::MIN, f64::max);
        let total_growth = series[series.len() - 1].iterations as f64 / series[0].iterations as f64;
        Self {
            terminal_ratio,
            maximum_ratio,
            slope: fitted_slope(series),
            total_growth,
        }
    }
}

fn fitted_slope(series: &[IterationCount]) -> f64 {
    let count = series.len() as f64;
    let points = series
        .iter()
        .map(|row| ((row.cells as f64).log2(), (row.iterations as f64).log2()))
        .collect::<Vec<_>>();
    let mean_x = points.iter().map(|point| point.0).sum::<f64>() / count;
    let mean_y = points.iter().map(|point| point.1).sum::<f64>() / count;
    let covariance = points
        .iter()
        .map(|point| (point.0 - mean_x) * (point.1 - mean_y))
        .sum::<f64>();
    let variance = points
        .iter()
        .map(|point| (point.0 - mean_x).powi(2))
        .sum::<f64>();
    assert!(variance > 0.0, "the refinement sequence must vary");
    covariance / variance
}

fn iteration_counts(rows: &[Measurement]) -> Vec<IterationCount> {
    rows.iter()
        .map(|row| IterationCount {
            cells: row.cells,
            preconditioner: row.preconditioner,
            iterations: row.iterations,
        })
        .collect()
}

fn series(counts: &[IterationCount], preconditioner: PreconditionerPolicy) -> Vec<IterationCount> {
    counts
        .iter()
        .filter(|row| row.preconditioner == preconditioner)
        .copied()
        .collect()
}

fn validate_probe_identity(bytes: &[u8]) -> Result<(), String> {
    let expected = declared_string("source_sha256");
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "probe source SHA-256 mismatch: declared {expected}, measured {actual}"
        ))
    }
}

fn peak_resident_bytes() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status")
        .expect("hosted Linux exposes /proc/self/status");
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .expect("/proc/self/status exposes the process high-water resident set");
    let mut fields = line.split_whitespace();
    let kibibytes = fields
        .next()
        .expect("VmHWM has a numeric value")
        .parse::<u64>()
        .expect("VmHWM is an integer");
    assert_eq!(fields.next(), Some("kB"), "Linux reports VmHWM in KiB");
    kibibytes
        .checked_mul(1024)
        .expect("resident bytes fit in u64")
}

fn plan(cells: usize, preconditioner: PreconditionerPolicy) -> RealizationPlan {
    RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(cells).expect("a declared level is non-zero"),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            RELATIVE_TOLERANCE,
            ABSOLUTE_TOLERANCE,
            NonZeroUsize::new(MAXIMUM_ITERATIONS).expect("the cap is non-zero"),
        )
        .expect("the declared solver controls are valid")
        .with_preconditioner(preconditioner),
        Target::HostCpu {
            threads: NonZeroUsize::MIN,
        },
        ExecutionSchedule::Offline,
    )
    .expect("the declared realization plan is valid")
}

fn revision_tag(level: usize, preconditioner: PreconditionerPolicy) -> u64 {
    2 * level as u64 + u64::from(preconditioner == PreconditionerPolicy::Jacobi)
}

fn compile_program() -> KernelProgram {
    let source = std::str::from_utf8(SOURCE_BYTES).expect("the pinned model source is UTF-8");
    let mut compiled = compile("constant-source-poisson.eqi", source).expect("the model compiles");
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("the transaction commits");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("the program elaborates")
}

fn preconditioner_label(preconditioner: PreconditionerPolicy) -> &'static str {
    match preconditioner {
        PreconditionerPolicy::Identity => "identity",
        PreconditionerPolicy::Jacobi => "jacobi",
    }
}

fn write_report(
    output: &mut dyn Write,
    run: &MeasurementRun,
    contract: &DeclaredContract,
) -> io::Result<()> {
    let counts = iteration_counts(&run.rows);
    let primary = Statistics::of(&series(&counts, PreconditionerPolicy::Jacobi));
    let identity = Statistics::of(&series(&counts, PreconditionerPolicy::Identity));
    let outcome = classify(&counts, contract);
    let discriminator_gap = (primary.slope - identity.slope).abs();
    let adequacy_a1 = primary.maximum_ratio <= contract.adequate_maximum_ratio;
    let adequacy_a2 = primary.slope <= contract.adequate_maximum_slope;
    let breach_b1 = primary.terminal_ratio >= contract.breach_terminal_ratio;
    let breach_b2 = primary.slope >= contract.breach_slope;
    let breach_b3 = primary.total_growth >= contract.breach_total_growth;

    writeln!(
        output,
        "level,preconditioner,unknowns,iterations,assembly_s,finalization_s,solve_s,peak_rss_bytes"
    )?;
    for row in &run.rows {
        writeln!(
            output,
            "{},{},{},{},{:.6},{:.6},{:.6},{}",
            row.cells,
            preconditioner_label(row.preconditioner),
            row.unknowns,
            row.iterations,
            row.assembly_seconds,
            row.finalization_seconds,
            row.solve_seconds,
            row.peak_resident_bytes
        )?;
    }
    writeln!(
        output,
        "adequacy_a1={adequacy_a1}; adequacy_a2={adequacy_a2}; \
         breach_b1={breach_b1}; breach_b2={breach_b2}; breach_b3={breach_b3}; \
         discriminator_d1={}; declared_outcome={}; maximum_ratio={:.6}; \
         terminal_ratio={:.6}; slope={:.6}; total_growth={:.6}; \
         discriminator_slope_gap={:.6}; complete_measurement_s={:.6}; resource_budget_s={}",
        discriminator_gap <= contract.discriminator_slope_gap,
        outcome.label(),
        primary.maximum_ratio,
        primary.terminal_ratio,
        primary.slope,
        primary.total_growth,
        discriminator_gap,
        run.elapsed.as_secs_f64(),
        contract.resource_budget.as_secs()
    )
}

fn declared_value(key: &str) -> &'static str {
    let prefix = format!("{key} = ");
    DECLARATION
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("declaration is missing `{key}`"))
}

fn declared_string(key: &str) -> &'static str {
    let value = declared_value(key);
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("declared `{key}` is not a string"))
}

fn require_declared_string(key: &str, expected: &str) {
    assert_eq!(
        declared_string(key),
        expected,
        "the executor must stop if `{key}` drifts from the admitted probe"
    );
}

fn declared_f64(key: &str) -> f64 {
    declared_value(key)
        .parse()
        .unwrap_or_else(|_| panic!("declared `{key}` is not an f64"))
}

fn declared_u64(key: &str) -> u64 {
    declared_value(key)
        .parse()
        .unwrap_or_else(|_| panic!("declared `{key}` is not a u64"))
}

fn declared_usize_array(key: &str) -> Vec<usize> {
    let value = declared_value(key);
    value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or_else(|| panic!("declared `{key}` is not an array"))
        .split(',')
        .map(|item| {
            item.trim()
                .parse()
                .unwrap_or_else(|_| panic!("declared `{key}` contains a non-usize value"))
        })
        .collect()
}

fn synthetic_measurement(iterations: [usize; 5]) -> Vec<Measurement> {
    let contract = DeclaredContract::read();
    [PreconditionerPolicy::Identity, PreconditionerPolicy::Jacobi]
        .into_iter()
        .flat_map(|preconditioner| {
            contract
                .levels
                .iter()
                .copied()
                .zip(iterations)
                .map(move |(cells, iterations)| Measurement {
                    cells,
                    preconditioner,
                    unknowns: (cells - 1).pow(DIMENSION as u32),
                    iterations,
                    residual_target: 2.0 * ABSOLUTE_TOLERANCE,
                    assembly_seconds: 1.0,
                    finalization_seconds: 1.0,
                    solve_seconds: 1.0,
                    peak_resident_bytes: 1,
                })
        })
        .collect()
}

#[test]
fn probe_identity_matches_declared_source_sha256() {
    validate_probe_identity(SOURCE_BYTES).expect("the exact admitted source must pass");
    let mut substituted = SOURCE_BYTES.to_vec();
    substituted[0] ^= 1;
    assert!(
        validate_probe_identity(&substituted).is_err(),
        "different model bytes must fail closed"
    );
}

#[test]
fn validity_clauses_void_invalid_measurements() {
    let contract = DeclaredContract::read();
    let valid = synthetic_measurement([4, 8, 16, 32, 48]);
    validate_measurement(&valid, &contract).expect("the synthetic baseline is valid");

    let mut absolute_dominated = valid.clone();
    absolute_dominated[0].residual_target = ABSOLUTE_TOLERANCE;
    assert_eq!(
        validate_measurement(&absolute_dominated, &contract),
        Err(ValidityError::RelativeDominatedStopping)
    );

    let mut censored = valid.clone();
    censored[0].iterations = MAXIMUM_ITERATIONS;
    assert_eq!(
        validate_measurement(&censored, &contract),
        Err(ValidityError::UncensoredCounts)
    );

    let one_iteration_throughout = synthetic_measurement([1; 5]);
    assert_eq!(
        validate_measurement(&one_iteration_throughout, &contract),
        Err(ValidityError::NonDegenerateKrylovProbe)
    );
}

#[test]
fn declared_predicates_ignore_phase_observations() {
    let contract = DeclaredContract::read();
    let rows = synthetic_measurement([4, 8, 16, 32, 64]);
    let expected = classify(&iteration_counts(&rows), &contract);
    let mut altered_phases = rows;
    for (index, row) in altered_phases.iter_mut().enumerate() {
        row.assembly_seconds = index as f64 * 101.0;
        row.finalization_seconds = index as f64 * 103.0;
        row.solve_seconds = index as f64 * 107.0;
        row.peak_resident_bytes = u64::MAX - index as u64;
    }
    assert_eq!(
        classify(&iteration_counts(&altered_phases), &contract),
        expected
    );
}

#[test]
fn missing_level_voids_the_run() {
    let contract = DeclaredContract::read();
    let mut incomplete = synthetic_measurement([4, 8, 16, 32, 48]);
    incomplete
        .retain(|row| !(row.cells == 48 && row.preconditioner == PreconditionerPolicy::Jacobi));
    assert_eq!(
        validate_measurement(&incomplete, &contract),
        Err(ValidityError::CompleteSequence)
    );
}

#[test]
fn repeated_measurements_have_identical_iteration_counts() {
    let run = measurement_run();
    assert_eq!(iteration_counts(&run.rows), run.repeated_counts);
}

#[test]
fn outcome_reporter_distinguishes_every_declared_band() {
    let contract = DeclaredContract::read();
    for (iterations, expected) in [
        ([10, 10, 11, 11, 12], DeclaredOutcome::Adequate),
        ([4, 8, 16, 32, 64], DeclaredOutcome::Breach),
        ([4, 8, 12, 20, 30], DeclaredOutcome::IndeterminateBand),
    ] {
        let rows = synthetic_measurement(iterations);
        assert_eq!(classify(&iteration_counts(&rows), &contract), expected);
        let run = MeasurementRun {
            repeated_counts: iteration_counts(&rows),
            rows,
            elapsed: Duration::ZERO,
        };
        let mut rendered = Vec::new();
        write_report(&mut rendered, &run, &contract).expect("the report renders");
        let rendered = String::from_utf8(rendered).expect("the report is UTF-8");
        assert!(
            rendered.contains(&format!("declared_outcome={}", expected.label())),
            "rendered report did not preserve {expected:?}: {rendered}"
        );
    }
}

#[test]
fn complete_measurement_reports_the_declared_outcome_and_phases() {
    let contract = DeclaredContract::read();
    let run = measurement_run();
    assert!(
        run.elapsed <= contract.resource_budget,
        "complete measurement took {:.3} s, over the declared {} s budget",
        run.elapsed.as_secs_f64(),
        contract.resource_budget.as_secs()
    );
    let stdout = io::stdout();
    write_report(&mut stdout.lock(), run, &contract).expect("the measurement report writes");
}
