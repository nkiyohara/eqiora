use eqiora_lang::{ExprKind, Item, format, parse};

#[test]
fn named_tensor_values_preserve_frame_components_ranges_and_format() {
    for value in [
        "tensor_value(frame = body, components = [[2,3],[5,7]])",
        "tensor_value(frame = parent.body, components = [2[V],3[V]])",
        "[tensor_value(frame = body, components = [math.complex(2[Pa],11[Pa]),math.complex(3[Pa],13[Pa])]) ]",
    ] {
        let source = format!("model M() {{ let coefficient = {value}; }}");
        let document = parse("tensor.eqi", &source).into_document().unwrap();
        let Item::Let(alias) = &document.models()[0].items()[0] else {
            panic!("alias")
        };
        assert_eq!(
            &source[alias.value().range().start() as usize..alias.value().range().end() as usize],
            value.trim_end()
        );
        let rendered = format(&document);
        assert_eq!(
            format(&parse("again.eqi", &rendered).into_document().unwrap()),
            rendered
        );
        if let ExprKind::Call { callee, arguments } = alias.value().kind() {
            assert_eq!(callee.as_str(), "tensor_value");
            assert_eq!(arguments.expressions().len(), 2);
            assert!(matches!(
                arguments.named().unwrap()[0].value().kind(),
                ExprKind::Name(_) | ExprKind::Path(_)
            ));
            assert!(matches!(
                arguments.named().unwrap()[1].value().kind(),
                ExprKind::Array(_)
            ));
        }
    }
}

#[test]
fn tensor_named_argument_grammar_rejects_invalid_roles() {
    for value in [
        "tensor_value(body, [2,3])",
        "tensor_value(frame = body)",
        "tensor_value(frame = body, frame = other)",
        "tensor_value(frame = body, components = [2,3], components = [5,7])",
        "tensor_value(frame = body + other, components = [2,3])",
        "tensor_value(frame = , components = [2,3])",
        "tensor_value(frame = body, components = [])",
    ] {
        assert!(
            parse("bad.eqi", &format!("model M() {{ let a = {value}; }}"))
                .into_document()
                .is_err(),
            "{value}"
        );
    }
}

#[test]
fn tensor_roles_are_order_independent_in_source_and_native_construction() {
    for call in [
        "tensor_value(frame = body, components = [2,3])",
        "tensor_value(components = [2,3], frame = body)",
    ] {
        let document = parse("roles.eqi", &format!("model M() {{ let value = {call}; }}"))
            .into_document()
            .unwrap();
        let Item::Let(alias) = &document.models()[0].items()[0] else {
            panic!("alias")
        };
        let ExprKind::Call { arguments, .. } = alias.value().kind() else {
            panic!("call")
        };
        let bindings = arguments.named().unwrap();
        assert_eq!(
            bindings[0].name(),
            if call.starts_with("tensor_value(frame") {
                "frame"
            } else {
                "components"
            }
        );
        let native = eqiora_lang::SourceAstFactory::expression(
            alias.value().kind().clone(),
            alias.value().range(),
        )
        .unwrap();
        assert_eq!(native, *alias.value());
        assert_eq!(
            parse("again.eqi", &format(&document))
                .into_document()
                .unwrap()
                .models()
                .len(),
            1
        );
    }
}
