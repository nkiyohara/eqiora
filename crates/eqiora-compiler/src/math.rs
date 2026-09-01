use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{NamePath, TextRange};

use crate::diagnostics::source_error;

pub(crate) const ROOT: &str = "math";

pub(crate) fn model_name_diagnostics(
    file: &str,
    model_name: &str,
    range: TextRange,
) -> Vec<Diagnostic> {
    (model_name == ROOT)
        .then(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "identifier `math` is reserved for compiler-owned scalar mathematics",
            )
        })
        .into_iter()
        .collect()
}

/// Whether a path belongs to the compiler-owned scalar-mathematics namespace.
pub(crate) fn is_namespaced(path: &NamePath) -> bool {
    path.segments().next() == Some(ROOT)
}

/// Whether a path names an admitted scalar mathematical function.
pub(crate) fn is_function(path: &NamePath) -> bool {
    path.as_str() == "math.sin"
}

/// Returns the compiler-owned value of a canonical mathematical constant.
///
/// Constants remain source paths for formatting and provenance, but lower to
/// the same dimensionless scalar expression as the equivalent numeric literal.
pub(crate) fn constant(path: &NamePath) -> Option<f64> {
    match path.as_str() {
        "math.pi" => Some(f64::from_bits(0x4009_21fb_5444_2d18)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use eqiora_lang::{NamePath, TextRange};

    #[test]
    fn pi_has_the_canonical_binary64_value() {
        let path = NamePath::from_segments(["math", "pi"], TextRange::default()).unwrap();
        assert_eq!(
            super::constant(&path).unwrap().to_bits(),
            0x4009_21fb_5444_2d18
        );
    }
}
