use super::NotationProfile;
use crate::{NotationAtom, NotationNode};

pub(super) fn render(node: &NotationNode, profile: NotationProfile) -> String {
    let size = emitted_len(node, profile);
    let mut output = String::with_capacity(size);
    write(node, profile, &mut output);
    debug_assert_eq!(output.len(), size);
    output
}

pub(super) fn emitted_len(node: &NotationNode, profile: NotationProfile) -> usize {
    let rich = profile == NotationProfile::Rich;
    match node {
        NotationNode::Atom(atom) => atom_name(*atom, rich).len(),
        NotationNode::Mark(mark) => mark.spelling().len() - usize::from(!rich),
        NotationNode::Styled(style, inner) => {
            emitted_len(inner, profile) + if rich { style.spelling().len() + 2 } else { 0 }
        }
        NotationNode::Accented(accent, inner) => {
            emitted_len(inner, profile) + accent.spelling().len() + 2 - usize::from(!rich)
        }
        NotationNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            emitted_len(base, profile)
                + if grouped_base(base, profile) { 2 } else { 0 }
                + [(false, subscript), (true, superscript)]
                    .into_iter()
                    .map(|(upper, script)| {
                        if script.is_empty() {
                            return 0;
                        }
                        let delimiters = if profile == NotationProfile::Speech {
                            (if upper {
                                " superscript ".len()
                            } else {
                                " subscript ".len()
                            }) + " end script".len()
                        } else {
                            3
                        };
                        delimiters + script.len() - 1
                            + script
                                .iter()
                                .map(|node| emitted_len(node, profile))
                                .sum::<usize>()
                    })
                    .sum::<usize>()
        }
    }
}

fn write(node: &NotationNode, profile: NotationProfile, output: &mut String) {
    let rich = profile == NotationProfile::Rich;
    let speech = profile == NotationProfile::Speech;
    match node {
        NotationNode::Atom(atom) => output.push_str(&atom_name(*atom, rich)),
        NotationNode::Mark(mark) => {
            let spelling = mark.spelling();
            output.push_str(if rich { spelling } else { &spelling[1..] });
        }
        NotationNode::Styled(style, inner) => {
            if rich {
                output.push_str(style.spelling());
                output.push('{');
            }
            write(inner, profile, output);
            if rich {
                output.push('}');
            }
        }
        NotationNode::Accented(accent, inner) => {
            let spelling = accent.spelling();
            output.push_str(if rich { spelling } else { &spelling[1..] });
            output.push(if rich { '{' } else { '(' });
            write(inner, profile, output);
            output.push(if rich { '}' } else { ')' });
        }
        NotationNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            if grouped_base(base, profile) {
                output.push('(');
            }
            write(base, profile, output);
            if grouped_base(base, profile) {
                output.push(')');
            }
            for (upper, script) in [(false, subscript), (true, superscript)] {
                if script.is_empty() {
                    continue;
                }
                output.push_str(if speech {
                    if upper {
                        " superscript "
                    } else {
                        " subscript "
                    }
                } else if upper {
                    "^{"
                } else {
                    "_{"
                });
                for (index, part) in script.iter().enumerate() {
                    if index != 0 {
                        output.push(' ');
                    }
                    write(part, profile, output);
                }
                output.push_str(if speech { " end script" } else { "}" });
            }
        }
    }
}

fn grouped_base(mut node: &NotationNode, profile: NotationProfile) -> bool {
    if profile == NotationProfile::Rich {
        return false;
    }
    while let NotationNode::Styled(_, inner) = node {
        node = inner;
    }
    matches!(node, NotationNode::Scripted { .. })
}

fn atom_name(atom: NotationAtom, rich: bool) -> String {
    if rich {
        return atom.spelling();
    }
    // Conservative accessible glyph collapse: variant glyphs lose their style;
    // Greek capitals identical to Latin capitals share their accessible token.
    use NotationAtom::*;
    match atom {
        AlphaUpper => "A".into(),
        BetaUpper => "B".into(),
        EpsilonUpper => "E".into(),
        ZetaUpper => "Z".into(),
        EtaUpper => "H".into(),
        IotaUpper => "I".into(),
        KappaUpper => "K".into(),
        MuUpper => "M".into(),
        NuUpper => "N".into(),
        OmicronUpper => "O".into(),
        RhoUpper => "P".into(),
        TauUpper => "T".into(),
        UpsilonUpper => "Y".into(),
        ChiUpper => "X".into(),
        Omicron => "o".into(),
        VariantEpsilon => "epsilon".into(),
        VariantTheta => "theta".into(),
        VariantKappa => "kappa".into(),
        VariantPi => "pi".into(),
        VariantRho => "rho".into(),
        VariantSigma => "sigma".into(),
        VariantPhi => "phi".into(),
        Ell => "l".into(),
        _ => atom.spelling().trim_start_matches('\\').to_owned(),
    }
}
