//! An exact species transfer must commit all populations or none of them.

use eqiora::api::ModelDocument;
use eqiora::kernel::RationalTime;
use eqiora::sem::{Interpreter, ReferenceConfig};

const SOURCE: &str = r#"
space Species = orthonormal(A, B);
model Transfer(output observed: counts<Species> at tick) {
  clock tick = periodic(1[s]);
  parameter change: coordinates<integer, Species> = coordinates(Species, [-1, 1]);
  state population: counts<Species> at tick;
  initial { population = counts(Species, [2, 9007199254740993]); }
  relation update at tick {
    next(population) = pre(population) + change;
    observed = next(population);
  }
}
"#;

#[test]
fn exact_stoichiometric_transfers_preserve_species_and_reject_underflow_atomically() {
    let document = ModelDocument::compile("species-transfer.eqi", SOURCE).unwrap();
    let observed = document.aliases()["observed"];
    let population = document.aliases()["population"];
    let digest = document.digest().unwrap();
    let interpreter = Interpreter::new();
    let config = ReferenceConfig::new(2.0, 1.0).unwrap();
    let mut session = interpreter
        .sampled_session(document.program(), config, [])
        .unwrap();
    let initial = session.field(population).unwrap();
    assert_eq!(initial.integer_component(0), Some(2));
    assert_eq!(initial.integer_component(1), Some(9_007_199_254_740_993));
    assert_eq!(session.advance_ticks(2).unwrap(), 2);
    // Each accepted A -> B transfer adds (-1,+1). The two accepted rows
    // preserve A+B=9007199254740995 without a binary64 conversion.
    let expected = [[1_i64, 9_007_199_254_740_994], [0, 9_007_199_254_740_995]];
    for (tick, expected) in expected.into_iter().enumerate() {
        let (time, value) = session.output(observed, tick as u64).unwrap();
        assert_eq!(time, RationalTime::new(tick as u64, 1).unwrap());
        assert_eq!(
            value.integer_components().unwrap().collect::<Vec<_>>(),
            expected
        );
        assert!(value.value_type().is_count());
        assert_eq!(
            value.value_type().finite_space(),
            initial.value_type().finite_space()
        );
        assert!(value.value_type().finite_space().is_some());
        assert!(value.real_scalar_value().is_none());
    }
    let accepted = session.output(observed, 1).unwrap().1.clone();
    let accepted_state = session.field(population).unwrap();
    assert_eq!(accepted_state, accepted);
    let checkpoint = session.checkpoint();
    let pending = session.next_tick();
    assert_eq!(pending, Some(RationalTime::new(2, 1).unwrap()));
    assert!(session.advance_ticks(1).is_err());
    assert_eq!(session.next_tick(), pending);
    assert_eq!(session.field(population).unwrap(), accepted_state);
    assert_eq!(session.output(observed, 1).unwrap().1, &accepted);
    assert!(session.output(observed, 2).is_none());
    assert_eq!(document.digest().unwrap(), digest);
    // Both the checkpoint and the failed session remain at the same rejected
    // boundary. Repeating the request cannot consume or publish a partial tick.
    let mut resumed = interpreter
        .resume_sampled(document.program(), &checkpoint)
        .unwrap();
    assert!(resumed.advance_ticks(1).is_err());
    assert!(session.advance_ticks(1).is_err());
    assert_eq!(session.next_tick(), resumed.next_tick());
    assert_eq!(session.field(population).unwrap(), accepted_state);
    assert_eq!(resumed.field(population).unwrap(), accepted_state);
    assert!(resumed.output(observed, 2).is_none());
}

#[test]
fn count_overflow_cannot_commit_another_species_decrement() {
    let source = SOURCE.replace("9007199254740993", "9223372036854775807");
    let document = ModelDocument::compile("count-overflow.eqi", &source).unwrap();
    let population = document.aliases()["population"];
    let observed = document.aliases()["observed"];
    let mut session = Interpreter::new()
        .sampled_session(
            document.program(),
            ReferenceConfig::new(0.0, 1.0).unwrap(),
            [],
        )
        .unwrap();
    let before = session.field(population).unwrap();
    assert_eq!(before.integer_component(0), Some(2));
    assert_eq!(before.integer_component(1), Some(i64::MAX));
    let pending = session.next_tick();
    assert!(session.advance_ticks(1).is_err());
    assert_eq!(session.field(population).unwrap(), before);
    assert_eq!(session.next_tick(), pending);
    assert!(session.output(observed, 0).is_none());
}

#[test]
fn species_identity_and_nonnegative_range_are_not_supplied_by_integer_storage() {
    ModelDocument::compile("count-contract.eqi", SOURCE).unwrap();
    for invalid in [
        SOURCE
            .replace(
                "space Species = orthonormal(A, B);",
                "space Species = orthonormal(A, B); space Foreign = orthonormal(A, B);",
            )
            .replace(
                "coordinates(Species, [-1, 1])",
                "coordinates(Foreign, [-1, 1])",
            ),
        SOURCE.replace(
            "counts(Species, [2, 9007199254740993])",
            "counts(Species, [-1, 9007199254740993])",
        ),
        SOURCE.replace(
            "next(population) = pre(population) + change",
            "derivative(population) = change",
        ),
        // Editable values may participate in arithmetic around nominal values,
        // but cannot be silently folded into constructor elements.
        SOURCE
            .replace(
                "  parameter change:",
                "  parameter editable: integer = 1;\n  parameter change:",
            )
            .replace(
                "coordinates(Species, [-1, 1])",
                "coordinates(Species, [-1, editable])",
            ),
    ] {
        let diagnostics = ModelDocument::compile("count-contract.eqi", &invalid).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_span().is_some()),
            "{diagnostics:?}"
        );
    }
}
