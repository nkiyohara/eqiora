//! Literal authored version constraints; exact release identity stays separate.

use crate::{ContractError, ExactVersion};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;

const MAX_REQUEST_BYTES: usize = 256;

/// An admitted authored request, not an exact selected package identity.
///
/// Complete versions are exact. Major/minor prefixes and explicitly bounded
/// ranges select stable versions only; matching makes no compatibility claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionRequest {
    spelling: String,
    selection: Selection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Selection {
    Exact(ExactVersion),
    Prefix {
        major: u64,
        minor: Option<u64>,
    },
    Range {
        lower: ExactVersion,
        upper: ExactVersion,
        include_lower: bool,
        include_upper: bool,
    },
}

impl VersionRequest {
    /// Parse the version portion of a package request, without the `name@` prefix.
    /// Only the canonical literal grammar is accepted, never caret or `latest`.
    pub fn parse(value: impl Into<String>) -> Result<Self, ContractError> {
        let spelling = value.into();
        if spelling.is_empty() || spelling.len() > MAX_REQUEST_BYTES || !spelling.is_ascii() {
            return Err(invalid("request must contain 1..=256 ASCII bytes"));
        }
        let selection = if spelling.starts_with('>') {
            let (lower, upper) = spelling
                .split_once(',')
                .ok_or_else(|| invalid("range requires both lower and upper endpoints"))?;
            let (lower, include_lower) = if let Some(value) = lower.strip_prefix(">=") {
                (value, true)
            } else {
                (lower.strip_prefix('>').expect("lower comparator"), false)
            };
            let (upper, include_upper) = if let Some(value) = upper.strip_prefix("<=") {
                (value, true)
            } else {
                (
                    upper
                        .strip_prefix('<')
                        .ok_or_else(|| invalid("range requires an upper comparator"))?,
                    false,
                )
            };
            let lower = stable_endpoint(lower)?;
            let upper = stable_endpoint(upper)?;
            match lower.precedence_cmp(&upper) {
                Ordering::Greater => return Err(invalid("range endpoints are reversed")),
                Ordering::Equal if !include_lower || !include_upper => {
                    return Err(invalid("range is empty"));
                }
                _ => {}
            }
            Selection::Range {
                lower,
                upper,
                include_lower,
                include_upper,
            }
        } else if let Ok(version) = ExactVersion::parse(&spelling) {
            Selection::Exact(version)
        } else {
            let mut pieces = spelling.split('.');
            let major = number(pieces.next().expect("nonempty request"))?;
            let minor = pieces.next().map(number).transpose()?;
            if pieces.next().is_some() || (major == 0 && minor.is_none()) {
                return Err(invalid(
                    "prefix requires a major, and pre-1.0 requires a minor",
                ));
            }
            Selection::Prefix { major, minor }
        };
        Ok(Self {
            spelling,
            selection,
        })
    }

    /// Canonical authored spelling; this never reports the selected version.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    /// Whether an exact release satisfies this request, independently of content.
    #[must_use]
    pub fn matches(&self, version: &ExactVersion) -> bool {
        let parsed = semver::Version::parse(version.as_str()).expect("admitted exact version");
        match &self.selection {
            Selection::Exact(exact) => exact == version,
            Selection::Prefix { major, minor } => {
                parsed.pre.is_empty()
                    && parsed.major == *major
                    && minor.is_none_or(|minor| parsed.minor == minor)
            }
            Selection::Range {
                lower,
                upper,
                include_lower,
                include_upper,
            } => {
                let lower_order = version.precedence_cmp(lower);
                let upper_order = version.precedence_cmp(upper);
                parsed.pre.is_empty()
                    && (lower_order.is_gt() || (*include_lower && lower_order.is_eq()))
                    && (upper_order.is_lt() || (*include_upper && upper_order.is_eq()))
            }
        }
    }
}

fn number(value: &str) -> Result<u64, ContractError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid(
            "prefix numbers must be canonical unsigned integers",
        ));
    }
    value
        .parse()
        .map_err(|_| invalid("prefix number exceeds u64"))
}

fn stable_endpoint(value: &str) -> Result<ExactVersion, ContractError> {
    let exact = ExactVersion::parse(value)?;
    let version = semver::Version::parse(value).expect("admitted exact endpoint");
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(invalid(
            "range endpoints must be complete stable versions without build metadata",
        ));
    }
    Ok(exact)
}

fn invalid(message: &str) -> ContractError {
    ContractError::new(format!("invalid version request: {message}"))
}

impl std::fmt::Display for VersionRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl Serialize for VersionRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for VersionRequest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_prefixes_ranges_and_prereleases_have_no_caret_semantics() {
        for (request, accepted, rejected) in [
            ("1", "1.9.2", "2.0.0"),
            ("1.2", "1.2.9", "1.3.0"),
            ("1.2.3", "1.2.3", "1.2.4"),
            ("0.3", "0.3.99", "0.4.0"),
            ("1.2.3-rc.1", "1.2.3-rc.1", "1.2.3"),
            (">=1.2.3,<2.0.0", "1.2.3", "2.0.0"),
            (">1.2.3,<=2.0.0", "2.0.0", "1.2.3"),
            ("1.2.3+one", "1.2.3+one", "1.2.3+two"),
        ] {
            let parsed = VersionRequest::parse(request).unwrap();
            assert!(
                parsed.matches(&ExactVersion::parse(accepted).unwrap()),
                "{request}"
            );
            assert!(
                !parsed.matches(&ExactVersion::parse(rejected).unwrap()),
                "{request}"
            );
            assert_eq!(parsed.as_str(), request);
            assert_eq!(
                serde_json::from_str::<VersionRequest>(&serde_json::to_string(&parsed).unwrap())
                    .unwrap(),
                parsed
            );
        }
        for (request, prerelease) in [
            ("1", "1.2.9-rc.1"),
            ("1.2", "1.2.9-rc.1"),
            ("0.3", "0.3.9-rc.1"),
            (">=1.2.3,<2.0.0", "1.2.9-rc.1"),
        ] {
            assert!(
                !VersionRequest::parse(request)
                    .unwrap()
                    .matches(&ExactVersion::parse(prerelease).unwrap())
            );
        }
        assert!(
            !VersionRequest::parse("1.2.3-rc.1")
                .unwrap()
                .matches(&ExactVersion::parse("1.2.3-rc.2").unwrap())
        );
        for request in [
            "0",
            "latest",
            "^1.2.3",
            "~1.2",
            "*",
            "1.2-rc.1",
            "01",
            "1.02",
            "@1",
            "1.2.3.4",
            ">=1.0.0",
            ">=2.0.0,<1.0.0",
            ">=1.0.0,<1.0.0",
            ">=1.0.0-rc.1,<2.0.0",
            ">=1.0.0+tag,<2.0.0",
        ] {
            assert!(VersionRequest::parse(request).is_err(), "{request}");
        }
        assert!(VersionRequest::parse("1".repeat(MAX_REQUEST_BYTES + 1)).is_err());
    }

    #[test]
    fn precedence_is_numeric_and_build_metadata_does_not_break_ties() {
        let v = |s| ExactVersion::parse(s).unwrap();
        assert!(v("1.10.0").precedence_cmp(&v("1.9.0")).is_gt());
        assert!(v("1.0.0-rc.10").precedence_cmp(&v("1.0.0-rc.2")).is_gt());
        assert!(v("1.0.0").precedence_cmp(&v("1.0.0-rc.10")).is_gt());
        assert!(v("1.0.0+one").precedence_cmp(&v("1.0.0+two")).is_eq());
    }
}
