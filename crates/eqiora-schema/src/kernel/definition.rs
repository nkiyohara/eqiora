//! Typed definitions for the nine Semantic Kernel node kinds.

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{
    Diagnostic, DimExponents, DynQuantity, EntityKind, GraphPath, Id, RawId, ValueShape,
};

use super::{BoundaryPhysicalConnector, ExprDag, RationalTime, ValueFrame};

/// One finite Cartesian coordinate interval in coherent SI length units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisBounds {
    lower: DynQuantity,
    upper: DynQuantity,
}

impl AxisBounds {
    /// Construct finite, increasing length bounds.
    ///
    /// # Errors
    /// Returns `EQ0302` when either value is not a length, is non-finite, or
    /// does not form an increasing interval.
    pub fn new(lower: DynQuantity, upper: DynQuantity) -> Result<Self, Diagnostic> {
        let length = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        if lower.dim() != length || upper.dim() != length {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian axis bounds must have physical dimension length",
            ));
        }
        if !lower.value().is_finite()
            || !upper.value().is_finite()
            || upper.value() <= lower.value()
        {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian axis bounds must be finite and strictly increasing",
            ));
        }
        Ok(Self { lower, upper })
    }

    /// Lower coordinate in coherent SI units.
    #[must_use]
    pub const fn lower(self) -> DynQuantity {
        self.lower
    }

    /// Upper coordinate in coherent SI units.
    #[must_use]
    pub const fn upper(self) -> DynQuantity {
        self.upper
    }
}

/// Outward side of one Cartesian coordinate axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoundarySide {
    /// Side at the lower coordinate bound.
    Lower,
    /// Side at the upper coordinate bound.
    Upper,
}

/// Canonical continuous-domain shape, independent of any mesh.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DomainKind {
    /// Identity-only domain retained for non-spatial and schema-defined uses.
    Abstract,
    /// Runtime-dimensional Cartesian box in physical space.
    CartesianBox { bounds: Vec<AxisBounds> },
    /// One oriented side of a parent Cartesian box. The parent is supplied by
    /// exactly one `BoundaryOf` graph edge.
    CartesianBoundary { axis: usize, side: BoundarySide },
    /// One nominal scalar conserving domain. The Domain ID is part of the
    /// physical type; dimensions alone never make two domains compatible.
    ScalarPhysical {
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    },
    /// One nominal field-valued boundary connector. The Domain ID plus closed
    /// trace/flux role is the exact quantity identity.
    BoundaryPhysical {
        connector: BoundaryPhysicalConnector,
    },
}

/// Domain definition. Continuous geometry is model meaning; meshes and
/// geometry maps remain realization concerns.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainDef {
    id: Id<kinds::Domain>,
    kind: DomainKind,
}

impl DomainDef {
    /// Construct an abstract domain for schema-defined or non-spatial use.
    #[must_use]
    pub const fn new(id: Id<kinds::Domain>) -> Self {
        Self {
            id,
            kind: DomainKind::Abstract,
        }
    }

    /// Construct a non-empty Cartesian box of arbitrary runtime dimension.
    ///
    /// # Errors
    /// Returns `EQ0302` for an empty axis set.
    pub fn cartesian_box(
        id: Id<kinds::Domain>,
        bounds: Vec<AxisBounds>,
    ) -> Result<Self, Diagnostic> {
        if bounds.is_empty() {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Cartesian Domain requires at least one coordinate axis",
            )
            .with_graph_path(kernel_path(id.erase())));
        }
        Ok(Self {
            id,
            kind: DomainKind::CartesianBox { bounds },
        })
    }

    /// Construct one oriented boundary selector. Whole-model validation
    /// checks the axis against its unique `BoundaryOf` parent.
    #[must_use]
    pub const fn cartesian_boundary(
        id: Id<kinds::Domain>,
        axis: usize,
        side: BoundarySide,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::CartesianBoundary { axis, side },
        }
    }

    /// Construct a nominal scalar conserving domain.
    #[must_use]
    pub const fn scalar_physical(
        id: Id<kinds::Domain>,
        across_dimension: DimExponents,
        through_dimension: DimExponents,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::ScalarPhysical {
                across_dimension,
                through_dimension,
            },
        }
    }

    /// Construct a nominal field-valued boundary connector Domain.
    #[must_use]
    pub const fn boundary_physical(
        id: Id<kinds::Domain>,
        connector: BoundaryPhysicalConnector,
    ) -> Self {
        Self {
            id,
            kind: DomainKind::BoundaryPhysical { connector },
        }
    }

    /// Typed node ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Domain> {
        self.id
    }

    /// Canonical domain kind.
    #[must_use]
    pub const fn kind(&self) -> &DomainKind {
        &self.kind
    }
}

/// Canonical field representation before a discrete space is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RepresentationKind {
    /// Identity-only representation retained for schema-defined uses.
    Abstract,
    /// A field over a continuous domain.
    Continuum,
}

/// Representation definition. Basis family, mesh, and DOF layout belong to
/// the Realization Graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDef {
    id: Id<kinds::Representation>,
    kind: RepresentationKind,
}

impl RepresentationDef {
    /// Construct an abstract representation.
    #[must_use]
    pub const fn new(id: Id<kinds::Representation>) -> Self {
        Self {
            id,
            kind: RepresentationKind::Abstract,
        }
    }

    /// Construct a canonical continuum representation.
    #[must_use]
    pub const fn continuum(id: Id<kinds::Representation>) -> Self {
        Self {
            id,
            kind: RepresentationKind::Continuum,
        }
    }

    /// Typed node ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Representation> {
        self.id
    }

    /// Canonical representation kind.
    #[must_use]
    pub const fn kind(&self) -> RepresentationKind {
        self.kind
    }
}

/// Exact mathematical Field definition before realization.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    id: Id<kinds::Field>,
    dimension: DimExponents,
    shape: ValueShape,
    frame: ValueFrame,
    initial: Option<DynQuantity>,
}

impl FieldDef {
    /// Define a scalar Field with no initial value.
    #[must_use]
    pub fn new(id: Id<kinds::Field>, dimension: DimExponents) -> Self {
        Self {
            id,
            dimension,
            shape: ValueShape::scalar(),
            frame: ValueFrame::Invariant,
            initial: None,
        }
    }

    /// Define a shaped Field. Spatial support compatibility is validated by
    /// the whole-model validator once its exact Domain is known.
    ///
    /// # Errors
    /// Returns `EQ0302` for an unrepresentable component product or a scalar
    /// carrying a non-invariant component frame.
    pub fn shaped(
        id: Id<kinds::Field>,
        dimension: DimExponents,
        shape: ValueShape,
        frame: ValueFrame,
    ) -> Result<Self, Diagnostic> {
        if shape.component_count().is_none()
            || (shape.is_scalar() && frame != ValueFrame::Invariant)
        {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Field shape/frame contract is not representable",
            )
            .with_graph_path(kernel_path(id.erase())));
        }
        Ok(Self {
            id,
            dimension,
            shape,
            frame,
            initial: None,
        })
    }

    /// Attach a dimension-checked initial value.
    ///
    /// # Errors
    /// Returns `EQ0401` when the value dimension differs from the Field.
    pub fn with_initial(mut self, initial: DynQuantity) -> Result<Self, Diagnostic> {
        if !self.shape.is_scalar() || self.frame != ValueFrame::Invariant {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "non-scalar Field initialization requires a future shaped-value contract",
            )
            .with_graph_path(kernel_path(self.id.erase())));
        }
        if initial.dim() != self.dimension {
            return Err(Diagnostic::error(
                codes::DIMENSION_MISMATCH,
                format!(
                    "Field initial dimension [{}] differs from declared [{}]",
                    initial.dim(),
                    self.dimension
                ),
            )
            .with_graph_path(kernel_path(self.id.erase())));
        }
        self.initial = Some(initial);
        Ok(self)
    }

    /// Typed Field ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Field> {
        self.id
    }

    /// Declared physical dimension.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Exact mathematical value shape.
    #[must_use]
    pub const fn shape(&self) -> &ValueShape {
        &self.shape
    }

    /// Coordinate-frame meaning of Field components.
    #[must_use]
    pub const fn frame(&self) -> ValueFrame {
        self.frame
    }

    /// Initial value when supplied by the model.
    #[must_use]
    pub const fn initial(&self) -> Option<DynQuantity> {
        self.initial
    }
}

/// Scalar Parameter definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDef {
    id: Id<kinds::Parameter>,
    value: DynQuantity,
}

impl ParameterDef {
    /// Define a Parameter and its dimensioned value.
    #[must_use]
    pub const fn new(id: Id<kinds::Parameter>, value: DynQuantity) -> Self {
        Self { id, value }
    }

    /// Typed Parameter ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Parameter> {
        self.id
    }

    /// Model value.
    #[must_use]
    pub const fn value(&self) -> DynQuantity {
        self.value
    }
}

/// Causal direction of a signal Port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalDirection {
    /// Value enters the owning relation network.
    Input,
    /// Value leaves the owning relation network.
    Output,
}

/// Closed kernel-level Port payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PortPayload {
    /// Causal signal with an explicit direction and scalar dimension.
    Signal {
        direction: SignalDirection,
        dimension: DimExponents,
    },
    /// Structural-only v1 conserving marker. It has no executable
    /// across/through interpretation.
    ConservingMarker { dimension: DimExponents },
    /// Scalar conserving connector typed nominally by one physical Domain.
    ScalarPhysical { domain: Id<kinds::Domain> },
    /// Field-valued boundary Port. Parent support and outward orientation are
    /// derived from the boundary's unique `BoundaryOf` edge.
    BoundaryPhysical {
        connector: Id<kinds::Domain>,
        boundary: Id<kinds::Domain>,
    },
}

/// Typed scalar Port definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDef {
    id: Id<kinds::Port>,
    payload: PortPayload,
}

impl PortDef {
    /// Define a causal signal Port.
    #[must_use]
    pub const fn signal(
        id: Id<kinds::Port>,
        direction: SignalDirection,
        dimension: DimExponents,
    ) -> Self {
        Self {
            id,
            payload: PortPayload::Signal {
                direction,
                dimension,
            },
        }
    }

    /// Preserve one structural-only v1 conserving Port marker.
    #[must_use]
    pub const fn conserving_marker(id: Id<kinds::Port>, dimension: DimExponents) -> Self {
        Self {
            id,
            payload: PortPayload::ConservingMarker { dimension },
        }
    }

    /// Define a scalar conserving Port with nominal Domain identity.
    #[must_use]
    pub const fn scalar_physical(id: Id<kinds::Port>, domain: Id<kinds::Domain>) -> Self {
        Self {
            id,
            payload: PortPayload::ScalarPhysical { domain },
        }
    }

    /// Define one field-valued physical Port on an exact boundary Domain.
    #[must_use]
    pub const fn boundary_physical(
        id: Id<kinds::Port>,
        connector: Id<kinds::Domain>,
        boundary: Id<kinds::Domain>,
    ) -> Self {
        Self {
            id,
            payload: PortPayload::BoundaryPhysical {
                connector,
                boundary,
            },
        }
    }

    /// Typed Port ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Port> {
        self.id
    }

    /// Closed Port payload.
    #[must_use]
    pub const fn payload(&self) -> PortPayload {
        self.payload
    }

    /// Signal direction and scalar dimension, if this is a signal Port.
    #[must_use]
    pub const fn signal_contract(&self) -> Option<(SignalDirection, DimExponents)> {
        match self.payload {
            PortPayload::Signal {
                direction,
                dimension,
            } => Some((direction, dimension)),
            PortPayload::ConservingMarker { .. }
            | PortPayload::ScalarPhysical { .. }
            | PortPayload::BoundaryPhysical { .. } => None,
        }
    }

    /// Structural marker dimension, if this is a v1 conserving marker.
    #[must_use]
    pub const fn marker_dimension(&self) -> Option<DimExponents> {
        match self.payload {
            PortPayload::ConservingMarker { dimension } => Some(dimension),
            PortPayload::Signal { .. }
            | PortPayload::ScalarPhysical { .. }
            | PortPayload::BoundaryPhysical { .. } => None,
        }
    }

    /// Nominal physical Domain, if this is a scalar physical Port.
    #[must_use]
    pub const fn physical_domain(&self) -> Option<Id<kinds::Domain>> {
        match self.payload {
            PortPayload::ScalarPhysical { domain } => Some(domain),
            PortPayload::Signal { .. }
            | PortPayload::ConservingMarker { .. }
            | PortPayload::BoundaryPhysical { .. } => None,
        }
    }

    /// Exact nominal connector and boundary support for a field-valued Port.
    #[must_use]
    pub const fn boundary_physical_contract(
        &self,
    ) -> Option<(Id<kinds::Domain>, Id<kinds::Domain>)> {
        match self.payload {
            PortPayload::BoundaryPhysical {
                connector,
                boundary,
            } => Some((connector, boundary)),
            PortPayload::Signal { .. }
            | PortPayload::ConservingMarker { .. }
            | PortPayload::ScalarPhysical { .. } => None,
        }
    }
}

/// Implicit residual Relation definition.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationDef {
    id: Id<kinds::Relation>,
    residuals: ExprDag,
}

impl RelationDef {
    /// Define one or more residual equations represented by an expression DAG.
    #[must_use]
    pub const fn new(id: Id<kinds::Relation>, residuals: ExprDag) -> Self {
        Self { id, residuals }
    }

    /// Typed Relation ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Relation> {
        self.id
    }

    /// Residual DAG; every root denotes `root = 0`.
    #[must_use]
    pub const fn residuals(&self) -> &ExprDag {
        &self.residuals
    }
}

/// Event zero-crossing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventDirection {
    /// Either crossing direction.
    Any,
    /// Negative to positive.
    Rising,
    /// Positive to negative.
    Falling,
}

/// Activation semantics independent of execution scheduling.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ActivationKind {
    /// Relation is active throughout continuous model time.
    Continuous,
    /// Relation activates at ticks of its `ClockedBy` ClockDomain edge.
    Periodic,
    /// Relation activates on a zero crossing of the guard expression.
    Event {
        /// Scalar guard expression; exactly one root is required.
        guard: ExprDag,
        /// Crossing direction.
        direction: EventDirection,
    },
    /// Relation is active while a scalar guard is positive.
    Guard {
        /// Scalar guard expression; exactly one root is required.
        guard: ExprDag,
    },
}

/// Activation node definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivationDef {
    id: Id<kinds::Activation>,
    kind: ActivationKind,
}

impl ActivationDef {
    /// Define and locally validate an Activation.
    ///
    /// # Errors
    /// Returns `EQ0302` when an event/guard has other than one expression root.
    pub fn new(id: Id<kinds::Activation>, kind: ActivationKind) -> Result<Self, Diagnostic> {
        let guard = match &kind {
            ActivationKind::Event { guard, .. } | ActivationKind::Guard { guard } => Some(guard),
            ActivationKind::Continuous | ActivationKind::Periodic => None,
        };
        if guard.is_some_and(|expression| expression.roots().len() != 1) {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "event and guard Activations require exactly one guard root",
            )
            .with_graph_path(kernel_path(id.erase())));
        }
        Ok(Self { id, kind })
    }

    /// Continuous Activation convenience constructor.
    #[must_use]
    pub fn continuous(id: Id<kinds::Activation>) -> Self {
        Self {
            id,
            kind: ActivationKind::Continuous,
        }
    }

    /// Periodic Activation convenience constructor.
    #[must_use]
    pub fn periodic(id: Id<kinds::Activation>) -> Self {
        Self {
            id,
            kind: ActivationKind::Periodic,
        }
    }

    /// Typed Activation ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Activation> {
        self.id
    }

    /// Activation semantics.
    #[must_use]
    pub const fn kind(&self) -> &ActivationKind {
        &self.kind
    }
}

/// Connection semantics. Runtime transport policy belongs to Realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ConnectionSemantics {
    /// Causal signal; a discrete source is held between activation instants.
    Signal,
    /// Acausal connection enforcing equality and conservation laws.
    Conserving,
    /// Field-valued conserving pair identified by a derived Cartesian
    /// lower-to-upper translation.
    SpatialPeriodic,
}

/// Connection node definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDef {
    id: Id<kinds::Connection>,
    semantics: ConnectionSemantics,
}

impl ConnectionDef {
    /// Define a Connection.
    #[must_use]
    pub const fn new(id: Id<kinds::Connection>, semantics: ConnectionSemantics) -> Self {
        Self { id, semantics }
    }

    /// Typed Connection ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Connection> {
        self.id
    }

    /// Closed signal, coincident-conserving, or spatial-periodic semantics.
    #[must_use]
    pub const fn semantics(&self) -> ConnectionSemantics {
        self.semantics
    }
}

/// Exact model-time definition of a ClockDomain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClockKind {
    /// Continuous model time.
    Continuous,
    /// Exact periodic model time.
    Periodic {
        /// Strictly positive period.
        period: RationalTime,
        /// Non-negative phase relative to model time zero.
        phase: RationalTime,
    },
    /// Tick times arrive explicitly as semantic events.
    Aperiodic,
    /// Clock is inferred from connected signal semantics.
    Inherited,
}

/// ClockDomain node definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockDomainDef {
    id: Id<kinds::ClockDomain>,
    kind: ClockKind,
}

impl ClockDomainDef {
    /// Continuous model-time ClockDomain.
    #[must_use]
    pub const fn continuous(id: Id<kinds::ClockDomain>) -> Self {
        Self {
            id,
            kind: ClockKind::Continuous,
        }
    }

    /// Periodic model-time ClockDomain.
    ///
    /// # Errors
    /// Returns `EQ0305` when `period` is zero.
    pub fn periodic(
        id: Id<kinds::ClockDomain>,
        period: RationalTime,
        phase: RationalTime,
    ) -> Result<Self, Diagnostic> {
        if period.is_zero() {
            return Err(Diagnostic::error(
                codes::INVALID_CLOCK,
                "periodic ClockDomain requires a strictly positive period",
            )
            .with_graph_path(kernel_path(id.erase())));
        }
        Ok(Self {
            id,
            kind: ClockKind::Periodic { period, phase },
        })
    }

    /// Aperiodic model-time ClockDomain.
    #[must_use]
    pub const fn aperiodic(id: Id<kinds::ClockDomain>) -> Self {
        Self {
            id,
            kind: ClockKind::Aperiodic,
        }
    }

    /// Inherited ClockDomain.
    #[must_use]
    pub const fn inherited(id: Id<kinds::ClockDomain>) -> Self {
        Self {
            id,
            kind: ClockKind::Inherited,
        }
    }

    /// Typed ClockDomain ID.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::ClockDomain> {
        self.id
    }

    /// Exact model-time semantics.
    #[must_use]
    pub const fn kind(&self) -> ClockKind {
        self.kind
    }
}

/// Type-erased storage form of a complete Semantic Kernel node.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum KernelNode {
    /// Domain definition.
    Domain(DomainDef),
    /// Representation definition.
    Representation(RepresentationDef),
    /// Field definition.
    Field(FieldDef),
    /// Parameter definition.
    Parameter(ParameterDef),
    /// Port definition.
    Port(PortDef),
    /// Relation definition.
    Relation(RelationDef),
    /// Activation definition.
    Activation(ActivationDef),
    /// Connection definition.
    Connection(ConnectionDef),
    /// ClockDomain definition.
    ClockDomain(ClockDomainDef),
}

impl KernelNode {
    /// Erased graph ID, derived from the variant's typed ID.
    #[must_use]
    pub fn id(&self) -> RawId {
        match self {
            Self::Domain(value) => value.id().erase(),
            Self::Representation(value) => value.id().erase(),
            Self::Field(value) => value.id().erase(),
            Self::Parameter(value) => value.id().erase(),
            Self::Port(value) => value.id().erase(),
            Self::Relation(value) => value.id().erase(),
            Self::Activation(value) => value.id().erase(),
            Self::Connection(value) => value.id().erase(),
            Self::ClockDomain(value) => value.id().erase(),
        }
    }

    /// Closed Semantic Kernel entity kind.
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        match self {
            Self::Domain(_) => EntityKind::Domain,
            Self::Representation(_) => EntityKind::Representation,
            Self::Field(_) => EntityKind::Field,
            Self::Parameter(_) => EntityKind::Parameter,
            Self::Port(_) => EntityKind::Port,
            Self::Relation(_) => EntityKind::Relation,
            Self::Activation(_) => EntityKind::Activation,
            Self::Connection(_) => EntityKind::Connection,
            Self::ClockDomain(_) => EntityKind::ClockDomain,
        }
    }

    /// Declared scalar dimension for values addressable by `SetValue`.
    #[must_use]
    pub const fn value_dimension(&self) -> Option<DimExponents> {
        match self {
            Self::Field(value) if value.shape().is_scalar() => Some(value.dimension()),
            Self::Field(_) => None,
            Self::Parameter(value) => Some(value.value().dim()),
            _ => None,
        }
    }

    /// Model value installed when the node is first defined.
    #[must_use]
    pub const fn initial_value(&self) -> Option<DynQuantity> {
        match self {
            Self::Field(value) => value.initial(),
            Self::Parameter(value) => Some(value.value()),
            _ => None,
        }
    }
}

macro_rules! kernel_from {
    ($definition:ident, $variant:ident) => {
        impl From<$definition> for KernelNode {
            fn from(value: $definition) -> Self {
                Self::$variant(value)
            }
        }
    };
}

kernel_from!(DomainDef, Domain);
kernel_from!(RepresentationDef, Representation);
kernel_from!(FieldDef, Field);
kernel_from!(ParameterDef, Parameter);
kernel_from!(PortDef, Port);
kernel_from!(RelationDef, Relation);
kernel_from!(ActivationDef, Activation);
kernel_from!(ConnectionDef, Connection);
kernel_from!(ClockDomainDef, ClockDomain);

fn kernel_path(id: RawId) -> GraphPath {
    GraphPath::new([
        "semantic".to_owned(),
        format!("{:?}", id.kind()),
        id.to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::Dimension;
    use eqiora_core::quantity::dim;

    #[test]
    fn field_initial_value_is_dimension_checked() {
        let field = Id::<kinds::Field>::new();
        let diagnostic = FieldDef::new(field, dim::TemperatureDim::EXPONENTS)
            .with_initial(DynQuantity::new(2.0, dim::TimeDim::EXPONENTS))
            .expect_err("time is not temperature");

        assert_eq!(diagnostic.code(), codes::DIMENSION_MISMATCH);
    }

    #[test]
    fn periodic_clock_rejects_zero_period() {
        let clock = Id::<kinds::ClockDomain>::new();
        let diagnostic = ClockDomainDef::periodic(clock, RationalTime::ZERO, RationalTime::ZERO)
            .expect_err("zero period never advances");

        assert_eq!(diagnostic.code(), codes::INVALID_CLOCK);
    }

    #[test]
    fn cartesian_bounds_require_increasing_lengths() {
        let length = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        assert!(
            AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(2.0, length)).is_ok()
        );
        assert_eq!(
            AxisBounds::new(DynQuantity::new(2.0, length), DynQuantity::new(0.0, length))
                .unwrap_err()
                .code(),
            codes::INVALID_KERNEL_DEFINITION
        );
        assert_eq!(
            AxisBounds::new(
                DynQuantity::new(0.0, DimExponents::DIMENSIONLESS),
                DynQuantity::new(2.0, DimExponents::DIMENSIONLESS)
            )
            .unwrap_err()
            .code(),
            codes::INVALID_KERNEL_DEFINITION
        );
    }
}
