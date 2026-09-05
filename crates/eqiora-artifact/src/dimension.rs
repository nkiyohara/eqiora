use eqiora_core::{Diagnostic, DimExponents};
use serde::{Deserialize, Serialize};

use crate::invalid_artifact;

/// Exact SI-order fractions, validated at deserialization rather than at each consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "[(i32, i32); 7]", into = "[(i32, i32); 7]")]
pub(crate) struct WireDimension(DimExponents);

impl WireDimension {
    pub(crate) const fn encode(value: DimExponents) -> Self {
        Self(value)
    }

    pub(crate) const fn decode(self) -> DimExponents {
        self.0
    }
}

impl From<WireDimension> for [(i32, i32); 7] {
    fn from(value: WireDimension) -> Self {
        value.0.exponents()
    }
}

impl TryFrom<[(i32, i32); 7]> for WireDimension {
    type Error = Diagnostic;

    fn try_from(value: [(i32, i32); 7]) -> Result<Self, Self::Error> {
        let dimension = DimExponents::from_rationals(value)
            .ok_or_else(|| invalid_artifact("invalid rational dimension exponents"))?;
        if dimension.exponents() != value {
            return Err(invalid_artifact(
                "dimension exponents must be canonical reduced fractions",
            ));
        }
        Ok(Self(dimension))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_dimension_round_trip_preserves_exact_fraction() {
        let dimension =
            DimExponents::from_rationals([(0, 1), (-1, 2), (1, 2), (0, 1), (0, 1), (0, 1), (0, 1)])
                .unwrap();
        let json = serde_json::to_string(&WireDimension::encode(dimension)).unwrap();
        assert_eq!(json, "[[0,1],[-1,2],[1,2],[0,1],[0,1],[0,1],[0,1]]");
        assert_eq!(
            serde_json::from_str::<WireDimension>(&json)
                .unwrap()
                .decode(),
            dimension
        );
    }

    #[test]
    fn decoder_rejects_invalid_and_noncanonical_fractions_and_integer_wire() {
        for pair in [
            (1, 0),
            (1, -2),
            (2, 4),
            (0, 2),
            (i32::MIN, 1),
            (1, i32::MIN),
        ] {
            let mut wire = [(0, 1); 7];
            wire[1] = pair;
            let json = serde_json::to_string(&wire).unwrap();
            assert!(
                serde_json::from_str::<WireDimension>(&json).is_err(),
                "{json}"
            );
        }
        for json in [
            "[0,1,0,0,0,0,0]",
            "{\"mass\":0,\"length\":1,\"time\":0,\"current\":0,\"temperature\":0,\"amount\":0,\"luminous_intensity\":0}",
            "[[0,1],[2147483648,1],[0,1],[0,1],[0,1],[0,1],[0,1]]",
        ] {
            assert!(
                serde_json::from_str::<WireDimension>(json).is_err(),
                "{json}"
            );
        }
    }
}
