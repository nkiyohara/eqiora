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
