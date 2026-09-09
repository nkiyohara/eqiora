use super::NotationProfile;
use crate::{NotationAccent, NotationAtom, NotationMark, NotationNode, NotationStyle};

pub(super) fn emit(node: &NotationNode, profile: NotationProfile, out: &mut impl FnMut(&str)) {
    let mathml = profile == NotationProfile::MathMl;
    match node {
        NotationNode::Atom(atom) => {
            let tag = if matches!(atom, NotationAtom::Digit(_)) {
                "mn"
            } else {
                "mi"
            };
            if mathml {
                out("<");
                out(tag);
                out(">");
            }
            out(&glyph(*atom));
            if mathml {
                out("</");
                out(tag);
                out(">");
            }
        }
        NotationNode::Mark(mark) => {
            if mathml {
                out("<mo>");
            }
            out(match mark {
                NotationMark::Prime => "′",
                NotationMark::Star => "⋆",
                NotationMark::Transpose => "⊤",
                NotationMark::Dagger => "†",
                NotationMark::Plus => "+",
                NotationMark::Minus => "−",
                NotationMark::PlusMinus => "±",
                NotationMark::MinusPlus => "∓",
            });
            if mathml {
                out("</mo>");
            }
        }
        NotationNode::Styled(style, value) => {
            let name = match style {
                NotationStyle::Roman => "normal",
                NotationStyle::Italic => "italic",
                NotationStyle::Bold => "bold",
                NotationStyle::SansSerif => "sans-serif",
                NotationStyle::Monospace => "monospace",
                NotationStyle::Calligraphic => "script",
                NotationStyle::BlackboardBold => "double-struck",
            };
            if mathml {
                out("<mstyle mathvariant=\"");
                out(name);
                out("\">");
            } else {
                out(name);
                out("(");
            }
            emit(value, profile, out);
            out(if mathml { "</mstyle>" } else { ")" });
        }
        NotationNode::Accented(accent, value) => {
            let (name, glyph) = match accent {
                NotationAccent::Hat => ("hat", "^"),
                NotationAccent::Tilde => ("tilde", "~"),
                NotationAccent::Bar => ("bar", "¯"),
                NotationAccent::Vector => ("vector", "→"),
                NotationAccent::Dot => ("dot", "˙"),
                NotationAccent::DoubleDot => ("double dot", "¨"),
            };
            if mathml {
                out("<mover accent=\"true\">");
            } else {
                out(name);
                out("(");
            }
            emit(value, profile, out);
            if mathml {
                out("<mo>");
                out(glyph);
                out("</mo></mover>");
            } else {
                out(")");
            }
        }
        NotationNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            let tag = match (subscript.is_empty(), superscript.is_empty()) {
                (false, false) => "msubsup",
                (false, true) => "msub",
                (true, false) => "msup",
                (true, true) => "mrow",
            };
            if mathml {
                out("<");
                out(tag);
                out(">");
            } else {
                out("(");
            }
            emit(base, profile, out);
            if !mathml {
                out(")");
            }
            for (upper, values) in [(false, subscript), (true, superscript)] {
                if values.is_empty() {
                    continue;
                }
                out(if mathml {
                    "<mrow>"
                } else if upper {
                    "^{"
                } else {
                    "_{"
                });
                for (index, value) in values.iter().enumerate() {
                    if !mathml && index != 0 {
                        out(" ");
                    }
                    emit(value, profile, out);
                }
                out(if mathml { "</mrow>" } else { "}" });
            }
            if mathml {
                out("</");
                out(tag);
                out(">");
            }
        }
    }
}

fn glyph(atom: NotationAtom) -> String {
    use NotationAtom::*;
    let text = match atom {
        Latin(c) => return c.to_string(),
        Digit(n) => return n.to_string(),
        Alpha => "α",
        AlphaUpper => "Α",
        Beta => "β",
        BetaUpper => "Β",
        Gamma => "γ",
        GammaUpper => "Γ",
        Delta => "δ",
        DeltaUpper => "Δ",
        Epsilon => "ε",
        EpsilonUpper => "Ε",
        Zeta => "ζ",
        ZetaUpper => "Ζ",
        Eta => "η",
        EtaUpper => "Η",
        Theta => "θ",
        ThetaUpper => "Θ",
        Iota => "ι",
        IotaUpper => "Ι",
        Kappa => "κ",
        KappaUpper => "Κ",
        Lambda => "λ",
        LambdaUpper => "Λ",
        Mu => "μ",
        MuUpper => "Μ",
        Nu => "ν",
        NuUpper => "Ν",
        Xi => "ξ",
        XiUpper => "Ξ",
        Omicron => "ο",
        OmicronUpper => "Ο",
        Pi => "π",
        PiUpper => "Π",
        Rho => "ρ",
        RhoUpper => "Ρ",
        Sigma => "σ",
        SigmaUpper => "Σ",
        Tau => "τ",
        TauUpper => "Τ",
        Upsilon => "υ",
        UpsilonUpper => "Υ",
        Phi => "φ",
        PhiUpper => "Φ",
        Chi => "χ",
        ChiUpper => "Χ",
        Psi => "ψ",
        PsiUpper => "Ψ",
        Omega => "ω",
        OmegaUpper => "Ω",
        VariantEpsilon => "ϵ",
        VariantTheta => "ϑ",
        VariantKappa => "ϰ",
        VariantPi => "ϖ",
        VariantRho => "ϱ",
        VariantSigma => "ς",
        VariantPhi => "ϕ",
        Hbar => "ℏ",
        Ell => "ℓ",
        Aleph => "ℵ",
    };
    text.into()
}
