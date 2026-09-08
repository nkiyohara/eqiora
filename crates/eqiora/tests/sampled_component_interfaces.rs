use eqiora::compiler::compile;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::kernel::RationalTime;
use eqiora::sem::{Interpreter, KernelProgram, ReferenceConfig};

const SOURCE: &str = r#"
component Delay(
  clock tick: periodic,
  parameter initial_value: V,
  input u: V at tick,
  output y: V at tick
) {
  state memory: V at tick;
  initial { memory = initial_value; }
  relation update at tick {
    y = pre(memory);
    next(memory) = u;
  }
}
component Accumulator(
  clock tick: periodic,
  parameter initial_value: V,
  input rate: V / s at tick,
  output y: V at tick
) {
  state memory: V at tick;
  initial { memory = initial_value; }
  relation update at tick {
    next(memory) = pre(memory) + period(tick) * rate;
    y = next(memory);
  }
}
component Driver(
  clock tick: periodic,
  output first: V at tick,
  output second: V at tick,
  output rate: V / s at tick
) {
  relation values at tick {
    first = 2[V];
    second = 7[V];
    rate = 2[V / s];
  }
}
model ClosedPair(
  output first: V at tick,
  output second: V at tick,
  output integrated: V at tick
) {
  clock tick = periodic(0.25[s]);
  instance driver: Driver(tick = tick);
  instance first_delay: Delay(tick = tick, initial_value = 5[V]);
  instance second_delay: Delay(tick = tick, initial_value = -3[V]);
  instance accumulator: Accumulator(tick = tick, initial_value = 1[V]);
  connect driver.first -> first_delay.u;
  connect driver.second -> second_delay.u;
  connect driver.rate -> accumulator.rate;
  relation expose at tick {
    first = first_delay.y;
    second = second_delay.y;
    integrated = accumulator.y;
  }
}
"#;

#[test]
fn two_delays_and_an_integrator_follow_independent_recurrences_across_restart() {
    let compiled = compile("sampled-components.eqi", SOURCE)
        .expect("ordinary source interfaces")
        .pop()
        .expect("one closed model");
    let (transaction, model, symbols) = compiled.into_parts();
    let outputs = ["first", "second", "integrated"].map(|name| symbols.get(name).unwrap());
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();

    const RESIDUAL_TOLERANCE: f64 = 1.0e-12;
    let config = ReferenceConfig::new(0.5, 0.25)
        .unwrap()
        .with_nonlinear_tolerances(RESIDUAL_TOLERANCE, 0.0)
        .unwrap();
    let interpreter = Interpreter::new();
    let mut session = interpreter.execution_session(&program, config, []).unwrap();
    assert!(outputs.iter().all(|id| session.output(*id, 0).is_none()));
    assert_eq!(session.advance_ticks(2).unwrap(), 2);

    // Independent recurrence: each delay publishes its own old memory; the
    // accumulator adds (1/4 s)*(2 V/s)=1/2 V, including at tick zero.
    let expected = [[5.0, -3.0, 1.5], [2.0, 7.0, 2.0], [2.0, 7.0, 2.5]];
    let checkpoint = session.checkpoint();
    let mut resumed = interpreter.resume_execution(&program, &checkpoint).unwrap();
    assert_eq!(resumed.advance_ticks(1).unwrap(), 1);
    for (tick, values) in expected.into_iter().enumerate() {
        let observed_session = if tick < 2 { &session } else { &resumed };
        for (output, expected) in outputs.into_iter().zip(values) {
            let (instant, value) = observed_session.output(output, tick as u64).unwrap();
            assert_eq!(instant, RationalTime::new(tick as u64, 4).unwrap());
            let actual = value.real_scalar_value().unwrap().value();
            // Unit-coefficient paths through three ticks accumulate fewer than
            // 32 absolute residual errors; no observed trajectory sets this bound.
            assert!((actual - expected).abs() <= 32.0 * RESIDUAL_TOLERANCE);
        }
    }
}
