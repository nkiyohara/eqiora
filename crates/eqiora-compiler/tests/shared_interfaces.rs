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
    for equation in ["next(memory)=u", "next(memory)=sample(u,other)", "next(memory)=alias", "next(memory)=sample(memory,tick)"] {
        let source = format!("model M(input u:1) {{ clock tick=periodic(1[s]); clock other=periodic(1[s]); state memory:1 at tick; let alias=u; initial {{memory=0;}} relation update at tick {{{equation};}} }}");
        assert!(compile("crossing.eqi",&source).is_err(),"{source}");
    }
    accepted("model M(input u:1) { clock tick=periodic(1[s]); state memory:1 at tick; let alias=sample(u,tick); initial {memory=0;} relation update at tick {next(memory)=alias;} }");
}
