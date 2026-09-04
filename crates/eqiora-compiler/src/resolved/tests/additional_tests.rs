use super::*;

#[test]
fn canonical_declarations_ignore_files_formatting_and_input_order() {
    let package = namespace("org.example.library");
    let split = ResolvedHierarchyInput::with_root_module(
        package.clone(),
        "parts.component".split('.'),
        vec![
            module_unit(
                &package,
                "parts.component",
                "z/component.eqi",
                "public component C { public parameter p: 1 = 2; public parameter q: 1 = 3; }",
            ),
            module_unit(
                &package,
                "parts.connector",
                "a/connector.eqi",
                "public connector Pin = scalar_physical(across = 1, through = 1);",
            ),
        ],
        vec![],
    )
    .expect("split root module");
    let moved = ResolvedHierarchyInput::with_root_module(
        package.clone(),
        "parts.component".split('.'),
        vec![
            module_unit(
                &package,
                "parts.connector",
                "elsewhere/pin.eqi",
                "// relocated\npublic connector Pin=scalar_physical(across=1,through=1);",
            ),
            module_unit(
                &package,
                "parts.component",
                "elsewhere/c.eqi",
                "public component C {\n public parameter q: 1=3;\n public parameter p: 1=2;\n}",
            ),
        ],
        vec![],
    )
    .expect("moved root module");

    let first = analyze_resolved_hierarchy(split).expect("first analysis");
    let second = analyze_resolved_hierarchy(moved).expect("second analysis");
    assert_eq!(
        first.canonical_declarations(),
        second.canonical_declarations()
    );
    assert_eq!(
        first.canonical_declarations()[0].path(),
        "parts.component.C",
        "canonical declarations sort by path"
    );
    assert!(
        first.canonical_declarations()[0]
            .canonical_form()
            .starts_with("eqiora.source-declaration.v1:sha256:")
    );
}

#[test]
fn compiler_owned_math_root_cannot_be_a_package_alias() {
    let root = namespace("root");
    let dependency = namespace("dependency");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root.eqi",
                "import dependency.main as math; model Main {}",
            ),
            unit(&dependency, "dependency.eqi", "public component C {}"),
        ],
        vec![dependency_edge(&root, &dependency)],
    );
    let diagnostics = analyze_resolved_hierarchy(input).expect_err("math alias must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("direct alias `math` is reserved for compiler-owned scalar mathematics")
    }));
}

#[test]
fn compiler_owned_math_root_cannot_be_a_property_declaration() {
    for source in [
        "public property contract math { scalar value: 1; } model Main {}",
        "public property contract C { scalar value: 1; } public property release math implements C { value = 1; source_unit: 1 = 1; validity = unconditional; citation = org.example; license = spdx.CC0_1_0; } model Main {}",
    ] {
        let root = namespace("root");
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![unit(&root, "root.eqi", source)],
            vec![],
        );
        let diagnostics =
            analyze_resolved_hierarchy(input).expect_err("math declaration must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("identifier `math` is reserved for compiler-owned scalar mathematics")
        }));
    }
}

#[test]
fn hierarchy_does_not_resolve_bare_sin_as_a_user_operator() {
    let root = namespace("root");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![unit(
            &root,
            "root.eqi",
            "public pure operator sin(value: scalar) -> scalar = rational(1, 1); component C { public support body: volume(ambient_dimension = 1); representation space = continuum; field value on body as space: 1 = 0; relation law continuous on body { value - sin(0) = 0; } }",
        )],
        vec![],
    );
    let diagnostics = analyze_resolved_hierarchy(input)
        .expect("resolved graph shape")
        .validate_definitions()
        .expect_err("bare sin call must remain invalid");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("bare `sin` is not language vocabulary")
        }),
        "{diagnostics:#?}"
    );
}
