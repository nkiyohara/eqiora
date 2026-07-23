use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{ContractError, PackageSemanticDigest};

/// A dot-separated package or declaration name with portable ASCII segments.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QualifiedName(String);

impl QualifiedName {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() || value.len() > 512 {
            return Err(ContractError::new(
                "qualified name must contain between 1 and 512 bytes",
            ));
        }
        if !value.split('.').all(valid_segment) {
            return Err(ContractError::new(format!(
                "invalid qualified name `{value}`; expected portable ASCII identifiers"
            )));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl fmt::Debug for QualifiedName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("QualifiedName")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for QualifiedName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for QualifiedName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// A canonical SemVer string used only as an exact publisher identity.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactVersion(String);

impl ExactVersion {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let parsed = semver::Version::parse(&value).map_err(|error| {
            ContractError::new(format!("invalid exact SemVer `{value}`: {error}"))
        })?;
        if parsed.to_string() != value {
            return Err(ContractError::new(format!(
                "SemVer `{value}` is not in canonical form `{parsed}`"
            )));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ExactVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ExactVersion")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ExactVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for ExactVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ExactVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// The exact executable identity of one model package.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPackageIdentityV1 {
    pub name: QualifiedName,
    pub version: ExactVersion,
    pub semantic_digest: PackageSemanticDigest,
}

impl ModelPackageIdentityV1 {
    #[must_use]
    pub fn new(
        name: QualifiedName,
        version: ExactVersion,
        semantic_digest: PackageSemanticDigest,
    ) -> Self {
        Self {
            name,
            version,
            semantic_digest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_retains_build_metadata_and_requires_canonical_form() {
        let version = ExactVersion::parse("1.2.3-rc.1+cuda.12").expect("valid version");
        assert_eq!(version.as_str(), "1.2.3-rc.1+cuda.12");
        assert!(ExactVersion::parse("01.2.3").is_err());
        assert!(ExactVersion::parse("1.2").is_err());
    }

    #[test]
    fn qualified_names_are_portable_and_segmented() {
        assert!(QualifiedName::parse("Eqiora.Electrical.Basic").is_ok());
        for invalid in [
            "",
            ".Eqiora",
            "Eqiora.",
            "Eqiora..Basic",
            "9Eqiora",
            "Eqiora/Basic",
        ] {
            assert!(QualifiedName::parse(invalid).is_err(), "accepted {invalid}");
        }
    }
}
