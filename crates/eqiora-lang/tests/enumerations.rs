use eqiora_lang::{
    ExprKind, Item, NamePath, SourceAstFactory as F, TextRange, ValueTypeSyntaxKind,
    VisibilitySyntax, format, parse,
};

#[test]
fn module_enum_and_case_preserve_tags_paths_order_ranges_and_comments() {
    let source = r#"// public operating modes
public enum Mode { Heating, Cooling, Fault }
model Controller() {
  state mode: Mode;
  let command = case mode { Mode.Heating => 1, // cooling law
      Mode.Cooling => -1, Mode.Fault => 0 };
}"#;
    let document = parse("enum.eqi", source).into_document().unwrap();
    let declaration = &document.enumerations()[0];
    assert_eq!(declaration.visibility(), VisibilitySyntax::Public);
    assert_eq!(
        declaration
            .tags()
            .iter()
            .map(NamePath::as_str)
            .collect::<Vec<_>>(),
        ["Heating", "Cooling", "Fault"]
    );
    for tag in declaration.tags() {
        assert_eq!(
            &source[tag.range().start() as usize..tag.range().end() as usize],
            tag.as_str()
        );
    }
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("field")
    };
    assert!(
        matches!(field.value_type().kind(), ValueTypeSyntaxKind::Named(name) if name.as_str() == "Mode")
    );
    assert!(field.dimension().is_none());
    let Item::Let(alias) = &document.models()[0].items()[1] else {
        panic!("alias")
    };
    let ExprKind::Case { value, arms } = alias.value().kind() else {
        panic!("case")
    };
    assert!(matches!(value.kind(), ExprKind::Name(name) if name == "mode"));
    assert_eq!(
        arms.iter()
            .map(|arm| arm.pattern().as_str())
            .collect::<Vec<_>>(),
        ["Mode.Heating", "Mode.Cooling", "Mode.Fault"]
    );
    assert_eq!(
        &source[arms[1].range().start() as usize..arms[1].range().end() as usize],
        "Mode.Cooling => -1"
    );
    let formatted = format(&document);
    assert!(
        formatted.contains("// cooling law") && formatted.contains("// public operating modes")
    );
    assert_eq!(
        format(&parse("again.eqi", &formatted).into_document().unwrap()),
        formatted
    );
}

#[test]
fn bare_and_qualified_type_names_stay_neutral_inside_numeric_type_constructors() {
    for name in ["V", "Mode", "Units.V", "Controls.Mode"] {
        let source =
            format!("model M() {{ state x: {name}; parameter a: array<vector<{name},3>,2> = 0; }}");
        let document = parse("types.eqi", &source).into_document().unwrap();
        let Item::Field(field) = &document.models()[0].items()[0] else {
            panic!("field")
        };
        assert!(
            matches!(field.value_type().kind(), ValueTypeSyntaxKind::Named(path) if path.as_str() == name)
        );
        assert!(field.value_type().dimension().is_none());
        assert!(field.value_type().scalar_domain().is_none());
        assert_eq!(
            format(
                &parse("again.eqi", &format(&document))
                    .into_document()
                    .unwrap()
            ),
            format(&document)
        );
    }
    let document = parse(
        "units.eqi",
        "model M() { variable x: V / m; let q = 1[V]; }",
    )
    .into_document()
    .unwrap();
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("field")
    };
    assert!(matches!(
        field.value_type().kind(),
        ValueTypeSyntaxKind::Scalar { .. }
    ));
    assert!(field.dimension().is_some());
    let Item::Let(alias) = &document.models()[0].items()[1] else {
        panic!("alias")
    };
    assert!(
        matches!(alias.value().kind(), ExprKind::Quantity { unit, .. } if matches!(unit.kind(), ExprKind::Name(name) if name == "V"))
    );
}

#[test]
fn native_enum_and_case_use_the_same_document_and_expression_owners() {
    let range = TextRange::new(0, 0);
    let path =
        |segments: &[&str]| NamePath::from_segments(segments.iter().copied(), range).unwrap();
    let declaration = F::enumeration(
        VisibilitySyntax::Public,
        "Mode",
        vec![path(&["Heating"]), path(&["Cooling"])],
        range,
    )
    .unwrap();
    let document = F::document(vec![declaration], vec![], vec![], vec![]).unwrap();
    assert_eq!(format(&document), "public enum Mode { Heating, Cooling }\n");
    assert!(
        parse("enumonly.eqi", &format(&document))
            .into_document()
            .is_ok()
    );
    let value = F::expression(
        ExprKind::Path(path(&["Controls", "Mode", "Heating"])),
        range,
    )
    .unwrap();
    let arm = F::case_arm(path(&["Controls", "Mode", "Heating"]), value.clone(), range).unwrap();
    let expression = F::expression(
        ExprKind::Case {
            value: Box::new(value),
            arms: vec![arm],
        },
        range,
    )
    .unwrap();
    let rewritten = expression.rewrite_name_paths(|name| {
        name.as_str().starts_with("Controls.").then(|| {
            NamePath::from_segments(
                ["Imported"].into_iter().chain(name.segments().skip(1)),
                name.range(),
            )
            .unwrap()
        })
    });
    let ExprKind::Case { value, arms } = rewritten.kind() else {
        panic!("case")
    };
    assert_eq!(arms[0].pattern().as_str(), "Imported.Mode.Heating");
    assert!(
        matches!(value.kind(), ExprKind::Path(name) if name.as_str() == "Imported.Mode.Heating")
    );
}

#[test]
fn malformed_tags_and_case_patterns_reject_without_pattern_bindings_or_wildcards() {
    for source in [
        "enum Mode {}",
        "enum Mode { A, A }",
        "enum Mode { _ }",
        "enum Mode { A.B }",
        "component C() { enum Mode { A } }",
        "model M() { enum Mode { A } }",
        "model M() { let x = case mode {}; }",
        "model M() { let x = case mode { _ => 0 }; }",
        "model M() { let x = case mode { Mode._ => 0 }; }",
        "model M() { let x = case mode { Mode.A => 0, Mode.A => 1 }; }",
        "model M() { let x = case mode { Mode.A(x) => x }; }",
        "model M() { let x = case mode { Mode.A = > 0 }; }",
        "model M() { let x = case mode { Mode.A => }; }",
    ] {
        assert!(
            parse("bad.eqi", source).into_document().is_err(),
            "{source}"
        );
    }
    // Coverage and foreign-declaration identity require lexical information.
    assert!(
        parse(
            "semantic.eqi",
            "enum Mode { A, B } model M() { let x = case mode { Other.A => 0 }; }"
        )
        .into_document()
        .is_ok()
    );
}

#[test]
fn nested_cases_and_selects_share_precedence_and_depth_limits() {
    for expression in [
        "case mode { Mode.A => if enabled then 1 else 2, Mode.B => 3 }",
        "case mode { Mode.A => case other { Other.A => 1 }, Mode.B => 2 }",
        "(case mode { Mode.A => 1 }) + 2",
    ] {
        let source = format!("model M() {{ let x = {expression}; }}");
        let document = parse("nested.eqi", &source).into_document().unwrap();
        assert_eq!(
            format(
                &parse("again.eqi", &format(&document))
                    .into_document()
                    .unwrap()
            ),
            format(&document)
        );
    }
    for (depth, accepted) in [(255, true), (256, false)] {
        let expression = format!(
            "{}0{}",
            "case mode { Mode.A => ".repeat(depth),
            " }".repeat(depth)
        );
        assert_eq!(
            parse(
                "depth.eqi",
                &format!("model M() {{ let x = {expression}; }}")
            )
            .into_document()
            .is_ok(),
            accepted
        );
    }
}

#[test]
fn checked_members_and_patterns_keep_exact_declaration_through_qualified_rewrites() {
    use eqiora_core::Id;
    use eqiora_schema::kernel::EnumDef;
    let definition = EnumDef::new(Id::new(), ["Heating".into(), "Cooling".into()]).unwrap();
    let foreign = EnumDef::new(Id::new(), ["Heating".into(), "Cooling".into()]).unwrap();
    let range = TextRange::new(0, 0);
    let declaration = NamePath::from_segments(["Controls", "Mode"], range).unwrap();
    let path = NamePath::from_segments(["Controls", "Mode", "Cooling"], range).unwrap();
    let mut member = F::expression(ExprKind::Path(path.clone()), range).unwrap();
    F::bind_enum_member(&mut member, &declaration, &definition).unwrap();
    assert_eq!(member.resolved_enum(), Some(&definition.value(1).unwrap()));
    assert_eq!(member.resolved_nominal(), Some(&definition.value_type()));
    assert!(F::bind_enum_member(&mut member, &declaration, &foreign).is_err());
    let mut arm = F::case_arm(path, member.clone(), range).unwrap();
    F::bind_case_pattern(&mut arm, &declaration, &definition).unwrap();
    assert!(F::bind_case_pattern(&mut arm, &declaration, &foreign).is_err());
    let expression = F::expression(
        ExprKind::Case {
            value: Box::new(member),
            arms: vec![arm],
        },
        range,
    )
    .unwrap();
    let rewritten = expression.rewrite_name_paths(|name| {
        NamePath::from_segments(
            ["Alias"].into_iter().chain(name.segments().skip(1)),
            name.range(),
        )
        .ok()
    });
    let ExprKind::Case { value, arms } = rewritten.kind() else {
        panic!("case")
    };
    assert_eq!(value.resolved_enum(), Some(&definition.value(1).unwrap()));
    assert_eq!(
        arms[0].resolved_pattern(),
        Some(&definition.value(1).unwrap())
    );
    assert_eq!(arms[0].pattern().as_str(), "Alias.Mode.Cooling");
    let mut ty = F::value_type(ValueTypeSyntaxKind::Named(declaration.clone()), range).unwrap();
    F::bind_nominal_value_type(&mut ty, definition.value_type()).unwrap();
    assert_eq!(ty.resolved_nominal(), Some(&definition.value_type()));
    assert!(F::bind_nominal_value_type(&mut ty, foreign.value_type()).is_err());
    let projected = eqiora_lang::ValueTypeSyntax::from_checked(&definition.value_type(), |id| {
        (id == definition.id().erase()).then(|| declaration.clone())
    })
    .unwrap();
    assert!(matches!(projected.kind(), ValueTypeSyntaxKind::Named(name) if name == &declaration));
    assert_eq!(projected.resolved_nominal(), Some(&definition.value_type()));
    for bad in [["Wrong", "Mode", "Heating"], ["Controls", "Mode", "Absent"]] {
        let mut expression = F::expression(
            ExprKind::Path(NamePath::from_segments(bad, range).unwrap()),
            range,
        )
        .unwrap();
        assert!(F::bind_enum_member(&mut expression, &declaration, &definition).is_err());
    }
}
