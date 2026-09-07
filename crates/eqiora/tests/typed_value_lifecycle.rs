use eqiora::api::ModelDocument;
use eqiora::kernel::KernelNode;
use eqiora::{DimExponents, ScalarDomain, ValueLiteral, ValueType};

const SOURCE: &str = r#"
model TypedValues() {
  parameter voltage: complex<V> = math.complex(2, 3);
  parameter channels: array<complex<V>, 2> = [math.complex(5, 7), math.complex(11, 13)];
  variable observed: complex<V>;
  relation read { observed = voltage + channels[1]; }
}
"#;

#[test]
fn imaginary_only_parameter_edit_survives_public_model_replay_without_new_unknowns() {
    let original = ModelDocument::compile("typed-values.eqi", SOURCE).unwrap();
    let voltage = original.aliases()["voltage"];
    let channels = original.aliases()["channels"];
    let before = original.program().typed_value(voltage).unwrap();
    assert_eq!(before.component(0), Some((2.0, 3.0)));
    assert!(before.real_scalar_value().is_none());
    assert_eq!(
        original
            .program()
            .typed_value(channels)
            .unwrap()
            .components()
            .collect::<Vec<_>>(),
        [(5.0, 7.0), (11.0, 13.0)],
    );
    let replacement = ValueLiteral::new(before.value_type().clone(), [(2.0, 17.0)]).unwrap();
    let plan = original
        .preview_value_edit(voltage, replacement.clone())
        .unwrap();
    assert_eq!(plan.before(), before);
    assert_eq!(plan.after(), &replacement);
    let changed = original
        .commit_value_edit(plan.clone())
        .unwrap()
        .into_document();
    let replay = ModelDocument::replay(&changed.canonical_json().unwrap()).unwrap();
    assert_eq!(replay.program().typed_value(voltage), Some(&replacement));
    assert_eq!(
        replay.program().typed_value(channels),
        original.program().typed_value(channels)
    );
    assert_eq!(replay.digest().unwrap(), changed.digest().unwrap());
    assert_ne!(replay.digest().unwrap(), original.digest().unwrap());
    assert_eq!(
        original
            .program()
            .typed_value(voltage)
            .unwrap()
            .component(0),
        Some((2.0, 3.0))
    );
    assert!(changed.commit_value_edit(plan).is_err());
    for document in [&original, &changed, &replay] {
        assert_eq!(
            document
                .program()
                .nodes()
                .filter(|node| matches!(node, KernelNode::Field(_)))
                .count(),
            1
        );
    }
}

#[test]
fn typed_edit_cannot_reinterpret_domain_shape_or_dimension() {
    let document = ModelDocument::compile("typed-values.eqi", SOURCE).unwrap();
    let target = document.aliases()["voltage"];
    let declared = document.program().typed_value(target).unwrap().value_type();
    for wrong in [
        declared.clone().array(1).unwrap(),
        ValueType::scalar(ScalarDomain::Real, declared.dimension()),
        ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS),
    ] {
        let replacement = ValueLiteral::new(wrong, [(2.0, 0.0)]).unwrap();
        assert!(document.preview_value_edit(target, replacement).is_err());
    }
}
