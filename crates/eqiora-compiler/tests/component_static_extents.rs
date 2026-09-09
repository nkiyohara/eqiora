use eqiora_compiler::compile;

#[test]
fn selected_closed_values_keep_target_units_without_relaxing_source_call_sites() {
    let source = "public component Length(parameter value:m){relation r{value-value=0;}}";
    let expressions = eqiora_lang::parse(
        "inputs.eqi",
        "model Inputs(){parameter literal:1=-2;parameter wrong:s=2[s];parameter general:1=1+1;}",
    )
    .into_document()
    .unwrap();
    let values = expressions.models()[0]
        .items()
        .iter()
        .filter_map(|item| match item {
            eqiora_lang::Item::Parameter(value) => Some(value.value()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (position, expression) in values.iter().enumerate() {
        let compiled = eqiora_compiler::CompiledModel::compile_selected(
            "closed.eqi",
            source,
            "Length",
            &[(
                "value",
                eqiora_compiler::StaticBindingValue::Expression(expression),
            )],
        );
        if position == 0 {
            compiled.unwrap();
        } else {
            let errors = compiled.unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains("dimension")),
                "{errors:?}"
            );
        }
    }
    let errors = compile(
        "call.eqi",
        &format!("{source} model M(){{instance length:Length(value=-2);}}"),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("dimension")),
        "{errors:?}"
    );
}

#[test]
fn concrete_instances_bind_independent_static_reduction_extents() {
    let source = "component Total(parameter n:integer,output y:1){indexset I=range(n);relation r{y=sum(to_real(ordinal(i)),over=(i in I));}} model M(){instance a:Total(n=2);instance b:Total(n=3);}";
    let models =
        compile("component-extents.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    let mut additions = models[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|op| match op {
            eqiora_graph::Op::DefineKernelNode {
                node: eqiora_schema::kernel::KernelNode::Relation(relation),
            } => Some(
                relation
                    .expression()
                    .nodes()
                    .iter()
                    .filter(|node| matches!(node, eqiora_schema::kernel::ExprNode::Add(..)))
                    .count(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    additions.sort_unstable();
    assert_eq!(additions, [1, 2]);
    let forwarded = source.replace("indexset I=range(n);", "let count=n+0;indexset I=range(count);")
        .replace("model M()", "component Wrapper(parameter n:integer,output y:1){instance child:Total(n=n);relation r{y=child.y;}} model M()")
        .replace("instance a:Total(n=2);instance b:Total(n=3);", "instance a:Wrapper(n=2);instance b:Wrapper(n=3);");
    compile("forwarded-extents.eqi", &forwarded).unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn actual_product_extent_must_match_the_declared_output_dimension() {
    let source = "component Product(parameter n:integer,output y:m^3){indexset I=range(n);relation r{y=product(2[m],over=(i in I));}} model M(){instance c:Product(n=3);}";
    compile("product-extent.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    for n in [0, 2, 1_000_000_000] {
        let errors = compile(
            "product-extent.eqi",
            &source.replace("n=3", &format!("n={n}")),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.source_span().is_some()),
            "{errors:?}"
        );
    }
}

#[test]
fn every_context_and_unused_definition_retains_body_validation() {
    let definition = "component Product(parameter n:integer,output y:m^3){indexset I=range(n);relation r{y=product(2[m],over=(i in I));}}";
    let errors = compile(
        "contexts.eqi",
        &format!("{definition} model M(){{instance good:Product(n=3);instance bad:Product(n=2);}}"),
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("dimension")),
        "{errors:?}"
    );
    for unused in [
        "component Bad(parameter n:integer,output y:1){indexset I=range(n);relation r{y=missing+sum(to_real(ordinal(i)),over=(i in I));}}",
        "component Bad(){relation r{missing=0;}}",
    ] {
        let errors = compile(
            "unused.eqi",
            &format!("{unused} model M(){{relation r{{1=1;}}}}"),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.source_span().is_some()),
            "{errors:?}"
        );
    }
    let capture = "component Total(parameter n:integer,parameter i:integer,output y:1){indexset I=range(n);relation r{y=sum(to_real(ordinal(i)),over=(i in I));}} model M(){instance c:Total(n=3,i=0);}";
    assert!(
        compile("capture.eqi", capture)
            .unwrap_err()
            .iter()
            .any(|error| error.message().contains("collides"))
    );
}

#[test]
fn context_dependent_physical_proofs_are_not_reused() {
    let source = "connector Pin {\n  across voltage: V;\n  through current: A;\n}component C(parameter n:integer,port p:Pin){indexset I=range(n);relation r[j in I]{p.voltage=sum(1[V],over=(i in I));}} model M(){instance a:C(n=1);instance b:C(n=2);connect a.p,b.p;}";
    let errors = compile("physical-contexts.eqi", source).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("cannot have more than one owning Relation")),
        "{errors:?}"
    );
}

#[test]
fn unrelated_symbolic_models_keep_their_existing_admission() {
    let generic = "component Cell(output y:1){relation r{y=0;}} public model Generic(parameter n:integer){indexset I=range(n);instance cell[i in I]:Cell();}";
    let baseline = format!("{generic} model M(){{relation r{{1=1;}}}}");
    eqiora_compiler::CompiledModel::compile_selected("generic.eqi", &baseline, "M", &[])
        .unwrap_or_else(|errors| panic!("baseline: {errors:?}"));
    let concrete = format!(
        "component Total(parameter n:integer,output y:1){{indexset I=range(n);relation r{{y=sum(to_real(ordinal(i)),over=(i in I));}}}} {generic} model M(){{instance total:Total(n=3);}}"
    );
    eqiora_compiler::CompiledModel::compile_selected("generic.eqi", &concrete, "M", &[])
        .unwrap_or_else(|errors| panic!("with reduction: {errors:?}"));
    let nested = concrete.replace("public model Generic(parameter n:integer){indexset I=range(n);instance cell[i in I]:Cell();}",
        "component Holder(parameter n:integer){indexset I=range(n);instance cell[i in I]:Cell();} public model Generic(parameter n:integer){instance holder:Holder(n=n);}");
    eqiora_compiler::CompiledModel::compile_selected("generic.eqi", &nested, "M", &[])
        .unwrap_or_else(|errors| panic!("symbolic nested extent: {errors:?}"));
    let known_only = concrete.replace("model M(){instance total:Total(n=3);}",
        "public model Known(parameter unused:integer){instance total:Total(n=3);} model M(){relation r{1=1;}}");
    eqiora_compiler::CompiledModel::compile_selected("generic.eqi", &known_only, "M", &[])
        .unwrap_or_else(|errors| panic!("unused required Parameter: {errors:?}"));
    let invalid = concrete.replace(
        "instance cell[i in I]:Cell();",
        "instance cell[i in I]:Cell();relation bad{missing=0;}",
    );
    let errors =
        eqiora_compiler::CompiledModel::compile_selected("generic.eqi", &invalid, "M", &[])
            .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("missing")),
        "{errors:?}"
    );
}

#[test]
fn selecting_a_generic_model_requires_its_actual_nonempty_bounded_extent() {
    let source = "component Cell(output y:1){relation r{y=0;}} public model Generic(parameter n:integer){indexset I=range(n);instance cell[i in I]:Cell();}";
    for n in [0, 2, 1_000_000_000] {
        let value = eqiora_lang::SourceAstFactory::expression(
            eqiora_lang::ExprKind::Number(
                eqiora_lang::DecimalLiteral::parse(&n.to_string()).unwrap(),
            ),
            Default::default(),
        )
        .unwrap();
        let compiled = eqiora_compiler::CompiledModel::compile_selected(
            "selected-generic.eqi",
            source,
            "Generic",
            &[("n", eqiora_compiler::StaticBindingValue::Expression(&value))],
        );
        if n == 2 {
            let model = compiled.unwrap();
            assert!(model.symbols().get("cell[0].y").is_some());
            assert!(model.symbols().get("cell[1].y").is_some());
            assert!(model.symbols().get("cell[2].y").is_none());
        } else {
            assert!(
                compiled
                    .unwrap_err()
                    .iter()
                    .any(|error| error.source_span().is_some())
            );
        }
    }
}
