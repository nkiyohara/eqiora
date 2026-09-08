//! Source finite extrema retain edited values, physical dimensions and sampled identity.
use eqiora::ValueLiteral;
use eqiora::api::ModelDocument;
use eqiora::sem::{Interpreter, ReferenceConfig};

const SOURCE: &str = r#"
model Extremes(output y: V at tick) {
  clock tick = periodic(1[s]);
  indexset Candidates = range(2);
  parameter increments: array<V, 2> = [5, 7];
  state memory: V at tick;
  variable doubled: V at tick;
  initial { memory = 0; }
  relation update at tick {
    next(memory) = min(pre(memory) + increments[ordinal(i)], over=(i in Candidates));
    doubled = next(memory) + next(memory);
    y = doubled;
  }
}
"#;

#[test]
fn finite_real_minimum_uses_current_parameter_values_and_exact_sampled_resume() {
    let original = ModelDocument::compile("finite-extrema.eqi", SOURCE).unwrap();
    let parameter = original.aliases()["increments"];
    let ty = original
        .program()
        .typed_value(parameter)
        .unwrap()
        .value_type()
        .clone();
    let edit = original
        .preview_value_edit(
            parameter,
            ValueLiteral::new(ty, [(3., 0.), (11., 0.)]).unwrap(),
        )
        .unwrap();
    let changed = original.commit_value_edit(edit).unwrap().into_document();
    assert_eq!(changed.aliases()["increments"], parameter);
    assert_eq!(changed.aliases()["memory"], original.aliases()["memory"]);
    for (document, expected) in [(&original, [10., 20., 30.]), (&changed, [6., 12., 18.])] {
        let output = document.aliases()["y"];
        let interpreter = Interpreter::new();
        let mut session = interpreter
            .sampled_session(
                document.program(),
                ReferenceConfig::new(2., 1.).unwrap(),
                [],
            )
            .unwrap();
        session.advance_ticks(1).unwrap();
        let mut resumed = interpreter
            .resume_sampled(document.program(), &session.checkpoint())
            .unwrap();
        resumed.advance_ticks(2).unwrap();
        for (tick, value) in expected.into_iter().enumerate() {
            let actual = resumed.output(output, tick as u64).unwrap().1;
            assert_eq!(actual.real_scalar_value().unwrap().value(), value);
            assert_eq!(
                actual.value_type().dimension(),
                document
                    .program()
                    .typed_value(parameter)
                    .unwrap()
                    .value_type()
                    .dimension()
            );
        }
    }
}

#[test]
fn finite_integer_maximum_preserves_adjacent_values_beyond_binary64() {
    let source = r#"model Exact(output y: integer at tick) {
      clock tick=periodic(1[s]); indexset Candidates=range(3);
      parameter values:array<integer,3>=[9007199254740992,9007199254740993,9007199254740993];
      relation choose at tick { y=max(values[ordinal(i)],over=(i in Candidates)); }
    }"#;
    let document = ModelDocument::compile("exact-max.eqi", source).unwrap();
    let mut session = Interpreter::new()
        .sampled_session(
            document.program(),
            ReferenceConfig::new(0., 1.).unwrap(),
            [],
        )
        .unwrap();
    session.advance_ticks(1).unwrap();
    assert_eq!(
        session
            .output(document.aliases()["y"], 0)
            .unwrap()
            .1
            .integer_scalar_value(),
        Some(9_007_199_254_740_993)
    );
}
