//! Exact bounded source decimals, before unit conversion or numerical rounding.

use crate::AstConstructionError;

/// An exact signed decimal coefficient times a power of ten.
///
/// Inputs use the decimal token grammar, optionally signed, and at most 256 bytes.
/// Exponents and their normalization must fit `i64`; no floating conversion occurs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecimalLiteral {
    coefficient: String,
    exponent10: i64,
    negative: bool,
}

impl DecimalLiteral {
    /// Check and normalize one exact decimal spelling.
    ///
    /// # Errors
    /// Rejects malformed decimal syntax, tokens over 256 bytes, and exponent overflow.
    pub fn parse(text: &str) -> Result<Self, AstConstructionError> {
        let invalid = || AstConstructionError::new("invalid exact decimal literal");
        if text.len() > 256 {
            return Err(AstConstructionError::new(
                "decimal token exceeds the 256-byte limit",
            ));
        }
        let negative = text.starts_with('-');
        let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
        let (mantissa, exponent) = unsigned.split_once(['e', 'E']).unwrap_or((unsigned, "0"));
        let exponent_digits = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        if exponent_digits.is_empty() || !exponent_digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid());
        }
        let exponent10 = exponent.parse::<i64>().map_err(|_| {
            AstConstructionError::new("decimal exponent exceeds the i64 resource bound")
        })?;
        let (integer, fraction) = mantissa
            .split_once('.')
            .map_or((mantissa, None), |(integer, fraction)| {
                (integer, Some(fraction))
            });
        if integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.is_some_and(|digits| {
                digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(invalid());
        }
        let digits = format!("{integer}{}", fraction.unwrap_or(""));
        let significant = digits.trim_start_matches('0');
        if significant.is_empty() {
            return Ok(Self {
                coefficient: "0".into(),
                exponent10: 0,
                negative: false,
            });
        }
        let coefficient = significant.trim_end_matches('0');
        let adjustment = i64::try_from(significant.len() - coefficient.len())
            .expect("bounded token")
            - i64::try_from(fraction.map_or(0, str::len)).expect("bounded token");
        let exponent10 = exponent10.checked_add(adjustment).ok_or_else(|| {
            AstConstructionError::new("normalized decimal exponent exceeds the i64 resource bound")
        })?;
        Ok(Self {
            coefficient: coefficient.into(),
            exponent10,
            negative,
        })
    }

    /// Project a finite native binary64 value through its shortest scientific decimal.
    ///
    /// # Errors
    /// Rejects non-finite input. Representable subnormals are retained.
    pub fn from_f64(value: f64) -> Result<Self, AstConstructionError> {
        if !value.is_finite() {
            return Err(AstConstructionError::new(
                "native decimal value must be finite",
            ));
        }
        Self::parse(&format!("{value:e}"))
    }

    /// Project this exact decimal into the real scalar domain once.
    ///
    /// # Errors
    /// Rejects non-finite overflow and nonzero underflow to binary64 zero.
    pub fn to_f64(&self) -> Result<f64, AstConstructionError> {
        let value = self.canonical_text().parse::<f64>().map_err(|_| {
            AstConstructionError::new("decimal literal cannot be represented as a real scalar")
        })?;
        if !value.is_finite() {
            return Err(AstConstructionError::new(
                "real literal exceeds the finite binary64 range",
            ));
        }
        if value == 0.0 && !self.is_zero() {
            return Err(AstConstructionError::new(
                "nonzero real literal underflows the binary64 range",
            ));
        }
        Ok(value)
    }

    /// Read an integral decimal exactly within the signed 64-bit domain.
    ///
    /// # Errors
    /// Rejects fractional values and signed integer overflow without rounding.
    pub fn to_i64(&self) -> Result<i64, AstConstructionError> {
        let invalid = || {
            AstConstructionError::new(
                "integer literal must be integral and within the signed i64 range",
            )
        };
        if self.is_zero() {
            return Ok(0);
        }
        if self.exponent10 < 0 || self.exponent10 > 18 || self.coefficient.len() > 19 {
            return Err(invalid());
        }
        let magnitude = self
            .coefficient
            .parse::<u64>()
            .map_err(|_| invalid())?
            .checked_mul(10_u64.pow(self.exponent10 as u32))
            .ok_or_else(invalid)?;
        if self.negative && magnitude == (1_u64 << 63) {
            return Ok(i64::MIN);
        }
        let value = i64::try_from(magnitude).map_err(|_| invalid())?;
        Ok(if self.negative { -value } else { value })
    }

    /// Unsigned digits, with no leading or trailing zeros except the unique zero `0`.
    #[must_use]
    pub fn coefficient(&self) -> &str {
        &self.coefficient
    }

    /// Exact decimal power multiplying the coefficient.
    #[must_use]
    pub const fn exponent10(&self) -> i64 {
        self.exponent10
    }

    /// Whether the exact value is zero, independently of binary64 range.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.coefficient == "0"
    }

    /// Whether the exact value is negative; canonical zero has no negative sign.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.negative
    }

    /// Canonical exact text, without expansion of enormous decimal exponents.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        let scientific = if self.exponent10 == 0 {
            format!("{sign}{}", self.coefficient)
        } else {
            format!("{sign}{}e{}", self.coefficient, self.exponent10)
        };
        let position = self.coefficient.len() as i128 + i128::from(self.exponent10);
        let plain_len = if position <= 0 {
            2 - position + self.coefficient.len() as i128
        } else {
            position.max(self.coefficient.len() as i128)
                + i128::from(position < self.coefficient.len() as i128)
        } + sign.len() as i128;
        if plain_len > 256 || (plain_len > 32 && scientific.len() <= 256) {
            return scientific;
        }
        if position <= 0 {
            format!(
                "{sign}0.{}{}",
                "0".repeat((-position) as usize),
                self.coefficient
            )
        } else if position >= self.coefficient.len() as i128 {
            format!(
                "{sign}{}{}",
                self.coefficient,
                "0".repeat((position - self.coefficient.len() as i128) as usize)
            )
        } else {
            let (integer, fraction) = self.coefficient.split_at(position as usize);
            format!("{sign}{integer}.{fraction}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DecimalLiteral;

    #[test]
    fn signed_integer_projection_preserves_adjacent_large_values_and_minimum() {
        for expected in [
            i64::MIN,
            -9007199254740993,
            0,
            9007199254740993,
            9007199254740994,
            i64::MAX,
        ] {
            let literal = DecimalLiteral::parse(&expected.to_string()).unwrap();
            assert_eq!(literal.to_i64().unwrap(), expected);
        }
        for text in [
            "9223372036854775808",
            "-9223372036854775809",
            "1.5",
            "1e9223372036854775807",
            "1e-9223372036854775807",
        ] {
            assert!(
                DecimalLiteral::parse(text).unwrap().to_i64().is_err(),
                "{text}"
            );
        }
        assert_eq!(
            DecimalLiteral::parse("1000e-3").unwrap().to_i64().unwrap(),
            1
        );
    }

    #[test]
    fn real_projection_rejects_underflow_without_rejecting_exact_syntax() {
        for text in ["1e-324", "-1e-324", "1e-999"] {
            let literal = DecimalLiteral::parse(text).expect("exact source decimal");
            assert!(!literal.is_zero());
            assert!(
                literal
                    .to_f64()
                    .unwrap_err()
                    .to_string()
                    .contains("underflows")
            );
        }
        for text in ["0", "-0", "0e-999", "-0e-999"] {
            let literal = DecimalLiteral::parse(text).unwrap();
            assert!(literal.is_zero());
            assert_eq!(literal.to_f64().unwrap(), 0.0);
        }
        // The smallest positive binary64 value is 2^-1074. Decimal 5e-324
        // rounds to that value rather than zero; its negative stays negative.
        assert_eq!(
            DecimalLiteral::parse("5e-324").unwrap().to_f64().unwrap(),
            f64::from_bits(1)
        );
        assert_eq!(
            DecimalLiteral::parse("-5e-324").unwrap().to_f64().unwrap(),
            -f64::from_bits(1)
        );
    }

    #[test]
    fn real_projection_is_explicit_and_rejects_nonfinite_overflow() {
        let literal = DecimalLiteral::parse("9007199254740993").unwrap();
        assert_eq!(literal.to_f64().unwrap(), 9007199254740992.0);
        assert_eq!(literal.to_i64().unwrap(), 9007199254740993);
        assert!(DecimalLiteral::parse("1e400").unwrap().to_f64().is_err());
        assert!(DecimalLiteral::from_f64(f64::NAN).is_err());
        assert!(DecimalLiteral::from_f64(f64::INFINITY).is_err());
    }
}
