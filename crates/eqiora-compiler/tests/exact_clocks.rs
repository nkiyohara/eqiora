use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::{ClockKind, KernelNode, RationalTime};

#[test]
fn flat_and_component_clocks_retain_exact_period_and_phase() {
    for source in [
        "model M() { clock tick = periodic(10[ms], phase=1[s]/3); state x:1 at tick; relation r at tick { next(x)=pre(x); } }",
        "component C() { clock tick = periodic(10[ms], phase=1[s]/3); state x:1 at tick; relation r at tick { next(x)=pre(x); } } model M() { instance c:C(); }",
    ] {
        let compiled = compile("clock.eqi", source).unwrap();
        let clocks = compiled[0]
            .transaction()
            .ops()
            .iter()
            .filter_map(|op| match op {
                Op::DefineKernelNode {
                    node: KernelNode::ClockDomain(clock),
                } => match clock.kind() {
                    ClockKind::Periodic { period, phase } => Some((period, phase)),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            clocks,
            [(
                RationalTime::new(1, 100).unwrap(),
                RationalTime::new(1, 3).unwrap()
            )]
        );
    }
}
