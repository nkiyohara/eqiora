use super::*;
use eqiora_lang::Document;

fn document(source: &str) -> (Document, RecordContext) {
    let document = eqiora_lang::parse("records.eqi", source)
        .into_document()
        .unwrap();
    let namespace = crate::identity::IdentityNamespace::new(["record-parameters"]).unwrap();
    let visible =
        crate::record::declarations("records.eqi", &document, &namespace, |_| None).unwrap();
    let mut context = RecordContext {
        visible,
        parameters: BTreeMap::new(),
    };
    for model in document.models() {
        for item in model.items() {
            if let Item::Parameter(parameter) = item {
                context.insert(parameter.name(), parameter.value_type());
            }
        }
    }
    for component in document.components() {
        for declaration in parameter_declarations(component).values() {
            context.insert(declaration.name(), declaration.value_type());
        }
    }
    (document, context)
}

#[test]
fn model_record_members_share_dependency_order_and_exact_scalar_type_admission() {
    let (document, context) = document(
        "record Config {gain:1, valid:bool} model M(){parameter result:1=c.gain*3;parameter c:Config=Config(valid=true,gain=p*2);parameter p:1=5;}",
    );
    let values = resolve_model_parameters(
        "records.eqi",
        &document.models()[0],
        |_| None,
        BTreeMap::new(),
        &context,
    )
    .unwrap();
    assert_eq!(
        values["result"]
            .value
            .real_scalar_value()
            .map(|value| value.value()),
        Some(30.0)
    );
    assert_eq!(
        values["c.gain"]
            .value
            .real_scalar_value()
            .map(|value| value.value()),
        Some(10.0)
    );
    assert_eq!(values["c.valid"].value.as_bool(), Some(true));
}

#[test]
fn required_record_parameter_leaves_remain_symbolic_without_invented_values() {
    let (document, context) = document(
        "record Config {gain:1,valid:bool} component C(parameter c:Config){parameter result:1=3*c.gain;}",
    );
    let values = resolve_component_parameters_symbolically(
        "records.eqi",
        &document.components()[0],
        |_| None,
        &context,
    )
    .unwrap();
    assert!(values["c.gain"].value.is_none());
    assert!(values["c.valid"].value.is_none());
    assert!(values["result"].value.is_none());
    assert_eq!(values["c.valid"].value_type, ValueType::boolean());
}

#[test]
fn record_cycles_member_type_errors_and_foreign_nominal_constructors_fail_closed() {
    for source in [
        "record Config {gain:1} model M(){parameter c:Config=Config(gain=n);parameter n:1=c.gain;}",
        "record Config {gain:1} model M(){parameter c:Config=Config(gain=true);}",
        "record Config {gain:1} record Other {gain:1} model M(){parameter c:Config=Other(gain=2);}",
    ] {
        let (document, context) = document(source);
        assert!(
            resolve_model_parameters(
                "records.eqi",
                &document.models()[0],
                |_| None,
                BTreeMap::new(),
                &context
            )
            .is_err(),
            "{source}"
        );
    }
}

#[test]
fn forwarded_and_derived_instance_members_preserve_parent_parameter_lineage() {
    let (document, context) = document(
        "record Config {gain:1,valid:bool} component Child(parameter incoming:Config){} model M(){parameter c:Config=Config(gain=5,valid=true);instance forwarded:Child(incoming=c);instance derived:Child(incoming=Config(gain=2*c.gain,valid=c.valid));}",
    );
    let model = &document.models()[0];
    let mut parent =
        resolve_model_parameters_symbolically("records.eqi", model, |_| None, &context).unwrap();
    use crate::identity::{DeclarationPath, ElaborationKey, IdentityNamespace, InstancePath};
    let identity = ElaborationKey::entity(
        IdentityNamespace::new(["owner"]).unwrap(),
        InstancePath::new(["root"]).unwrap(),
        DeclarationPath::new(["gain"]).unwrap(),
        eqiora_core::EntityKind::Parameter,
    )
    .unwrap()
    .full_identity()
    .unwrap();
    let source = ResolvedParameter::model_parameter(
        parent["c.gain"].value.clone().unwrap(),
        identity,
        "original_gain".into(),
        TextRange::default(),
    );
    parent.insert("c.gain".into(), source.clone().into());
    let instances = model
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Instance(instance) => Some(instance),
            _ => None,
        })
        .collect::<Vec<_>>();
    let resolve = |instance: &InstanceDecl| {
        resolve_instance_parameters_symbolically(
            ("records.eqi", "records.eqi"),
            &document.components()[0],
            instance,
            &parent,
            &mut |_| None,
            &mut |_| None,
            (&context, &context),
        )
        .unwrap()
    };
    let forwarded = resolve(instances[0]);
    assert_eq!(
        forwarded["incoming.gain"].lineage,
        Some(ParameterLineage::Parameter(identity))
    );
    assert_eq!(
        forwarded["incoming.gain"].expression,
        Some(source.expression.clone())
    );
    let derived = resolve(instances[1]);
    assert_eq!(
        derived["incoming.gain"]
            .value
            .as_ref()
            .and_then(ValueLiteral::real_scalar_value)
            .map(|value| value.value()),
        Some(10.0)
    );
    assert_eq!(
        derived["incoming.gain"].lineage,
        Some(ParameterLineage::Derived)
    );
    assert_ne!(
        derived["incoming.gain"].expression,
        Some(LoweringExpression::literal(
            derived["incoming.gain"].value.clone().unwrap(),
            TextRange::default()
        ))
    );
}

#[test]
fn selected_closed_record_inputs_enter_existing_typed_parameter_binding_owner() {
    let (document, context) = document(
        "record Config {gain:1,valid:bool} component C(parameter c:Config){} model M(){parameter selected:Config=Config(valid=true,gain=7);}",
    );
    let Item::Parameter(selected) = &document.models()[0].items()[0] else {
        panic!("parameter")
    };
    let values = super::super::resolve_selected_parameters(
        "records.eqi",
        document.components()[0].signature(),
        &[("c", crate::StaticBindingValue::Expression(selected.value()))],
        BTreeMap::new(),
        &context,
    )
    .unwrap();
    assert_eq!(
        values
            .iter()
            .map(|value| value.parameter())
            .collect::<Vec<_>>(),
        ["c.gain", "c.valid"]
    );
    assert_eq!(
        values[0]
            .value()
            .real_scalar_value()
            .map(|value| value.value()),
        Some(7.0)
    );
    assert_eq!(values[1].value().as_bool(), Some(true));
}
