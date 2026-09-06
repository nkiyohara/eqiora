//! One Kernel node on the current Model wire.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_schema::kernel::{
    ActivationDef, BoundaryPhysicalConnector, ConnectionDef, DomainDef, DomainKind, FieldDef,
    GeometryDigest, KernelNode, ParameterDef, PortDef, PortPayload, RelationDef, RepresentationDef,
};
use serde::{Deserialize, Serialize};

use crate::{ArtifactDigest, invalid_artifact};

use super::value_type::WireValueType;
use super::*;
use super::{expression::*, primitive::*, vocabulary::*};

impl WireNode {
    pub(crate) fn encode(node: &KernelNode) -> Result<Self, Diagnostic> {
        let definition = match node {
            KernelNode::Domain(value) => WireNodeDefinition::Domain {
                domain: WireDomainKind::encode(value.kind())?,
            },
            KernelNode::Representation(value) => WireNodeDefinition::Representation {
                representation: WireRepresentationKind::encode(value.kind())?,
            },
            KernelNode::Field(value) => WireNodeDefinition::Field {
                value_type: WireValueType::encode(value.value_type())?,
                initial: value.initial().map(eqiora_core::ValueLiteral::literal),
            },
            KernelNode::Parameter(value) => WireNodeDefinition::Parameter {
                value_type: WireValueType::encode(value.value_type())?,
                literal: value.literal(),
            },
            KernelNode::Port(value) => match value.payload() {
                PortPayload::ScalarPhysical { domain } => WireNodeDefinition::ScalarPhysicalPort {
                    domain: WireId::from_raw(domain.erase()),
                },
                PortPayload::BoundaryPhysical {
                    connector,
                    boundary,
                } => WireNodeDefinition::BoundaryPhysicalPort {
                    connector: WireId::from_raw(connector.erase()),
                    boundary: WireId::from_raw(boundary.erase()),
                },
                PortPayload::Signal {
                    direction,
                    value_type,
                } => WireNodeDefinition::SignalPort {
                    direction: WireSignalDirection::encode(direction),
                    value_type: WireValueType::encode(&value_type)?,
                },
                _ => return Err(invalid_artifact("unsupported Port payload")),
            },
            KernelNode::Relation(value) => WireNodeDefinition::Relation {
                residuals: WireExpression::encode(value.residuals())?,
            },
            KernelNode::Activation(value) => WireNodeDefinition::Activation {
                activation: WireActivationKind::encode(value.kind())?,
            },
            KernelNode::Connection(value) => WireNodeDefinition::Connection {
                connection: WireConnectionKind::encode(value.semantics())?,
            },
            KernelNode::ClockDomain(value) => WireNodeDefinition::ClockDomain {
                clock: WireClockKind::encode(value.kind())?,
            },
            _ => {
                return Err(invalid_artifact(
                    "kernel node variant is newer than the current Model wire",
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
            WireNodeDefinition::Field {
                value_type,
                initial,
            } => {
                let id = self.id.typed::<kinds::Field>()?;
                let mut definition = FieldDef::new(id, value_type.decode()?);
                if let Some(initial) = initial {
                    if *initial == 0.0 && initial.is_sign_negative() {
                        return Err(invalid_artifact(
                            "Field initial literal has noncanonical negative zero",
                        ));
                    }
                    let literal =
                        eqiora_core::ValueLiteral::new(definition.value_type().clone(), *initial)
                            .map_err(|error| invalid_artifact(error.to_string()))?;
                    definition = definition
                        .with_initial(literal)
                        .map_err(|error| invalid_artifact(error.message()))?;
                }
                Ok(definition.into())
            }
            WireNodeDefinition::Parameter {
                value_type,
                literal,
            } => {
                if *literal == 0.0 && literal.is_sign_negative() {
                    return Err(invalid_artifact(
                        "Parameter literal has noncanonical negative zero",
                    ));
                }
                Ok(ParameterDef::new(
                    self.id.typed::<kinds::Parameter>()?,
                    value_type.decode()?,
                    *literal,
                )
                .map_err(|error| invalid_artifact(error.message()))?
                .into())
            }
            WireNodeDefinition::SignalPort {
                direction,
                value_type,
            } => Ok(PortDef::signal(
                self.id.typed::<kinds::Port>()?,
                direction.decode(),
                value_type.decode()?,
            )
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

    pub(crate) fn validate_pure_operator_features(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Relation { residuals } => {
                residuals.validate_pure_operator_features()
            }
            WireNodeDefinition::Activation { activation } => {
                activation.validate_pure_operator_features()
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn canonicalize_pure_operator_definitions(&mut self) -> Result<(), Diagnostic> {
        match &mut self.definition {
            WireNodeDefinition::Relation { residuals } => {
                residuals.canonicalize_pure_operator_definitions()
            }
            WireNodeDefinition::Activation { activation } => {
                activation.canonicalize_pure_operator_definitions()
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_value_shape_limits(
        &self,
        limits: ModelDecoderLimits,
    ) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { value_type, .. }
            | WireNodeDefinition::SignalPort { value_type, .. }
            | WireNodeDefinition::Parameter { value_type, .. } => value_type.ensure_limits(limits),
            WireNodeDefinition::Relation { residuals } => {
                residuals.ensure_value_shape_limits(limits)
            }
            WireNodeDefinition::Activation { activation } => {
                activation.ensure_value_shape_limits(limits)
            }
            WireNodeDefinition::Domain {
                domain:
                    WireDomainKind::ScalarPhysical {
                        across_type,
                        through_type,
                    },
            } => {
                across_type.ensure_limits(limits)?;
                through_type.ensure_limits(limits)
            }
            WireNodeDefinition::Domain {
                domain:
                    WireDomainKind::BoundaryPhysical {
                        trace_type,
                        flux_type,
                        ..
                    },
            } => {
                trace_type.ensure_limits(limits)?;
                flux_type.ensure_limits(limits)
            }
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
            WireNodeDefinition::Domain {
                domain: WireDomainKind::CartesianBoxSources { coordinates },
            } => coordinates
                .iter()
                .flat_map(WireCartesianAxisDefinition::semantic_references)
                .collect(),
            _ => Vec::new(),
        }
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
        value_type: WireValueType,
        initial: Option<f64>,
    },
    Parameter {
        value_type: WireValueType,
        literal: f64,
    },
    SignalPort {
        direction: WireSignalDirection,
        value_type: WireValueType,
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
    CartesianBoxSources {
        coordinates: Vec<WireCartesianAxisDefinition>,
    },
    CartesianBoundary {
        axis: usize,
        side: WireBoundarySide,
    },
    GeometryRegion {
        geometry: String,
        entity_set: String,
    },
    GeometryBoundary {
        entity_set: String,
    },
    ScalarPhysical {
        across_type: WireValueType,
        through_type: WireValueType,
    },
    BoundaryPhysical {
        trace_type: WireValueType,
        flux_type: WireValueType,
        pairing: WireBoundaryPairing,
    },
}

impl WireDomainKind {
    pub(crate) fn encode(value: &DomainKind) -> Result<Self, Diagnostic> {
        Ok(match value {
            DomainKind::Abstract => Self::Abstract,
            DomainKind::CartesianBox { coordinates } => Self::CartesianBoxSources {
                coordinates: coordinates
                    .iter()
                    .copied()
                    .map(WireCartesianAxisDefinition::encode)
                    .collect(),
            },
            DomainKind::CartesianBoundary { axis, side } => Self::CartesianBoundary {
                axis: *axis,
                side: WireBoundarySide::encode(*side),
            },
            DomainKind::GeometryRegion {
                geometry,
                entity_set,
            } => Self::GeometryRegion {
                geometry: encode_geometry_digest(*geometry),
                entity_set: entity_set.clone(),
            },
            DomainKind::GeometryBoundary { entity_set } => Self::GeometryBoundary {
                entity_set: entity_set.clone(),
            },
            DomainKind::ScalarPhysical {
                across_type,
                through_type,
            } => Self::ScalarPhysical {
                across_type: WireValueType::encode(across_type)?,
                through_type: WireValueType::encode(through_type)?,
            },
            DomainKind::BoundaryPhysical { connector } => Self::BoundaryPhysical {
                trace_type: WireValueType::encode(connector.trace_type())?,
                flux_type: WireValueType::encode(connector.flux_type())?,
                pairing: WireBoundaryPairing::encode(connector.pairing()),
            },
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
            Self::CartesianBoxSources { coordinates } => DomainDef::cartesian_box_from_sources(
                id,
                coordinates
                    .iter()
                    .map(WireCartesianAxisDefinition::decode)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|error| invalid_artifact(error.message())),
            Self::CartesianBoundary { axis, side } => {
                Ok(DomainDef::cartesian_boundary(id, *axis, side.decode()))
            }
            Self::GeometryRegion {
                geometry,
                entity_set,
            } => DomainDef::geometry_region(id, decode_geometry_digest(geometry)?, entity_set)
                .map_err(|error| invalid_artifact(error.message())),
            Self::GeometryBoundary { entity_set } => DomainDef::geometry_boundary(id, entity_set)
                .map_err(|error| invalid_artifact(error.message())),
            Self::ScalarPhysical {
                across_type,
                through_type,
            } => DomainDef::scalar_physical(id, across_type.decode()?, through_type.decode()?)
                .map_err(|error| invalid_artifact(error.message())),
            Self::BoundaryPhysical {
                trace_type,
                flux_type,
                pairing,
            } => Ok(DomainDef::boundary_physical(
                id,
                BoundaryPhysicalConnector::new(
                    trace_type.decode()?,
                    flux_type.decode()?,
                    pairing.decode(),
                )
                .map_err(|_| invalid_artifact("invalid boundary physical connector contract"))?,
            )),
        }
    }
}

/// Lowercase hex, matching how every other digest crosses this wire.
fn encode_geometry_digest(digest: GeometryDigest) -> String {
    ArtifactDigest::from_sha256(digest.bytes()).to_string()
}

/// A malformed digest is refused rather than truncated or padded, because a
/// Domain naming an unreadable geometry names nothing.
///
/// The shared digest type validates first, so a geometry reference is held to
/// the same canonical hex form as every other digest on this wire; the bytes
/// are then re-read because the Kernel holds bytes rather than text.
fn decode_geometry_digest(value: &str) -> Result<GeometryDigest, Diagnostic> {
    let canonical = ArtifactDigest::from_hex(value.to_owned())?.to_string();
    let mut bytes = [0_u8; 32];
    for (slot, pair) in bytes
        .iter_mut()
        .zip(canonical.as_bytes().as_chunks::<2>().0.iter())
    {
        let text = std::str::from_utf8(pair)
            .map_err(|_| invalid_artifact("geometry digest is not hexadecimal"))?;
        *slot = u8::from_str_radix(text, 16)
            .map_err(|_| invalid_artifact("geometry digest is not hexadecimal"))?;
    }
    Ok(GeometryDigest::new(bytes))
}
