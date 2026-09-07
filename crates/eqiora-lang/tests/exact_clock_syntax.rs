use eqiora_lang::{ComponentItem, ExprKind, Item, SourceAstFactory, TextRange};

#[test]
fn exact_clock_expressions_and_default_phase_round_trip() {
    for container in ["model M()", "component C()"] {
        for args in [
            "10[ms]",
            "1[s] / 3",
            "18446744073709551615[s] / 18446744073709551614, phase = 0[s]",
            "(1e-400[km^100] * 2) / 3, phase = -1[ms]",
        ] {
            let source = format!(
                "{container} {{\n/// sample clock\nclock c = periodic({args}); // retained\n}}"
            );
            let document = eqiora_lang::parse("clock.eqi", &source)
                .into_document()
                .unwrap();
            let clock = if let Some(model) = document.models().first() {
                let Item::Clock(clock) = &model.items()[0] else {
                    panic!("clock")
                };
                clock
            } else {
                let ComponentItem::Clock(clock) = &document.components()[0].items()[0] else {
                    panic!("clock")
                };
                clock
            };
            if !args.contains("phase") {
                assert!(
                    matches!(clock.phase().kind(), ExprKind::Quantity { value, unit } if value.is_zero() && matches!(unit.kind(), ExprKind::Name(name) if name == "s"))
                );
            }
            let rebuilt = SourceAstFactory::clock(
                "c",
                clock.period().clone(),
                clock.phase().clone(),
                TextRange::new(0, 0),
            )
            .unwrap();
            assert_eq!(rebuilt.period(), clock.period());
            assert_eq!(rebuilt.phase(), clock.phase());
            let formatted = eqiora_lang::format(&document);
            assert!(formatted.contains("sample clock"));
            assert!(formatted.contains("retained"));
            assert_eq!(
                eqiora_lang::format(
                    &eqiora_lang::parse("again.eqi", &formatted)
                        .into_document()
                        .unwrap()
                ),
                formatted
            );
        }
    }
}

#[test]
fn bare_clock_leaves_are_exact_but_ordinary_numbers_are_unchanged() {
    let source = "model M() { clock c = periodic(1[s] / 18446744073709551615); let ordinary = 2; }";
    let document = eqiora_lang::parse("clock.eqi", source)
        .into_document()
        .unwrap();
    let Item::Clock(clock) = &document.models()[0].items()[0] else {
        panic!("clock")
    };
    let ExprKind::Binary { right, .. } = clock.period().kind() else {
        panic!("division")
    };
    assert!(
        matches!(right.kind(), ExprKind::Quantity { value, unit } if value.coefficient() == "18446744073709551615" && value.exponent10() == 0 && matches!(unit.kind(), ExprKind::Number(literal) if literal.to_i64().ok() == Some(1)))
    );
    let Item::Let(alias) = &document.models()[0].items()[1] else {
        panic!("let")
    };
    assert!(
        matches!(alias.value().kind(), ExprKind::Number(literal) if literal.to_i64().ok() == Some(2))
    );
    let failed = eqiora_lang::parse(
        "recover.eqi",
        "model M() { clock c = periodic(1 + ); let ordinary = 2; }",
    );
    let alias = failed.document().unwrap().models()[0]
        .items()
        .iter()
        .find_map(|item| {
            if let Item::Let(alias) = item {
                Some(alias)
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        matches!(alias.value().kind(), ExprKind::Number(literal) if literal.to_i64().ok() == Some(2))
    );
}

#[test]
fn retired_clock_labels_and_unbounded_expressions_reject() {
    for args in [
        "period = 1 / 10, phase = 0 / 1",
        "1[s], phase = 0[s], phase = 1[s]",
        "1[s], other = 0[s]",
    ] {
        assert!(
            eqiora_lang::parse(
                "invalid.eqi",
                &format!("model M() {{ clock c = periodic({args}); }}")
            )
            .into_document()
            .is_err()
        );
    }
    for value in [
        "1".repeat(257),
        format!("{}1[s]{}", "(".repeat(256), ")".repeat(256)),
    ] {
        assert!(
            eqiora_lang::parse(
                "bounded.eqi",
                &format!("model M() {{ clock c = periodic({value}); }}")
            )
            .into_document()
            .is_err()
        );
    }
}
