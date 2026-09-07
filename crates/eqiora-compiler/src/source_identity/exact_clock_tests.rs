use super::*;

fn identity(source: &str) -> LocalSourceIdentity {
    let document = eqiora_lang::parse("clock.eqi", source)
        .into_document()
        .unwrap();
    LocalSourceIdentity::from_document(&document).unwrap()
}

#[test]
fn clock_expression_identity_preserves_exact_authored_values() {
    let original = identity("model M { clock c=periodic(10[ms]); }");
    assert_eq!(
        original,
        identity("// formatting\nmodel M { clock c = periodic(1e1[ms], phase=0[s]); }")
    );
    assert_ne!(original, identity("model M { clock c=periodic(11[ms]); }"));
    assert_ne!(
        original,
        identity("model M { clock c=periodic(10[ms], phase=1[ms]); }")
    );
    assert_ne!(
        identity("model M { clock c=periodic(9007199254740992[s]); }"),
        identity("model M { clock c=periodic(9007199254740993[s]); }")
    );
    assert_ne!(
        identity("model M { clock c=periodic(1[s]/3); }"),
        identity("model M { clock c=periodic(1[s]/4); }")
    );
}
