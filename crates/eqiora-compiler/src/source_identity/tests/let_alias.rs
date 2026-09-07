use super::identity;

#[test]
fn complete_let_type_annotations_change_source_identity() {
    let real = "model m { let x: m = 0; }";
    let complex = "model m { let x: complex<m> = 0; }";
    let array = "model m { let x: array<complex<m>, 3> = 0; }";
    assert_ne!(identity(real), identity(complex));
    assert_ne!(identity(complex), identity(array));
}

#[test]
fn source_structure_has_exact_identity() {
    let base = "model m { parameter p: m = 2; let k: 1 / m = math.pi / p; }";
    let reformatted = "model m {\n parameter p: m = 2;\n let k: 1/m = math.pi/p;\n}";
    let renamed = "model m { parameter p: m = 2; let wave: 1 / m = math.pi / p; }";
    let changed = "model m { parameter p: m = 2; let k: 1 / m = 2 / p; }";

    assert_eq!(identity(base), identity(reformatted));
    assert_ne!(identity(base), identity(renamed));
    assert_ne!(identity(base), identity(changed));
}

#[test]
fn omitted_dimension_has_distinct_deterministic_identity() {
    let annotated = "model m { parameter p: m = 2; let k: 1 / m = math.pi / p; }";
    let inferred = "model m { parameter p: m = 2; let k = math.pi / p; }";
    let reformatted = "model m {\n parameter p: m = 2;\n let k=math.pi/p;\n}";

    assert_ne!(identity(annotated), identity(inferred));
    assert_eq!(identity(inferred), identity(reformatted));
}

#[test]
fn support_assertions_change_identity_in_both_containers() {
    for container in ["model M", "component C()"] {
        let omitted = format!("{container} {{ let q: m = value; }}");
        let asserted = format!("{container} {{ let q: m on body = value; }}");
        let other = format!("{container} {{ let q: m on other = value; }}");
        assert_ne!(identity(&omitted), identity(&asserted));
        assert_ne!(identity(&asserted), identity(&other));
        let document = eqiora_lang::parse("asserted.eqi", &asserted)
            .into_document()
            .unwrap();
        assert_eq!(
            identity(&asserted),
            identity(&eqiora_lang::format(&document))
        );
    }
}

#[test]
fn omitted_activation_keeps_the_existing_record_contract() {
    use crate::source_identity::{
        Budget, Encoder, LocalSourceIdentityLimits, compile_time, encode_expression, encode_name,
        value_type,
    };
    for (annotation, support) in [("", ""), (": m", ""), ("", " on body"), (": m", " on body")] {
        let source = format!("model M {{ let q{annotation}{support} = value; }}");
        let document = eqiora_lang::parse("omitted.eqi", &source)
            .into_document()
            .unwrap();
        let eqiora_lang::Item::Let(alias) = &document.models()[0].items()[0] else {
            panic!("let")
        };
        let limits = LocalSourceIdentityLimits::default();
        let mut actual = Encoder::new(limits.max_canonical_bytes);
        compile_time::encode_let(&mut actual, alias, &mut Budget::new(limits)).unwrap();
        // Existing alias record: name=1, optional type=2, expression=3, optional support=4.
        // Absence of the new assertion must emit no tag or sentinel.
        let mut expected = Encoder::new(limits.max_canonical_bytes);
        let mut budget = Budget::new(limits);
        expected
            .field(1, |encoder| encode_name(encoder, "q", &mut budget))
            .unwrap();
        if let Some(value_type) = alias.value_type() {
            expected
                .field(2, |encoder| {
                    value_type::encode_value_type(encoder, value_type, &mut budget, 1)
                })
                .unwrap();
        }
        expected
            .field(3, |encoder| {
                encode_expression(encoder, alias.value(), &mut budget, 1)
            })
            .unwrap();
        if !support.is_empty() {
            expected
                .field(4, |encoder| encode_name(encoder, "body", &mut budget))
                .unwrap();
        }
        assert_eq!(actual.finish().unwrap(), expected.finish().unwrap());
    }
}

#[test]
fn activation_assertions_change_identity_in_both_containers() {
    for container in ["model M", "component C()"] {
        for support in ["", " on body"] {
            let omitted = format!("{container} {{ let q: m{support} = value; }}");
            let asserted = format!("{container} {{ let q: m{support} at sample = value; }}");
            let other = format!("{container} {{ let q: m{support} at other = value; }}");
            assert_ne!(identity(&omitted), identity(&asserted));
            assert_ne!(identity(&asserted), identity(&other));
            let document = eqiora_lang::parse("asserted.eqi", &asserted)
                .into_document()
                .unwrap();
            assert_eq!(
                identity(&asserted),
                identity(&eqiora_lang::format(&document))
            );
        }
    }
}
