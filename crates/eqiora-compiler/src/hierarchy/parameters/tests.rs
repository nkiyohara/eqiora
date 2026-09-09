use std::fmt::Write as _;

use eqiora_lang::{ComponentItem, Document, parse};

use super::*;

fn document(source: &str) -> Document {
    let source = format!("{source}\nmodel Root() {{}}\n");
    parse("parameters.eqi", &source)
        .into_compilation_document()
        .expect("test source parses")
}

fn component<'a>(document: &'a Document, name: &str) -> &'a ComponentDecl {
    document
        .components()
        .iter()
        .find(|component| component.name() == name)
        .expect("component exists")
}

fn length(exponent: i32) -> DimExponents {
    DimExponents::from_integers([0, exponent, 0, 0, 0, 0, 0]).expect("bounded dimension")
}

#[test]
fn required_public_parameters_are_typed_free_variables() {
    let document = document(
        r#"
component Symbolic(parameter base: m, parameter exponent: 1, parameter area: m ^ 2 = base ^ exponent) {
parameter offset: m = 2;
}
"#,
    );
    let parameters = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&document, "Symbolic"),
        |_| None,
        &RecordContext::default(),
    )
    .expect("open typed interface resolves");

    assert_eq!(parameters["base"].value, None);
    assert_eq!(parameters["base"].value_type.dimension(), length(1));
    assert_eq!(parameters["exponent"].value, None);
    assert_eq!(
        parameters["exponent"].value_type.dimension(),
        DimExponents::DIMENSIONLESS
    );
    assert_eq!(parameters["area"].value, None);
    assert_eq!(parameters["area"].value_type.dimension(), length(2));
    assert_eq!(
        parameters["offset"]
            .value
            .as_ref()
            .and_then(|value| value.real_scalar_value())
            .map(|quantity| quantity.value()),
        Some(2.0)
    );
    assert_eq!(parameters["offset"].value_type.dimension(), length(1));
}

#[test]
fn required_private_parameter_has_no_symbolic_witness() {
    let document = document("component Invalid() { parameter hidden: m; }");
    let diagnostics = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&document, "Invalid"),
        |_| None,
        &RecordContext::default(),
    )
    .expect_err("private required Parameter is uninhabitable");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("required private Parameter `hidden` has no default")
    }));
}

#[test]
fn nested_instance_specializes_symbolic_parent_bindings() {
    let document = document(
        r#"
component Child(parameter base: m, parameter exponent: 1, parameter area: m ^ 2 = base ^ exponent) {
}
component Parent(parameter length: m) {
instance child: Child(base = length, exponent = 2);
}
"#,
    );
    let parent = component(&document, "Parent");
    let child = component(&document, "Child");
    let instance = parent
        .items()
        .iter()
        .find_map(|item| match item {
            ComponentItem::Instance(instance) => Some(instance),
            _ => None,
        })
        .expect("nested instance exists");
    let parent_parameters = resolve_component_parameters_symbolically(
        "parameters.eqi",
        parent,
        |_| None,
        &RecordContext::default(),
    )
    .expect("parent interface resolves");
    resolve_instance_parameters_symbolically(
        ("parameters.eqi", "parameters.eqi"),
        child,
        instance,
        &parent_parameters,
        &mut |_| None,
        &mut |_| None,
        (&RecordContext::default(), &RecordContext::default()),
    )
    .expect("actual binding context validates the definition edge");
}

#[test]
fn symbolic_instances_preserve_binding_diagnostics() {
    let document = document(
        r#"
component Child(parameter required: m) {
parameter hidden: m = 1;
}
component Parent(parameter length: m) {
instance missing: Child();
  instance unknown: Child(other = length);
  instance private: Child(hidden = length);
  instance duplicate: Child(required = length, required = length);
}
"#,
    );
    let parent = component(&document, "Parent");
    let child = component(&document, "Child");
    let parent_parameters = resolve_component_parameters_symbolically(
        "parameters.eqi",
        parent,
        |_| None,
        &RecordContext::default(),
    )
    .expect("parent interface resolves");
    let instances = parent
        .items()
        .iter()
        .filter_map(|item| match item {
            ComponentItem::Instance(instance) => Some((instance.name(), instance)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    let expected = [
        (
            "missing",
            "required Parameter `required` has no instance binding",
        ),
        ("unknown", "`other` is not a public requirement of `Child`"),
        (
            "private",
            "private Parameter `hidden` cannot be bound on instance `private`",
        ),
        (
            "duplicate",
            "duplicate binding for Parameter `required` in instance `duplicate`",
        ),
    ];
    for (instance, message) in expected {
        let diagnostics = resolve_instance_parameters_symbolically(
            ("parameters.eqi", "parameters.eqi"),
            child,
            instances[instance],
            &parent_parameters,
            &mut |_| None,
            &mut |_| None,
            (&RecordContext::default(), &RecordContext::default()),
        )
        .expect_err("invalid binding fails closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(message)),
            "expected `{message}`, got {diagnostics:#?}"
        );
    }
}

#[test]
fn ten_thousand_parameter_chains_and_cycles_are_iterative() {
    const COUNT: usize = 10_000;

    let mut chain = String::from("component Chain() {\n");
    writeln!(chain, "  parameter p00000: 1 = 1;").expect("write to String");
    for index in 1..COUNT {
        writeln!(chain, "  parameter p{index:05}: 1 = p{:05};", index - 1)
            .expect("write to String");
    }
    chain.push_str("}\n");
    let chain_document = document(&chain);
    let parameters = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&chain_document, "Chain"),
        |_| None,
        &RecordContext::default(),
    )
    .expect("deep acyclic graph resolves without recursive calls");
    assert_eq!(parameters.len(), COUNT);
    assert_eq!(
        parameters["p09999"]
            .value
            .as_ref()
            .and_then(|value| value.real_scalar_value())
            .map(|quantity| quantity.value()),
        Some(1.0)
    );

    let mut cycle = String::from("component Cycle() {\n");
    for index in 0..COUNT {
        writeln!(
            cycle,
            "  parameter p{index:05}: 1 = p{:05};",
            (index + 1) % COUNT
        )
        .expect("write to String");
    }
    cycle.push_str("}\n");
    let cycle_document = document(&cycle);
    let diagnostics = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&cycle_document, "Cycle"),
        |_| None,
        &RecordContext::default(),
    )
    .expect_err("one large SCC fails without recursive calls");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
    assert!(diagnostics[0].source_span().is_some());
    assert!(
        diagnostics[0]
            .message()
            .starts_with("component Parameter dependency cycle: p00000 -> p00001")
    );
    assert!(diagnostics[0].message().ends_with("p09999 -> p00000"));
}

#[test]
fn parameter_self_loop_has_one_source_spanned_type_diagnostic() {
    let document = document("component Loop() { parameter value: 1 = value; }");
    let diagnostics = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&document, "Loop"),
        |_| None,
        &RecordContext::default(),
    )
    .expect_err("self dependency is a cycle");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
    let span = diagnostics[0]
        .source_span()
        .expect("cycle points to the dependency name");
    assert_eq!(span.file, "parameters.eqi");
    assert!(span.start < span.end);
    assert_eq!(
        diagnostics[0].message(),
        "component Parameter dependency cycle: value -> value"
    );
}

#[test]
fn symbolic_default_outcome_is_declaration_order_independent() {
    let forward = document(
        r#"
component Ordered() {
  parameter base: 1 = 2;
  parameter shifted: 1 = base + 3;
  parameter scaled: 1 = shifted * 4;
}
"#,
    );
    let reverse = document(
        r#"
component Ordered() {
  parameter scaled: 1 = shifted * 4;
  parameter shifted: 1 = base + 3;
  parameter base: 1 = 2;
}
"#,
    );

    let forward = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&forward, "Ordered"),
        |_| None,
        &RecordContext::default(),
    )
    .expect("forward declarations resolve");
    let reverse = resolve_component_parameters_symbolically(
        "parameters.eqi",
        component(&reverse, "Ordered"),
        |_| None,
        &RecordContext::default(),
    )
    .expect("reverse declarations resolve");
    assert_eq!(forward, reverse);
    assert_eq!(
        forward["scaled"]
            .value
            .as_ref()
            .and_then(|value| value.real_scalar_value())
            .map(|quantity| quantity.value()),
        Some(20.0)
    );
}
