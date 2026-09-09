//! Structural aliases consume the existing exact, acyclic module import graph.
use super::*;
use crate::resolved::{AnalyzedSourceUnit, ResolvedAlias};

pub(crate) fn bind_resolved(
    units: &mut [AnalyzedSourceUnit],
    imports: &[ResolvedAlias],
) -> Result<(), Vec<Diagnostic>> {
    let index = units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.module.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut edges = vec![Vec::new(); units.len()];
    let mut consumers = vec![Vec::new(); units.len()];
    for edge in imports {
        let owner = index[edge.declaring_module()];
        let target = index[edge.target_module()];
        edges[owner].push((target, edge.alias()));
        consumers[target].push(owner);
    }
    let mut remaining = edges.iter().map(Vec::len).collect::<Vec<_>>();
    let mut ready = remaining
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<BTreeSet<_>>();
    let mut resolved = vec![BTreeMap::<String, DimExponents>::new(); units.len()];
    let mut completed = 0;
    while let Some(index) = ready.pop_first() {
        let mut visible = BTreeMap::new();
        for &(target, alias) in &edges[index] {
            for (name, value) in &resolved[target] {
                visible.insert(format!("{alias}.{name}"), *value);
            }
        }
        let unit = &mut units[index];
        let visible = resolve_aliases(&unit.file, &unit.document, visible)?;
        resolved[index] = unit
            .document
            .dimensions()
            .iter()
            .filter(|declaration| declaration.visibility() == eqiora_lang::VisibilitySyntax::Public)
            .map(|declaration| (declaration.name().to_owned(), visible[declaration.name()]))
            .collect();
        rewrite_document(&mut unit.document, &visible);
        completed += 1;
        for &consumer in &consumers[index] {
            remaining[consumer] -= 1;
            if remaining[consumer] == 0 {
                ready.insert(consumer);
            }
        }
    }
    if completed != units.len() {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            "modules",
            TextRange::new(0, 0),
            "dimension resolution requires an acyclic explicit module graph",
        )]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolved::{
        ResolvedDependency, ResolvedHierarchyInput, ResolvedSourceUnit, analyze_resolved_hierarchy,
    };

    fn analyze(
        library: &str,
        consumer: &str,
        reverse: bool,
    ) -> Result<crate::AnalyzedResolvedHierarchy, Vec<Diagnostic>> {
        let root = crate::CompilationNamespaceId::new(["org.example.consumer"]).unwrap();
        let package = crate::CompilationNamespaceId::new(["org.example.library"]).unwrap();
        let mut units = vec![
            ResolvedSourceUnit::new(package.clone(), "src/units.eqi", library).unwrap(),
            ResolvedSourceUnit::new(root.clone(), "src/main.eqi", consumer).unwrap(),
        ];
        if reverse {
            units.reverse();
        }
        analyze_resolved_hierarchy(
            ResolvedHierarchyInput::with_root_module(
                root.clone(),
                ["main"],
                units,
                vec![ResolvedDependency::new(root, package)],
            )
            .unwrap(),
        )
    }

    #[test]
    fn public_dimensions_resolve_forward_dependencies_and_exact_external_modules() {
        let library = "public dimension Speed = Length / s; dimension Length = m;";
        let source = "import org.example.library.units as u; dimension Rate = u.Speed / s; model Main() { variable x: Rate; initial { x = 0; } relation law { x = 0; } }";
        let expected = DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).unwrap();
        for reverse in [false, true] {
            let analysis = analyze(library, source, reverse).expect("exact imported dimensions");
            assert_eq!(analysis.dimension_alias("u.Speed").unwrap(), expected);
            assert!(analysis.dimension_alias("u.Length").is_err());
            assert!(analysis.dimension_alias("u.other.Speed").is_err());
            let (_, use_file, use_range, definition_file, definition_range) = analysis
                .resolved_references()
                .find(|(declaration, file, _, _, _)| {
                    declaration.path() == "units.Speed" && file.ends_with("src/main.eqi")
                })
                .expect("qualified alias retains its declaration reference");
            assert!(use_file.ends_with("src/main.eqi"));
            assert!(definition_file.ends_with("src/units.eqi"));
            assert_eq!(
                &source[use_range.start() as usize..use_range.end() as usize],
                "u.Speed"
            );
            assert_eq!(
                &library[definition_range.start() as usize..definition_range.end() as usize],
                "public dimension Speed = Length / s;"
            );
            assert!(
                analysis
                    .resolved_declarations()
                    .any(|(declaration, _, _)| declaration.kind()
                        == crate::CanonicalDeclarationKind::Dimension
                        && declaration.visibility() == eqiora_lang::VisibilitySyntax::Public)
            );
            analysis
                .validate_definitions()
                .expect("validate all definitions")
                .compile_root("Main")
                .expect("ordinary executable model");
        }
        let local = eqiora_lang::parse("local.eqi", "dimension Speed = Length / s; model Main() { variable x: Speed; } dimension Length = m;").into_document().unwrap();
        super::super::elaborate_dimension_aliases("local.eqi", &local)
            .expect("complete local scope");
    }

    #[test]
    fn invalid_and_unused_dimension_definitions_reject() {
        let consumer =
            "import org.example.library.units as u; model Main() { variable x: u.Speed; }";
        for library in [
            "dimension Speed = m / s;",
            "public dimension Other = m / s;",
            "public dimension Speed = Missing;",
            "public dimension Speed = Other; dimension Other = Speed;",
            "public dimension Speed = m; dimension Speed = s;",
            "public dimension Speed = m; dimension Broken = Broken;",
            "public dimension Speed = m; dimension math = s;",
        ] {
            assert!(
                analyze(library, consumer, false)
                    .and_then(|analysis| analysis.validate_definitions())
                    .is_err(),
                "{library}"
            );
        }
        assert!(
            analyze(
                "public dimension Speed = m / s;",
                &consumer.replace("u.Speed", "u.transitive.Speed"),
                false
            )
            .and_then(|analysis| analysis.validate_definitions())
            .is_err()
        );
        let quantities = "import org.example.library.units as u; model Main() { parameter x: u.Speed = 1 [u.Speed]; }";
        assert!(
            analyze("public dimension Speed = m / s;", quantities, false)
                .unwrap()
                .validate_definitions()
                .is_err(),
            "dimensions never extend input units"
        );
    }
}
