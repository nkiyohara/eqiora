//! Ordinary authored buses retain heterogeneous state through artifact replay.
use eqiora_artifact::{ModelDecoderLimits, ModelEnvelope};
use eqiora_compiler::compile;
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};

const SOURCE: &str = "enum Mode {Off,On} record Bus {voltage:V, mode:Mode, valid:bool} model Sensor(){clock tick=periodic(1[s]);state bus:Bus at tick;initial{bus.voltage=0[V];bus.mode=Mode.Off;bus.valid=false;}relation update at tick{next(bus.voltage)=pre(bus.voltage)+1[V];next(bus.mode)=case pre(bus.mode){Mode.Off=>Mode.On,Mode.On=>Mode.Off};next(bus.valid)=true;}}";

#[test]
fn typed_bus_executes_and_resumes_with_exact_discrete_members() {
    let compiled = compile("sensor.eqi", SOURCE).unwrap().pop().unwrap();
    let (transaction, model, symbols) = compiled.into_parts();
    let voltage = symbols.get("bus.voltage").unwrap();
    let mode = symbols.get("bus.mode").unwrap();
    let valid = symbols.get("bus.valid").unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let original = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let bytes = ModelEnvelope::from_program(&original)
        .unwrap()
        .canonical_json()
        .unwrap();
    let program = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default())
        .unwrap()
        .to_program()
        .unwrap();
    const RESIDUAL_TOLERANCE: f64 = 1.0e-12;
    let config = ReferenceConfig::new(2.0, 1.0)
        .unwrap()
        .with_nonlinear_tolerances(RESIDUAL_TOLERANCE, 0.0)
        .unwrap();
    let interpreter = Interpreter::new();
    let mut session = interpreter.execution_session(&program, config, []).unwrap();
    assert_eq!(session.field(mode).unwrap().enum_tag(), Some(0));
    assert_eq!(session.field(valid).unwrap().as_bool(), Some(false));
    for tick in 0..3 {
        assert_eq!(session.advance_ticks(1).unwrap(), 1);
        // Each tick adds one volt, toggles Off/On, and sets valid. Three
        // unit-coefficient updates accumulate fewer than eight residual errors.
        let actual = session
            .field(voltage)
            .unwrap()
            .real_scalar_value()
            .unwrap()
            .value();
        assert!((actual - f64::from(tick + 1)).abs() <= 8.0 * RESIDUAL_TOLERANCE);
        assert_eq!(
            session.field(mode).unwrap().enum_tag(),
            Some((tick + 1) % 2)
        );
        assert_eq!(session.field(valid).unwrap().as_bool(), Some(true));
        let checkpoint = session.checkpoint();
        session = interpreter.resume_execution(&program, &checkpoint).unwrap();
    }
}

#[test]
fn record_member_temporal_contracts_cannot_be_laundered_by_selection() {
    for invalid in [
        SOURCE
            .replace(
                "clock tick=periodic(1[s]);",
                "clock tick=periodic(1[s]);clock other=periodic(2[s]);",
            )
            .replace("relation update at tick", "relation update at other"),
        SOURCE.replace("next(bus.voltage)", "derivative(bus.voltage)"),
        SOURCE.replace("Mode.Off=>Mode.On,Mode.On=>Mode.Off", "Mode.Off=>Mode.On"),
    ] {
        assert!(compile("invalid-bus.eqi", &invalid).is_err(), "{invalid}");
    }
}
