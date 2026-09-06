use eqiora_core::{Diagnostic, ScalarDomain, ValueShape};
use eqiora_core::{ValueFrame, ValueType};

use serde::{Deserialize, Serialize};

use super::{
    ModelDecoderLimits,
    vocabulary::{WireValueFrame, WireValueShape},
};
use crate::{dimension::WireDimension, invalid_artifact};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireValueType {
    domain: WireScalarDomain,
    dimension: WireDimension,
    shape: WireValueShape,
    frame: WireValueFrame,
    array_rank: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireScalarDomain {
    Real,
    Complex,
}

impl WireValueType {
    pub(super) fn encode(value: &ValueType) -> Result<Self, Diagnostic> {
        Ok(Self {
            domain: match value.scalar_domain() {
                ScalarDomain::Real => WireScalarDomain::Real,
                ScalarDomain::Complex => WireScalarDomain::Complex,
            },
            dimension: WireDimension::encode(value.dimension()),
            shape: WireValueShape::encode(value.shape()),
            frame: WireValueFrame::encode(value.frame()),
            array_rank: u32::try_from(value.array_rank())
                .map_err(|_| invalid_artifact("array rank exceeds u32"))?,
        })
    }

    pub(super) fn decode(&self) -> Result<ValueType, Diagnostic> {
        let shape = self.shape.decode()?;
        let rank = usize::try_from(self.array_rank)
            .map_err(|_| invalid_artifact("array rank exceeds usize"))?;
        if rank > shape.rank()
            || (self.frame.decode() == ValueFrame::Invariant && rank != shape.rank())
        {
            return Err(invalid_artifact(
                "mathematical type has inconsistent array and spatial axis roles",
            ));
        }
        let (arrays, spatial) = shape.extents().split_at(rank);
        let mut value = ValueType::shaped(
            match self.domain {
                WireScalarDomain::Real => ScalarDomain::Real,
                WireScalarDomain::Complex => ScalarDomain::Complex,
            },
            self.dimension.decode(),
            ValueShape::new(spatial.iter().map(|n| n.get()))
                .map_err(|error| invalid_artifact(error.to_string()))?,
            self.frame.decode(),
        )
        .map_err(|error| invalid_artifact(error.to_string()))?;
        for extent in arrays.iter().rev() {
            value = value
                .array(extent.get())
                .map_err(|error| invalid_artifact(error.to_string()))?;
        }
        Ok(value)
    }

    pub(super) fn ensure_limits(&self, limits: ModelDecoderLimits) -> Result<(), Diagnostic> {
        self.shape.ensure_limits(limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_initial_wire_rejects_noncanonical_and_invalid_literals() {
        let value_type = ValueType::scalar(
            ScalarDomain::Complex,
            eqiora_core::DimExponents::DIMENSIONLESS,
        )
        .array(2)
        .unwrap();
        let node = eqiora_schema::kernel::KernelNode::from(
            eqiora_schema::kernel::FieldDef::new(eqiora_core::Id::new(), value_type.clone())
                .with_initial(eqiora_core::ValueLiteral::new(value_type, 0.0).unwrap())
                .unwrap(),
        );
        assert_eq!(WireNode::encode(&node).unwrap().decode().unwrap(), node);
        for invalid in [-0.0, 1.0, f64::INFINITY, f64::NAN] {
            let mut wire = WireNode::encode(&node).unwrap();
            let crate::model::WireNodeDefinition::Field { initial, .. } = &mut wire.definition
            else {
                panic!("Field");
            };
            *initial = Some(invalid);
            assert!(wire.decode().is_err());
        }
    }
    use crate::model::WireNode;
    use eqiora_core::{DimExponents, Id};
    use eqiora_schema::kernel::{FieldDef, KernelNode};

    #[test]
    fn field_wire_retains_complete_mathematical_type() {
        let id = Id::new();
        let scalar = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
        let vector = ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let tensor = ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let mut encodings = std::collections::BTreeSet::new();
        for value in [
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS),
            scalar.clone(),
            scalar.array(2).unwrap().array(2).unwrap(),
            vector.array(2).unwrap(),
            tensor,
        ] {
            let node = KernelNode::from(FieldDef::new(id, value));
            let bytes = serde_json::to_vec(&WireNode::encode(&node).unwrap()).unwrap();
            assert!(encodings.insert(bytes.clone()));
            let replayed: WireNode = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(replayed.decode().unwrap(), node);
        }
    }

    #[test]
    fn decoder_rejects_inconsistent_axis_roles_and_unknown_scalar_domains() {
        let value = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
            .array(2)
            .unwrap();
        let wire = WireValueType::encode(&value).unwrap();
        for rank in [0, 2] {
            let mut invalid = wire.clone();
            invalid.array_rank = rank;
            assert!(invalid.decode().is_err());
        }
        let mut invalid = wire.clone();
        invalid.frame = WireValueFrame::SpatialCartesian;
        assert!(invalid.decode().is_err());
        let mut json = serde_json::to_value(&wire).unwrap();
        json["domain"] = serde_json::json!("floating-point");
        assert!(serde_json::from_value::<WireValueType>(json).is_err());
    }
}
