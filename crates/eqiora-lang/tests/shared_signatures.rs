use eqiora_lang::{SignatureItem, SourceAstFactory, format, parse};

#[test]
fn models_and_components_share_complete_signatures() {
    for container in ["model M", "component M"] {
        let source = format!(
            "{container}(parameter gain:1=2, clock tick:periodic, input u:V at tick, output y:V at tick, support body:volume(ambient_dimension=2), state x:V on body at tick, property fluid:Viscosity) {{ relation r at tick {{ y=gain*u; }} }}"
        );
        let document = parse("signature.eqi", &source).into_document().unwrap();
        let signature = if document.models().is_empty() {
            document.components()[0].signature()
        } else {
            document.models()[0].signature()
        };
        assert_eq!(signature.len(), 7);
        assert!(matches!(signature[2], SignatureItem::Input(_)));
        assert!(matches!(signature[3], SignatureItem::Output(_)));
        let formatted = format(&document);
        let reparsed = parse("signature.eqi", &formatted).into_document().unwrap();
        assert_eq!(format(&reparsed), formatted);
    }
}

#[test]
fn instances_have_only_category_free_named_arguments() {
    let document=parse("binding.eqi","model M() { instance c:C(gain=2, body=region, tick=clock, fluid=water.viscosity, walls=boundaries(left,right)); }").into_document().unwrap();
    let eqiora_lang::Item::Instance(instance) = &document.models()[0].items()[0] else {
        panic!("instance")
    };
    assert_eq!(
        instance
            .bindings()
            .iter()
            .map(|binding| binding.name())
            .collect::<Vec<_>>(),
        ["gain", "body", "tick", "fluid", "walls"]
    );
    let native = SourceAstFactory::instance(
        instance.name(),
        instance.definition().clone(),
        instance.bindings().to_vec(),
        instance.range(),
    )
    .unwrap();
    assert_eq!(&native, instance);
    for rejected in [
        "model M {}",
        "model M() { instance c:C(clock tick=clock); }",
        "component C() { public parameter p:1=1; }",
        "component C(clock tick) {}",
        "model M() { boundary p; }",
    ] {
        assert!(
            parse("old.eqi", rejected).into_document().is_err(),
            "{rejected}"
        );
    }
}

#[test]
fn signature_comments_dimensions_and_private_clocked_ports_round_trip() {
    let source = "model M(\n /// Input documentation\n input u:Length at tick,\n clock tick:periodic,\n output y:Length at tick\n) { port hidden:signal input Length at tick; connect u -> hidden; relation r at tick { y=hidden; } }";
    let mut document = parse("interface.eqi", source).into_document().unwrap();
    assert!(
        document
            .doc_comment(document.models()[0].signature()[0].range())
            .is_some()
    );
    SourceAstFactory::rewrite_dimension_expressions(&mut document, |dimension| {
        if matches!(dimension.kind(),eqiora_lang::ExprKind::Name(name) if name=="Length") {
            SourceAstFactory::expression(eqiora_lang::ExprKind::Name("m".into()), dimension.range())
                .unwrap()
        } else {
            dimension.clone()
        }
    });
    let formatted = format(&document);
    assert!(formatted.contains("input u: m at tick"));
    assert!(formatted.contains("signal input m at tick"));
    assert!(formatted.contains("Input documentation"));
    assert_eq!(
        format(&parse("interface.eqi", &formatted).into_document().unwrap()),
        formatted
    );
    let model = &document.models()[0];
    let native = SourceAstFactory::model(
        model.visibility(),
        model.name(),
        model.signature().to_vec(),
        model.items().to_vec(),
        model.range(),
    )
    .unwrap();
    assert_eq!(native.signature(), model.signature());
}
