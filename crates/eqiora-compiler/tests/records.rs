use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::KernelNode;

#[test]
fn clocked_record_bus_retains_nominal_declaration_and_exact_ordered_leaves() {
    let source = "enum Mode {Off,On} record Bus {voltage: V, mode: Mode} model M(){clock tick=periodic(1[s]);state bus:Bus at tick;initial{bus.voltage=0[V];bus.mode=Mode.Off;}relation update at tick{next(bus.voltage)=pre(bus.voltage)+1[V];next(bus.mode)=case pre(bus.mode){Mode.Off=>Mode.On,Mode.On=>Mode.Off};}}";
    let models = compile("bus.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let model = &models[0];
    let nodes = model
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode { node } => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();
    let record = nodes
        .iter()
        .find_map(|node| match node {
            KernelNode::Record(record) => Some(record),
            _ => None,
        })
        .unwrap();
    let instance = nodes
        .iter()
        .find_map(|node| match node {
            KernelNode::RecordInstance(instance) => Some(instance),
            _ => None,
        })
        .unwrap();
    assert_eq!(instance.definition(), record.id());
    assert_eq!(
        instance
            .expression()
            .roots()
            .iter()
            .map(|root| {
                match instance.expression().node(*root).unwrap() {
                    eqiora_schema::kernel::ExprNode::Symbol(
                        eqiora_schema::kernel::SymbolRef::Field(id),
                    ) => id.erase(),
                    node => panic!("bus root must retain its exact Field: {node:?}"),
                }
            })
            .collect::<Vec<_>>(),
        vec![
            model.symbols().get("bus.voltage").unwrap(),
            model.symbols().get("bus.mode").unwrap()
        ]
    );
    for invalid in [
        source.replace("pre(bus.voltage)", "pre(bus.unknown)"),
        source.replace("bus.mode=Mode.Off", "bus.mode=1"),
    ] {
        assert!(compile("bus.eqi", &invalid).is_err(), "{invalid}");
    }
}

#[test]
fn component_record_buses_keep_distinct_occurrence_members() {
    let source = "record Bus {value:V, valid:bool} component Sensor(){clock tick=periodic(1[s]);state bus:Bus at tick;initial{bus.value=0[V];bus.valid=false;}relation update at tick{next(bus.value)=pre(bus.value)+1[V];next(bus.valid)=true;}} model M(){instance a:Sensor();instance b:Sensor();}";
    let models = compile("components.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let symbols = models[0].symbols();
    for member in ["value", "valid"] {
        assert_ne!(
            symbols.get(&format!("a.bus.{member}")).unwrap(),
            symbols.get(&format!("b.bus.{member}")).unwrap()
        );
    }
}

#[test]
fn static_record_parameters_preserve_nominal_forwarding_and_derived_roots() {
    let source = "record Config {gain:1, ready:bool} component Sink(parameter config:Config){variable y:1;relation law{y=config.gain;}} component Forward(parameter config:Config){instance sink:Sink(config=config);} model M(){parameter p:1=3;instance c:Forward(config=Config(ready=true,gain=2*p));}";
    let models = compile("static-record.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let model = &models[0];
    let parameter = model.symbols().get("p").unwrap();
    let instances = model
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::RecordInstance(instance),
            } => Some(instance),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    for instance in instances {
        assert!(instance.expression().nodes().iter().any(|node| matches!(node,eqiora_schema::kernel::ExprNode::Symbol(eqiora_schema::kernel::SymbolRef::Parameter(id)) if id.erase()==parameter)));
        assert!(
            instance
                .expression()
                .nodes()
                .iter()
                .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::Mul(..)))
        );
    }
    assert!(model.symbols().get("c.config.gain").is_none());
    for invalid in [
        source.replace("ready=true,gain=2*p", "gain=2*p"),
        source.replace("ready=true,gain=2*p", "ready=true,gain=2*p,extra=0"),
        source.replace("ready=true,gain=2*p", "ready=true,gain=2*p,gain=3"),
        source.replace("ready=true,gain=2*p", "ready=1,gain=2*p"),
        source
            .replace(
                "record Config",
                "record Other {gain:1,ready:bool} record Config",
            )
            .replace("config=Config(", "config=Other("),
    ] {
        assert!(
            compile("invalid-static.eqi", &invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn model_record_defaults_become_ordered_independently_editable_parameter_members() {
    let source = "record Config {gain:1,ready:bool} model M(){parameter config:Config=Config(ready=true,gain=4);variable y:1;relation law{y=config.gain;}}";
    let models = compile("model-config.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    for name in ["config.gain", "config.ready"] {
        assert!(models[0].symbols().get(name).is_some());
    }
}

#[test]
fn record_member_alias_retains_exact_evolution_target() {
    let source = "record Bus {value:1} model M(){clock tick=periodic(1[s]);state bus:Bus at tick;let value=bus.value;initial{bus.value=0;}relation update at tick{next(value)=pre(value)+1;}}";
    compile("record-alias.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn selected_model_record_binding_reconstructs_exact_closed_members() {
    let values = eqiora_lang::parse(
        "value.eqi",
        "model Value(){parameter config:Config=Config(gain=4,ready=true);}",
    )
    .into_document()
    .unwrap();
    let eqiora_lang::Item::Parameter(value) = &values.models()[0].items()[0] else {
        panic!("Parameter");
    };
    let model=eqiora_compiler::CompiledModel::compile_selected(
        "selected-record.eqi",
        "record Config {gain:1,ready:bool} model M(parameter config:Config){variable y:1;relation r{y=config.gain;}}",
        "M", &[("config",eqiora_compiler::StaticBindingValue::Expression(value.value()))],
    ).unwrap_or_else(|errors|panic!("{errors:?}"));
    assert!(model.symbols().get("config.gain").is_some());
    assert!(model.symbols().get("config.ready").is_some());
}

#[test]
fn finite_mode_record_checks_every_case_before_compile_time_selection() {
    let source = "enum Mode{Off,On} record Config{mode:Mode} component C(parameter config:Config){variable y:1;relation law{y=case config.mode{Mode.Off=>0,Mode.On=>1};}} model M(){instance a:C(config=Config(mode=Mode.On));}";
    let models = compile("record-mode.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(models[0].symbols().get("a.law").is_some());
    for invalid in [
        source.replace("Mode.Off=>0", "Mode.Off=>1[V]"),
        source.replace("Mode.Off=>0,", ""),
    ] {
        assert!(compile("invalid-mode.eqi", &invalid).is_err(), "{invalid}");
    }
}

#[test]
fn module_aliases_share_exact_record_identity_and_private_types_stay_hidden() {
    use eqiora_compiler::{
        CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit,
        analyze_resolved_hierarchy,
    };
    let root = "import org.example.records.types as first;import org.example.records.types as second;model Main(){instance c:first.Sink(config=second.Config(value=2));}";
    let library = "public record Config{value:1} public record Other{value:1} record Hidden{value:1} public component Sink(parameter config:Config){variable y:1;relation law{y=config.value;}}";
    let compile_modules = |source: &str| {
        let owner =
            CompilationNamespaceId::new(["org.example.records", "0.1.0", "record-test"]).unwrap();
        let input = ResolvedHierarchyInput::new(
            owner.clone(),
            vec![
                ResolvedSourceUnit::new(owner.clone(), "src/main.eqi", source).unwrap(),
                ResolvedSourceUnit::new(owner, "src/types.eqi", library).unwrap(),
            ],
            vec![],
        );
        analyze_resolved_hierarchy(input)?
            .validate_definitions()?
            .compile_root("Main")
    };
    compile_modules(root).unwrap_or_else(|errors| panic!("{errors:?}"));
    for invalid in [
        root.replace("second.Config(", "second.Other("),
        root.replace("second.Config(", "second.Hidden("),
    ] {
        assert!(compile_modules(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn shaped_complex_record_members_keep_dimensions_channels_and_scalar_selection() {
    let source = "record Payload{samples:array<complex<V>,2>,valid:bool} model M(){parameter data:Payload=Payload(samples=[math.complex(1,4),math.complex(2,-3)],valid=true);variable y:complex<V>;relation law{y=data.samples[1];}}";
    let models = compile("record-shape.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let id = models[0].symbols().get("data.samples").unwrap();
    let values = models[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|op| match op {
            Op::DefineKernelNode {
                node: KernelNode::Parameter(value),
            } if value.id().erase() == id => Some(value.value()),
            _ => None,
        })
        .unwrap();
    assert_eq!(values.component_count(), 2);
    assert_eq!(values.component(0), Some((1.0, 4.0)));
    assert_eq!(values.component(1), Some((2.0, -3.0)));
    assert!(values.real_scalar_value().is_none());
    assert!(
        compile(
            "bad-shape.eqi",
            &source.replace("array<complex<V>,2>", "array<complex<V>,3>")
        )
        .is_err()
    );
}
