//! Quantities, dimensions and identifiers shared by every wire shape above.

use std::str::FromStr;

use crate::dimension::WireDimension;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DynQuantity, Entity, EntityKind, Id, RawId};
use eqiora_graph::EdgeKind;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::invalid_artifact;

use super::expression::*;

impl WireQuantity {
    pub(crate) fn encode(value: DynQuantity) -> Self {
        Self {
            value: value.value(),
            dimension: WireDimension::encode(value.dim()),
        }
    }

    pub(crate) fn decode(&self) -> Result<DynQuantity, Diagnostic> {
        if !self.value.is_finite() {
            return Err(invalid_artifact("wire quantity value must be finite"));
        }
        Ok(DynQuantity::new(self.value, self.dimension.decode()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireId {
    pub(crate) kind: WireEntityKind,
    pub(crate) ulid: String,
}

impl WireId {
    pub(crate) fn from_raw(value: RawId) -> Self {
        Self {
            kind: WireEntityKind::encode(value.kind()),
            ulid: value.ulid().to_string(),
        }
    }

    pub(crate) fn typed<E: Entity>(&self) -> Result<Id<E>, Diagnostic> {
        if self.kind != WireEntityKind::encode(E::KIND) {
            return Err(invalid_artifact(format!(
                "wire ID kind {:?} does not match expected {:?}",
                self.kind,
                E::KIND
            )));
        }
        Ok(Id::from_ulid(parse_ulid(&self.ulid)?))
    }

    pub(crate) fn decode_raw(&self) -> Result<RawId, Diagnostic> {
        macro_rules! typed {
            ($kind:ty) => {
                self.typed::<$kind>()?.erase()
            };
        }
        Ok(match self.kind {
            WireEntityKind::Domain => typed!(kinds::Domain),
            WireEntityKind::Representation => typed!(kinds::Representation),
            WireEntityKind::Field => typed!(kinds::Field),
            WireEntityKind::Parameter => typed!(kinds::Parameter),
            WireEntityKind::Port => typed!(kinds::Port),
            WireEntityKind::Relation => typed!(kinds::Relation),
            WireEntityKind::Activation => typed!(kinds::Activation),
            WireEntityKind::Connection => typed!(kinds::Connection),
            WireEntityKind::ClockDomain => typed!(kinds::ClockDomain),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireEntityKind {
    Domain,
    Representation,
    Field,
    Parameter,
    Port,
    Relation,
    Activation,
    Connection,
    ClockDomain,
}

impl WireEntityKind {
    pub(crate) fn encode(value: EntityKind) -> Self {
        match value {
            EntityKind::Domain => Self::Domain,
            EntityKind::Representation => Self::Representation,
            EntityKind::Field => Self::Field,
            EntityKind::Parameter => Self::Parameter,
            EntityKind::Port => Self::Port,
            EntityKind::Relation => Self::Relation,
            EntityKind::Activation => Self::Activation,
            EntityKind::Connection => Self::Connection,
            EntityKind::ClockDomain => Self::ClockDomain,
            _ => unreachable!("ModelEnvelope contains Semantic Kernel nodes only"),
        }
    }
}

pub(crate) fn parse_ulid(value: &str) -> Result<Ulid, Diagnostic> {
    Ulid::from_str(value)
        .map_err(|error| invalid_artifact(format!("invalid canonical ULID `{value}`: {error}")))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireEdge {
    pub(crate) from: WireId,
    pub(crate) to: WireId,
    pub(crate) kind: WireEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireEdgeKind {
    DefinedOn,
    AppliesOn,
    BoundaryOf,
    DependsOn,
    HasPort,
    Activates,
    Connects,
    ClockedBy,
}

impl WireEdgeKind {
    pub(crate) fn encode(value: EdgeKind) -> Result<Self, Diagnostic> {
        match value {
            EdgeKind::DefinedOn => Ok(Self::DefinedOn),
            EdgeKind::AppliesOn => Ok(Self::AppliesOn),
            EdgeKind::BoundaryOf => Ok(Self::BoundaryOf),
            EdgeKind::DependsOn => Ok(Self::DependsOn),
            EdgeKind::HasPort => Ok(Self::HasPort),
            EdgeKind::Activates => Ok(Self::Activates),
            EdgeKind::Connects => Ok(Self::Connects),
            EdgeKind::ClockedBy => Ok(Self::ClockedBy),
            _ => Err(invalid_artifact(
                "non-semantic edge cannot enter a Semantic Model envelope",
            )),
        }
    }

    pub(crate) const fn decode(self) -> EdgeKind {
        match self {
            Self::DefinedOn => EdgeKind::DefinedOn,
            Self::AppliesOn => EdgeKind::AppliesOn,
            Self::BoundaryOf => EdgeKind::BoundaryOf,
            Self::DependsOn => EdgeKind::DependsOn,
            Self::HasPort => EdgeKind::HasPort,
            Self::Activates => EdgeKind::Activates,
            Self::Connects => EdgeKind::Connects,
            Self::ClockedBy => EdgeKind::ClockedBy,
        }
    }
}
