use eqiora::compiler::compile;
use eqiora::graph::{GraphStore, InMemoryGraphStore};
use eqiora::runtime::{CpuExecutor, CpuProgram};
use eqiora::sem::{Interpreter, KernelProgram, ReferenceConfig};

#[test]
fn source_lowers_to_the_reference_thermal_controller_trajectory() {
    let source = r#"
model thermal_controller {
  field temperature: K = 293;
  field command: 1 = 0;
  parameter ambient: K = 293;
  parameter tau: s = 10;
  parameter heating_gain: K / s = 2;
  parameter setpoint: K = 300;
  parameter controller_gain: 1 / K = 0.1;
  port control_out: signal output 1;
  port control_in: signal input 1;

  clock control = periodic(period = 1 / 1, phase = 0 / 1);

  relation plant continuous {
    derivative(temperature)
      - ((ambient - temperature) / tau + heating_gain * control_in) = 0;
  }

  relation controller periodic(control) {
    next(command) - controller_gain * (setpoint - temperature) = 0;
    control_out - next(command) = 0;
  }

  connect signal control_out -> control_in;
}
"#;
    let mut models = compile("thermal_controller.eqi", source).expect("typed source");
    let compiled = models.pop().expect("one model");
    let temperature = compiled
        .symbols()
        .get("temperature")
        .expect("temperature ID");
    let command = compiled.symbols().get("command").expect("command ID");
    let (transaction, model, _) = compiled.into_parts();

    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).expect("atomic source commit");
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).expect("whole model");
    let config = ReferenceConfig::new(0.5, 0.1).expect("reference config");
    let trajectory = Interpreter::new()
        .run(&program, config)
        .expect("reference execution");
    let cpu_program = CpuProgram::lower(&program).expect("Operator IR lowering");
    let cpu_trajectory = CpuExecutor::new()
        .run(&cpu_program, config)
        .expect("CPU execution");

    assert_eq!(cpu_trajectory, trajectory);

    let expected_temperature = 307.0 - 14.0 / 1.01_f64.powi(5);
    assert!(
        (trajectory
            .last_value(temperature)
            .expect("temperature sample")
            .value()
            - expected_temperature)
            .abs()
            < 1.0e-8
    );
    assert!(
        (trajectory
            .last_value(command)
            .expect("command sample")
            .value()
            - 0.7)
            .abs()
            < 1.0e-10
    );
}
