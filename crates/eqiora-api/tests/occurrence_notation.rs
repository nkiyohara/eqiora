use eqiora_api::ModelDocument;
use eqiora_lang::NotationProfile;
use std::collections::BTreeSet;

#[test]
fn source_labels_do_not_change_model_meaning_and_bare_replay_does_not_guess_them() {
    let plain = "model M() {parameter p:1=2;variable x:1;relation law {x=p;}}";
    let marked = plain.replace("p:", "p @{z}:").replace("x:", "x @{z}:");
    let original = ModelDocument::compile("m.eqi", plain).unwrap();
    let compiled = ModelDocument::compile("m.eqi", &marked).unwrap();
    assert_eq!(original.digest().unwrap(), compiled.digest().unwrap());
    assert!(original.structurally_equivalent(&compiled).unwrap());
    let value_type = compiled
        .program()
        .typed_value(compiled.aliases()["p"])
        .unwrap()
        .value_type()
        .clone();
    let edit = compiled
        .preview_value_edit(
            compiled.aliases()["p"],
            eqiora_core::ValueLiteral::from_real(value_type, 3.).unwrap(),
        )
        .unwrap();
    let updated = compiled.commit_value_edit(edit).unwrap();
    assert_eq!(compiled.notation(), updated.document().notation());
    assert_eq!(compiled.notation().iter().len(), 2);
    let replayed = ModelDocument::replay(&compiled.canonical_json().unwrap()).unwrap();
    assert_eq!(compiled.digest().unwrap(), replayed.digest().unwrap());
    assert_eq!(replayed.notation().iter().len(), 2);
    for entry in replayed.notation().iter() {
        assert!(entry.definition_span().is_none());
        assert!(entry.instance_span().is_none());
        assert!(entry.graph_id().is_some());
        assert_ne!(entry.render(NotationProfile::Plain), "z");
    }
    for profile in [
        NotationProfile::Latex,
        NotationProfile::MathMl,
        NotationProfile::Unicode,
        NotationProfile::Plain,
        NotationProfile::Speech,
    ] {
        assert_eq!(
            replayed
                .notation()
                .iter()
                .map(|entry| entry.render(profile))
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
    }
    assert!(
        compiled
            .notation()
            .view(replayed.notation().iter().map(|entry| entry.identity()))
            .is_empty()
    );
}

#[test]
fn native_model_draft_uses_labels_without_synthetic_source_locations() {
    use eqiora_core::{DimExponents, ScalarDomain, ValueLiteral, ValueType};
    use eqiora_lang::{DraftField, DraftParameter, DraftRelation, FieldRoleSyntax, Module};
    let value_type = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap();
    let p = DraftParameter::new(
        "p",
        ValueLiteral::from_real(value_type.clone(), 2.).unwrap(),
    );
    let x = DraftField::new("x", value_type, FieldRoleSyntax::Variable);
    let law = DraftRelation::continuous("law", [(x.expression(), p.expression())]);
    let draft = Module::new("M", [p.into(), x.into(), law.into()]).unwrap();
    let model = ModelDocument::compile_module(&draft, None, &[]).unwrap();
    assert_eq!(model.notation().iter().len(), 2);
    assert!(
        model
            .notation()
            .iter()
            .all(|entry| entry.definition_span().is_none() && entry.instance_span().is_none())
    );
}
