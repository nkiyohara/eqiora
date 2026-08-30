use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, RawId};
use eqiora_lang::BinaryOp;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::{AuthoredFormExpression, AuthoredFormExpressionKind};

const SCHEMA: &str = "eqiora.authored-scalar-primal-form/v1";
const MAX_BYTES: usize = 1024 * 1024;

/// Exact compiler-owned projection of one authored scalar-primal Formulation.
///
/// This is a source-compilation sidecar rather than Model meaning. Its
/// canonical bytes are retained in resolved Plan identity and may be decoded
/// during Plan replay; callers cannot construct a projection without passing
/// the closed canonical decoder.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoredFormulationProjection {
    wire: WireForm,
    canonical_bytes: Box<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireForm {
    schema: String,
    source_identity: String,
    relation_ulid: String,
    domain_ulid: String,
    trial_ulid: String,
    left: AuthoredFormExpressionV1,
    right: AuthoredFormExpressionV1,
}

/// Closed expression vocabulary persisted by
/// [`AuthoredFormulationProjection`].
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum AuthoredFormExpressionV1 {
    Number {
        value: f64,
    },
    Field {
        ulid: String,
    },
    Parameter {
        ulid: String,
    },
    Coordinate {
        axis: usize,
    },
    Test {
        field_ulid: String,
    },
    Neg {
        value: Box<Self>,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Sub {
        left: Box<Self>,
        right: Box<Self>,
    },
    Mul {
        left: Box<Self>,
        right: Box<Self>,
    },
    Div {
        left: Box<Self>,
        right: Box<Self>,
    },
    Pow {
        base: Box<Self>,
        exponent: i32,
    },
    Gradient {
        value: Box<Self>,
    },
    Sin {
        value: Box<Self>,
    },
    Dot {
        left: Box<Self>,
        right: Box<Self>,
    },
    Integrate {
        domain_ulid: String,
        integrand: Box<Self>,
    },
}

impl AuthoredFormulationProjection {
    pub(super) fn encode(
        source_identity: String,
        relation: RawId,
        domain: RawId,
        trial: RawId,
        left: &AuthoredFormExpression,
        right: &AuthoredFormExpression,
    ) -> Self {
        let wire = WireForm {
            schema: SCHEMA.to_owned(),
            source_identity,
            relation_ulid: ulid(relation),
            domain_ulid: ulid(domain),
            trial_ulid: ulid(trial),
            left: expression(left),
            right: expression(right),
        };
        let canonical_bytes = serde_json::to_vec(&wire)
            .expect("typed authored Formulation is canonical JSON")
            .into_boxed_slice();
        Self {
            wire,
            canonical_bytes,
        }
    }

    /// Decode exactly one bounded canonical v1 projection.
    ///
    /// # Errors
    /// Returns a diagnostic for an oversized, malformed, noncanonical, or
    /// identity-malformed projection.
    pub fn decode(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_BYTES {
            return Err(rejection("projection exceeds the decoder limit"));
        }
        let wire: WireForm = serde_json::from_slice(bytes)
            .map_err(|_| rejection("projection is not the closed canonical wire"))?;
        if wire.schema != SCHEMA || serde_json::to_vec(&wire).ok().as_deref() != Some(bytes) {
            return Err(rejection(
                "projection schema or canonical encoding is invalid",
            ));
        }
        if wire.source_identity.len() != 64
            || !wire
                .source_identity
                .bytes()
                .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        {
            return Err(rejection(
                "source identity is not one canonical SHA-256 digest",
            ));
        }
        for (label, value) in [
            ("Relation", wire.relation_ulid.as_str()),
            ("Domain", wire.domain_ulid.as_str()),
            ("trial Field", wire.trial_ulid.as_str()),
        ] {
            let parsed = value
                .parse::<Ulid>()
                .map_err(|_| rejection(&format!("{label} identity is not one canonical ULID")))?;
            if parsed.to_string() != value {
                return Err(rejection(&format!(
                    "{label} identity is not one canonical ULID"
                )));
            }
        }
        Ok(Self {
            wire,
            canonical_bytes: bytes.into(),
        })
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    #[must_use]
    pub fn source_identity(&self) -> &str {
        &self.wire.source_identity
    }

    #[must_use]
    pub fn relation_ulid(&self) -> &str {
        &self.wire.relation_ulid
    }

    #[must_use]
    pub fn domain_ulid(&self) -> &str {
        &self.wire.domain_ulid
    }

    #[must_use]
    pub fn trial_ulid(&self) -> &str {
        &self.wire.trial_ulid
    }

    #[must_use]
    pub const fn left(&self) -> &AuthoredFormExpressionV1 {
        &self.wire.left
    }

    #[must_use]
    pub const fn right(&self) -> &AuthoredFormExpressionV1 {
        &self.wire.right
    }
}

fn expression(value: &AuthoredFormExpression) -> AuthoredFormExpressionV1 {
    match &value.kind {
        AuthoredFormExpressionKind::Number(value) => {
            AuthoredFormExpressionV1::Number { value: *value }
        }
        AuthoredFormExpressionKind::Field(id) => AuthoredFormExpressionV1::Field {
            ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Parameter(id) => AuthoredFormExpressionV1::Parameter {
            ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Coordinate(axis) => {
            AuthoredFormExpressionV1::Coordinate { axis: *axis }
        }
        AuthoredFormExpressionKind::Test(id) => AuthoredFormExpressionV1::Test {
            field_ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Neg(value) => AuthoredFormExpressionV1::Neg {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Binary {
            operator,
            left,
            right,
        } => {
            let left = Box::new(expression(left));
            let right = Box::new(expression(right));
            match operator {
                BinaryOp::Add => AuthoredFormExpressionV1::Add { left, right },
                BinaryOp::Sub => AuthoredFormExpressionV1::Sub { left, right },
                BinaryOp::Mul => AuthoredFormExpressionV1::Mul { left, right },
                BinaryOp::Div => AuthoredFormExpressionV1::Div { left, right },
                BinaryOp::Pow => unreachable!("power is represented by the typed Pow node"),
            }
        }
        AuthoredFormExpressionKind::Pow(base, exponent) => AuthoredFormExpressionV1::Pow {
            base: Box::new(expression(base)),
            exponent: *exponent,
        },
        AuthoredFormExpressionKind::Gradient(value) => AuthoredFormExpressionV1::Gradient {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Sin(value) => AuthoredFormExpressionV1::Sin {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Dot(left, right) => AuthoredFormExpressionV1::Dot {
            left: Box::new(expression(left)),
            right: Box::new(expression(right)),
        },
        AuthoredFormExpressionKind::Integrate { domain, integrand } => {
            AuthoredFormExpressionV1::Integrate {
                domain_ulid: ulid(domain.erase()),
                integrand: Box::new(expression(integrand)),
            }
        }
    }
}

fn ulid(id: RawId) -> String {
    id.ulid().to_string()
}

fn rejection(message: &str) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_DISCRETIZATION,
        format!("authored scalar-primal Formulation rejected: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use eqiora_core::entity::kinds;
    use eqiora_core::{DimExponents, Id, ValueShape};

    use super::*;

    fn projection() -> AuthoredFormulationProjection {
        let id = |value: &str| value.parse::<Ulid>().expect("fixed ULID");
        let expression = AuthoredFormExpression {
            kind: AuthoredFormExpressionKind::Number(1.0),
            dimension: DimExponents::DIMENSIONLESS,
            shape: ValueShape::scalar(),
            support: None,
        };
        AuthoredFormulationProjection::encode(
            "a".repeat(64),
            Id::<kinds::Relation>::from_ulid(id("01ARZ3NDEKTSV4RRFFQ69G5FAV")).erase(),
            Id::<kinds::Domain>::from_ulid(id("01ARZ3NDEKTSV4RRFFQ69G5FAW")).erase(),
            Id::<kinds::Field>::from_ulid(id("01ARZ3NDEKTSV4RRFFQ69G5FAX")).erase(),
            &expression,
            &expression,
        )
    }

    #[test]
    fn one_codec_owns_canonical_round_trip_and_fail_closed_decode() {
        let projection = projection();
        let bytes = projection.canonical_bytes();
        assert_eq!(
            AuthoredFormulationProjection::decode(bytes).unwrap(),
            projection
        );

        let mut trailing = bytes.to_vec();
        trailing.push(b' ');
        assert!(AuthoredFormulationProjection::decode(&trailing).is_err());

        let unknown = String::from_utf8(bytes.to_vec())
            .unwrap()
            .replace("\"relation_ulid\"", "\"unknown\":0,\"relation_ulid\"");
        assert!(AuthoredFormulationProjection::decode(unknown.as_bytes()).is_err());

        let malformed_identity = String::from_utf8(bytes.to_vec())
            .unwrap()
            .replace("01ARZ3NDEKTSV4RRFFQ69G5FAV", "not-a-canonical-ulid-value");
        assert!(AuthoredFormulationProjection::decode(malformed_identity.as_bytes()).is_err());

        assert!(AuthoredFormulationProjection::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
    }
}
