use eqiora_lang::{DecimalLiteral, ExprKind, Item, SourceAstFactory, TextRange};

#[test]
fn decimal_normalization_is_exact_and_bounded() {
    for (source, digits, exponent, negative) in [
        ("0.1", "1", -1, false),
        ("001.2500e+2", "125", 0, false),
        ("1e-400", "1", -400, false),
        ("-1e400", "1", 400, true),
        ("-0e-999", "0", 0, false),
        ("1e9223372036854775807", "1", i64::MAX, false),
    ] {
        let value = DecimalLiteral::parse(source).unwrap();
        assert_eq!(
            (value.coefficient(), value.exponent10(), value.is_negative()),
            (digits, exponent, negative)
        );
        assert_eq!(
            DecimalLiteral::parse(&value.canonical_text()).unwrap(),
            value
        );
    }
    assert_ne!(
        DecimalLiteral::parse("0.100000000000000001").unwrap(),
        DecimalLiteral::parse("0.1").unwrap()
    );
    for invalid in [
        "",
        ".1",
        "1.",
        "1e",
        "1e+",
        "1e--1",
        "1e2e3",
        "NaN",
        "inf",
        " 1",
        "1_0",
        "1/2",
        "0e9223372036854775808",
        "10e9223372036854775807",
    ] {
        assert!(DecimalLiteral::parse(invalid).is_err(), "{invalid}");
    }
    assert!(DecimalLiteral::parse(&"1".repeat(256)).is_ok());
    assert!(DecimalLiteral::parse(&"1".repeat(257)).is_err());
    let long_fraction = format!("1.{}", "2".repeat(254));
    let value = DecimalLiteral::parse(&long_fraction).unwrap();
    assert!(value.canonical_text().len() <= 256);
    assert_eq!(
        DecimalLiteral::parse(&value.canonical_text()).unwrap(),
        value
    );
}

#[test]
fn quantity_parser_defers_numeric_range_admission_and_preserves_exact_text() {
    for literal in [
        "1e-400[km^100]",
        "1e400[nm^100]",
        "0.1[nm]",
        "0.100000000000000001[m]",
    ] {
        let source = format!("model M() {{ let value = {literal}; }}");
        let document = eqiora_lang::parse("quantity.eqi", &source)
            .into_document()
            .unwrap();
        let Item::Let(alias) = &document.models()[0].items()[0] else {
            panic!("let")
        };
        let ExprKind::Quantity { value, .. } = alias.value().kind() else {
            panic!("quantity")
        };
        assert!(!value.is_zero());
        assert_eq!(
            &source[alias.value().range().start() as usize..alias.value().range().end() as usize],
            literal
        );
        let formatted = eqiora_lang::format(&document);
        let reparsed = eqiora_lang::parse("again.eqi", &formatted)
            .into_document()
            .unwrap();
        let Item::Let(replayed) = &reparsed.models()[0].items()[0] else {
            panic!("let")
        };
        let ExprKind::Quantity {
            value: replayed, ..
        } = replayed.value().kind()
        else {
            panic!("quantity")
        };
        assert_eq!(value, replayed);
        assert_eq!(eqiora_lang::format(&reparsed), formatted);
    }
    for literal in ["1e-400", "1e400", "1e-400[2]"] {
        assert!(
            eqiora_lang::parse(
                "invalid.eqi",
                &format!("model M() {{ let x = {literal}; }}")
            )
            .into_document()
            .is_err()
        );
    }
}

#[test]
fn native_projection_preserves_subnormals_and_normalizes_zero() {
    for number in [f64::from_bits(1), -f64::from_bits(1), f64::MAX, 0.1, -0.0] {
        let value = DecimalLiteral::from_f64(number).unwrap();
        let replayed: f64 = value.canonical_text().parse().unwrap();
        assert_eq!(
            replayed.to_bits(),
            if number == 0.0 {
                0.0f64.to_bits()
            } else {
                number.to_bits()
            }
        );
        assert_eq!(
            DecimalLiteral::parse(&value.canonical_text()).unwrap(),
            value
        );
        let range = TextRange::new(0, 0);
        let unit = SourceAstFactory::expression(ExprKind::Name("m".into()), range).unwrap();
        assert!(
            SourceAstFactory::expression(
                ExprKind::Quantity {
                    value,
                    unit: Box::new(unit)
                },
                range
            )
            .is_ok()
        );
    }
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(DecimalLiteral::from_f64(number).is_err());
    }
}
