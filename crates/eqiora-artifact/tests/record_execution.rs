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

#[test]
fn derived_numeric_member_retains_parameter_derivative_after_replay() {
    use eqiora_ir::{DifferentiationRole, LinearizedRelation, RelationTangent, ScalarOperatorIr};
    use eqiora_schema::kernel::{KernelNode, SymbolRef};
    let source = "record Config {gain:1,ready:bool} component C(parameter config:Config){variable y:1;relation law{y=config.gain;}} model M(){parameter p:1=3;instance c:C(config=Config(gain=2*p,ready=true));}";
    let compiled = compile("record-ad.eqi", source).unwrap().pop().unwrap();
    let (transaction, model, symbols) = compiled.into_parts();
    let parameter = symbols.get("p").unwrap();
    let relation = symbols.get("c.law").unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let bytes = ModelEnvelope::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap();
    let program = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default())
        .unwrap()
        .to_program()
        .unwrap();
    let Some(KernelNode::Relation(relation)) = program.node(relation) else {
        panic!("retained law");
    };
    let ir = ScalarOperatorIr::lower(relation.expression()).unwrap();
    let (point, roles): (Vec<_>, Vec<_>) = ir
        .symbols()
        .iter()
        .map(|symbol| match symbol {
            SymbolRef::Parameter(id) if id.erase() == parameter => {
                (3.0, DifferentiationRole::Parameter)
            }
            SymbolRef::Field(_) => (6.0, DifferentiationRole::Unknown),
            other => panic!("unexpected frozen or foreign input: {other:?}"),
        })
        .unzip();
    assert_eq!(
        roles
            .iter()
            .filter(|role| **role == DifferentiationRole::Parameter)
            .count(),
        1
    );
    let linearized = ir.linearize(&point, &roles).unwrap();
    let mut tangent = [f64::NAN; 2];
    linearized
        .jvp(RelationTangent::Parameter(&[1.0]), &mut tangent)
        .unwrap();
    // The retained ordered equation sides are y and 2*p. Their exact
    // derivatives at fixed y are 0 and 2; Boolean ready has no real channel.
    assert_eq!(tangent, [0.0, 2.0]);
}

const INTERACTION: &str = r"
record SensorBus {voltage:V, valid:bool}
record CommandBus {drive:V, enabled:bool}
component Sensor(clock tick:periodic,state voltage:V at tick,state valid:bool at tick) {
    relation sample at tick {next(voltage)=pre(voltage)+1[V];next(valid)=true;}
}
component Controller(clock tick:periodic,state voltage:V at tick,state valid:bool at tick,
                     state drive:V at tick,state enabled:bool at tick) {
    relation command at tick {
        next(drive)=if pre(valid) then 2*pre(voltage) else 0[V];
        next(enabled)=pre(valid);
    }
}
model Loop() {
    clock tick=periodic(1[s]);
    state sensor:SensorBus at tick;
    state command:CommandBus at tick;
    initial {sensor.voltage=1[V];sensor.valid=false;command.drive=0[V];command.enabled=false;}
    instance sample:Sensor(tick=tick,voltage=sensor.voltage,valid=sensor.valid);
    instance control:Controller(tick=tick,voltage=sensor.voltage,valid=sensor.valid,
                                drive=command.drive,enabled=command.enabled);
}
";

#[test]
fn components_exchange_sensor_and_command_bus_members_on_one_exact_clock() {
    let compiled = compile("bus-loop.eqi", INTERACTION).unwrap().pop().unwrap();
    let (transaction, model, symbols) = compiled.into_parts();
    let voltage = symbols.get("sensor.voltage").unwrap();
    let drive = symbols.get("command.drive").unwrap();
    let enabled = symbols.get("command.enabled").unwrap();
    let sample = symbols.get("sample.sample").unwrap();
    let control = symbols.get("control.command").unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    let bytes = ModelEnvelope::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap();
    let program = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default())
        .unwrap()
        .to_program()
        .unwrap();
    use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};
    // Borrowing creates no duplicate Fields, and both relations refer to the
    // exact sensor member retained by the nominal record instance.
    assert_eq!(
        program
            .nodes()
            .filter(|node| matches!(node, KernelNode::Field(_)))
            .count(),
        4
    );
    for relation_id in [sample, control] {
        let Some(KernelNode::Relation(relation)) = program.node(relation_id) else {
            panic!("retained component relation");
        };
        assert!(relation.expression().nodes().iter().any(
            |node| matches!(node, ExprNode::Symbol(SymbolRef::Pre(id)) if id.erase() == voltage)
        ));
    }
    let config = ReferenceConfig::new(2.0, 1.0)
        .unwrap()
        .with_nonlinear_tolerances(1.0e-12, 0.0)
        .unwrap();
    let interpreter = Interpreter::new();
    let mut session = interpreter.execution_session(&program, config, []).unwrap();
    // Controller reads the previous sample: invalid at the first tick, then
    // twice the preceding voltages 2 and 3. Sensor advances independently.
    for (sensor, command, valid) in [(2.0, 0.0, false), (3.0, 4.0, true), (4.0, 6.0, true)] {
        assert_eq!(session.advance_ticks(1).unwrap(), 1);
        for (field, expected) in [(voltage, sensor), (drive, command)] {
            let actual = session
                .field(field)
                .unwrap()
                .real_scalar_value()
                .unwrap()
                .value();
            assert!((actual - expected).abs() <= 8.0e-12);
        }
        assert_eq!(session.field(enabled).unwrap().as_bool(), Some(valid));
        session = interpreter
            .resume_execution(&program, &session.checkpoint())
            .unwrap();
    }
    for invalid in [
        INTERACTION
            .replace(
                "clock tick=periodic(1[s]);",
                "clock tick=periodic(1[s]);clock other=periodic(1[s]);",
            )
            .replace(
                "control:Controller(tick=tick",
                "control:Controller(tick=other",
            ),
        INTERACTION.replace("voltage=sensor.voltage", "voltage=sensor.valid"),
        INTERACTION.replace("voltage=sensor.voltage", "voltage=sensor.missing"),
        INTERACTION.replace("voltage=sensor.voltage", "voltage=2*sensor.voltage"),
    ] {
        assert!(
            compile("invalid-bus-binding.eqi", &invalid).is_err(),
            "{invalid}"
        );
    }
}
