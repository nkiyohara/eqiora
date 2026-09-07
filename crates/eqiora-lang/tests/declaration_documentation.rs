use eqiora_lang::{Item, SourceAstFactory, format, parse};

#[test]
fn declaration_documentation_formats_structurally_and_survives_document_reconstruction() {
    let source = "/// Model summary.\nmodel M{\n/// State summary.\nfield x:1=0; // keep with x\n// balance\nrelation r{x=0;}\n}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let model = &document.models()[0];
    assert_eq!(
        document.doc_comment(model.range()).unwrap().summary(),
        "Model summary."
    );
    let Item::Field(field) = &model.items()[0] else {
        panic!("Field")
    };
    assert_eq!(
        document.doc_comment(field.range()).unwrap().summary(),
        "State summary."
    );
    let reconstructed = SourceAstFactory::flat_document(vec![model.clone()]).unwrap();
    let formatted = format(&reconstructed);
    assert_eq!(
        formatted,
        "/// Model summary.\nmodel M {\n  /// State summary.\n  field x: 1 = 0; // keep with x\n  // balance\n  relation r {\n    x = 0;\n  }\n}\n"
    );
    let reparsed = parse("formatted.eqi", &formatted).into_document().unwrap();
    assert_eq!(format(&reparsed), formatted);
}

#[test]
fn ordinary_leading_comment_and_doc_block_share_the_reconstructed_declaration() {
    let source = "// motivation\n/// Summary.\nmodel M {}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let rebuilt = SourceAstFactory::flat_document(document.models().to_vec()).unwrap();
    let formatted = format(&rebuilt);
    assert_eq!(formatted, "// motivation\n/// Summary.\nmodel M {\n}\n");
    assert_eq!(
        format(&parse("formatted.eqi", &formatted).into_document().unwrap()),
        formatted
    );
}

#[test]
fn component_signature_and_inline_equation_trivia_have_a_canonical_roundtrip() {
    let source = "/// Library import.\nimport lib as lib;\n/// Component summary.\ncomponent C {\n/// Rate summary.\npublic parameter rate: // dimension\n1;\nfield x:1=0;\n/// Balance summary.\nrelation r {x // left\n=rate // right\n;}\n}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let formatted = format(&document);
    assert!(formatted.contains("rate: // dimension\n"));
    assert!(formatted.contains("x // left\n"));
    assert!(formatted.contains("rate // right\n"));
    let reparsed = parse("formatted.eqi", &formatted).into_document().unwrap();
    assert_eq!(format(&reparsed), formatted);
    let component = &reparsed.components()[0];
    let eqiora_lang::ComponentItem::Parameter(parameter) = &component.items()[0] else {
        panic!("Parameter")
    };
    assert_eq!(
        reparsed.doc_comment(parameter.range()).unwrap().summary(),
        "Rate summary."
    );
}

#[test]
fn blank_line_and_trailing_documentation_do_not_attach() {
    let source = "/// detached\n\nmodel M {\n  field x:1=0; /// trailing\n  field y:1=0;\n  /// dangling\n}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let model = &document.models()[0];
    assert!(document.doc_comment(model.range()).is_none());
    for item in model.items() {
        let Item::Field(field) = item else {
            panic!("Field")
        };
        assert!(document.doc_comment(field.range()).is_none());
    }
    let formatted = format(&document);
    let reparsed = parse("formatted.eqi", &formatted).into_document().unwrap();
    assert!(reparsed.doc_comment(reparsed.models()[0].range()).is_none());
    assert_eq!(format(&reparsed), formatted);
    for text in ["/// detached", "/// trailing", "/// dangling"] {
        assert!(formatted.contains(text));
    }
}

#[test]
fn trailing_block_does_not_capture_next_standalone_documentation() {
    let source = "model M {\nfield a:1=0; /// trailing a\n/// docs for b\nfield b:1=0;\n}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let Item::Field(field) = &document.models()[0].items()[1] else {
        panic!("Field")
    };
    assert_eq!(
        document.doc_comment(field.range()).unwrap().summary(),
        "docs for b"
    );
    let formatted = format(&document);
    let reparsed = parse("formatted.eqi", &formatted).into_document().unwrap();
    let Item::Field(field) = &reparsed.models()[0].items()[1] else {
        panic!("Field")
    };
    assert_eq!(
        reparsed.doc_comment(field.range()).unwrap().summary(),
        "docs for b"
    );
    assert_eq!(format(&reparsed), formatted);
}

#[test]
fn utf8_crlf_documentation_ranges_slice_the_exact_original_block() {
    let source = "// 🧪\r\n/// 温度。\r\n/// Further prose.\r\nmodel M {}\r\n";
    let document = parse("温度.eqi", source).into_document().unwrap();
    let doc = document.doc_comment(document.models()[0].range()).unwrap();
    assert_eq!(
        &source[doc.range().start() as usize..doc.range().end() as usize],
        "/// 温度。\r\n/// Further prose.\r"
    );
    assert_eq!(doc.summary(), "温度。 Further prose.");
    let formatted = format(&document);
    assert_eq!(
        format(&parse("温度.eqi", &formatted).into_document().unwrap()),
        formatted
    );
}

#[test]
fn repeated_sibling_syntax_carries_only_its_own_docs_when_rebuilt_or_removed() {
    let source = "model M {\n/// docs a\nfield a:1=0; // tail a\n/// docs b\nfield b:1=0; // tail b\n/// docs c\nfield c:1=0; // tail c\n}\n";
    let document = parse("docs.eqi", source).into_document().unwrap();
    let model = &document.models()[0];
    for order in [vec![2, 0, 1], vec![2, 0], vec![1]] {
        let rewritten = SourceAstFactory::model(
            model.visibility(),
            model.name(),
            order
                .iter()
                .map(|index| model.items()[*index].clone())
                .collect(),
            model.range(),
        )
        .unwrap();
        let text = format(&SourceAstFactory::flat_document(vec![rewritten]).unwrap());
        let parsed = parse("rebuilt.eqi", &text).into_document().unwrap();
        for item in parsed.models()[0].items() {
            let Item::Field(field) = item else {
                panic!("Field")
            };
            assert_eq!(
                parsed.doc_comment(field.range()).unwrap().summary(),
                format!("docs {}", field.name())
            );
            assert!(text.contains(&format!(
                "field {}: 1 = 0; // tail {}",
                field.name(),
                field.name()
            )));
        }
        assert_eq!(format(&parsed), text);
    }
}

#[test]
fn recovery_does_not_attach_displaced_docs_to_a_surviving_declaration() {
    let source = "model M {\n/// docs for broken\nfield ;\nfield retained:1=0;\n}\n";
    let parsed = parse("incomplete.eqi", source);
    assert!(!parsed.diagnostics().is_empty());
    let document = parsed.document().unwrap();
    let Item::Field(field) = &document.models()[0].items()[0] else {
        panic!("Field")
    };
    assert!(document.doc_comment(field.range()).is_none());
    let text = format(document);
    assert!(text.contains("/// docs for broken"));
    let reparsed = parse("recovered.eqi", &text).into_document().unwrap();
    let Item::Field(field) = &reparsed.models()[0].items()[0] else {
        panic!("Field")
    };
    assert!(reparsed.doc_comment(field.range()).is_none());
    assert_eq!(format(&reparsed), text);
}
