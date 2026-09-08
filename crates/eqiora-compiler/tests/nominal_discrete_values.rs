use eqiora_compiler::CompiledModel;

#[test]
fn source_counts_keep_exact_nominal_components_and_clocked_equations() {
    let source = r#"
space Species = orthonormal(A,B);
model Transfer(output observed: counts<Species> at tick) {
  clock tick = periodic(1[s]);
  parameter change: coordinates<integer,Species> = coordinates(Species,[-1,1]);
  state population: counts<Species> at tick;
  initial { pre(population) = counts(Species,[2,9007199254740993]); }
  relation update at tick { next(population) = pre(population) + change; observed = next(population); }
}
"#;
    let model = CompiledModel::compile_selected("counts.eqi", source, "Transfer", &[]);
    assert!(model.is_ok(), "{model:?}");
}

#[test]
fn indexed_members_and_transitive_extent_keep_structural_dependencies() {
    let source = r#"
component Cell(parameter value: integer, output y: integer at tick, clock tick: periodic) {
    relation emit at tick { y=value; }
}
model M(output first: integer at tick) {
    clock tick=periodic(1[s]);
    parameter n:integer=2;
    let extent=n+0;
    indexset Rows=range(extent);
    instance cell[i in Rows]:Cell(value=ordinal(i),tick=tick);
    connect cell[index(Rows,0)].y -> first;
}
"#;
    let compiled = CompiledModel::compile_selected("family.eqi", source, "M", &[])
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(compiled.symbols().get("cell[0].y").is_some());
    assert!(compiled.symbols().get("cell[1].y").is_some());
    let parameter = compiled.symbols().get("n").unwrap();
    let set = compiled.symbols().get("Rows").unwrap();
    assert!(compiled.transaction().ops().iter().any(|op| matches!(op,eqiora_graph::Op::Connect { from,to,edge:eqiora_graph::EdgeKind::DependsOn } if *from==set && *to==parameter)));
}

#[test]
fn named_input_arguments_drive_each_indexed_occurrence() {
    let source = r#"
component Cell(input u:1,output y:1) { relation emit {y=u;} }
model M(output first:1) {
    port drive:signal output 1;
    relation source {drive=7;}
    indexset Rows=range(2);
    instance cell[i in Rows]:Cell(u=drive);
    connect cell[index(Rows,0)].y -> first;
}
"#;
    let result = CompiledModel::compile_selected("family-input.eqi", source, "M", &[]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn family_selection_rejects_foreign_sets_bounds_and_mutable_selector_dependencies_are_retained() {
    let source = r#"
component Cell(output y:1) { relation emit {y=1;} }
model M(output observed:1) {
    parameter choice:integer=0;
    indexset Rows=range(2);
    indexset Other=range(2);
    instance cell[i in Rows]:Cell();
    connect cell[index(Rows,choice)].y -> observed;
}
"#;
    let model = CompiledModel::compile_selected("selector.eqi", source, "M", &[])
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    let set = model.symbols().get("Rows").unwrap();
    let parameter = model.symbols().get("choice").unwrap();
    assert!(model.transaction().ops().iter().any(|op|matches!(op,eqiora_graph::Op::Connect{from,to,edge:eqiora_graph::EdgeKind::DependsOn} if *from==set && *to==parameter)));
    for changed in [
        source.replace("index(Rows,choice)", "index(Other,choice)"),
        source.replace("index(Rows,choice)", "index(Rows,2)"),
    ] {
        assert!(CompiledModel::compile_selected("selector.eqi", &changed, "M", &[]).is_err());
    }
}

#[test]
fn unresolved_index_extent_cannot_be_published_as_a_checked_package_definition() {
    use eqiora_compiler::{CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit};
    let namespace = CompilationNamespaceId::new(["example.indexed", "1.0.0", "digest"]).unwrap();
    let source = "public component Family(parameter n:integer) { indexset Rows=range(n); }";
    let unit = ResolvedSourceUnit::new(namespace.clone(), "src/main.eqi", source).unwrap();
    let analysis = ResolvedHierarchyInput::new(namespace, vec![unit], vec![])
        .analyze_with_cancellation(|| false)
        .unwrap()
        .unwrap();
    let errors = analysis
        .validate_definitions()
        .err()
        .expect("unresolved extent must fail closed");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("unresolved IndexSet extent")),
        "{errors:?}"
    );
}

#[test]
fn ordinary_integer_instance_bindings_keep_required_type_context() {
    let source = r#"
component Cell(parameter value:integer,output y:integer at tick,clock tick:periodic) {
    relation emit at tick {y=value;}
}
model M(output observed:integer at tick) {
    clock tick=periodic(1[s]);
    instance cell:Cell(value=9007199254740993,tick=tick);
    connect cell.y -> observed;
}
"#;
    let model = CompiledModel::compile_selected("integer-binding.eqi", source, "M", &[]);
    assert!(model.is_ok(), "{model:?}");
    let fractional = source.replace("value=9007199254740993", "value=0.5");
    assert!(CompiledModel::compile_selected("integer-binding.eqi", &fractional, "M", &[]).is_err());
}

#[test]
fn selected_required_integer_extent_is_specialized_before_expansion() {
    use eqiora_compiler::StaticBindingValue;
    let source = r#"
component Cell(output y:1) { relation emit {y=1;} }
model M(parameter n:integer,output first:1) {
    indexset Rows=range(n);
    instance cell[i in Rows]:Cell();
    connect cell[index(Rows,0)].y -> first;
}
"#;
    let value = eqiora_lang::SourceAstFactory::expression(
        eqiora_lang::ExprKind::Number(eqiora_lang::DecimalLiteral::parse("2").unwrap()),
        eqiora_lang::TextRange::new(0, 0),
    )
    .unwrap();
    let result = CompiledModel::compile_selected(
        "selected-extent.eqi",
        source,
        "M",
        &[("n", StaticBindingValue::Expression(&value))],
    );
    let model = result.unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(model.symbols().get("cell[1].y").is_some());
    assert!(model.symbols().get("cell[2].y").is_none());
}
