//! Exact integer values cross the ordinary source, native, edit, and replay paths.

use eqiora::api::ModelDocument;
use eqiora::language::{DraftField, DraftParameter, DraftRelation, FieldRoleSyntax, ModelDraft};
use eqiora::{DimExponents, ScalarDomain, ValueLiteral, ValueType};

fn integer(value: i64) -> ValueLiteral {
    ValueLiteral::from_integer(
        ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS),
        value,
    )
    .unwrap()
}

fn parameter(document: &ModelDocument, name: &str) -> ValueLiteral {
    document
        .program()
        .typed_value(
            *document
                .aliases()
                .get(name)
                .unwrap_or_else(|| panic!("missing {name} in {:?}", document.aliases())),
        )
        .unwrap()
        .clone()
}

#[test]
fn adjacent_exact_values_survive_source_native_edit_and_artifact_replay() {
    // Binary64's consecutive-integer range ends at 2^53. These two values must
    // remain distinct before any explicitly requested conversion to real.
    const FIRST: i64 = 9_007_199_254_740_993;
    const NEXT: i64 = 9_007_199_254_740_994;
    let original = ModelDocument::compile(
        "exact-integer.eqi",
        "model Exact() { parameter count: integer = 9007199254740993; variable witness: 1; relation law { witness = 0; } }",
    )
    .unwrap();
    assert_eq!(parameter(&original, "count"), integer(FIRST));
    assert!(parameter(&original, "count").real_scalar_value().is_none());

    let witness = DraftField::new(
        "witness",
        ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
        FieldRoleSyntax::Variable,
    );
    let law = DraftRelation::continuous("law", [witness.expression()]);
    let native = ModelDocument::define(
        &ModelDraft::new(
            "Exact",
            [
                DraftParameter::new("count", integer(FIRST)).into(),
                witness.into(),
                law.into(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(parameter(&native, "count"), integer(FIRST));
    let target = original.aliases()["count"];
    let plan = original.preview_value_edit(target, integer(NEXT)).unwrap();
    assert_eq!(plan.before(), &integer(FIRST));
    let changed = original
        .commit_value_edit(plan.clone())
        .unwrap()
        .into_document();
    assert_eq!(parameter(&changed, "count"), integer(NEXT));
    assert_ne!(original.digest().unwrap(), changed.digest().unwrap());
    assert_eq!(parameter(&original, "count"), integer(FIRST));
    assert!(changed.commit_value_edit(plan).is_err());
    for document in [&original, &native, &changed] {
        let replay = ModelDocument::replay(&document.canonical_json().unwrap()).unwrap();
        let target = document.aliases()["count"];
        assert_eq!(
            replay.program().typed_value(target),
            document.program().typed_value(target)
        );
        assert_eq!(replay.digest().unwrap(), document.digest().unwrap());
    }
}

#[test]
fn signed_bounds_division_and_explicit_conversions_follow_the_catalog() {
    let document = ModelDocument::compile(
        "integer-arithmetic.eqi",
        r#"model Arithmetic() {
          parameter minimum: integer = -9223372036854775808;
          parameter maximum: integer = 9223372036854775807;
          parameter next: integer = 9007199254740993 + 1;
          parameter product: integer = -7 * 3;
          parameter difference: integer = -7 - 3;
          parameter negated: integer = -(-7);
          parameter q: integer = quotient(-7, 3);
          parameter r: integer = remainder(-7, 3);
          parameter positive_q: integer = quotient(7, -3);
          parameter positive_r: integer = remainder(7, -3);
          parameter rounded: 1 = to_real(9007199254740993);
          parameter minimum_back: integer = to_integer(-9223372036854775808.0);
          parameter round_trip: integer = to_integer(to_real(9007199254740993));
          parameter half: 1 = 1 / 2;
          variable witness: 1;
          relation law { witness = half; }
        }"#,
    )
    .unwrap();
    for (name, expected) in [
        ("minimum", i64::MIN),
        ("maximum", i64::MAX),
        ("next", 9_007_199_254_740_994),
        ("product", -21),
        ("difference", -10),
        ("negated", 7),
        ("q", -2),
        ("r", -1),
        ("positive_q", -2),
        ("positive_r", 1),
        ("minimum_back", i64::MIN),
        ("round_trip", 9_007_199_254_740_992),
    ] {
        assert_eq!(
            parameter(&document, name).integer_scalar_value(),
            Some(expected)
        );
    }
    // 2^53+1 is halfway between representable even-spaced reals; ties-to-even
    // chooses 2^53. The explicit reverse conversion cannot recover the lost bit.
    assert_eq!(
        parameter(&document, "rounded")
            .real_scalar_value()
            .unwrap()
            .value(),
        9_007_199_254_740_992.0,
    );
    assert_eq!(
        parameter(&document, "half")
            .real_scalar_value()
            .unwrap()
            .value(),
        0.5
    );
}

#[test]
fn integer_overflow_and_invalid_conversions_fail_at_the_authored_expression() {
    for expression in [
        "9223372036854775808",
        "-9223372036854775809",
        "9223372036854775807 + 1",
        "-9223372036854775808 - 1",
        "-9223372036854775808 * -1",
        "-(-9223372036854775808)",
        "quotient(1, 0)",
        "remainder(1, 0)",
        "quotient(-9223372036854775808, -1)",
        "to_integer(0.5)",
        "to_integer(9223372036854775808.0)",
        "to_integer(1[m])",
    ] {
        let source = format!(
            "model Invalid() {{ parameter value: integer = {expression}; variable witness: 1; relation law {{ witness = 0; }} }}"
        );
        let diagnostics =
            ModelDocument::compile("integer-invalid.eqi", &source).expect_err(expression);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message().contains("at least one Relation")),
            "wrong rejection boundary: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_span().is_some()),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn integer_domain_does_not_silently_promote_or_acquire_a_continuous_derivative() {
    for source in [
        "model Invalid() { parameter n: integer = 2; parameter x: 1 = n + 0.5; variable witness: 1; relation law { witness = 0; } }",
        "model Invalid() { parameter n: integer = 2; parameter x: integer = n / n; variable witness: 1; relation law { witness = 0; } }",
        "model Invalid() { state n: integer; initial { n = 2; } relation flow { derivative(n) = 0; } }",
    ] {
        let diagnostics = ModelDocument::compile("integer-domain.eqi", source).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message().contains("at least one Relation")),
            "wrong rejection boundary: {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source_span().is_some()),
            "{source}: {diagnostics:?}"
        );
    }
}
