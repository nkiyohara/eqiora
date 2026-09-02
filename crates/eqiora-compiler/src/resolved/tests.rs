mod additional_tests;
use eqiora_graph::{GraphStore, InMemoryGraphStore};

use super::*;

fn namespace(name: &str) -> CompilationNamespaceId {
    CompilationNamespaceId::new([name, "1.0.0", "semantic-digest"]).expect("namespace")
}

fn unit(namespace: &CompilationNamespaceId, file: &str, source: &str) -> ResolvedSourceUnit {
    ResolvedSourceUnit::new(namespace.clone(), file, source)
}

fn module_unit(
    namespace: &CompilationNamespaceId,
    module_name: &str,
    file: &str,
    source: &str,
) -> ResolvedSourceUnit {
    ResolvedSourceUnit::in_module(namespace.clone(), module_name.split('.'), file, source)
        .expect("module source unit")
}

fn alias(
    declaring: &CompilationNamespaceId,
    name: &str,
    target: &CompilationNamespaceId,
) -> ResolvedAlias {
    ResolvedAlias::new(declaring.clone(), name, target.clone())
}

const LIBRARY: &str = r#"
public component Resistor {
  public parameter resistance: 1;
  relation law continuous { resistance - 2 = 0; }
}
"#;

#[test]
fn module_labels_preserve_implicit_main_and_name_explicit_modules() {
    let owner = namespace("org.example.project");
    assert_eq!(
        CompilationModuleId::main(owner.clone()).to_string(),
        owner.to_string()
    );
    assert_eq!(
        CompilationModuleId::new(
            owner.clone(),
            ModuleName::new(["library", "primitives"]).expect("module name"),
        )
        .to_string(),
        format!("{owner}::library.primitives")
    );
}

#[test]
fn hierarchy_footprint_fails_before_source_input_allocation() {
    let limits = ResolvedHierarchyResourceLimits {
        source_units: 2,
        aliases: 1,
        source_unit_bytes: 8,
        total_source_bytes: 10,
    };
    assert!(
        preflight_resolved_hierarchy_with_limits([1], 2, limits)
            .expect_err("alias overflow")
            .message()
            .contains("direct-alias limit")
    );
    assert!(
        preflight_resolved_hierarchy_with_limits([1, 1, 1], 0, limits)
            .expect_err("source-unit overflow")
            .message()
            .contains("source-unit limit")
    );
    assert!(
        preflight_resolved_hierarchy_with_limits([9], 0, limits)
            .expect_err("per-source overflow")
            .message()
            .contains("byte hierarchy limit")
    );
    assert!(
        preflight_resolved_hierarchy_with_limits([6, 5], 0, limits)
            .expect_err("aggregate overflow")
            .message()
            .contains("total source-byte limit")
    );
    preflight_resolved_hierarchy_with_limits([4, 6], 1, limits).expect("exact footprint limit");
}

#[test]
fn parser_diagnostics_are_independent_of_source_unit_input_order() {
    let root = namespace("root");
    let units = vec![
        unit(&root, "z.eqi", "model Z { relation broken"),
        unit(&root, "a.eqi", "model A { parameter p:"),
    ];
    let forward = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
        root.clone(),
        units.clone(),
        vec![],
    ))
    .expect_err("both source units are invalid");
    let reverse = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
        root,
        units.into_iter().rev().collect(),
        vec![],
    ))
    .expect_err("input permutation remains invalid");

    assert_eq!(forward, reverse);
}

#[test]
fn exact_direct_alias_elaborates_with_cross_file_provenance() {
    let root = namespace("org.example.root");
    let electrical = namespace("org.eqiora.electrical");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root/main.eqi",
                "model Main { instance load: electrical.Resistor(resistance = 2); }",
            ),
            unit(&electrical, "electrical/resistor.eqi", LIBRARY),
        ],
        vec![alias(&root, "electrical", &electrical)],
    );

    let analysis = analyze_resolved_hierarchy(input).expect("resolved graph analyzes");
    assert_eq!(analysis.canonical_declarations().len(), 2);
    let compiled = analysis
        .validate_definitions()
        .expect("definitions validate")
        .compile_root("Main")
        .expect("root elaborates");
    assert!(
        compiled.symbols().get("load.resistance").is_none(),
        "literal component arguments do not fabricate Kernel Parameters"
    );
    let law = compiled
        .symbols()
        .get("load.law")
        .expect("imported relation symbol");
    let provenance = compiled.provenance().expect("hierarchy provenance");
    let source = provenance
        .get_by_graph_id(law)
        .expect("relation provenance");
    assert!(
        source
            .definition_span()
            .file
            .ends_with("electrical/resistor.eqi")
    );
    assert!(source.instance_span().file.ends_with("root/main.eqi"));
    assert!(source.binding_spans()[0].file.ends_with("root/main.eqi"));
    assert_ne!(
        source.definition_span().file,
        source.instance_span().file,
        "package-qualified source labels remain unambiguous"
    );

    let (transaction, _, _) = compiled.into_parts();
    InMemoryGraphStore::new()
        .commit(transaction)
        .expect("complete transaction commits");
}

#[test]
fn explicit_local_module_import_elaborates_component_and_operator() {
    let owner = namespace("org.example.project");
    let root_source = r#"
import library.primitives as lib;
model Main {
  domain d = box(0, 1, 0, 1);
  representation s = continuum;
  field a on d as s: 1 shape spatial_vector;
  field b on d as s: 1 shape spatial_vector;
  instance load: lib.Resistor(resistance = 2);
  relation doubled continuous on d { div(div(lib.outer(a, b))) = 0; }
}
"#;
    let library_source = r#"
public pure operator outer(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
public component Resistor {
  public parameter resistance: 1;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    let input = ResolvedHierarchyInput::with_root_module(
        owner.clone(),
        "models.main".split('.'),
        vec![
            ResolvedSourceUnit::in_module(
                owner.clone(),
                "library.primitives".split('.'),
                "src/library/primitives.eqi",
                library_source,
            )
            .expect("library module"),
            ResolvedSourceUnit::in_module(
                owner.clone(),
                "models.main".split('.'),
                "src/models/main.eqi",
                root_source,
            )
            .expect("root module"),
        ],
        vec![],
    )
    .expect("root module identity");

    let analysis = analyze_resolved_hierarchy(input).expect("local modules analyze");
    let compiled = analysis
        .validate_definitions()
        .expect("local module definitions validate")
        .compile_root("Main")
        .expect("qualified component and operator elaborate");
    assert!(compiled.symbols().get("load.law").is_some());
    assert!(compiled.symbols().get("doubled").is_some());
}

#[test]
fn one_logical_module_composes_multiple_source_units() {
    let owner = namespace("org.example.project");
    let input = ResolvedHierarchyInput::with_root_module(
        owner.clone(),
        ["models"],
        vec![
            module_unit(&owner, "models", "src/models/components.eqi", LIBRARY),
            module_unit(
                &owner,
                "models",
                "src/models/main.eqi",
                "model Main { instance load: Resistor(resistance = 2); }",
            ),
        ],
        vec![],
    )
    .expect("root module identity");

    let compiled = analyze_resolved_hierarchy(input)
        .expect("multi-source module analyzes")
        .validate_definitions()
        .expect("multi-source definitions validate")
        .compile_root("Main")
        .expect("declarations compose within one module");
    assert!(compiled.symbols().get("load.law").is_some());
}

#[test]
fn explicit_local_import_graph_rejects_missing_duplicate_reserved_and_cycle() {
    let owner = namespace("org.example.project");
    let analyze = |root_name: &str, sources: &[(&str, &str)]| {
        analyze_resolved_hierarchy(
            ResolvedHierarchyInput::with_root_module(
                owner.clone(),
                root_name.split('.'),
                sources
                    .iter()
                    .map(|(name, source)| {
                        ResolvedSourceUnit::in_module(
                            owner.clone(),
                            name.split('.'),
                            format!("src/{}.eqi", name.replace('.', "/")),
                            *source,
                        )
                        .expect("module source unit")
                    })
                    .collect(),
                vec![],
            )
            .expect("root module identity"),
        )
    };

    let missing = analyze(
        "models.main",
        &[(
            "models.main",
            "import library.missing as lib; model Main {}",
        )],
    )
    .expect_err("missing local module");
    assert!(missing.iter().any(|diagnostic| {
        diagnostic.message().contains("unknown target module") && diagnostic.source_span().is_some()
    }));

    let missing_export = analyze(
        "models.main",
        &[
            (
                "models.main",
                "import library.one as lib; model Main { instance value: lib.Missing; }",
            ),
            ("library.one", "public component One {}"),
        ],
    )
    .expect_err("missing exported declaration");
    assert!(missing_export.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("unresolved component `lib.Missing`")
            && diagnostic.source_span().is_some()
    }));

    let duplicate = analyze(
        "models.main",
        &[
            (
                "models.main",
                "import library.one as lib; import library.two as lib; model Main {}",
            ),
            ("library.one", "public component One {}"),
            ("library.two", "public component Two {}"),
        ],
    )
    .expect_err("duplicate alias");
    assert!(duplicate.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("duplicate direct alias `lib`")
    }));

    let reserved = analyze(
        "models.main",
        &[
            ("models.main", "import library.one as math; model Main {}"),
            ("library.one", "public component One {}"),
        ],
    )
    .expect_err("reserved root");
    assert!(
        reserved
            .iter()
            .any(|diagnostic| diagnostic.message().contains("alias `math` is reserved"))
    );

    let cycle = analyze(
        "modules.a",
        &[
            ("modules.a", "import modules.b as b; model Main {}"),
            ("modules.b", "import modules.a as a; public component B {}"),
        ],
    )
    .expect_err("import cycle");
    assert!(cycle.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("semantic module import cycle")
            && diagnostic.source_span().is_some()
    }));
}

#[test]
fn canonical_declarations_normalize_aliases_to_exact_targets() {
    let root = namespace("root");
    let target = namespace("target");
    let renamed = |alias_name: &str| {
        ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    &format!("model Main {{ instance c: {alias_name}.Resistor; }}"),
                ),
                unit(&target, "target.eqi", LIBRARY),
            ],
            vec![alias(&root, alias_name, &target)],
        )
    };
    let first = analyze_resolved_hierarchy(renamed("electrical")).expect("first alias");
    let second = analyze_resolved_hierarchy(renamed("components")).expect("renamed alias");
    assert_eq!(
        first.canonical_declarations(),
        second.canonical_declarations(),
        "resolution aliases are not package semantics"
    );

    let other_target = namespace("other-target");
    let changed = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root.eqi",
                "model Main { instance c: electrical.Resistor; }",
            ),
            unit(&other_target, "target.eqi", LIBRARY),
        ],
        vec![alias(&root, "electrical", &other_target)],
    ))
    .expect("changed exact target");
    let root_form = |analysis: &AnalyzedResolvedHierarchy| {
        analysis
            .canonical_declarations()
            .iter()
            .find(|declaration| declaration.namespace() == &root && declaration.path() == "Main")
            .expect("root declaration")
            .canonical_form()
            .to_owned()
    };
    assert_ne!(root_form(&first), root_form(&changed));
}

#[test]
fn pure_operator_declarations_and_calls_are_file_and_alias_invariant() {
    let root = namespace("root");
    let operators = namespace("operators");
    let dependency = r#"
public pure operator outer(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
"#;
    let analyzed = |alias_name: &str, operator_file: &str| {
        analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(
                    &root,
                    "root.eqi",
                    &format!(
                        "model Main {{ domain d = box(0,1,0,1); representation s = continuum; field a on d as s: 1 shape spatial_vector; field b on d as s: 1 shape spatial_vector; relation r continuous on d {{ div(div({alias_name}.outer(a,b))) = 0; }} }}"
                    ),
                ),
                unit(&operators, operator_file, dependency),
            ],
            vec![alias(&root, alias_name, &operators)],
        ))
        .expect("resolved pure operator")
    };

    let first = analyzed("ops", "a/operator.eqi");
    let renamed = analyzed("tensor_ops", "relocated/definition.eqi");
    assert_eq!(
        first.canonical_declarations(),
        renamed.canonical_declarations()
    );
    let operator = first
        .canonical_declarations()
        .iter()
        .find(|declaration| declaration.kind() == CanonicalDeclarationKind::PureOperator)
        .expect("pure declaration");
    assert!(
        operator
            .canonical_form()
            .starts_with("eqiora.pure-operator-definition.v1:sha256:")
    );
}

#[test]
fn private_pure_operator_cannot_cross_an_exact_package_boundary() {
    let root = namespace("root");
    let dependency = namespace("operators");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root.eqi",
                "model Main { domain d = box(0,1); representation s = continuum; field a on d as s: 1 shape spatial_vector; field b on d as s: 1 shape spatial_vector; relation r continuous on d { div(ops.outer(a,b)) = 0; } }",
            ),
            unit(
                &dependency,
                "operator.eqi",
                "private pure operator outer(a: spatial[1], b: spatial[1]) -> spatial[2] = component(a,0) * component(b,1);",
            ),
        ],
        vec![alias(&root, "ops", &dependency)],
    );
    let diagnostics = analyze_resolved_hierarchy(input)
        .expect("global package shape")
        .validate_definitions()
        .expect_err("private exact definitions are not importable");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("private pure operator `ops.outer` cannot be imported")
    }));
}

#[test]
fn private_unknown_and_transitive_imports_fail_during_analysis() {
    let root = namespace("root");
    let dependency = namespace("dependency");
    let cases = [
        (
            "model Main { instance c: dep.Private; }",
            "component Private {}",
            "private component `dep.Private` cannot be imported",
        ),
        (
            "model Main { instance c: missing.C; }",
            "public component C {}",
            "unknown direct package alias `missing`",
        ),
        (
            "model Main { instance c: dep.nested.C; }",
            "public component C {}",
            "uses transitive or member qualification",
        ),
    ];
    for (root_source, dependency_source, expected) in cases {
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![
                unit(&root, "root.eqi", root_source),
                unit(&dependency, "dependency.eqi", dependency_source),
            ],
            vec![alias(&root, "dep", &dependency)],
        );
        let diagnostics = analyze_resolved_hierarchy(input).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "expected `{expected}`, got {diagnostics:#?}"
        );
    }
}

#[test]
fn package_local_names_do_not_collide_but_duplicates_and_aliases_do() {
    let root = namespace("root");
    let first = namespace("first");
    let second = namespace("second");
    let valid = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root.eqi",
                "model Main { instance a: one.C; instance b: two.C; }",
            ),
            unit(
                &first,
                "first.eqi",
                "public component C { parameter p: 1 = 1; relation law continuous { p - 1 = 0; } }",
            ),
            unit(
                &second,
                "second.eqi",
                "public component C { parameter p: 1 = 2; relation law continuous { p - 2 = 0; } }",
            ),
        ],
        vec![alias(&root, "one", &first), alias(&root, "two", &second)],
    );
    let analysis = analyze_resolved_hierarchy(valid).expect("names are package-local");
    let compiled = analysis
        .validate_definitions()
        .expect("definitions validate")
        .compile_root("Main")
        .expect("both definitions resolve");
    let (transaction, _, _) = compiled.into_parts();
    InMemoryGraphStore::new()
        .commit(transaction)
        .expect("both package-local definitions elaborate atomically");

    let duplicate = ResolvedHierarchyInput::new(
        first.clone(),
        vec![unit(
            &first,
            "a.eqi",
            "public component C {} public component C {}",
        )],
        vec![],
    );
    let diagnostics = analyze_resolved_hierarchy(duplicate).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("duplicate component declaration `C`")
    }));

    let duplicate_alias = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(&root, "root.eqi", "model Main {}"),
            unit(&first, "first.eqi", "public component C {}"),
            unit(&second, "second.eqi", "public component D {}"),
        ],
        vec![alias(&root, "lib", &first), alias(&root, "lib", &second)],
    );
    let diagnostics = analyze_resolved_hierarchy(duplicate_alias).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("duplicate direct alias `lib`")
    }));
}

#[test]
fn cross_package_recursion_fails_before_a_transaction_exists() {
    let root = namespace("root");
    let dependency = namespace("dependency");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![
            unit(
                &root,
                "root.eqi",
                "public component A { instance b: dep.B; } model Main {}",
            ),
            unit(
                &dependency,
                "dependency.eqi",
                "public component B { instance a: app.A; }",
            ),
        ],
        vec![
            alias(&root, "dep", &dependency),
            alias(&dependency, "app", &root),
        ],
    );
    let analysis = analyze_resolved_hierarchy(input).expect("all names resolve");
    let diagnostics = analysis.validate_definitions().unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("recursive component definition graph")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.graph_path().is_none())
    );
}

#[test]
fn unused_connector_contract_fails_definition_validation() {
    let root = namespace("root");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![unit(
            &root,
            "root.eqi",
            "public connector Broken = scalar_physical(across = mystery, through = A); model Main {}",
        )],
        vec![],
    );
    let analysis = analyze_resolved_hierarchy(input).expect("declarations index");
    let diagnostics = analysis.validate_definitions().unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("unknown SI base-dimension symbol `mystery`")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.graph_path().is_none())
    );
}

#[test]
fn symbolic_component_interfaces_validate_without_occurrence_values() {
    let root = namespace("root");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![unit(
            &root,
            "root.eqi",
            r#"
public component Leaf {
  public parameter period: s;
  public parameter offset: s = period;
  relation invariant continuous { offset - period = 0; }
}
public component Wrapper {
  public parameter period: s;
  instance leaf: Leaf(period = period);
}
model Empty {}
"#,
        )],
        vec![],
    );
    analyze_resolved_hierarchy(input)
        .expect("declarations analyze")
        .validate_definitions()
        .expect("required public Parameters remain typed free variables");
}

#[test]
fn unused_nested_parameter_contracts_fail_before_root_selection() {
    let root = namespace("root");
    let input = ResolvedHierarchyInput::new(
        root.clone(),
        vec![unit(
            &root,
            "root.eqi",
            r#"
public component Leaf { public parameter period: s; }
public component Missing { instance leaf: Leaf; }
public component WrongDimension {
  public parameter length: m;
  instance leaf: Leaf(period = length);
}
public component InvalidPrivate { parameter hidden: s; }
model Empty {}
"#,
        )],
        vec![],
    );
    let diagnostics = analyze_resolved_hierarchy(input)
        .expect("declarations analyze")
        .validate_definitions()
        .unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("required Parameter `period` has no instance binding")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("Parameter binding has dimension")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("required private Parameter `hidden` has no default")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.graph_path().is_none())
    );
}
