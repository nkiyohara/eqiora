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

#[test]
fn selected_property_values_require_the_exact_release_owner() {
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    use eqiora_lang::{ExprKind, NamePath, SourceAstFactory as F, TextRange};
    let range = TextRange::default();
    let declarations = "property contract Gain():1 {derivatives value_only;} property contract Other():1 {derivatives value_only;} property release Measured implements Gain {value=2;source_unit:1=1;validity=unconditional;citation=org.example.measurement;license=spdx.CC0_1_0;} property release Wrong implements Other {value=2;source_unit:1=1;validity=unconditional;citation=org.example.measurement;license=spdx.CC0_1_0;} material composition Material {property gain=Measured;}";
    for container in ["model M", "public component M"] {
        let source = format!(
            "{declarations} {container}(property gain:Gain,output y:1) {{relation equation {{y=gain;}}}}"
        );
        for reference in ["Measured", "Material.gain"] {
            let path = NamePath::from_segments(reference.split('.'), range).unwrap();
            let value = F::expression(
                if path.is_qualified() {
                    ExprKind::Path(path)
                } else {
                    ExprKind::Name(reference.to_owned())
                },
                range,
            )
            .unwrap();
            CompiledModel::compile_selected(
                "property.eqi",
                &source,
                "M",
                &[("gain", StaticBindingValue::Expression(&value))],
            )
            .unwrap_or_else(|errors| panic!("{errors:?}"));
        }
        for kind in [ExprKind::Number(2.0), ExprKind::Name("Wrong".to_owned())] {
            let value = F::expression(kind, range).unwrap();
            assert!(
                CompiledModel::compile_selected(
                    "property.eqi",
                    &source,
                    "M",
                    &[("gain", StaticBindingValue::Expression(&value))]
                )
                .is_err()
            );
        }
    }
}

#[test]
fn borrowed_alias_clocks_are_checked_at_the_exact_occurrence() {
    use eqiora_compiler::{CompiledModel, StaticBindingValue};
    use eqiora_schema::kernel::{ClockDomainDef, RationalTime};
    let clock = || {
        ClockDomainDef::periodic(
            eqiora_core::Id::new(),
            RationalTime::new(1, 4).unwrap(),
            RationalTime::ZERO,
        )
        .unwrap()
    };
    let shared = clock();
    let distinct = clock();
    for container in ["model M", "public component M"] {
        for expression in ["memory", "memory + other", "forward"] {
            let source = format!(
                "{container}(clock first:periodic,clock second:periodic) {{state memory:1 at first;state other:1 at second;variable observed:1 at first;initial {{pre(memory)=1;pre(other)=2;}}relation hold1 at first {{next(memory)=pre(memory);observed=current;}}relation hold2 at second {{next(other)=pre(other);}} let current at second={expression};let forward=memory;}}"
            );
            CompiledModel::compile_selected(
                "clock.eqi",
                &source,
                "M",
                &[
                    ("first", StaticBindingValue::Clock(&shared)),
                    ("second", StaticBindingValue::Clock(&shared)),
                ],
            )
            .unwrap_or_else(|e| panic!("{e:?}"));
            let errors = CompiledModel::compile_selected(
                "clock.eqi",
                &source,
                "M",
                &[
                    ("first", StaticBindingValue::Clock(&shared)),
                    ("second", StaticBindingValue::Clock(&distinct)),
                ],
            )
            .unwrap_err();
            assert!(
                errors.iter().any(|error| error
                    .message()
                    .contains("exact occurrence dependency clock")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn closed_and_selected_compilation_share_local_property_admission() {
    use eqiora_compiler::CompiledModel;
    let source = "property contract Gain():1 {derivatives value_only;} property release Measured implements Gain {value=2;source_unit:1=1;validity=unconditional;citation=org.example.measurement;license=spdx.CC0_1_0;} material composition Material {property gain=Measured;} component Amplifier(property gain:Gain,output y:1) {relation value {y=gain;}} model First() {instance amplifier:Amplifier(gain=Material.gain);} model Second() {instance amplifier:Amplifier(gain=Measured);}";
    let closed = eqiora_compiler::compile("local.eqi", source).unwrap();
    assert_eq!(closed.len(), 2);
    for (model, entry) in closed.iter().zip(["First", "Second"]) {
        let selected = CompiledModel::compile_selected("local.eqi", source, entry, &[]).unwrap();
        assert_eq!(model.model(), selected.model());
    }
    let invalid = source.replace("gain=Material.gain", "gain=2");
    assert!(eqiora_compiler::compile("local.eqi", &invalid).is_err());
    assert!(CompiledModel::compile_selected("local.eqi", &invalid, "First", &[]).is_err());
}

#[test]
fn directed_connections_retain_wrapper_endpoints_and_allow_fanout() {
    let source = "component Identity(input u:1,output y:1) {relation value {y=u;}} component Wrapper(input u:1,output y:1) {instance inner:Identity();connect u -> inner.u;connect inner.y -> y;} model M(input u:1,output first:1,output second:1) {instance a:Wrapper();instance b:Wrapper();connect u -> a.u;connect u -> b.u;connect a.y -> first;connect b.y -> second;}";
    let models = eqiora_compiler::compile("relay.eqi", source).unwrap_or_else(|e| panic!("{e:?}"));
    let model = &models[0];
    for name in [
        "u",
        "first",
        "second",
        "a.u",
        "a.y",
        "a.inner.u",
        "a.inner.y",
        "b.u",
        "b.y",
    ] {
        assert!(
            model.symbols().get(name).is_some(),
            "missing retained endpoint {name}"
        );
    }
    let invalid = source.replace("connect b.y -> second;", "connect b.y -> first;");
    assert!(eqiora_compiler::compile("relay.eqi", &invalid).is_err());
    let reversed = source.replace("connect u -> inner.u;", "connect inner.u -> u;");
    assert!(eqiora_compiler::compile("relay.eqi", &reversed).is_err());
}

#[test]
fn clocked_variables_have_tick_local_reads_without_state_privileges() {
    let source = "component Read(clock tick:periodic,variable observed:1 at tick,output y:1 at tick) {relation copy at tick {y=observed;}} model M() {clock tick=periodic(1[s]);clock other=periodic(1[s]);variable x:1 at tick;let alias=x;relation value at tick {x=1;}instance read:Read(tick=tick,observed=x);relation inspect at tick {alias=1;}}";
    eqiora_compiler::compile("variable.eqi", source).unwrap_or_else(|e| panic!("{e:?}"));
    for relation in [
        "relation inspect at other {alias=1;}",
        "relation inspect {alias=1;}",
        "initial {alias=1;}",
        "relation inspect at tick {pre(alias)=1;}",
    ] {
        let invalid = source.replace("relation inspect at tick {alias=1;}", relation);
        assert!(
            eqiora_compiler::compile("variable.eqi", &invalid).is_err(),
            "{relation}"
        );
    }
}
