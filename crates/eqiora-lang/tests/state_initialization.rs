use eqiora_lang::{ActivationSyntax, FieldRoleSyntax, Item, format, parse};

#[test]
fn unknown_roles_and_initial_equations_have_separate_owners() {
    let source = "model Decay() {\r\n/// 温度の状態\r\nstate temperature: K;\r\nvariable offset: K;\r\ninitial { temperature = 293[K]; }\r\nrelation balance { derivative(temperature) = -temperature; }\r\n}";
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
        "model Sampled() { state memory: V on region at tick; variable pressure: Pa on fluid; }";
    let document = parse("sampled.eqi", source).into_document().unwrap();
    let Item::Field(state) = &document.models()[0].items()[0] else {
        panic!("state owner")
    };
    assert_eq!(state.role(), FieldRoleSyntax::State);
    assert_eq!(state.domain(), Some("region"));
    assert_eq!(state.activation(), &ActivationSyntax::Named("tick".into()));
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
        "model Bad() { state x: 1 = 1; }",
        "model Bad() { variable x: 1 = 0; }",
        "model Bad() { field x: 1; }",
        "model Bad() { state x on region as continuum: 1; }",
        "model Bad() { representation space = continuum; }",
        "model Bad() { initial {} }",
    ] {
        assert!(
            parse("bad.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
}

#[test]
fn borrowed_clocks_roundtrip_with_exact_target_and_comment_owner() {
    let source = "component Delay(clock tick: periodic, state memory: V at tick) {}\nmodel Root() { clock sample = periodic(1[s] / 1, phase = 2[s] / 1); state held: V at sample; instance delay: Delay(\n/// 同じクロック\ntick = sample, memory = held); }";
    let document = parse("clocks.eqi", source).into_document().unwrap();
    let Item::Instance(instance) = &document.models()[0].items()[2] else {
        panic!("instance");
    };
    let binding = &instance.bindings()[0];
    assert_eq!(binding.name(), "tick");
    assert!(
        matches!(binding.value().kind(), eqiora_lang::ExprKind::Name(name) if name == "sample")
    );
    assert_eq!(
        &source[binding.range().start() as usize..binding.range().end() as usize],
        "tick = sample"
    );
    assert_eq!(
        document.doc_comment(binding.range()).unwrap().summary(),
        "同じクロック"
    );
    let formatted = format(&document);
    assert_eq!(
        format(&parse("clocks.eqi", &formatted).into_document().unwrap()),
        formatted
    );

    let range = eqiora_lang::TextRange::new(0, 0);
    let value = eqiora_lang::SourceAstFactory::expression(
        eqiora_lang::ExprKind::Name("sample".into()),
        range,
    )
    .unwrap();
    let instance = eqiora_lang::SourceAstFactory::instance(
        "clock_only",
        eqiora_lang::NamePath::from_segments(["Delay"], range).unwrap(),
        None,
        vec![eqiora_lang::SourceAstFactory::named_binding("tick", value, range).unwrap()],
        range,
    )
    .unwrap();
    let model = eqiora_lang::SourceAstFactory::model(
        eqiora_lang::VisibilitySyntax::Private,
        "M",
        vec![],
        vec![Item::Instance(instance)],
        range,
    )
    .unwrap();
    let document = eqiora_lang::SourceAstFactory::flat_document(vec![model]).unwrap();
    assert!(format(&document).contains("Delay(tick = sample)"));
}

#[test]
fn component_signature_has_one_spelling_and_preserves_borrowed_roles() {
    let source = "component Shared(\n  /// 読み取り参照\n  variable pressure: Pa on fluid,\n  state memory: V at tick,\n  clock tick: periodic,\n  support fluid: volume(ambient_dimension = 2)\n) { variable private_value: V; }\ncomponent Empty() {}\nmodel Root() {}";
    let document = parse("shared.eqi", source).into_document().unwrap();
    let items = document.components()[0].signature();
    let eqiora_lang::SignatureItem::Field(variable) = &items[0] else {
        panic!("borrowed variable")
    };
    assert_eq!(variable.role(), FieldRoleSyntax::Variable);
    assert_eq!(
        document.doc_comment(variable.range()).unwrap().summary(),
        "読み取り参照"
    );
    let eqiora_lang::SignatureItem::Field(state) = &items[1] else {
        panic!("borrowed state")
    };
    assert_eq!(state.role(), FieldRoleSyntax::State);
    assert_eq!(state.activation(), &ActivationSyntax::Named("tick".into()));
    assert!(matches!(items[2], eqiora_lang::SignatureItem::Clock(_)));
    assert!(matches!(items[3], eqiora_lang::SignatureItem::Support(_)));
    assert!(matches!(
        document.components()[0].items()[0],
        eqiora_lang::ComponentItem::Field(_)
    ));
    let formatted = format(&document);
    assert!(formatted.contains("component Empty() {"));
    assert!(formatted.contains("model Root() {"));
    assert_eq!(
        format(&parse("shared.eqi", &formatted).into_document().unwrap()),
        formatted
    );
    for source in [
        "component Old {}",
        "component Old() { public field slot x on region as continuum: 1; }",
        "component Old() { public support region: volume(ambient_dimension=2); }",
        "component Bad(state x: 1 = 0) {}",
        "model Bad {}",
    ] {
        assert!(
            parse("bad.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
}
