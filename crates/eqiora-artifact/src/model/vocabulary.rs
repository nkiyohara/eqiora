//! Wire spellings of the closed Kernel vocabularies.
//!
//! Each is a small closed enum whose only job is to cross the wire without
//! widening what the Kernel means.

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, ValueShape};
use eqiora_schema::kernel::pure_operator::{
    CalculusBuilder, CalculusNode, CalculusNodeId, ExactRational, PureOperatorDefinition,
    PureValueClass, ResultAxis,
};
use eqiora_schema::kernel::{
    ActivationKind, AxisBounds, BoundaryPairing, BoundarySide, ClockDomainDef, ClockKind,
    ConnectionSemantics, EventDirection, PortDef, PortPayload, RationalTime, RepresentationKind,
    SignalDirection, ValueFrame,
};
use serde::{Deserialize, Serialize};

use crate::invalid_artifact;

use super::*;
use super::{expression::*, node::*, primitive::*};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct WireValueShape(Vec<u32>);

impl WireValueShape {
    pub(crate) fn encode(value: &ValueShape) -> Self {
        Self(value.extents().iter().map(|extent| extent.get()).collect())
    }

    pub(crate) fn decode(&self) -> Result<ValueShape, Diagnostic> {
        ValueShape::new(self.0.iter().copied()).map_err(|error| {
            invalid_artifact(format!(
                "wire value shape has an invalid extent at axis {}",
                error.axis()
            ))
        })
    }

    pub(crate) fn ensure_limits(&self, limits: ModelDecoderLimits) -> Result<(), Diagnostic> {
        require_decoder_count(
            "value-shape rank",
            self.0.len(),
            limits.max_value_shape_rank,
        )?;
        let shape = self.decode()?;
        let components = shape.component_count().ok_or_else(|| {
            invalid_artifact("wire value-shape component product exceeds local usize")
        })?;
        require_decoder_count(
            "value-shape scalar components",
            components,
            limits.max_value_shape_components,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireValueFrame {
    Invariant,
    SpatialCartesian,
}

impl WireValueFrame {
    pub(crate) const fn encode(value: ValueFrame) -> Self {
        match value {
            ValueFrame::Invariant => Self::Invariant,
            ValueFrame::SpatialCartesian => Self::SpatialCartesian,
        }
    }

    pub(crate) const fn decode(self) -> ValueFrame {
        match self {
            Self::Invariant => ValueFrame::Invariant,
            Self::SpatialCartesian => ValueFrame::SpatialCartesian,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireBoundaryPairing {
    EuclideanBoundaryDuality,
}

impl WireBoundaryPairing {
    pub(crate) const fn encode(value: BoundaryPairing) -> Self {
        match value {
            BoundaryPairing::EuclideanBoundaryDuality => Self::EuclideanBoundaryDuality,
        }
    }

    pub(crate) const fn decode(self) -> BoundaryPairing {
        match self {
            Self::EuclideanBoundaryDuality => BoundaryPairing::EuclideanBoundaryDuality,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireAxisBounds {
    pub(crate) lower: WireQuantity,
    pub(crate) upper: WireQuantity,
}

impl WireAxisBounds {
    pub(crate) fn encode(value: AxisBounds) -> Self {
        Self {
            lower: WireQuantity::encode(value.lower()),
            upper: WireQuantity::encode(value.upper()),
        }
    }

    pub(crate) fn decode(&self) -> Result<AxisBounds, Diagnostic> {
        AxisBounds::new(self.lower.decode()?, self.upper.decode()?)
            .map_err(|error| invalid_artifact(error.message()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireBoundarySide {
    Lower,
    Upper,
}

impl WireBoundarySide {
    pub(crate) const fn encode(value: BoundarySide) -> Self {
        match value {
            BoundarySide::Lower => Self::Lower,
            BoundarySide::Upper => Self::Upper,
        }
    }

    pub(crate) const fn decode(self) -> BoundarySide {
        match self {
            Self::Lower => BoundarySide::Lower,
            Self::Upper => BoundarySide::Upper,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireRepresentationKind {
    Abstract,
    Continuum,
}

impl WireRepresentationKind {
    pub(crate) fn encode(value: RepresentationKind) -> Result<Self, Diagnostic> {
        match value {
            RepresentationKind::Abstract => Ok(Self::Abstract),
            RepresentationKind::Continuum => Ok(Self::Continuum),
            _ => Err(invalid_artifact(
                "representation kind is newer than model wire v1",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WirePortKind {
    Signal { direction: WireSignalDirection },
    Conserving,
}

impl WirePortKind {
    pub(crate) fn encode(value: PortPayload) -> Result<Self, Diagnostic> {
        match value {
            PortPayload::Signal { direction, .. } => Ok(Self::Signal {
                direction: WireSignalDirection::encode(direction),
            }),
            PortPayload::ConservingMarker { .. } => Ok(Self::Conserving),
            _ => Err(invalid_artifact(
                "Port payload is newer than the supported model wire vocabulary",
            )),
        }
    }

    pub(crate) const fn decode(self, id: Id<kinds::Port>, dimension: DimExponents) -> PortDef {
        match self {
            Self::Signal { direction } => PortDef::signal(id, direction.decode(), dimension),
            Self::Conserving => PortDef::conserving_marker(id, dimension),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireSignalDirection {
    Input,
    Output,
}

impl WireSignalDirection {
    pub(crate) const fn encode(value: SignalDirection) -> Self {
        match value {
            SignalDirection::Input => Self::Input,
            SignalDirection::Output => Self::Output,
        }
    }

    pub(crate) const fn decode(self) -> SignalDirection {
        match self {
            Self::Input => SignalDirection::Input,
            Self::Output => SignalDirection::Output,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireActivationKind {
    Continuous,
    Periodic,
    Event {
        guard: WireExpression,
        direction: WireEventDirection,
    },
    Guard {
        guard: WireExpression,
    },
}

impl WireActivationKind {
    pub(crate) fn encode(value: &ActivationKind, version: WireVersion) -> Result<Self, Diagnostic> {
        match value {
            ActivationKind::Continuous => Ok(Self::Continuous),
            ActivationKind::Periodic => Ok(Self::Periodic),
            ActivationKind::Event { guard, direction } => Ok(Self::Event {
                guard: WireExpression::encode(guard, version)?,
                direction: WireEventDirection::encode(*direction),
            }),
            ActivationKind::Guard { guard } => Ok(Self::Guard {
                guard: WireExpression::encode(guard, version)?,
            }),
            _ => Err(invalid_artifact(
                "Activation kind is newer than the supported model wire vocabulary",
            )),
        }
    }

    pub(crate) fn decode(&self) -> Result<ActivationKind, Diagnostic> {
        Ok(match self {
            Self::Continuous => ActivationKind::Continuous,
            Self::Periodic => ActivationKind::Periodic,
            Self::Event { guard, direction } => ActivationKind::Event {
                guard: guard.decode()?,
                direction: direction.decode(),
            },
            Self::Guard { guard } => ActivationKind::Guard {
                guard: guard.decode()?,
            },
        })
    }

    pub(crate) fn expression_node_count(&self) -> usize {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.nodes.len(),
            Self::Continuous | Self::Periodic => 0,
        }
    }

    pub(crate) fn expression_root_count(&self) -> usize {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.roots.len(),
            Self::Continuous | Self::Periodic => 0,
        }
    }

    pub(crate) fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.pure_operator_counts(),
            Self::Continuous | Self::Periodic => Ok(PureOperatorWireCounts::default()),
        }
    }

    pub(crate) fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.validate_v5_features(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => {
                guard.canonicalize_v5_definitions()
            }
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn ensure_v1(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v1(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn ensure_v2(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v2(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn ensure_v3(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v3(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn ensure_v4(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.ensure_v4(),
            Self::Continuous | Self::Periodic => Ok(()),
        }
    }

    pub(crate) fn semantic_references(&self) -> Vec<&WireId> {
        match self {
            Self::Event { guard, .. } | Self::Guard { guard } => guard.semantic_references(),
            Self::Continuous | Self::Periodic => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireEventDirection {
    Any,
    Rising,
    Falling,
}

impl WireEventDirection {
    pub(crate) const fn encode(value: EventDirection) -> Self {
        match value {
            EventDirection::Any => Self::Any,
            EventDirection::Rising => Self::Rising,
            EventDirection::Falling => Self::Falling,
        }
    }

    pub(crate) const fn decode(self) -> EventDirection {
        match self {
            Self::Any => EventDirection::Any,
            Self::Rising => EventDirection::Rising,
            Self::Falling => EventDirection::Falling,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireConnectionKind {
    Signal,
    Conserving,
    SpatialPeriodic,
}

impl WireConnectionKind {
    pub(crate) fn encode(
        value: ConnectionSemantics,
        version: WireVersion,
    ) -> Result<Self, Diagnostic> {
        match value {
            ConnectionSemantics::Signal => Ok(Self::Signal),
            ConnectionSemantics::Conserving => Ok(Self::Conserving),
            ConnectionSemantics::SpatialPeriodic if version == WireVersion::V6 => {
                Ok(Self::SpatialPeriodic)
            }
            ConnectionSemantics::SpatialPeriodic => Err(invalid_artifact(
                "spatial-periodic Connection semantics require model wire v6",
            )),
            _ => Err(invalid_artifact(
                "connection semantics are newer than model wire v1",
            )),
        }
    }

    pub(crate) const fn decode(self) -> ConnectionSemantics {
        match self {
            Self::Signal => ConnectionSemantics::Signal,
            Self::Conserving => ConnectionSemantics::Conserving,
            Self::SpatialPeriodic => ConnectionSemantics::SpatialPeriodic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireClockKind {
    Continuous,
    Periodic {
        period: WireRationalTime,
        phase: WireRationalTime,
    },
    Aperiodic,
    Inherited,
}

impl WireClockKind {
    pub(crate) fn encode(value: ClockKind) -> Result<Self, Diagnostic> {
        match value {
            ClockKind::Continuous => Ok(Self::Continuous),
            ClockKind::Periodic { period, phase } => Ok(Self::Periodic {
                period: WireRationalTime::encode(period),
                phase: WireRationalTime::encode(phase),
            }),
            ClockKind::Aperiodic => Ok(Self::Aperiodic),
            ClockKind::Inherited => Ok(Self::Inherited),
            _ => Err(invalid_artifact("clock kind is newer than model wire v1")),
        }
    }

    pub(crate) fn decode(&self, id: Id<kinds::ClockDomain>) -> Result<ClockDomainDef, Diagnostic> {
        match self {
            Self::Continuous => Ok(ClockDomainDef::continuous(id)),
            Self::Periodic { period, phase } => {
                ClockDomainDef::periodic(id, period.decode()?, phase.decode()?)
                    .map_err(|error| invalid_artifact(error.message()))
            }
            Self::Aperiodic => Ok(ClockDomainDef::aperiodic(id)),
            Self::Inherited => Ok(ClockDomainDef::inherited(id)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireRationalTime {
    pub(crate) numerator: u64,
    pub(crate) denominator: u64,
}

impl WireRationalTime {
    pub(crate) const fn encode(value: RationalTime) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }

    pub(crate) fn decode(self) -> Result<RationalTime, Diagnostic> {
        let value = RationalTime::new(self.numerator, self.denominator)
            .map_err(|error| invalid_artifact(error.message()))?;
        if value.numerator() != self.numerator || value.denominator() != self.denominator {
            return Err(invalid_artifact(
                "wire rational time must already be in canonical reduced form",
            ));
        }
        Ok(value)
    }
}

pub(crate) const PURE_COMPONENT_CALCULUS_V1: &str = "eqiora.pure-component-calculus/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WirePureOperatorDefinition {
    pub(crate) digest: String,
    pub(crate) required_features: Vec<String>,
    pub(crate) formals: Vec<WirePureValueClass>,
    pub(crate) result: WirePureValueClass,
    pub(crate) nodes: Vec<WirePureCalculusNode>,
    pub(crate) root: u32,
}

impl WirePureOperatorDefinition {
    pub(crate) fn encode(definition: &PureOperatorDefinition) -> Self {
        Self {
            digest: definition.digest().to_string(),
            required_features: vec![PURE_COMPONENT_CALCULUS_V1.to_owned()],
            formals: definition
                .formals()
                .iter()
                .copied()
                .map(WirePureValueClass::encode)
                .collect(),
            result: WirePureValueClass::encode(definition.result_rule()),
            nodes: definition
                .nodes()
                .iter()
                .map(WirePureCalculusNode::encode)
                .collect(),
            root: definition.root().index(),
        }
    }

    pub(crate) fn validate_features(&self) -> Result<(), Diagnostic> {
        if self.required_features.is_empty() {
            return Err(invalid_artifact(
                "pure-operator definition is missing its required component-calculus feature",
            ));
        }
        if let Some(feature) = self
            .required_features
            .iter()
            .find(|feature| feature.as_str() != PURE_COMPONENT_CALCULUS_V1)
        {
            return Err(invalid_artifact(format!(
                "pure-operator definition requires unknown feature `{feature}`"
            )));
        }
        if self
            .required_features
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid_artifact(
                "pure-operator required features must be sorted and duplicate-free",
            ));
        }
        Ok(())
    }

    pub(crate) fn rebuild_and_validate_digest(&self) -> Result<PureOperatorDefinition, Diagnostic> {
        let formals = self
            .formals
            .iter()
            .copied()
            .map(WirePureValueClass::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.result.decode()?;
        let mut builder = CalculusBuilder::new(formals, result).map_err(|error| {
            invalid_artifact(format!("invalid pure-operator definition: {error}"))
        })?;
        let mut ids = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let id = node.decode(&mut builder, &ids)?;
            ids.push(id);
        }
        let root = calculus_operand(&ids, self.root)?;
        let definition = builder.finish(root).map_err(|error| {
            invalid_artifact(format!("invalid pure-operator definition: {error}"))
        })?;
        validate_operator_digest(&self.digest)?;
        let actual = definition.digest().to_string();
        if self.digest != actual {
            return Err(invalid_artifact(format!(
                "pure-operator definition digest mismatch: claimed {}, rebuilt {actual}",
                self.digest
            )));
        }
        Ok(definition)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WirePureValueClass {
    InvariantScalar,
    SpatialTensor { rank: u16 },
}

impl WirePureValueClass {
    pub(crate) const fn encode(value: PureValueClass) -> Self {
        match value.spatial_rank() {
            None => Self::InvariantScalar,
            Some(rank) => Self::SpatialTensor { rank },
        }
    }

    pub(crate) fn decode(self) -> Result<PureValueClass, Diagnostic> {
        match self {
            Self::InvariantScalar => Ok(PureValueClass::invariant_scalar()),
            Self::SpatialTensor { rank } => PureValueClass::spatial_tensor(rank)
                .map_err(|error| invalid_artifact(format!("invalid pure value class: {error}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WirePureCalculusNode {
    Rational { numerator: i64, denominator: u64 },
    FormalComponent { formal: u16, axes: Vec<u16> },
    KroneckerDelta { left_axis: u16, right_axis: u16 },
    Neg { value: u32 },
    Add { left: u32, right: u32 },
    Mul { left: u32, right: u32 },
}

impl WirePureCalculusNode {
    pub(crate) fn encode(node: &CalculusNode) -> Self {
        match node {
            CalculusNode::Rational(value) => Self::Rational {
                numerator: value.numerator(),
                denominator: value.denominator(),
            },
            CalculusNode::FormalComponent { formal, axes } => Self::FormalComponent {
                formal: *formal,
                axes: axes.iter().map(|axis| axis.index()).collect(),
            },
            CalculusNode::KroneckerDelta(left, right) => Self::KroneckerDelta {
                left_axis: left.index(),
                right_axis: right.index(),
            },
            CalculusNode::Neg(value) => Self::Neg {
                value: value.index(),
            },
            CalculusNode::Add(left, right) => Self::Add {
                left: left.index(),
                right: right.index(),
            },
            CalculusNode::Mul(left, right) => Self::Mul {
                left: left.index(),
                right: right.index(),
            },
        }
    }

    pub(crate) fn decode(
        &self,
        builder: &mut CalculusBuilder,
        ids: &[CalculusNodeId],
    ) -> Result<CalculusNodeId, Diagnostic> {
        let node = match self {
            Self::Rational {
                numerator,
                denominator,
            } => CalculusNode::Rational(
                ExactRational::from_canonical_parts(*numerator, *denominator).map_err(|error| {
                    invalid_artifact(format!("invalid canonical pure rational: {error}"))
                })?,
            ),
            Self::FormalComponent { formal, axes } => CalculusNode::FormalComponent {
                formal: *formal,
                axes: axes
                    .iter()
                    .copied()
                    .map(ResultAxis::new)
                    .collect::<Box<[_]>>(),
            },
            Self::KroneckerDelta {
                left_axis,
                right_axis,
            } => CalculusNode::KroneckerDelta(
                ResultAxis::new(*left_axis),
                ResultAxis::new(*right_axis),
            ),
            Self::Neg { value } => CalculusNode::Neg(calculus_operand(ids, *value)?),
            Self::Add { left, right } => CalculusNode::Add(
                calculus_operand(ids, *left)?,
                calculus_operand(ids, *right)?,
            ),
            Self::Mul { left, right } => CalculusNode::Mul(
                calculus_operand(ids, *left)?,
                calculus_operand(ids, *right)?,
            ),
        };
        builder
            .push(node)
            .map_err(|error| invalid_artifact(format!("invalid pure calculus node: {error}")))
    }
}

pub(crate) fn calculus_operand(
    ids: &[CalculusNodeId],
    index: u32,
) -> Result<CalculusNodeId, Diagnostic> {
    usize::try_from(index)
        .ok()
        .and_then(|index| ids.get(index))
        .copied()
        .ok_or_else(|| {
            invalid_artifact(format!(
                "pure calculus operand {index} is not topologically prior"
            ))
        })
}

pub(crate) fn validate_operator_digest(digest: &str) -> Result<(), Diagnostic> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_artifact(
            "pure-operator digest must be exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}
