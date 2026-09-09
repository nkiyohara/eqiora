use eqiora_lang::{Notation, NotationLabel, NotationNode, NotationProfile};

#[test]
fn generated_qualifiers_merge_scripts_and_keep_source_admission_unchanged() {
    let source = Notation::parse(r"@{x_{i}^{\prime}}").unwrap();
    let instance = Notation::parse(r"@{r_2}").unwrap();
    let label = NotationLabel::qualified(&source, &[&instance], &["left"]).unwrap();
    assert_eq!(
        label.render(NotationProfile::Rich),
        r"x_{i r_{2} l e f t}^{\prime}"
    );
    let NotationNode::Scripted {
        base,
        subscript,
        superscript,
    } = label.root()
    else {
        panic!("one script owner");
    };
    assert!(matches!(base.as_ref(), NotationNode::Atom(_)));
    assert_eq!((subscript.len(), superscript.len()), (6, 1));
    assert_eq!(source.canonical(), r"@{x_{i}^{\prime}}");
    let identity = NotationLabel::identifier(&"a".repeat(200)).unwrap();
    assert!(Notation::parse(&format!("@{{{}}}", identity.render(NotationProfile::Rich))).is_err());
    assert!(NotationLabel::identifier(&"a".repeat(511)).is_none());
    assert!(label.qualify(&[], &[&"z".repeat(1000)]).is_none());
}

#[test]
fn profiles_deliberately_collapse_style_and_homoglyphs() {
    for (first, second) in [
        (r"@{x}", r"@{\mathbf{x}}"),
        (r"@{A}", r"@{\Alpha}"),
        (r"@{\phi}", r"@{\varphi}"),
    ] {
        let first = NotationLabel::from_notation(&Notation::parse(first).unwrap());
        let second = NotationLabel::from_notation(&Notation::parse(second).unwrap());
        assert_ne!(
            first.render(NotationProfile::Rich),
            second.render(NotationProfile::Rich)
        );
        for profile in [NotationProfile::Plain, NotationProfile::Speech] {
            assert_eq!(first.render(profile), second.render(profile));
        }
    }
}

#[test]
fn dropping_a_style_never_creates_a_double_subscript() {
    let source = Notation::parse(r"@{\mathbf{x_i}}").unwrap();
    let label = NotationLabel::qualified(&source, &[], &["r"]).unwrap();
    assert_eq!(label.render(NotationProfile::Rich), r"\mathbf{x_{i r}}");
    assert_eq!(label.render(NotationProfile::Plain), "x_{i r}");
    let intrinsically_nested =
        NotationLabel::from_notation(&Notation::parse(r"@{\mathbf{x_i}_j}").unwrap());
    assert_eq!(
        intrinsically_nested.render(NotationProfile::Plain),
        "(x_{i})_{j}"
    );
    assert!(
        intrinsically_nested
            .render(NotationProfile::Speech)
            .contains("end script) subscript")
    );
}
