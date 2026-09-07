use eqiora_lang::{ComponentItem, Item};

#[test]
fn named_activation_is_optional_and_round_trips_with_both_containers() {
    for container in ["model M()", "component C()"] {
        for annotation in ["", ": m"] {
            for support in ["", " on body"] {
                for activation in ["", " at sample"] {
                    let head = format!("let q{annotation}{support}{activation}");
                    let source = format!(
                        "{container} {{\n/// derived value\n{head} // assertion\n = value; // expression\n}}"
                    );
                    let document = eqiora_lang::parse("activation.eqi", &source)
                        .into_document()
                        .unwrap();
                    let alias = if let Some(model) = document.models().first() {
                        let Item::Let(alias) = &model.items()[0] else {
                            panic!("let")
                        };
                        alias
                    } else {
                        let ComponentItem::Let(alias) = &document.components()[0].items()[0] else {
                            panic!("let")
                        };
                        alias
                    };
                    assert_eq!(
                        alias.activation(),
                        (!activation.is_empty()).then_some("sample")
                    );
                    assert_eq!(alias.domain(), (!support.is_empty()).then_some("body"));
                    assert_eq!(
                        &source[alias.range().start() as usize..alias.range().end() as usize],
                        format!("{head} // assertion\n = value;")
                    );
                    let formatted = eqiora_lang::format(&document);
                    for comment in ["derived value", "assertion", "expression"] {
                        assert!(formatted.contains(comment));
                    }
                    let reparsed = eqiora_lang::parse("again.eqi", &formatted)
                        .into_document()
                        .unwrap();
                    assert_eq!(eqiora_lang::format(&reparsed), formatted);
                }
            }
        }
    }
}

#[test]
fn activation_assertions_reject_duplicate_reordered_or_missing_clauses() {
    for container in ["model M()", "component C()"] {
        for head in [
            "let q at sample at other",
            "let q at sample on body",
            "let q at sample: m",
            "let q at",
            "let q on body at",
            "let q at periodic(1[s])",
        ] {
            let source = format!("{container} {{ {head} = value; }}");
            assert!(
                eqiora_lang::parse("invalid.eqi", &source)
                    .into_document()
                    .is_err(),
                "{source}"
            );
        }
    }
}
