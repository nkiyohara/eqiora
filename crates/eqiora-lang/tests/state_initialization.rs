use eqiora_lang::{ActivationSyntax, FieldRoleSyntax, Item, format, parse};

#[test]
fn unknown_roles_and_initial_equations_have_separate_owners() {
    let source = "model Decay {\r\n/// 温度の状態\r\nstate temperature: K;\r\nvariable offset: K;\r\ninitial { temperature = 293[K]; }\r\nrelation balance { derivative(temperature) = -temperature; }\r\n}";
    let document = parse("decay.eqi", source).into_document().unwrap();
    let items = document.models()[0].items();
    let Item::Field(state) = &items[0] else {
        panic!("state owner")
    };
    assert_eq!(state.role(), FieldRoleSyntax::State);
    assert_eq!(state.activation(), &ActivationSyntax::Continuous);
    assert_eq!(state.domain(), None);
    assert_eq!(
        &source[state.range().start() as usize..state.range().end() as usize],
        "state temperature: K;"
    );
    let Item::Field(variable) = &items[1] else {
        panic!("variable owner")
    };
    assert_eq!(variable.role(), FieldRoleSyntax::Variable);
    let Item::Initial(initial) = &items[2] else {
        panic!("initial equation owner")
    };
    assert_eq!(initial.equations().len(), 1);
    let formatted = format(&document);
    // Structural comment ownership preserves the original comment bytes, including CRLF.
    assert!(formatted.contains("/// 温度の状態\r\n"));
    assert!(formatted.contains("initial {\n    temperature = 293 [K];\n  }"));
    assert_eq!(
        format(&parse("decay.eqi", &formatted).into_document().unwrap()),
        formatted
    );
}

#[test]
fn support_and_clock_are_independent_syntax() {
    let source =
        "model Sampled { state memory: V on region at tick; variable pressure: Pa on fluid; }";
    let document = parse("sampled.eqi", source).into_document().unwrap();
    let Item::Field(state) = &document.models()[0].items()[0] else {
        panic!("state owner")
    };
    assert_eq!(state.role(), FieldRoleSyntax::State);
    assert_eq!(state.domain(), Some("region"));
    assert_eq!(
        state.activation(),
        &ActivationSyntax::Periodic("tick".into())
    );
    assert_eq!(
        format(
            &parse("sampled.eqi", &format(&document))
                .into_document()
                .unwrap()
        ),
        format(&document)
    );
}

#[test]
fn declarations_cannot_encode_initial_conditions_or_representation_aliases() {
    for source in [
        "model Bad { state x: 1 = 1; }",
        "model Bad { variable x: 1 = 0; }",
        "model Bad { field x: 1; }",
        "model Bad { state x on region as continuum: 1; }",
        "model Bad { initial {} }",
    ] {
        assert!(
            parse("bad.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
}

#[test]
fn component_signature_has_one_spelling_and_preserves_borrowed_roles() {
    let source = "component Shared(\n  /// 読み取り参照\n  variable pressure: Pa on fluid,\n  state memory: V at tick,\n  clock tick,\n  support fluid: volume(ambient_dimension = 2)\n) { variable private_value: V; }\ncomponent Empty() {}\nmodel Root {}";
    let document = parse("shared.eqi", source).into_document().unwrap();
    let items = document.components()[0].items();
    let eqiora_lang::ComponentItem::FieldRequirement(variable) = &items[0] else {
        panic!("borrowed variable")
    };
    assert_eq!(variable.role(), FieldRoleSyntax::Variable);
    assert_eq!(
        document.doc_comment(variable.range()).unwrap().summary(),
        "読み取り参照"
    );
    let eqiora_lang::ComponentItem::FieldRequirement(state) = &items[1] else {
        panic!("borrowed state")
    };
    assert_eq!(state.role(), FieldRoleSyntax::State);
    assert_eq!(
        state.activation(),
        &ActivationSyntax::Periodic("tick".into())
    );
    assert!(matches!(
        items[2],
        eqiora_lang::ComponentItem::ClockRequirement(_)
    ));
    assert!(matches!(items[3], eqiora_lang::ComponentItem::Support(_)));
    assert!(matches!(items[4], eqiora_lang::ComponentItem::Field(_)));
    let formatted = format(&document);
    assert!(formatted.contains("component Empty() {"));
    assert!(formatted.contains("model Root {"));
    assert_eq!(
        format(&parse("shared.eqi", &formatted).into_document().unwrap()),
        formatted
    );
    for source in [
        "component Old {}",
        "component Old() { public field slot x on region as continuum: 1; }",
        "component Old() { public support region: volume(ambient_dimension=2); }",
        "component Bad(state x: 1 = 0) {}",
        "model Bad() {}",
    ] {
        assert!(
            parse("bad.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
}
