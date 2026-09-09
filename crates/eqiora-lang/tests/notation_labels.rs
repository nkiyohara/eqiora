use eqiora_lang::{Notation, NotationLabel, NotationNode, NotationProfile};

#[test]
fn generated_qualifiers_merge_scripts_and_keep_source_admission_unchanged() {
    let source = Notation::parse(r"@{x_{i}^{\prime}}").unwrap();
    let instance = Notation::parse(r"@{r_2}").unwrap();
    let label = NotationLabel::qualified(&source, &[&instance], &["left"]).unwrap();
    assert_eq!(
        label.render(NotationProfile::Latex),
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
    assert!(Notation::parse(&format!("@{{{}}}", identity.render(NotationProfile::Latex))).is_err());
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
            first.render(NotationProfile::Latex),
            second.render(NotationProfile::Latex)
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
    assert_eq!(label.render(NotationProfile::Latex), r"\mathbf{x_{i r}}");
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

#[test]
fn mathml_unicode_and_accessibility_preserve_typed_notation_structure() {
    for source in [
        r"@{\mathbb{R}}",
        r"@{\hat{x}_i}",
        r"@{\mathbf{x_i}_j}",
        r"@{T_{i j}^{\top}}",
        r"@{\alpha^{\prime}}",
    ] {
        let notation = Notation::parse(source).unwrap();
        let roundtrip = Notation::parse(&notation.canonical()).unwrap();
        let label = NotationLabel::from_notation(&notation);
        let reopened = NotationLabel::from_notation(&roundtrip);
        for profile in [
            NotationProfile::Latex,
            NotationProfile::MathMl,
            NotationProfile::Unicode,
            NotationProfile::Plain,
            NotationProfile::Speech,
        ] {
            assert_eq!(label.render(profile), reopened.render(profile));
        }
        let mathml = label.render(NotationProfile::MathMl);
        assert!(!mathml.contains("@{"));
        assert!(!mathml.contains('\\'));
        assert!(!mathml.contains("href"));
    }
    let set = NotationLabel::from_notation(&Notation::parse(r"@{\mathbb{R}}").unwrap());
    assert_eq!(
        set.render(NotationProfile::MathMl),
        "<mstyle mathvariant=\"double-struck\"><mi>R</mi></mstyle>"
    );
    let tensor = NotationLabel::from_notation(&Notation::parse(r"@{T_{i j}^{\top}}").unwrap());
    assert_eq!(
        tensor.render(NotationProfile::MathMl),
        "<msubsup><mi>T</mi><mrow><mi>i</mi><mi>j</mi></mrow><mrow><mo>⊤</mo></mrow></msubsup>"
    );
    assert!(
        tensor
            .render(NotationProfile::Speech)
            .contains("superscript top end script")
    );
}
