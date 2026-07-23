use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};

use crate::ContractError;

macro_rules! digest_type {
    ($name:ident, $domain:literal) => {
        #[doc = concat!("A lowercase SHA-256 digest in the `", $domain, "` domain.")]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) fn compute(canonical_bytes: &[u8]) -> Self {
                let mut hasher = Sha256::new();
                hasher.update($domain.as_bytes());
                hasher.update([0]);
                hasher.update(canonical_bytes);
                Self(hasher.finalize().into())
            }

            pub fn parse(value: &str) -> Result<Self, ContractError> {
                if value.len() != 64
                    || !value
                        .as_bytes()
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                {
                    return Err(ContractError::new(concat!(
                        stringify!($name),
                        " must be 64 lowercase hexadecimal characters"
                    )));
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
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
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

digest_type!(PackageSemanticDigest, "eqiora.package-semantic.sha256.v1");
digest_type!(SourceBundleDigest, "eqiora.source-bundle.sha256.v1");
digest_type!(ResolutionDigest, "eqiora.package-resolution.sha256.v1");
digest_type!(
    PackageCompilationDigest,
    "eqiora.package-compilation.sha256.v1"
);
digest_type!(
    PackageRunBindingDigest,
    "eqiora.package-run-binding.sha256.v1"
);
digest_type!(
    PackageExecutionBindingDigest,
    "eqiora.package-execution-binding.sha256.v1"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_domains_are_distinct_and_lowercase() {
        let bytes = b"same canonical bytes";
        let semantic = PackageSemanticDigest::compute(bytes);
        let source = SourceBundleDigest::compute(bytes);
        let resolution = ResolutionDigest::compute(bytes);
        let compilation = PackageCompilationDigest::compute(bytes);
        let binding = PackageRunBindingDigest::compute(bytes);
        let execution_binding = PackageExecutionBindingDigest::compute(bytes);

        assert_ne!(semantic.to_hex(), source.to_hex());
        assert_ne!(source.to_hex(), resolution.to_hex());
        assert_ne!(resolution.to_hex(), compilation.to_hex());
        assert_ne!(compilation.to_hex(), binding.to_hex());
        assert_ne!(binding.to_hex(), execution_binding.to_hex());
        assert_eq!(
            PackageSemanticDigest::parse(&semantic.to_hex()),
            Ok(semantic)
        );
        assert!(PackageSemanticDigest::parse(&semantic.to_hex().to_uppercase()).is_err());
    }
}
