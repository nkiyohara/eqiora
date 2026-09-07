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
    use crate::model::WireNode;
    use eqiora_core::{DimExponents, Id};
    use eqiora_schema::kernel::{FieldDef, KernelNode};

    #[test]
    fn field_wire_preserves_role_and_rejects_displaced_initial_payload() {
        let value_type = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
            .array(2)
            .unwrap();
        for role in [
            eqiora_schema::kernel::FieldRole::Variable,
            eqiora_schema::kernel::FieldRole::State,
        ] {
            let node = KernelNode::from(FieldDef::new(Id::new(), value_type.clone(), role));
            let wire = WireNode::encode(&node).unwrap();
            assert_eq!(wire.decode().unwrap(), node);
            let mut json = serde_json::to_value(&wire).unwrap();
            json["definition"]["initial"] = serde_json::json!(0.0);
            assert!(serde_json::from_value::<WireNode>(json).is_err());
        }
    }

    #[test]
    fn scalar_physical_wire_retains_domains_and_rejects_channel_substitution() {
        use crate::model::{WireNodeDefinition, node::WireDomainKind};
        use eqiora_schema::kernel::DomainDef;
        let across = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
        let through = ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::from_integers([0, 0, 0, 1, 0, 0, 0]).unwrap(),
        );
        let node = KernelNode::from(
            DomainDef::scalar_physical(Id::new(), across.clone(), through.clone()).unwrap(),
        );
        let wire = WireNode::encode(&node).unwrap();
        assert_eq!(wire.decode().unwrap(), node);
        for change_across in [true, false] {
            let mut malformed = wire.clone();
            let WireNodeDefinition::Domain {
                domain:
                    WireDomainKind::ScalarPhysical {
                        across_type,
                        through_type,
                    },
            } = &mut malformed.definition
            else {
                panic!("scalar physical domain");
            };
            let target = if change_across {
                across_type
            } else {
                through_type
            };
            *target = WireValueType::encode(&across.clone().array(1).unwrap()).unwrap();
            assert!(malformed.decode().is_err());
        }
        assert!(
            DomainDef::scalar_physical(
                Id::new(),
                across.clone().array(1).unwrap(),
                through.clone()
            )
            .is_err()
        );
        assert!(DomainDef::scalar_physical(Id::new(), across, through.array(1).unwrap()).is_err());
    }

    #[test]
    fn boundary_connector_wire_preserves_component_roles_and_checks_both_types() {
        use crate::model::WireNodeDefinition;
        use crate::model::node::WireDomainKind;
        use eqiora_schema::kernel::{BoundaryPairing, BoundaryPhysicalConnector, DomainDef};
        let vector = ValueType::shaped(
            ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap();
        let nested = vector.array(2).unwrap();
        let connector = BoundaryPhysicalConnector::new(
            nested.clone(),
            nested.clone(),
            BoundaryPairing::EuclideanBoundaryDuality,
        )
        .unwrap();
        let node = KernelNode::from(DomainDef::boundary_physical(Id::new(), connector));
        let wire = WireNode::encode(&node).unwrap();
        assert_eq!(wire.decode().unwrap(), node);
        for trace in [true, false] {
            let mut malformed = wire.clone();
            let WireNodeDefinition::Domain {
                domain:
                    WireDomainKind::BoundaryPhysical {
                        trace_type,
                        flux_type,
                        ..
                    },
            } = &mut malformed.definition
            else {
                panic!("boundary connector");
            };
            let changed = if trace { trace_type } else { flux_type };
            changed.array_rank = 0;
            assert!(
                malformed.decode().is_err(),
                "equal extents cannot erase array roles on one quantity"
            );

            let mut oversized = wire.clone();
            let WireNodeDefinition::Domain {
                domain:
                    WireDomainKind::BoundaryPhysical {
                        trace_type,
                        flux_type,
                        ..
                    },
            } = &mut oversized.definition
            else {
                panic!("boundary connector");
            };
            let changed = if trace { trace_type } else { flux_type };
            *changed = WireValueType::encode(&nested.clone().array(4097).unwrap()).unwrap();
            assert!(
                oversized
                    .ensure_value_shape_limits(ModelDecoderLimits::default())
                    .is_err()
            );
        }
    }

    #[test]
    fn constant_wire_preserves_types_and_rejects_invalid_literals() {
        use crate::model::expression::{WireExpression, WireExpressionNode};
        use eqiora_core::ValueLiteral;
        use eqiora_schema::kernel::ExprDagBuilder;
        let value_type = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
            .array(3)
            .unwrap();
        let mut builder = ExprDagBuilder::new();
        let root = builder
            .constant(ValueLiteral::from_real(value_type, 0.0).unwrap())
            .unwrap();
        let expression = builder.finish([root]).unwrap();
        let wire = WireExpression::encode(&expression).unwrap();
        assert_eq!(wire.decode().unwrap(), expression);
        for invalid in [-0.0, f64::INFINITY, f64::NAN] {
            let mut malformed = wire.clone();
            let WireExpressionNode::Constant { value, .. } = &mut malformed.nodes[0] else {
                panic!("constant");
            };
            value.components = crate::model::literal::WireComponents::Dense {
                values: vec![(invalid, 0.0)],
            };
            assert!(malformed.decode().is_err());
        }
    }

    #[test]
    fn relation_and_guard_constant_wire_obey_shape_limits() {
        use eqiora_core::ValueLiteral;
        use eqiora_schema::kernel::{ActivationDef, ActivationKind, ExprDagBuilder, RelationDef};
        let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        for value_type in [
            scalar.clone().array(4097).unwrap(),
            (0..9).fold(scalar, |value, _| value.array(1).unwrap()),
        ] {
            let mut builder = ExprDagBuilder::new();
            let root = builder
                .constant(ValueLiteral::from_real(value_type, 0.0).unwrap())
                .unwrap();
            let expression = builder.finish([root]).unwrap();
            for node in [
                KernelNode::from(RelationDef::new(Id::new(), expression.clone())),
                KernelNode::from(
                    ActivationDef::new(Id::new(), ActivationKind::Guard { guard: expression })
                        .unwrap(),
                ),
            ] {
                assert!(
                    WireNode::encode(&node)
                        .unwrap()
                        .ensure_value_shape_limits(ModelDecoderLimits::default())
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn parameter_wire_rejects_noncanonical_or_invalid_literals() {
        let value_type = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
            .array(2)
            .unwrap();
        let node = KernelNode::from(eqiora_schema::kernel::ParameterDef::new(
            Id::new(),
            eqiora_core::ValueLiteral::from_real(value_type, 0.0).unwrap(),
        ));
        assert_eq!(WireNode::encode(&node).unwrap().decode().unwrap(), node);
        for invalid in [-0.0, f64::INFINITY, f64::NAN] {
            let mut wire = WireNode::encode(&node).unwrap();
            let crate::model::WireNodeDefinition::Parameter { value, .. } = &mut wire.definition
            else {
                unreachable!()
            };
            value.components = crate::model::literal::WireComponents::Dense {
                values: vec![(invalid, 0.0)],
            };
            assert!(wire.decode().is_err());
        }
    }

    #[test]
    fn parameter_and_signal_wire_obey_the_model_shape_limits() {
        let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        for value_type in [
            scalar.clone().array(4097).unwrap(),
            (0..9).fold(scalar, |value, _| value.array(1).unwrap()),
        ] {
            for node in [
                KernelNode::from(eqiora_schema::kernel::ParameterDef::new(
                    Id::new(),
                    eqiora_core::ValueLiteral::from_real(value_type.clone(), 0.0).unwrap(),
                )),
                KernelNode::from(eqiora_schema::kernel::PortDef::signal(
                    Id::new(),
                    eqiora_schema::kernel::SignalDirection::Input,
                    value_type,
                )),
            ] {
                let wire = WireNode::encode(&node).unwrap();
                assert!(
                    wire.ensure_value_shape_limits(ModelDecoderLimits::default())
                        .is_err()
                );
            }
        }
    }

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
            let node = KernelNode::from(FieldDef::new(
                id,
                value,
                eqiora_schema::kernel::FieldRole::Variable,
            ));
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
