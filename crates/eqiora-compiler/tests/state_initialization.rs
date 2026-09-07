use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{FieldRole, KernelNode};

fn nodes(source: &str) -> Vec<KernelNode> {
    compile("state.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}\n{source}"))[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::DefineKernelNode { node } => Some(node.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn algebraic_state_support_and_initial_owners_are_independent() {
    let result = nodes(
        "model M { domain body = box(0,1); variable a: 1; state x: 1; variable p: 1 on body; state t: 1 on body; initial { x = 1; t = 0; } relation law { derivative(x) = 0; a = x; } }",
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Field(f) if f.role() == FieldRole::State))
            .count(),
        2
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Field(f) if f.role() == FieldRole::Variable))
            .count(),
        2
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Representation(_)))
            .count(),
        2
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Relation(r) if r.is_initial()))
            .count(),
        1
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Activation(_)))
            .count(),
        1
    );
}

#[test]
fn only_declared_continuous_states_have_authored_time_derivatives() {
    nodes(
        "model M { state x: 1; initial { derivative(x) = 0; } relation law { derivative(x) = 0; } }",
    );
    for source in [
        "model M { variable x: 1; relation law { derivative(x) = 0; } }",
        "model M { variable x: 1; initial { derivative(x) = 0; } }",
        "model M { clock c = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 at c; relation law { derivative(x) = 0; } }",
    ] {
        assert!(compile("bad.eqi", source).is_err(), "{source}");
    }
}

#[test]
fn clocked_state_initializes_pre_and_requires_exact_clock() {
    nodes(
        "model M { domain body = box(0,1); clock c = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 on body at c; initial { pre(x) = 0; } relation law on body at c { next(x) = pre(x); } }",
    );
    for source in [
        "model M { clock c = periodic(period = 1 / 1, phase = 0 / 1); clock d = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 at c; relation law at d { next(x) = pre(x); } }",
        "model M { clock c = periodic(period = 1 / 1, phase = 0 / 1); variable x: 1 at c; relation law at c { next(x) = pre(x); } }",
        "model M { clock c = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 at c; initial { next(x) = 0; } }",
    ] {
        assert!(compile("bad.eqi", source).is_err(), "{source}");
    }
}

#[test]
fn borrowed_roles_preserve_target_and_cannot_launder_state_eligibility() {
    let source = "component Reader(variable value: 1) { relation law { value = 0; } } model M { state x: 1; instance r: Reader(field value = x); }";
    assert_eq!(
        nodes(source)
            .iter()
            .filter(|n| matches!(n, KernelNode::Field(_)))
            .count(),
        1
    );
    for source in [
        "component Writer(state value: 1) { relation law { derivative(value) = 0; } } model M { variable x: 1; instance w: Writer(field value = x); }",
        "component Writer(state value: 1) { relation law { derivative(value) = 0; } } component Forward(variable value: 1) { instance w: Writer(field value = value); } model M { state x: 1; instance f: Forward(field value = x); }",
        "component Reader(variable value: 1) { relation law { derivative(value) = 0; } } model M { state x: 1; instance r: Reader(field value = x); }",
    ] {
        assert!(compile("bad.eqi", source).is_err(), "{source}");
    }
}

#[test]
fn borrowed_clocks_and_states_forward_exact_targets() {
    let source = "component Delay(clock tick, state value: 1 at tick) { initial { pre(value) = 0; } relation law at tick { next(value) = pre(value); } } component Forward(clock tick, state value: 1 at tick) { instance d: Delay(clock tick = tick, field value = value); } model M { clock c = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 at c; instance f: Forward(clock tick = c, field value = x); }";
    let result = nodes(source);
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::Field(_)))
            .count(),
        1
    );
    assert_eq!(
        result
            .iter()
            .filter(|n| matches!(n, KernelNode::ClockDomain(_)))
            .count(),
        1
    );
    let wrong = source
        .replace("clock tick = c, field", "clock tick = other, field")
        .replace(
            "state x: 1 at c;",
            "clock other = periodic(period = 1 / 1, phase = 0 / 1); state x: 1 at c;",
        );
    assert!(compile("wrong.eqi", &wrong).is_err());
}
