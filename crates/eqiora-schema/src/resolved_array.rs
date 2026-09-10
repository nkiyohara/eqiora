//! Immutable finite binary64 values with the existing resolved-array identity.
use eqiora_core::{Diagnostic, diagnostic::codes};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// Verified row-major binary64 array; all zero values are positive zero.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedF64Array {
    shape: Vec<u64>,
    values: Vec<f64>,
}
// Construction rejects NaN and normalizes signed zero.
impl Eq for ResolvedF64Array {}

impl ResolvedF64Array {
    /// Validate shape and finite values, normalizing zeros.
    ///
    /// # Errors
    /// Returns `EQ0901` for empty/zero/overflowing shape, count mismatch or nonfinite values.
    pub fn new(shape: Vec<u64>, mut values: Vec<f64>) -> Result<Self, Diagnostic> {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid("resolved f64 array values must all be finite"));
        }
        if shape.is_empty() || shape.contains(&0) {
            return Err(invalid(
                "resolved array shape must contain positive dimensions",
            ));
        }
        let count = shape.iter().try_fold(1_usize, |product, &dimension| {
            let dimension = usize::try_from(dimension)
                .map_err(|_| invalid("resolved array dimension exceeds local usize"))?;
            product
                .checked_mul(dimension)
                .ok_or_else(|| invalid("resolved array shape product overflows usize"))
        })?;
        if count != values.len() {
            return Err(invalid(format!(
                "resolved array shape requires {count} values, received {}",
                values.len()
            )));
        }
        for value in &mut values {
            if *value == 0.0 {
                *value = 0.0;
            }
        }
        Ok(Self { shape, values })
    }
    /// Ordered positive dimensions.
    #[must_use]
    pub fn shape(&self) -> &[u64] {
        &self.shape
    }
    /// Finite row-major values.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }
    /// Canonical bytes of the existing `eqiora.resolved-array/v1` DTO.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, Diagnostic> {
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'static str,
            encoding: &'static str,
            scalar: &'static str,
            shape: &'a [u64],
            values: &'a [f64],
        }
        serde_json::to_vec(&Wire {
            schema: "eqiora.resolved-array/v1",
            encoding: "eqiora.canonical-json/v1",
            scalar: "f64",
            shape: &self.shape,
            values: &self.values,
        })
        .map_err(|error| invalid(format!("cannot serialize resolved array: {error}")))
    }
    /// Domain-separated SHA-256 over the complete canonical DTO.
    ///
    /// # Errors
    /// Returns `EQ0901` if serialization fails.
    pub fn digest(&self) -> Result<[u8; 32], Diagnostic> {
        let mut digest = Sha256::new();
        digest.update(b"eqiora.resolved-array/v1\0");
        digest.update(self.canonical_json()?);
        Ok(digest.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_existing_bytes_and_independent_identity() {
        let array = ResolvedF64Array::new(vec![2, 2], vec![-0.0, 1.5, -2.0, 4.0]).unwrap();
        let expected = br#"{"schema":"eqiora.resolved-array/v1","encoding":"eqiora.canonical-json/v1","scalar":"f64","shape":[2,2],"values":[0.0,1.5,-2.0,4.0]}"#;
        assert_eq!(array.canonical_json().unwrap(), expected);
        let mut oracle = Sha256::new();
        oracle.update(b"eqiora.resolved-array/v1");
        oracle.update([0]);
        oracle.update(expected);
        assert_eq!(array.digest().unwrap(), <[u8; 32]>::from(oracle.finalize()));
        assert_eq!(array.values()[0].to_bits(), 0.0_f64.to_bits());
        let reshaped = ResolvedF64Array::new(vec![4], array.values().to_vec()).unwrap();
        assert_ne!(array.digest().unwrap(), reshaped.digest().unwrap());
        let changed = ResolvedF64Array::new(vec![2, 2], vec![0., 1.5, -2., 5.]).unwrap();
        assert_ne!(array.digest().unwrap(), changed.digest().unwrap());
    }
    #[test]
    fn invalid_shape_count_and_nonfinite_values_reject() {
        for (shape, values) in [
            (vec![], vec![]),
            (vec![0], vec![]),
            (vec![2], vec![1.]),
            (vec![u64::MAX, 2], vec![]),
            (vec![1], vec![f64::NAN]),
            (vec![1], vec![f64::INFINITY]),
            (vec![1], vec![f64::NEG_INFINITY]),
        ] {
            assert!(ResolvedF64Array::new(shape, values).is_err());
        }
    }
}
