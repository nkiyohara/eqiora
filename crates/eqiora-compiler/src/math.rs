use eqiora_lang::NamePath;

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
