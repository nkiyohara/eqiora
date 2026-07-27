//! Falsifier for the stable preconditioner vocabulary on refined 3D Poisson.
//!
//! Every threshold in this file was declared in
//! `verify/numerics/preconditioner-scaling-envelope/README.md` before any
//! iteration count was measured. The file measures the declared refinement
//! sequence and asserts the declared breach predicate; it implements no new
//! preconditioner and admits no new solver tuple.

use std::num::{NonZeroU16, NonZeroUsize};
use std::sync::OnceLock;

use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_numerics::scalar::{
    ResolvedScalarEllipticCartesianSolution, solve_resolved_scalar_elliptic_cartesian,
};
use eqiora_realization::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy,
    RealizationCapabilities, RealizationPlan, RealizationRequest, RealizationRequirements,
    RealizationRevision, SemanticRevision, Space, Target, VectorLayoutKind, resolve,
};
use eqiora_sem::KernelProgram;
use eqiora_solver::{
    LinearSolver, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ScalarType, SolverPlan,
};

const SOURCE: &str = include_str!(
    "../../../verify/numerics/preconditioner-scaling-envelope/models/constant-source-poisson.eqi"
);
const EXPECTED: &str = include_str!(
    "../../../verify/numerics/preconditioner-scaling-envelope/expected/iterations.csv"
);

const DIMENSION: usize = 3;

/// Declared refinement sequence; each step halves `h = 1 / n` exactly.
const LEVELS: [usize; 4] = [4, 8, 16, 32];

/// Declared solver controls, identical at every level.
const RELATIVE_TOLERANCE: f64 = 1.0e-10;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-14;
const MAXIMUM_ITERATIONS: usize = 50_000;

/// Declared adequacy thresholds (A1, A2).
const ADEQUATE_MAXIMUM_RATIO: f64 = 1.4;
const ADEQUATE_MAXIMUM_SLOPE: f64 = 0.5;

/// Declared breach thresholds (B1, B2, B3).
const BREACH_TERMINAL_RATIO: f64 = 1.8;
const BREACH_SLOPE: f64 = 0.85;
const BREACH_TOTAL_GROWTH: f64 = 5.0;

/// Declared discriminator threshold (D1).
const DISCRIMINATOR_SLOPE_GAP: f64 = 0.10;

/// Declared probe-validity threshold (V3). A right-hand side confined to a
/// low-dimensional invariant subspace terminates a Krylov method in as many
/// steps as that subspace has dimensions, independently of the mesh, and would
/// counterfeit scalability.
const MINIMUM_COARSE_ITERATIONS: usize = 3;

/// Portability allowance on a recorded integer count, matching the tolerance
/// the neighbouring preconditioner-stress case already accepts. It relaxes the
/// regression lock only, never a declared envelope threshold.
const ITERATION_ALLOWANCE: usize = 2;

/// One accepted solve of the declared sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Measurement {
    cells: usize,
    unknowns: usize,
    iterations: usize,
}

/// B1, B2, and B3 evaluated on one series.
#[derive(Debug, Clone, Copy)]
struct Statistics {
    terminal_ratio: f64,
    maximum_ratio: f64,
    slope: f64,
    total_growth: f64,
}

/// The declared sequence, measured once and shared by every claim below.
fn measured_table() -> &'static [(&'static str, &'static str, Vec<Measurement>)] {
    static TABLE: OnceLock<Vec<(&'static str, &'static str, Vec<Measurement>)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let program = compile_program();
        let mut table = Vec::new();
        for method in [
            DiscretizationMethod::ContinuousGalerkin,
            DiscretizationMethod::CellCenteredFiniteVolume,
        ] {
            for preconditioner in [PreconditionerPolicy::Identity, PreconditionerPolicy::Jacobi] {
                let series = measure_series(&program, method, preconditioner);
                assert!(
                    series[0].iterations > MINIMUM_COARSE_ITERATIONS,
                    "V3 voided: {} / {} terminated in {} iterations at the coarsest level, so the \
                     right-hand side is a Krylov-degenerate probe",
                    method_label(method),
                    preconditioner_label(preconditioner),
                    series[0].iterations
                );
                assert!(
                    series
                        .iter()
                        .any(|measurement| measurement.iterations != series[0].iterations),
                    "V3 voided: {} / {} is constant across the refinement sequence",
                    method_label(method),
                    preconditioner_label(preconditioner)
                );
                table.push((
                    method_label(method),
                    preconditioner_label(preconditioner),
                    series,
                ));
            }
        }
        table
    })
}

fn series(method: &str, preconditioner: &str) -> &'static [Measurement] {
    measured_table()
        .iter()
        .find(|row| row.0 == method && row.1 == preconditioner)
        .map(|row| row.2.as_slice())
        .expect("the declared series was measured")
}

#[test]
fn jacobi_conjugate_gradients_breaches_the_declared_iteration_growth_envelope() {
    let series = series("fem-q1", "jacobi");
    let statistics = Statistics::of(series);

    assert!(
        statistics.terminal_ratio >= BREACH_TERMINAL_RATIO,
        "B1 not met: terminal ratio {} < {BREACH_TERMINAL_RATIO}; series {series:?}",
        statistics.terminal_ratio
    );
    assert!(
        statistics.slope >= BREACH_SLOPE,
        "B2 not met: fitted slope {} < {BREACH_SLOPE}; series {series:?}",
        statistics.slope
    );
    assert!(
        statistics.total_growth >= BREACH_TOTAL_GROWTH,
        "B3 not met: total growth {} < {BREACH_TOTAL_GROWTH}; series {series:?}",
        statistics.total_growth
    );
    assert!(
        !(statistics.maximum_ratio <= ADEQUATE_MAXIMUM_RATIO
            && statistics.slope <= ADEQUATE_MAXIMUM_SLOPE),
        "the same series cannot satisfy both the adequacy and the breach predicate: {statistics:?}"
    );
}

#[test]
fn the_stable_vocabulary_changes_the_constant_and_not_the_growth_order() {
    let identity = Statistics::of(series("fem-q1", "identity"));
    let jacobi = Statistics::of(series("fem-q1", "jacobi"));

    assert!(
        (identity.slope - jacobi.slope).abs() <= DISCRIMINATOR_SLOPE_GAP,
        "D1 not met: identity slope {} and Jacobi slope {} differ by more than \
         {DISCRIMINATOR_SLOPE_GAP}",
        identity.slope,
        jacobi.slope
    );
    assert!(
        identity.slope >= BREACH_SLOPE && jacobi.slope >= BREACH_SLOPE,
        "neither policy in the stable vocabulary may hide the growth: \
         identity {}, Jacobi {}",
        identity.slope,
        jacobi.slope
    );
}

#[test]
fn the_recorded_series_reproduce_the_registered_evidence_table() {
    let expected = parse_expected();
    assert_eq!(
        expected.len(),
        4 * LEVELS.len(),
        "the evidence table must record two methods times two policies times \
         every declared level"
    );

    let observed = measured_table()
        .iter()
        .flat_map(|(method, preconditioner, series)| {
            series
                .iter()
                .map(move |measurement| (*method, *preconditioner, *measurement))
        })
        .collect::<Vec<_>>();
    assert_eq!(observed.len(), expected.len());

    for (row, expected) in observed.iter().zip(&expected) {
        assert_eq!(row.0, expected.0, "evidence row order drifted: {row:?}");
        assert_eq!(row.1, expected.1, "evidence row order drifted: {row:?}");
        assert_eq!(row.2.cells, expected.2.cells);
        assert_eq!(row.2.unknowns, expected.2.unknowns);
        assert!(
            row.2.iterations.abs_diff(expected.2.iterations) <= ITERATION_ALLOWANCE,
            "recorded {} iterations, observed {} for {} / {} at n = {}",
            expected.2.iterations,
            row.2.iterations,
            row.0,
            row.1,
            row.2.cells
        );
    }
}

impl Statistics {
    fn of(series: &[Measurement]) -> Self {
        assert_eq!(series.len(), LEVELS.len());
        let ratios = series
            .windows(2)
            .map(|pair| pair[1].iterations as f64 / pair[0].iterations as f64)
            .collect::<Vec<_>>();
        let terminal_ratio = *ratios.last().expect("the sequence has two or more levels");
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

/// Least-squares slope of `log2(iterations)` regressed on `log2(cells)`.
fn fitted_slope(series: &[Measurement]) -> f64 {
    let count = series.len() as f64;
    let points = series
        .iter()
        .map(|measurement| {
            (
                (measurement.cells as f64).log2(),
                (measurement.iterations as f64).log2(),
            )
        })
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

fn measure_series(
    program: &KernelProgram,
    method: DiscretizationMethod,
    preconditioner: PreconditionerPolicy,
) -> Vec<Measurement> {
    LEVELS
        .iter()
        .enumerate()
        .map(|(level, &cells)| measure(program, level, cells, method, preconditioner))
        .collect()
}

fn measure(
    program: &KernelProgram,
    level: usize,
    cells: usize,
    method: DiscretizationMethod,
    preconditioner: PreconditionerPolicy,
) -> Measurement {
    let capabilities = RealizationCapabilities::scalar_elliptic_reference();
    let requirements = RealizationRequirements::new(
        NonZeroUsize::new(DIMENSION).expect("three is non-zero"),
        ScalarType::F64,
        VectorLayoutKind::Replicated,
    );
    let realization_revision =
        RealizationRevision::new(revision_tag(level, method, preconditioner));
    let resolved = resolve(
        &RealizationRequest::explicit(
            program.model(),
            SemanticRevision::new(program.revision().0),
            realization_revision,
            plan(cells, method, preconditioner),
        ),
        requirements,
        &capabilities,
    )
    .expect("the declared tuple is inside the reference scalar elliptic capabilities");
    let (_, solution) =
        solve_resolved_scalar_elliptic_cartesian(program, &resolved, &REFERENCE_LINEAR_SOLVER)
            .expect("the declared refinement level solves");

    let (report, unknowns) = match &solution {
        ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => {
            (solution.solve_report(), solution.algebraic_values().len())
        }
        ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
            (solution.solve_report(), solution.cell_values().len())
        }
    };

    assert_eq!(report.preconditioner(), preconditioner);
    assert!(
        report.true_residual_norm() <= report.residual_target(),
        "the independently recomputed true residual must be accepted before a \
         count is recorded"
    );
    // V1: the relative criterion, not the absolute floor, governs every level.
    assert!(
        report.residual_target() > ABSOLUTE_TOLERANCE,
        "V1 voided at n = {cells}: the absolute floor bound the stopping criterion"
    );
    // V2: no count is censored by the iteration cap.
    assert!(
        report.completed_iterations() < MAXIMUM_ITERATIONS,
        "V2 voided at n = {cells}: the solve terminated on the iteration cap"
    );

    let measurement = Measurement {
        cells,
        unknowns,
        iterations: report.completed_iterations(),
    };
    println!(
        "{},{},{},{},{}",
        method_label(method),
        preconditioner_label(preconditioner),
        measurement.cells,
        measurement.unknowns,
        measurement.iterations
    );
    measurement
}

fn plan(
    cells: usize,
    method: DiscretizationMethod,
    preconditioner: PreconditionerPolicy,
) -> RealizationPlan {
    let (space, quadrature) = match method {
        DiscretizationMethod::ContinuousGalerkin => (
            Space::continuous_lagrange(NonZeroU16::MIN),
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
        ),
        DiscretizationMethod::CellCenteredFiniteVolume => {
            (Space::cell_constant(), QuadraturePolicy::CellCentroid)
        }
    };
    RealizationPlan::new(
        space,
        Discretization::new(
            method,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(cells).expect("a level is non-zero"),
            },
            quadrature,
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

fn revision_tag(
    level: usize,
    method: DiscretizationMethod,
    preconditioner: PreconditionerPolicy,
) -> u64 {
    let method = u64::from(method == DiscretizationMethod::CellCenteredFiniteVolume);
    let preconditioner = u64::from(preconditioner == PreconditionerPolicy::Jacobi);
    4 * level as u64 + 2 * method + preconditioner
}

fn method_label(method: DiscretizationMethod) -> &'static str {
    match method {
        DiscretizationMethod::ContinuousGalerkin => "fem-q1",
        DiscretizationMethod::CellCenteredFiniteVolume => "fvm-tpfa",
    }
}

fn preconditioner_label(preconditioner: PreconditionerPolicy) -> &'static str {
    match preconditioner {
        PreconditionerPolicy::Identity => "identity",
        PreconditionerPolicy::Jacobi => "jacobi",
    }
}

fn parse_expected() -> Vec<(String, String, Measurement)> {
    EXPECTED
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let columns = line.split(',').collect::<Vec<_>>();
            assert_eq!(columns.len(), 5, "unexpected evidence row: {line}");
            (
                columns[0].to_owned(),
                columns[1].to_owned(),
                Measurement {
                    cells: columns[2].parse().expect("cells column"),
                    unknowns: columns[3].parse().expect("unknowns column"),
                    iterations: columns[4].parse().expect("iterations column"),
                },
            )
        })
        .collect()
}

fn compile_program() -> KernelProgram {
    let mut compiled = compile("constant-source-poisson.eqi", SOURCE).expect("the model compiles");
    let (transaction, model, _) = compiled.remove(0).into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("the transaction commits");
    KernelProgram::from_snapshot(&store.snapshot(), model).expect("the program elaborates")
}
