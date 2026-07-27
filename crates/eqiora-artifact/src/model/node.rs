//! One Kernel node on the Model wire, and the generation that admits it.
//!
//! A generation is named rather than compared, so adding one is an edit per
//! capability instead of a search for inline version lists.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_schema::kernel::{
    ActivationDef, BoundaryPhysicalConnector, ConnectionDef, DomainDef, DomainKind, FieldDef,
    KernelNode, ParameterDef, PortDef, PortPayload, RelationDef, RepresentationDef, ValueFrame,
};
use serde::{Deserialize, Serialize};

use crate::invalid_artifact;

use super::*;
use super::{expression::*, primitive::*, vocabulary::*};

impl WireNode {
    pub(crate) fn encode_v1(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V1)
    }

    pub(crate) fn encode_v2(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V2)
    }

    pub(crate) fn encode_v3(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V3)
    }

    pub(crate) fn encode_v4(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V4)
    }

    pub(crate) fn encode_v5(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V5)
    }

    pub(crate) fn encode_v6(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V6)
    }

    pub(crate) fn encode(node: &KernelNode, version: WireVersion) -> Result<Self, Diagnostic> {
        let definition = match node {
            KernelNode::Domain(value) => WireNodeDefinition::Domain {
                domain: WireDomainKind::encode(value.kind(), version)?,
            },
            KernelNode::Representation(value) => WireNodeDefinition::Representation {
                representation: WireRepresentationKind::encode(value.kind())?,
            },
            KernelNode::Field(value) if version.supports_shaped_fields() => {
                WireNodeDefinition::ShapedField {
                    dimension: WireDimension::encode(value.dimension()),
                    shape: WireValueShape::encode(value.shape()),
                    frame: WireValueFrame::encode(value.frame()),
                    initial: value.initial().map(WireQuantity::encode),
                }
            }
            KernelNode::Field(value)
                if value.shape().is_scalar() && value.frame() == ValueFrame::Invariant =>
            {
                WireNodeDefinition::Field {
                    dimension: WireDimension::encode(value.dimension()),
                    initial: value.initial().map(WireQuantity::encode),
                }
            }
            KernelNode::Field(_) => {
                return Err(invalid_artifact("shaped Field requires model wire v3"));
            }
            KernelNode::Parameter(value) => WireNodeDefinition::Parameter {
                value: WireQuantity::encode(value.value()),
            },
            KernelNode::Port(value) => match value.payload() {
                PortPayload::ScalarPhysical { domain } if version.supports_scalar_physical() => {
                    WireNodeDefinition::ScalarPhysicalPort {
                        domain: WireId::from_raw(domain.erase()),
                    }
                }
                PortPayload::ScalarPhysical { .. } => {
                    return Err(invalid_artifact(
                        "scalar physical Port requires model wire v2",
                    ));
                }
                PortPayload::BoundaryPhysical {
                    connector,
                    boundary,
                } if version.supports_boundary_physical() => {
                    WireNodeDefinition::BoundaryPhysicalPort {
                        connector: WireId::from_raw(connector.erase()),
                        boundary: WireId::from_raw(boundary.erase()),
                    }
                }
                PortPayload::BoundaryPhysical { .. } => {
                    return Err(invalid_artifact(
                        "field-valued boundary physical Port requires model wire v3",
                    ));
                }
                payload => WireNodeDefinition::Port {
                    port: WirePortKind::encode(payload)?,
                    dimension: WireDimension::encode(
                        value
                            .signal_contract()
                            .map(|(_, dimension)| dimension)
                            .or_else(|| value.marker_dimension())
                            .ok_or_else(|| invalid_artifact("Port payload has no v1 dimension"))?,
                    ),
                },
            },
            KernelNode::Relation(value) => WireNodeDefinition::Relation {
                residuals: WireExpression::encode(value.residuals(), version)?,
            },
            KernelNode::Activation(value) => WireNodeDefinition::Activation {
                activation: WireActivationKind::encode(value.kind(), version)?,
            },
            KernelNode::Connection(value) => WireNodeDefinition::Connection {
                connection: WireConnectionKind::encode(value.semantics(), version)?,
            },
            KernelNode::ClockDomain(value) => WireNodeDefinition::ClockDomain {
                clock: WireClockKind::encode(value.kind())?,
            },
            _ => {
                return Err(invalid_artifact(
                    "kernel node variant is newer than wire v1",
                ));
            }
        };
        Ok(Self {
            id: WireId::from_raw(node.id()),
            definition,
        })
    }

    pub(crate) fn decode(&self) -> Result<KernelNode, Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain { domain } => {
                let id = self.id.typed::<kinds::Domain>()?;
                Ok(domain.decode(id)?.into())
            }
            WireNodeDefinition::Representation { representation } => {
                let id = self.id.typed::<kinds::Representation>()?;
                Ok(match representation {
                    WireRepresentationKind::Abstract => RepresentationDef::new(id),
                    WireRepresentationKind::Continuum => RepresentationDef::continuum(id),
                }
                .into())
            }
            WireNodeDefinition::Field { dimension, initial } => {
                let id = self.id.typed::<kinds::Field>()?;
                let mut definition = FieldDef::new(id, dimension.decode());
                if let Some(initial) = initial {
                    definition = definition
                        .with_initial(initial.decode()?)
                        .map_err(|error| invalid_artifact(error.message()))?;
                }
                Ok(definition.into())
            }
            WireNodeDefinition::ShapedField {
                dimension,
                shape,
                frame,
                initial,
            } => {
                let id = self.id.typed::<kinds::Field>()?;
                let mut definition =
                    FieldDef::shaped(id, dimension.decode(), shape.decode()?, frame.decode())
                        .map_err(|error| invalid_artifact(error.message()))?;
                if let Some(initial) = initial {
                    definition = definition
                        .with_initial(initial.decode()?)
                        .map_err(|error| invalid_artifact(error.message()))?;
                }
                Ok(definition.into())
            }
            WireNodeDefinition::Parameter { value } => {
                Ok(ParameterDef::new(self.id.typed::<kinds::Parameter>()?, value.decode()?).into())
            }
            WireNodeDefinition::Port { port, dimension } => Ok(port
                .decode(self.id.typed::<kinds::Port>()?, dimension.decode())
                .into()),
            WireNodeDefinition::ScalarPhysicalPort { domain } => Ok(PortDef::scalar_physical(
                self.id.typed::<kinds::Port>()?,
                domain.typed::<kinds::Domain>()?,
            )
            .into()),
            WireNodeDefinition::BoundaryPhysicalPort {
                connector,
                boundary,
            } => Ok(PortDef::boundary_physical(
                self.id.typed::<kinds::Port>()?,
                connector.typed::<kinds::Domain>()?,
                boundary.typed::<kinds::Domain>()?,
            )
            .into()),
            WireNodeDefinition::Relation { residuals } => Ok(RelationDef::new(
                self.id.typed::<kinds::Relation>()?,
                residuals.decode()?,
            )
            .into()),
            WireNodeDefinition::Activation { activation } => Ok(ActivationDef::new(
                self.id.typed::<kinds::Activation>()?,
                activation.decode()?,
            )
            .map_err(|error| invalid_artifact(error.message()))?
            .into()),
            WireNodeDefinition::Connection { connection } => Ok(ConnectionDef::new(
                self.id.typed::<kinds::Connection>()?,
                connection.decode(),
            )
            .into()),
            WireNodeDefinition::ClockDomain { clock } => {
                Ok(clock.decode(self.id.typed::<kinds::ClockDomain>()?)?.into())
            }
        }
    }

    pub(crate) fn expression_node_count(&self) -> usize {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.nodes.len(),
            WireNodeDefinition::Activation { activation } => activation.expression_node_count(),
            _ => 0,
        }
    }

    pub(crate) fn expression_root_count(&self) -> usize {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.roots.len(),
            WireNodeDefinition::Activation { activation } => activation.expression_root_count(),
            _ => 0,
        }
    }

    pub(crate) fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.pure_operator_counts(),
            WireNodeDefinition::Activation { activation } => activation.pure_operator_counts(),
            _ => Ok(PureOperatorWireCounts::default()),
        }
    }

    pub(crate) fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.validate_v5_features(),
            WireNodeDefinition::Activation { activation } => activation.validate_v5_features(),
            _ => Ok(()),
        }
    }

    pub(crate) fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        match &mut self.definition {
            WireNodeDefinition::Relation { residuals } => residuals.canonicalize_v5_definitions(),
            WireNodeDefinition::Activation { activation } => {
                activation.canonicalize_v5_definitions()
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v1(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain {
                domain: WireDomainKind::ScalarPhysical { .. },
            }
            | WireNodeDefinition::ScalarPhysicalPort { .. }
            | WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { .. },
            }
            | WireNodeDefinition::ShapedField { .. }
            | WireNodeDefinition::BoundaryPhysicalPort { .. }
            | WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "model wire v1 cannot contain physical interface semantics or shaped Fields",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v1(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v1(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v2(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { .. },
            }
            | WireNodeDefinition::ShapedField { .. }
            | WireNodeDefinition::BoundaryPhysicalPort { .. }
            | WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "model wire v2 cannot contain boundary physical semantics or shaped Fields",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v2(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v2(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v3(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v3 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v3(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v3(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v4(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v4 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            WireNodeDefinition::Relation { residuals } => residuals.ensure_v4(),
            WireNodeDefinition::Activation { activation } => activation.ensure_v4(),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v5(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v5 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Connection {
                connection: WireConnectionKind::SpatialPeriodic,
            } => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v6(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v6 requires the single shaped Field representation",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_value_shape_limits(
        &self,
        limits: ModelDecoderLimits,
    ) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::ShapedField { shape, .. }
            | WireNodeDefinition::Domain {
                domain: WireDomainKind::BoundaryPhysical { shape, .. },
            } => shape.ensure_limits(limits),
            _ => Ok(()),
        }
    }

    pub(crate) fn semantic_references(&self) -> Vec<&WireId> {
        match &self.definition {
            WireNodeDefinition::ScalarPhysicalPort { domain } => vec![domain],
            WireNodeDefinition::BoundaryPhysicalPort {
                connector,
                boundary,
            } => vec![connector, boundary],
            WireNodeDefinition::Relation { residuals } => residuals.semantic_references(),
            WireNodeDefinition::Activation { activation } => activation.semantic_references(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
}

impl WireVersion {
    pub(crate) const fn supports_scalar_physical(self) -> bool {
        matches!(self, Self::V2 | Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    pub(crate) const fn supports_boundary_physical(self) -> bool {
        matches!(self, Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }

    pub(crate) const fn supports_shaped_fields(self) -> bool {
        matches!(self, Self::V3 | Self::V4 | Self::V5 | Self::V6)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireNodeDefinition {
    Domain {
        domain: WireDomainKind,
    },
    Representation {
        representation: WireRepresentationKind,
    },
    Field {
        dimension: WireDimension,
        initial: Option<WireQuantity>,
    },
    ShapedField {
        dimension: WireDimension,
        shape: WireValueShape,
        frame: WireValueFrame,
        initial: Option<WireQuantity>,
    },
    Parameter {
        value: WireQuantity,
    },
    Port {
        port: WirePortKind,
        dimension: WireDimension,
    },
    ScalarPhysicalPort {
        domain: WireId,
    },
    BoundaryPhysicalPort {
        connector: WireId,
        boundary: WireId,
    },
    Relation {
        residuals: WireExpression,
    },
    Activation {
        activation: WireActivationKind,
    },
    Connection {
        connection: WireConnectionKind,
    },
    ClockDomain {
        clock: WireClockKind,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireDomainKind {
    Abstract,
    CartesianBox {
        bounds: Vec<WireAxisBounds>,
    },
    CartesianBoundary {
        axis: usize,
        side: WireBoundarySide,
    },
    ScalarPhysical {
        across_dimension: WireDimension,
        through_dimension: WireDimension,
    },
    BoundaryPhysical {
        trace_dimension: WireDimension,
        flux_dimension: WireDimension,
        shape: WireValueShape,
        frame: WireValueFrame,
        pairing: WireBoundaryPairing,
    },
}

impl WireDomainKind {
    pub(crate) fn encode(value: &DomainKind, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(match value {
            DomainKind::Abstract => Self::Abstract,
            DomainKind::CartesianBox { bounds } => Self::CartesianBox {
                bounds: bounds.iter().copied().map(WireAxisBounds::encode).collect(),
            },
            DomainKind::CartesianBoundary { axis, side } => Self::CartesianBoundary {
                axis: *axis,
                side: WireBoundarySide::encode(*side),
            },
            DomainKind::ScalarPhysical {
                across_dimension,
                through_dimension,
            } if version.supports_scalar_physical() => Self::ScalarPhysical {
                across_dimension: WireDimension::encode(*across_dimension),
                through_dimension: WireDimension::encode(*through_dimension),
            },
            DomainKind::BoundaryPhysical { connector } if version.supports_boundary_physical() => {
                Self::BoundaryPhysical {
                    trace_dimension: WireDimension::encode(connector.trace_dimension()),
                    flux_dimension: WireDimension::encode(connector.flux_dimension()),
                    shape: WireValueShape::encode(connector.shape()),
                    frame: WireValueFrame::encode(connector.frame()),
                    pairing: WireBoundaryPairing::encode(connector.pairing()),
                }
            }
            _ => {
                return Err(invalid_artifact(
                    "the model contains a Domain kind unsupported by this model wire",
                ));
            }
        })
    }

    pub(crate) fn decode(&self, id: Id<kinds::Domain>) -> Result<DomainDef, Diagnostic> {
        match self {
            Self::Abstract => Ok(DomainDef::new(id)),
            Self::CartesianBox { bounds } => DomainDef::cartesian_box(
                id,
                bounds
                    .iter()
                    .map(WireAxisBounds::decode)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|error| invalid_artifact(error.message())),
            Self::CartesianBoundary { axis, side } => {
                Ok(DomainDef::cartesian_boundary(id, *axis, side.decode()))
            }
            Self::ScalarPhysical {
                across_dimension,
                through_dimension,
            } => Ok(DomainDef::scalar_physical(
                id,
                across_dimension.decode(),
                through_dimension.decode(),
            )),
            Self::BoundaryPhysical {
                trace_dimension,
                flux_dimension,
                shape,
                frame,
                pairing,
            } => Ok(DomainDef::boundary_physical(
                id,
                BoundaryPhysicalConnector::new(
                    trace_dimension.decode(),
                    flux_dimension.decode(),
                    shape.decode()?,
                    frame.decode(),
                    pairing.decode(),
                )
                .map_err(|_| invalid_artifact("invalid boundary physical connector contract"))?,
            )),
        }
    }
}
