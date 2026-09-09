use super::{MAX_OUTPUT, Math, MathReference, MathRendering};
use eqiora_core::Diagnostic;
use eqiora_lang::NotationProfile;

mod value_type;

pub(super) fn render(
    tree: Math,
    references: Vec<MathReference>,
    profile: NotationProfile,
) -> Result<MathRendering, Diagnostic> {
    let plain = project(&tree, NotationProfile::Plain)?;
    let speech = project(&tree, NotationProfile::Speech)?;
    let projected = project(&tree, profile);
    let used_fallback = projected.is_err();
    let text = projected.unwrap_or_else(|_| plain.clone());
    Ok(MathRendering {
        profile,
        text,
        plain,
        speech,
        references,
        used_fallback,
    })
}

fn project(tree: &Math, profile: NotationProfile) -> Result<String, Diagnostic> {
    let mut writer = Writer {
        profile,
        output: String::new(),
    };
    if profile == NotationProfile::MathMl {
        writer.push("<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mrow>")?;
    }
    writer.emit(tree)?;
    if profile == NotationProfile::MathMl {
        writer.push("</mrow></math>")?;
    }
    Ok(writer.output)
}

struct Writer {
    profile: NotationProfile,
    output: String,
}

impl Writer {
    fn push(&mut self, text: &str) -> Result<(), Diagnostic> {
        if self.output.len().saturating_add(text.len()) > MAX_OUTPUT {
            return Err(super::expression::failure(
                "mathematical output exceeds bounded presentation size",
            ));
        }
        self.output.push_str(text);
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), Diagnostic> {
        for character in text.chars() {
            let replacement = match (self.profile, character) {
                (NotationProfile::MathMl, '&') => "&amp;",
                (NotationProfile::MathMl, '<') => "&lt;",
                (NotationProfile::MathMl, '>') => "&gt;",
                (NotationProfile::MathMl, '\"') => "&quot;",
                (NotationProfile::Latex, '_') => "\\_",
                (NotationProfile::Latex, '{') => "\\{",
                (NotationProfile::Latex, '}') => "\\}",
                (NotationProfile::Latex, '\\') => "\\backslash{}",
                (NotationProfile::Latex, '%') => "\\%",
                (NotationProfile::Latex, '#') => "\\#",
                (NotationProfile::Latex, '&') => "\\&",
                (NotationProfile::Latex, '$') => "\\$",
                _ => {
                    self.push(character.encode_utf8(&mut [0; 4]))?;
                    continue;
                }
            };
            self.push(replacement)?;
        }
        Ok(())
    }

    fn group(&mut self, value: &Math) -> Result<(), Diagnostic> {
        let (open, close) = match self.profile {
            NotationProfile::MathMl => ("<mrow><mo>(</mo>", "<mo>)</mo></mrow>"),
            NotationProfile::Latex => ("\\left(", "\\right)"),
            NotationProfile::Speech => ("begin group ", " end group"),
            _ => ("(", ")"),
        };
        self.push(open)?;
        self.emit(value)?;
        self.push(close)
    }

    fn emit(&mut self, tree: &Math) -> Result<(), Diagnostic> {
        let ml = self.profile == NotationProfile::MathMl;
        let latex = self.profile == NotationProfile::Latex;
        let speech = self.profile == NotationProfile::Speech;
        match tree {
            Math::Type(value) => self.value_type(value)?,
            Math::Typed(value, value_type) => {
                self.group(value)?;
                self.push(if ml {
                    "<mo>:</mo>"
                } else if speech {
                    " of type "
                } else {
                    " : "
                })?;
                self.value_type(value_type)?;
            }
            Math::Number(number) => {
                if ml {
                    self.push("<mn>")?;
                }
                self.text(number)?;
                if ml {
                    self.push("</mn>")?;
                }
            }
            Math::Label(label) => self.push(&label.render(self.profile))?,
            Math::Gradient(value) => {
                self.push(if ml {
                    "<mo>∇</mo>"
                } else if latex {
                    "\\nabla "
                } else if self.profile == NotationProfile::Unicode {
                    "∇ "
                } else {
                    "gradient "
                })?;
                self.group(value)?;
            }
            Math::Negative(value) => {
                self.push(if ml {
                    "<mo>−</mo>"
                } else if speech {
                    "negative "
                } else {
                    "-"
                })?;
                self.group(value)?;
            }
            Math::Derivative(value) => {
                self.push(if ml {
                    "<mfrac><mrow><mi mathvariant=\"normal\">d</mi>"
                } else if latex {
                    "\\frac{\\mathrm{d}"
                } else {
                    "time derivative of "
                })?;
                self.group(value)?;
                self.push(if ml {
                    "</mrow><mrow><mi mathvariant=\"normal\">d</mi><mi>t</mi></mrow></mfrac>"
                } else if latex {
                    "}{\\mathrm{d}t}"
                } else {
                    " end derivative"
                })?;
            }
            Math::Inner(left, right) => {
                self.push(if ml {
                    "<mrow><mo>⟨</mo>"
                } else if latex {
                    "\\left\\langle "
                } else if speech {
                    "real inner product of first "
                } else {
                    "inner("
                })?;
                self.emit(left)?;
                self.push(if ml {
                    "<mo>,</mo>"
                } else if speech {
                    " and second "
                } else {
                    ", "
                })?;
                self.emit(right)?;
                self.push(if ml {
                    "<mo>⟩</mo></mrow>"
                } else if latex {
                    "\\right\\rangle"
                } else if speech {
                    " end inner product"
                } else {
                    ")"
                })?;
            }
            Math::Binary(operator, left, right) => {
                if *operator == "/" && (ml || latex) {
                    self.push(if ml { "<mfrac><mrow>" } else { "\\frac{" })?;
                    self.emit(left)?;
                    self.push(if ml { "</mrow><mrow>" } else { "}{" })?;
                    self.emit(right)?;
                    self.push(if ml { "</mrow></mfrac>" } else { "}" })?;
                } else {
                    self.group(left)?;
                    if ml {
                        self.push("<mo>")?;
                    } else {
                        self.push(" ")?;
                    }
                    let symbol = match (*operator, self.profile) {
                        ("*", NotationProfile::Latex) => "\\cdot",
                        ("*", NotationProfile::MathMl | NotationProfile::Unicode) => "·",
                        ("+", NotationProfile::Speech) => "plus",
                        ("-", NotationProfile::Speech) => "minus",
                        ("*", NotationProfile::Speech) => "times",
                        ("/", NotationProfile::Speech) => "divided by",
                        ("=", NotationProfile::Speech) => "equals",
                        ("!=", NotationProfile::Speech) => "not equal to",
                        ("<", NotationProfile::Speech) => "less than",
                        ("<=", NotationProfile::Speech) => "less than or equal to",
                        (">", NotationProfile::Speech) => "greater than",
                        (">=", NotationProfile::Speech) => "greater than or equal to",
                        _ => operator,
                    };
                    if latex && symbol == "\\cdot" {
                        self.push(symbol)?;
                    } else {
                        self.text(symbol)?;
                    }
                    if ml {
                        self.push("</mo>")?;
                    } else {
                        self.push(" ")?;
                    }
                    self.group(right)?;
                }
            }
            Math::Power(base, exponent) => {
                if ml {
                    self.push("<msup>")?;
                }
                self.group(base)?;
                self.push(if ml {
                    "<mn>"
                } else if speech {
                    " raised to "
                } else {
                    "^{"
                })?;
                self.text(&exponent.to_string())?;
                self.push(if ml {
                    "</mn></msup>"
                } else if speech {
                    " end power"
                } else {
                    "}"
                })?;
            }
            Math::Index(base, index) => {
                self.group(base)?;
                self.push(if ml {
                    "<mo>[</mo><mn>"
                } else if speech {
                    " at zero based index "
                } else {
                    "["
                })?;
                self.text(&index.to_string())?;
                self.push(if ml {
                    "</mn><mo>]</mo>"
                } else if speech {
                    " end index"
                } else {
                    "]"
                })?;
            }
            Math::Function(name, arguments) => {
                if ml {
                    self.push("<mi mathvariant=\"normal\">")?;
                } else if latex {
                    self.push("\\operatorname{")?;
                }
                self.text(name)?;
                if ml {
                    self.push("</mi><mo>(</mo>")?;
                } else if latex {
                    self.push("}\\left(")?;
                } else if speech {
                    self.push(" of ")?;
                } else {
                    self.push("(")?;
                }
                self.arguments(arguments)?;
                self.push(if ml {
                    "<mo>)</mo>"
                } else if latex {
                    "\\right)"
                } else if speech {
                    " end function"
                } else {
                    ")"
                })?;
            }
            Math::Array(values) => {
                self.push(if ml {
                    "<mrow><mo>[</mo>"
                } else if speech {
                    "ordered array "
                } else {
                    "["
                })?;
                self.arguments(values)?;
                self.push(if ml {
                    "<mo>]</mo></mrow>"
                } else if speech {
                    " end array"
                } else {
                    "]"
                })?;
            }
            Math::Integral(domain, value) => {
                self.push(if ml {
                    "<mrow><msub><mo>∫</mo><mrow>"
                } else if latex {
                    "\\int_{"
                } else {
                    "integral over "
                })?;
                self.emit(domain)?;
                self.push(if ml {
                    "</mrow></msub>"
                } else if latex {
                    "} "
                } else {
                    " of "
                })?;
                self.group(value)?;
                self.push(if ml {
                    "</mrow>"
                } else if latex {
                    ""
                } else {
                    " end integral"
                })?;
            }
        }
        Ok(())
    }

    fn arguments(&mut self, values: &[Math]) -> Result<(), Diagnostic> {
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                self.push(if self.profile == NotationProfile::MathMl {
                    "<mo>,</mo>"
                } else if self.profile == NotationProfile::Speech {
                    " then "
                } else {
                    ", "
                })?;
            }
            self.emit(value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rich_size_failure_retains_bounded_plain_and_speech() {
        let tree = Math::Array((0..4000).map(|_| Math::Number("0".into())).collect());
        let result = render(tree, vec![], NotationProfile::MathMl).unwrap();
        assert!(result.used_fallback());
        assert_eq!(result.text(), result.plain());
        assert!(!result.speech().is_empty());
        assert!(result.speech().len() <= MAX_OUTPUT);
        assert!(!result.text().contains("<math"));
    }
}
