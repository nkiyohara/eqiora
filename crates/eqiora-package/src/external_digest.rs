use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ContractError;

macro_rules! external_digest_type {
    ($name:ident, $description:literal, $error:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, ContractError> {
                if value.len() != 64
                    || !value
                        .as_bytes()
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                {
                    return Err(ContractError::new($error));
                }
                let mut bytes = [0_u8; 32];
                for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
                    bytes[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
                }
                Ok(Self(bytes))
            }

            #[must_use]
            pub fn to_hex(self) -> String {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let mut output = String::with_capacity(64);
                for byte in self.0 {
                    output.push(char::from(HEX[usize::from(byte >> 4)]));
                    output.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
                output
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.to_hex())
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.to_hex())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_hex())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

fn nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("validated hexadecimal digit"),
    }
}

external_digest_type!(
    CanonicalModelDigest,
    "SHA-256 emitted by the canonical model-artifact boundary. This wrapper validates and carries that identity without redefining its preimage.",
    "canonical model digest must be 64 lowercase hexadecimal characters"
);

external_digest_type!(
    CanonicalRunDigest,
    "SHA-256 emitted by a canonical run-manifest boundary. This wrapper validates and carries that identity without redefining its preimage.",
    "canonical run digest must be 64 lowercase hexadecimal characters"
);

external_digest_type!(
    CanonicalRealizationDigest,
    "SHA-256 emitted by a canonical Realization-artifact boundary. This wrapper validates and carries that identity without redefining its preimage.",
    "canonical Realization digest must be 64 lowercase hexadecimal characters"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_digests_are_canonical_and_strongly_typed() {
        let value = "4a".repeat(32);
        let model = CanonicalModelDigest::parse(&value).expect("model digest");
        let run = CanonicalRunDigest::parse(&value).expect("run digest");
        let realization = CanonicalRealizationDigest::parse(&value).expect("Realization digest");
        assert_eq!(model.to_hex(), value);
        assert_eq!(run.to_hex(), value);
        assert_eq!(realization.to_hex(), value);
        assert!(CanonicalRunDigest::parse(&"4A".repeat(32)).is_err());
        assert!(CanonicalRealizationDigest::parse(&"4A".repeat(32)).is_err());
        assert!(CanonicalRunDigest::parse("4a").is_err());
    }
}
