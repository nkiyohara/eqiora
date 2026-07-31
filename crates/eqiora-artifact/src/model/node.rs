//! One Kernel node on the Model wire, and the generation that admits it.
//!
//! A generation is named rather than compared, so adding one is an edit per
//! capability instead of a search for inline version lists.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};
use eqiora_schema::kernel::{
    ActivationDef, AxisBounds, BoundaryPhysicalConnector, CartesianCoordinateSource, ConnectionDef,
    DomainDef, DomainKind, FieldDef, GeometryDigest, KernelNode, ParameterDef, PortDef,
    PortPayload, RelationDef, RepresentationDef, ValueFrame,
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

    pub(crate) fn encode_v7(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V7)
    }

    pub(crate) fn encode_v8(node: &KernelNode) -> Result<Self, Diagnostic> {
        Self::encode(node, WireVersion::V8)
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
        self.reject_coordinate_sources_before_v8()?;
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
        self.reject_coordinate_sources_before_v8()?;
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
        self.reject_coordinate_sources_before_v8()?;
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
        self.reject_coordinate_sources_before_v8()?;
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
        self.reject_coordinate_sources_before_v8()?;
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

    pub(crate) fn ensure_v7(&self) -> Result<(), Diagnostic> {
        // V7 inherits the whole v6 grammar and adds only the geometry Domain
        // kinds, so it admits exactly what v6 admits and more.
        self.ensure_v6()
    }

    pub(crate) fn ensure_v6(&self) -> Result<(), Diagnostic> {
        self.reject_coordinate_sources_before_v8()?;
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v6 requires the single shaped Field representation",
            )),
            _ => Ok(()),
        }
    }

    pub(crate) fn ensure_v8(&self) -> Result<(), Diagnostic> {
        match &self.definition {
            WireNodeDefinition::Field { .. } => Err(invalid_artifact(
                "model wire v8 requires the single shaped Field representation",
            )),
            WireNodeDefinition::Domain {
                domain: WireDomainKind::CartesianBox { .. },
            } => Err(invalid_artifact(
                "model wire v8 requires Cartesian coordinate-source definitions",
            )),
            _ => Ok(()),
        }
    }

    fn reject_coordinate_sources_before_v8(&self) -> Result<(), Diagnostic> {
        if matches!(
            &self.definition,
            WireNodeDefinition::Domain {
                domain: WireDomainKind::CartesianBoxSources { .. },
            }
        ) {
            Err(invalid_artifact(
                "Cartesian coordinate sources require model wire v8",
            ))
        } else {
            Ok(())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireVersion {
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
}

impl WireVersion {
    pub(crate) const fn supports_scalar_physical(self) -> bool {
        matches!(
            self,
            Self::V2 | Self::V3 | Self::V4 | Self::V5 | Self::V6 | Self::V7 | Self::V8
        )
    }

    pub(crate) const fn supports_boundary_physical(self) -> bool {
        matches!(
            self,
            Self::V3 | Self::V4 | Self::V5 | Self::V6 | Self::V7 | Self::V8
        )
    }

    pub(crate) const fn supports_shaped_fields(self) -> bool {
        matches!(
            self,
            Self::V3 | Self::V4 | Self::V5 | Self::V6 | Self::V7 | Self::V8
        )
    }

    /// Whether a Domain may name an authored geometry rather than describe a
    /// box. Only the newest generation may, because an older decoder shown a
    /// geometry reference would have to guess at a shape it cannot read.
    pub(crate) const fn supports_geometry_region(self) -> bool {
        matches!(self, Self::V7 | Self::V8)
    }

    pub(crate) const fn supports_coordinate_sources(self) -> bool {
        matches!(self, Self::V8)
    }

    pub(crate) const fn supports_spatial_periodic(self) -> bool {
        matches!(self, Self::V6 | Self::V7 | Self::V8)
    }

    pub(crate) const fn supports_tensor_operators(self) -> bool {
        matches!(self, Self::V4 | Self::V5 | Self::V6 | Self::V7 | Self::V8)
    }

    pub(crate) const fn supports_pure_operators(self) -> bool {
        matches!(self, Self::V5 | Self::V6 | Self::V7 | Self::V8)
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
            DomainKind::CartesianBox { coordinates } if version.supports_coordinate_sources() => {
                Self::CartesianBoxSources {
                    coordinates: coordinates
                        .iter()
                        .copied()
                        .map(WireCartesianAxisDefinition::encode)
                        .collect(),
                }
            }
            DomainKind::CartesianBox { coordinates } => Self::CartesianBox {
                bounds: coordinates
                    .iter()
                    .copied()
                    .map(|coordinate| {
                        let (
                            CartesianCoordinateSource::Fixed(lower),
                            CartesianCoordinateSource::Fixed(upper),
                        ) = (coordinate.lower(), coordinate.upper())
                        else {
                            return Err(invalid_artifact(
                                "Cartesian coordinate sources require model wire v8",
                            ));
                        };
                        AxisBounds::new(lower, upper)
                            .map(WireAxisBounds::encode)
                            .map_err(|error| invalid_artifact(error.message()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            },
            DomainKind::CartesianBoundary { axis, side } => Self::CartesianBoundary {
                axis: *axis,
                side: WireBoundarySide::encode(*side),
            },
            DomainKind::GeometryRegion {
                geometry,
                entity_set,
            } if version.supports_geometry_region() => Self::GeometryRegion {
                geometry: encode_geometry_digest(*geometry),
                entity_set: entity_set.clone(),
            },
            DomainKind::GeometryBoundary { entity_set } if version.supports_geometry_region() => {
                Self::GeometryBoundary {
                    entity_set: entity_set.clone(),
                }
            }
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
    for (slot, pair) in bytes.iter_mut().zip(canonical.as_bytes().chunks_exact(2)) {
        let text = std::str::from_utf8(pair)
            .map_err(|_| invalid_artifact("geometry digest is not hexadecimal"))?;
        *slot = u8::from_str_radix(text, 16)
            .map_err(|_| invalid_artifact("geometry digest is not hexadecimal"))?;
    }
    Ok(GeometryDigest::new(bytes))
}
