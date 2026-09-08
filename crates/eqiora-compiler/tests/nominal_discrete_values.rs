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
