//! Independent 12 V / 1 kOhm / 2 kOhm circuit and missing-reference witnesses.

use std::num::NonZeroUsize;

use eqiora::compiler::{ModelSymbols, compile};
use eqiora::entity::kinds;
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore};
use eqiora::kernel::KernelNode;
use eqiora::sem::{KernelProgram, PhysicalUnknown};
use eqiora::solver::{LinearSolveRequest, LinearSolver, ReductionPolicy, SolverPlan};
use eqiora::{Id, RawId};
use eqiora_backend_faer::FaerLinearSolver;
use eqiora_numerics::scalar::{lower_scalar_physical_affine, solve_scalar_physical_affine};

const COMPONENTS: &str = include_str!("../../../packages/Eqiora.Electrical.Basic/src/basic.eqi");
const DIVIDER: &str = include_str!("../../../examples/voltage_divider.eqi");

fn fixture(model: &str) -> (KernelProgram, ModelSymbols, Id<kinds::Connection>) {
    fixture_with_components(COMPONENTS, model)
}

fn fixture_with_components(
    components: &str,
    model: &str,
) -> (KernelProgram, ModelSymbols, Id<kinds::Connection>) {
    let source = format!("{components}\n{model}");
    let compiled = compile("divider.eqi", &source)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
        .remove(0);
    let symbols = compiled.symbols().clone();
    let model = compiled.model();
    let (transaction, _, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let member = port(&symbols, "source.positive");
    let connection = program
        .nodes()
        .find_map(|node| {
            let KernelNode::Connection(connection) = node else {
                return None;
            };
            program
                .edges()
                .iter()
                .any(|edge| {
                    edge.kind() == EdgeKind::Connects
                        && edge.from() == connection.id().erase()
                        && edge.to() == member.erase()
                })
                .then_some(connection.id())
        })
        .unwrap();
    (program, symbols, connection)
}

fn port(symbols: &ModelSymbols, name: &str) -> Id<kinds::Port> {
    symbols
        .get(name)
        .and_then(RawId::downcast)
        .unwrap_or_else(|| panic!("missing Port {name}"))
}

#[test]
fn standard_named_divider_has_independent_current_voltage_power_and_equation_owners() {
    let (program, symbols, connection) = fixture(DIVIDER);
    let problem = lower_scalar_physical_affine(&program, connection, None).unwrap();
    let composed = problem.composed_system();
    assert_eq!(composed.unknowns().len(), 14);
    assert_eq!(
        composed
            .relations()
            .iter()
            .map(|r| r.dag().roots().len())
            .sum::<usize>(),
        7
    );
    let mut generated = composed
        .junctions()
        .iter()
        .map(|r| r.dag().roots().len())
        .collect::<Vec<_>>();
    generated.sort_unstable();
    assert_eq!(generated, [2, 2, 3]);
    assert_eq!(
        (
            problem.canonical_system().rows(),
            problem.canonical_system().columns()
        ),
        (14, 14)
    );
    let plan = SolverPlan::new(
        LinearSolver::SparseLu,
        1e-12,
        1e-14,
        NonZeroUsize::new(100).unwrap(),
    )
    .unwrap()
    .with_reduction(ReductionPolicy::Fast);
    let solution =
        solve_scalar_physical_affine(&problem, LinearSolveRequest::new(&FaerLinearSolver, plan))
            .unwrap();
    // Ohm and Kirchhoff independently: I=12/(1000+2000)=.004 A;
    // lower-node voltage=.004*2000=8 V. Positive current enters each component.
    for (name, voltage, current) in [
        ("source.positive", 12.0, -0.004),
        ("source.negative", 0.0, 0.004),
        ("upper.positive", 12.0, 0.004),
        ("upper.negative", 8.0, -0.004),
        ("lower.positive", 8.0, 0.004),
        ("lower.negative", 0.0, -0.004),
        ("ground.terminal", 0.0, 0.0),
    ] {
        let terminal = port(&symbols, name);
        assert!(
            (solution.value(PhysicalUnknown::Across(terminal)).unwrap() - voltage).abs() < 1e-10,
            "{name} voltage"
        );
        assert!(
            (solution.value(PhysicalUnknown::Through(terminal)).unwrap() - current).abs() < 1e-12,
            "{name} current"
        );
    }
    let power = |positive: &str, negative: &str| {
        let positive = port(&symbols, positive);
        let negative = port(&symbols, negative);
        (solution.value(PhysicalUnknown::Across(positive)).unwrap()
            - solution.value(PhysicalUnknown::Across(negative)).unwrap())
            * solution.value(PhysicalUnknown::Through(positive)).unwrap()
    };
    let upper = power("upper.positive", "upper.negative");
    let lower = power("lower.positive", "lower.negative");
    let supplied = power("source.positive", "source.negative");
    assert!((upper - 0.016).abs() < 1e-12);
    assert!((lower - 0.032).abs() < 1e-12);
    assert!((supplied + 0.048).abs() < 1e-12);
    assert!((upper + lower + supplied).abs() < 1e-12);
}

#[test]
fn removing_the_entire_ground_is_rejected_despite_a_square_satisfied_system() {
    let floating = DIVIDER
        .replace("  instance ground: Ground();\n", "")
        .replace(", ground.terminal", "");
    let (program, symbols, connection) = fixture(&floating);
    let composed = program
        .compose_scalar_physical_subsystem(connection)
        .unwrap();
    assert_eq!(composed.unknowns().len(), 12);
    assert_eq!(
        composed
            .relations()
            .iter()
            .map(|r| r.dag().roots().len())
            .sum::<usize>()
            + composed
                .junctions()
                .iter()
                .map(|r| r.dag().roots().len())
                .sum::<usize>(),
        12
    );
    let parameters = composed
        .parameters()
        .iter()
        .map(|id| program.value(id.erase()).unwrap().value())
        .collect::<Vec<_>>();
    for shift in [0.0, 5.0] {
        let guess = composed
            .unknowns()
            .iter()
            .map(|unknown| {
                for (name, voltage, current) in [
                    ("source.positive", 12.0, -0.004),
                    ("source.negative", 0.0, 0.004),
                    ("upper.positive", 12.0, 0.004),
                    ("upper.negative", 8.0, -0.004),
                    ("lower.positive", 8.0, 0.004),
                    ("lower.negative", 0.0, -0.004),
                ] {
                    if unknown.port() == port(&symbols, name) {
                        return match unknown {
                            PhysicalUnknown::Across(_) => voltage + shift,
                            PhysicalUnknown::Through(_) => current,
                        };
                    }
                }
                panic!("unexpected physical unknown")
            })
            .collect::<Vec<_>>();
        assert!(
            composed
                .evaluate_reference(&guess, &parameters, None)
                .unwrap()
                .iter()
                .all(|r| r.abs() < 1e-12)
        );
    }
    let error = lower_scalar_physical_affine(&program, connection, None).unwrap_err();
    assert!(error.message().contains("unreferenced uniform shift"));
}

#[test]
fn an_explicit_subnormal_reference_is_not_misclassified_as_zero() {
    let components = COMPONENTS.replace("terminal.voltage = 0;", "5e-324 * terminal.voltage = 0;");
    let (program, _, connection) = fixture_with_components(&components, DIVIDER);
    lower_scalar_physical_affine(&program, connection, None)
        .expect("nonzero reference remains an anchor; solver conditioning is a separate concern");
}

#[test]
fn one_relation_owning_two_islands_does_not_let_one_ground_hide_the_other_gauge() {
    let components = format!("{COMPONENTS}\ncomponent TwoSources(port positive:Pin,port negative:Pin,port other_positive:Pin,port other_negative:Pin) {{
relation law {{positive.voltage-negative.voltage=12[V];positive.current+negative.current=0;
other_positive.voltage-other_negative.voltage=12[V];other_positive.current+other_negative.current=0;}} }}");
    let model = "model TwoIslands() {
instance source:TwoSources();
instance upper:Resistor(resistance=1[kOhm]);instance lower:Resistor(resistance=2[kOhm]);instance ground:Ground();
instance upper2:Resistor(resistance=1[kOhm]);instance lower2:Resistor(resistance=2[kOhm]);
connect source.positive,upper.positive;
connect upper.negative,lower.positive;
connect lower.negative,source.negative,ground.terminal;
connect source.other_positive,upper2.positive;
connect upper2.negative,lower2.positive;
connect lower2.negative,source.other_negative;
}";
    let (program, _, connection) = fixture_with_components(&components, model);
    let composed = program
        .compose_scalar_physical_subsystem(connection)
        .unwrap();
    assert_eq!(composed.unknowns().len(), 26);
    assert_eq!(
        composed
            .relations()
            .iter()
            .map(|r| r.dag().roots().len())
            .sum::<usize>()
            + composed
                .junctions()
                .iter()
                .map(|r| r.dag().roots().len())
                .sum::<usize>(),
        26
    );
    assert!(
        lower_scalar_physical_affine(&program, connection, None)
            .unwrap_err()
            .message()
            .contains("unreferenced uniform shift")
    );
}
