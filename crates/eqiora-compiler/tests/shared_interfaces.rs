use eqiora_compiler::compile;

fn accepted(source: &str) {
    compile("interfaces.eqi", source).unwrap_or_else(|errors| panic!("{source}\n{errors:?}"));
}

#[test]
fn signature_parameters_and_borrowed_state_keep_their_owners() {
    accepted(
        "component C(parameter gain:1=2, state x:1) { relation r { derivative(x)=gain*x/1[s]; } } model M() { state x:1; initial {x=1;} instance c:C(x=x); }",
    );
}

#[test]
fn explicit_sample_and_hold_preserve_private_clock_owner() {
    accepted(
        "model M(output y:1) { clock tick=periodic(1[s]); state memory:1 at tick; initial {memory=0;} relation update at tick {next(memory)=sample(time/1[s],tick);} relation emit {y=hold(memory);} }",
    );
}

#[test]
fn signature_endpoints_can_reference_a_private_owned_clock() {
    accepted(
        "model M(output y:1 at tick) { clock tick=periodic(1[s]); state memory:1 at tick; initial {memory=0;} relation update at tick {next(memory)=1; y=pre(memory);} }",
    );
}

#[test]
fn transitions_reject_wrong_clocks_and_alias_hidden_crossings() {
    for equation in [
        "next(memory)=u",
        "next(memory)=sample(u,other)",
        "next(memory)=alias",
        "next(memory)=sample(memory,tick)",
    ] {
        let source = format!(
            "model M(input u:1) {{ clock tick=periodic(1[s]); clock other=periodic(1[s]); state memory:1 at tick; let alias=u; initial {{memory=0;}} relation update at tick {{{equation};}} }}"
        );
        assert!(compile("crossing.eqi", &source).is_err(), "{source}");
    }
    accepted(
        "model M(input u:1) { clock tick=periodic(1[s]); state memory:1 at tick; let alias=sample(u,tick); initial {memory=0;} relation update at tick {next(memory)=alias;} }",
    );
}

#[test]
fn borrowed_clock_period_projects_time_without_an_extra_parameter() {
    accepted(
        "component Driver(clock tick:periodic, output y:1/s at tick) {relation drive at tick {y=1[1/s];}} component Acc(clock tick:periodic, input u:1/s at tick, output y:1 at tick) { state memory:1 at tick; initial {memory=0;} relation update at tick {y=pre(memory); next(memory)=pre(memory)+period(tick)*u;} } model M() { clock tick=periodic(0.25[s]); instance driver:Driver(tick=tick); instance acc:Acc(tick=tick); connect driver.y -> acc.u; } ",
    );
}

#[test]
fn selected_root_keeps_exact_external_clock_identity() {
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    use eqiora_graph::Op;
    use eqiora_schema::kernel::{ClockDomainDef, KernelNode, RationalTime};
    let tick = ClockDomainDef::periodic(
        eqiora_core::Id::new(),
        RationalTime::new(1, 4).unwrap(),
        RationalTime::ZERO,
    )
    .unwrap();
    for source in [
        "model M(clock tick:periodic, input u:1 at tick, output y:1 at tick) { state memory:1 at tick; initial {memory=0;} relation update at tick {y=pre(memory);next(memory)=u;} }",
        "public component M(clock tick:periodic, input u:1 at tick, output y:1 at tick) { state memory:1 at tick; initial {memory=0;} relation update at tick {y=pre(memory);next(memory)=u;} }",
    ] {
        let model = CompiledModel::compile_selected(
            "selected.eqi",
            source,
            "M",
            &[("tick", StaticBindingValue::Clock(&tick))],
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"));
        assert_eq!(model.transaction().ops().iter().filter(|op|matches!(op,Op::DefineKernelNode {node: KernelNode::ClockDomain(clock)} if clock.id()==tick.id())).count(),1);
    }
}

#[test]
fn selected_shared_clock_aliases_and_conflicting_payloads_are_exact() {
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    use eqiora_schema::kernel::{ClockDomainDef, RationalTime};
    let shared = ClockDomainDef::periodic(
        eqiora_core::Id::new(),
        RationalTime::new(1, 4).unwrap(),
        RationalTime::ZERO,
    )
    .unwrap();
    let source = "model M(clock a:periodic,clock b:periodic,input u:1 at b,output y:1 at b) {relation equation at b {y=u;}}";
    let model = CompiledModel::compile_selected(
        "shared.eqi",
        source,
        "M",
        &[
            ("a", StaticBindingValue::Clock(&shared)),
            ("b", StaticBindingValue::Clock(&shared)),
        ],
    )
    .unwrap();
    assert_eq!(model.symbols().get("a"), Some(shared.id().erase()));
    assert_eq!(model.symbols().get("b"), Some(shared.id().erase()));
    let conflict = ClockDomainDef::periodic(
        shared.id(),
        RationalTime::new(1, 2).unwrap(),
        RationalTime::ZERO,
    )
    .unwrap();
    assert!(
        CompiledModel::compile_selected(
            "shared.eqi",
            source,
            "M",
            &[
                ("a", StaticBindingValue::Clock(&shared)),
                ("b", StaticBindingValue::Clock(&conflict))
            ]
        )
        .is_err()
    );
    let distinct = ClockDomainDef::periodic(
        eqiora_core::Id::new(),
        RationalTime::new(1, 4).unwrap(),
        RationalTime::ZERO,
    )
    .unwrap();
    let model = CompiledModel::compile_selected(
        "shared.eqi",
        source,
        "M",
        &[
            ("a", StaticBindingValue::Clock(&shared)),
            ("b", StaticBindingValue::Clock(&distinct)),
        ],
    )
    .unwrap();
    assert_ne!(model.symbols().get("a"), model.symbols().get("b"));
}
