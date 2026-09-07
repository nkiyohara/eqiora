use eqiora_compiler::{compile, lower_model};
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

#[test]
fn invalid_unused_clock_definitions_keep_operand_ranges() {
    for container in ["model M()", "component Unused()"] {
        let source = format!(
            "{container} {{ clock tick = periodic(1[s] + 1[m]); }} model Root() {{ variable x:1; relation r {{ x=0; }} }}"
        );
        let errors = compile("clock.eqi", &source).unwrap_err();
        let error = errors
            .iter()
            .find(|error| error.message().contains("equal dimensions"))
            .expect("dimension diagnostic");
        let span = error.source_span().unwrap();
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "1[s] + 1[m]"
        );
    }
    let source = "model M() { clock tick = periodic(1[s] + 1[m]); }";
    let document = eqiora_lang::parse("clock.eqi", source)
        .into_document()
        .unwrap();
    let errors = lower_model("clock.eqi", &document.models()[0]).unwrap_err();
    let span = errors
        .iter()
        .find(|error| error.message().contains("equal dimensions"))
        .unwrap()
        .source_span()
        .unwrap();
    assert_eq!(
        &source[span.start as usize..span.end as usize],
        "1[s] + 1[m]"
    );
}
