use eqiora_compiler::compile;

const CELL: &str =
    "component Cell(clock tick:periodic, output y:1 at tick) { relation r at tick { y=1; } }";
const CLOCKS: &str = "clock a=periodic(1[s]); clock b=periodic(1[s]);";

fn selected_source(selection: &str, asserted: &str) -> String {
    format!(
        "{CELL} model M() {{ {CLOCKS} indexset Stages=range(2); indexset Other=range(2); instance cells[i in Stages]:Cell(tick=a); let observed at {asserted}={selection}; relation r at a {{ observed=1; }} }}"
    )
}

#[test]
fn indexed_output_alias_retains_exact_declared_clock() {
    let source = selected_source("cells[index(Stages,0)].y", "a");
    compile("indexed-clock.eqi", &source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let errors = compile(
        "indexed-clock.eqi",
        &selected_source("cells[index(Stages,0)].y", "b"),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("exact declared dependency clock")),
        "{errors:?}"
    );
    assert!(errors.iter().all(|error| error.source_span().is_some()));
}

#[test]
fn indexed_alias_clocks_are_rebound_for_each_wrapper_occurrence() {
    let wrapper = "component Wrapper(clock actual:periodic, clock expected:periodic, output y:1 at actual) { indexset Stages=range(2); instance cells[i in Stages]:Cell(tick=actual); let observed at expected=cells[index(Stages,1)].y; relation r at actual { y=observed; } }";
    let source = format!(
        "{CELL} {wrapper} model M() {{ {CLOCKS} instance first:Wrapper(actual=a,expected=a); instance second:Wrapper(actual=b,expected=b); }}"
    );
    compile("rebound.eqi", &source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let errors = compile(
        "rebound.eqi",
        &source.replace("actual=b,expected=b", "actual=b,expected=a"),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("activation assertion")),
        "{errors:?}"
    );
    assert!(errors.iter().all(|error| error.source_span().is_some()));
}

#[test]
fn activation_inference_does_not_bypass_static_nominal_selection_checks() {
    for (selection, message) in [
        ("cells[index(Other,0)].y", "same nominal IndexSet"),
        ("cells[index(Stages,2)].y", "bounds"),
        ("cells[index(Stages,time)].y", "runtime value"),
    ] {
        let errors = compile("selector.eqi", &selected_source(selection, "a")).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message().contains(message)),
            "{selection}: {errors:?}"
        );
        assert!(errors.iter().all(|error| error.source_span().is_some()));
    }
}
