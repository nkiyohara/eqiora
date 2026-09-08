use eqiora_compiler::compile;

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
    let source = "connector Pin=scalar_physical(across=V,through=A);component C(parameter n:integer,port p:conserving on Pin){indexset I=range(n);relation r[j in I]{across(p)=sum(1[V],over=(i in I));}} model M(){instance a:C(n=1);instance b:C(n=2);connect conserving a.p,b.p;}";
    let errors = compile("physical-contexts.eqi", source).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("changes the physical definition contract")),
        "{errors:?}"
    );
}
