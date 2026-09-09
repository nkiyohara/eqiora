use eqiora_compiler::source_identity::LocalSourceIdentity;
use eqiora_lang::{format, parse};

#[test]
fn notation_is_presentation_not_local_source_semantic_identity() {
    let plain = "model Main() { parameter viscosity: 1 = 2; variable stress: 1; relation law { stress = viscosity; } }";
    let decorated = plain
        .replace("Main()", r"Main @{\mathcal{M}}()")
        .replace("viscosity:", r"viscosity @{\mu}:")
        .replace("stress:", r"stress @{\sigma_{ij}}:")
        .replace("law {", r"law @{L} {");
    let original = parse("plain.eqi", plain).into_document().unwrap();
    let edited = parse("edited.eqi", &decorated).into_document().unwrap();
    assert_ne!(format(&original), format(&edited));
    assert_eq!(
        LocalSourceIdentity::from_document(&original).unwrap(),
        LocalSourceIdentity::from_document(&edited).unwrap()
    );
    let renamed = parse("renamed.eqi", &decorated.replace("stress", "traction"))
        .into_document()
        .unwrap();
    assert_ne!(
        LocalSourceIdentity::from_document(&edited).unwrap(),
        LocalSourceIdentity::from_document(&renamed).unwrap()
    );
}
