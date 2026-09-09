use eqiora_lang::{
    ActivationSyntax, ComponentItem, ConnectionSyntax, Item, SourceAstFactory, format, parse,
};

#[test]
fn model_and_component_index_families_share_ordered_syntax_and_native_factories() {
    let body = r#"
  /// One equation per exact member.
  relation law[i in Stages] on body at tick { cell[index(Stages, ordinal(i))].value = ordinal(i); }
  connect [j in Links] cell[index(Stages, ordinal(j))].negative, cell[index(Stages, ordinal(j) + 1)].positive;
  connect [i in Stages] cell[index(Stages, ordinal(i))].output -> sink[index(Stages, ordinal(i))].input;
"#;
    for declaration in ["model M()", "component C()"] {
        let source = format!("{declaration} {{{body}}}");
        let document = parse("families.eqi", &source).into_document().unwrap();
        let (relation, conserving, signal) = if declaration.starts_with("model") {
            let items = document.models()[0].items();
            let [
                Item::RelationFamily(relation),
                Item::Connection(conserving),
                Item::Connection(signal),
            ] = items
            else {
                panic!("model families")
            };
            (relation, conserving, signal)
        } else {
            let items = document.components()[0].items();
            let [
                ComponentItem::RelationFamily(relation),
                ComponentItem::Connection(conserving),
                ComponentItem::Connection(signal),
            ] = items
            else {
                panic!("component families")
            };
            (relation, conserving, signal)
        };
        assert_eq!(relation.binder().member(), "i");
        assert_eq!(relation.binder().set().as_str(), "Stages");
        assert_eq!(relation.relation().domain(), Some("body"));
        assert_eq!(
            relation.relation().activation(),
            &ActivationSyntax::Named("tick".to_owned())
        );
        assert!(document.doc_comment(relation.range()).is_some());
        let constructed = SourceAstFactory::relation_family(
            relation.relation().clone(),
            relation.binder().clone(),
        )
        .unwrap();
        assert_eq!(constructed, *relation);
        for (connection, syntax, binder_name) in [
            (conserving, ConnectionSyntax::Conserving, "j"),
            (signal, ConnectionSyntax::Signal, "i"),
        ] {
            assert_eq!(connection.syntax(), syntax);
            assert_eq!(connection.binder().unwrap().member(), binder_name);
            let constructed = SourceAstFactory::connection(
                syntax,
                connection.binder().cloned(),
                connection.port_expressions().to_vec(),
                connection.range(),
            )
            .unwrap();
            assert_eq!(constructed.syntax(), connection.syntax());
            assert_eq!(constructed.binder(), connection.binder());
            assert_eq!(
                constructed.port_expressions(),
                connection.port_expressions()
            );
            assert_eq!(constructed.range(), connection.range());
        }
        let rendered = format(&document);
        assert_eq!(
            format(&parse("again.eqi", &rendered).into_document().unwrap()),
            rendered
        );
        assert!(rendered.contains("connect [j in Links]"));
        assert!(rendered.contains("connect [i in Stages]"));
    }
}

#[test]
fn binder_clauses_reject_duplicates_reordering_and_periodic_identifications() {
    for source in [
        "relation r on body[i in Stages] { 0 = 0; }",
        "relation r[i in Stages][j in Links] { 0 = 0; }",
        "relation r[i in Stages] at tick on body { 0 = 0; }",
        "connect [i in Stages][j in Links] a, b;",
        "connect periodic [i in Stages] a, b;",
        "connect [i in Stages] a, b;",
    ] {
        assert!(
            parse("bad.eqi", &format!("model M() {{ {source} }}"))
                .into_document()
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn exact_boundary_selector_connections_keep_their_distinct_representation() {
    let source = "component C() { connect [b in exterior] left.p[face = b], right.p[face = b]; }";
    let document = parse("boundary.eqi", source).into_document().unwrap();
    let ComponentItem::BoundaryConnection(connection) = &document.components()[0].items()[0] else {
        panic!("boundary selection")
    };
    assert_eq!(connection.binder().unwrap().member(), "b");
    assert!(
        connection
            .ports()
            .iter()
            .all(|port| port.selector().unwrap().target() == "b")
    );
    let rendered = format(&document);
    assert_eq!(
        format(&parse("again.eqi", &rendered).into_document().unwrap()),
        rendered
    );
}
