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
