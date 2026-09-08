//! Explicit Sample/Hold uses initialized State memory and simultaneous tick equations.
use eqiora::api::ModelDocument;
use eqiora::compiler::{ModelSymbols, compile};
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::runtime::{CpuExecutor, CpuProgram};
use eqiora::sem::{Interpreter, KernelProgram, ReferenceConfig, Trajectory};

const SOURCE: &str = r#"
model Ramp() {
  clock tick = periodic(1[s], phase = 0.5[s]);
  state x: V;
  state memory: V at tick;
  variable held: V;
  variable before: V at tick;
  variable after: V at tick;
  initial { x = 0[V]; memory = -1[V]; }
  relation ramp { derivative(x) = 1[V/s]; }
  relation output { held = hold(memory); }
  relation update at tick {
    next(memory) = sample(x, tick);
    before = pre(memory);
    after = next(memory);
  }
}
"#;

fn admit(source: &str) -> (KernelProgram, ModelSymbols) {
    let compiled = compile("temporal-conversions.eqi", source)
        .unwrap()
        .pop()
        .unwrap();
    let (transaction, model, symbols) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    (
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap(),
        symbols,
    )
}

fn at(trajectory: &Trajectory, symbols: &ModelSymbols, name: &str, time: f64) -> f64 {
    let field = symbols.get(name).unwrap();
    trajectory
        .samples()
        .iter()
        .find(|sample| sample.field() == field && sample.time() == time)
        .unwrap_or_else(|| panic!("missing {name} at {time}"))
        .value()
        .value()
}

#[test]
fn delayed_sample_and_hold_observe_initialized_pre_and_committed_post_values() {
    let (program, symbols) = admit(SOURCE);
    // Quarter-second steps and half-second phase are exactly binary representable.
    let config = ReferenceConfig::new(1.75, 0.25).unwrap();
    let reference = Interpreter::new().run(&program, config).unwrap();
    let cpu = CpuExecutor::new()
        .run(&CpuProgram::lower(&program).unwrap(), config)
        .unwrap();
    assert_eq!(reference, cpu);
    // x(t)=t V. No tick occurs before0.5; memory then changes to0.5 and1.5 V.
    for (time, expected) in [
        (0.0, -1.0),
        (0.25, -1.0),
        (0.5, 0.5),
        (0.75, 0.5),
        (1.25, 0.5),
        (1.5, 1.5),
        (1.75, 1.5),
    ] {
        assert!((at(&reference, &symbols, "held", time) - expected).abs() < 1e-10);
    }
    for (time, pre, post) in [(0.5, -1.0, 0.5), (1.5, 0.5, 1.5)] {
        assert!((at(&reference, &symbols, "before", time) - pre).abs() < 1e-10);
        assert!((at(&reference, &symbols, "after", time) - post).abs() < 1e-10);
    }
}

#[test]
fn missing_hold_memory_initialization_rejects_before_execution() {
    let (program, _) = admit(&SOURCE.replace(" memory = -1[V];", ""));
    let config = ReferenceConfig::new(0.25, 0.25).unwrap();
    assert!(Interpreter::new().initialize(&program, config).is_err());
    assert!(Interpreter::new().run(&program, config).is_err());
}

#[test]
fn sampling_phase_is_model_meaning_while_numerical_steps_are_not() {
    let document = ModelDocument::compile("temporal-conversions.eqi", SOURCE).unwrap();
    let changed = ModelDocument::compile(
        "temporal-conversions.eqi",
        &SOURCE.replace("phase = 0.5[s]", "phase = 0.75[s]"),
    )
    .unwrap();
    assert!(!document.structurally_equivalent(&changed).unwrap());
    let bytes = document.canonical_json().unwrap();
    let identity = document.artifact_reference().unwrap();
    for step in [0.25, 0.125] {
        Interpreter::new()
            .run(
                document.program(),
                ReferenceConfig::new(1.75, step).unwrap(),
            )
            .unwrap();
        assert_eq!(document.artifact_reference().unwrap(), identity);
        assert_eq!(document.canonical_json().unwrap(), bytes);
    }
}
