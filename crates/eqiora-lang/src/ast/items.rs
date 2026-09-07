//! Source container item ownership.

use super::*;

/// Component signature requirements and private implementation declarations.
/// Parameter, Port, and property interface convergence is owned by its later slice.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ComponentItem {
    /// Private immutable static expression, expanded without a Kernel entity.
    Let(LetDecl),
    /// Scalar compile-time Parameter.
    Parameter(ComponentParameterDecl),
    /// Causal or conserving interface.
    Port(ComponentPortDecl),
    /// Field-physical Port family over one complete exterior.
    PortFamily(ComponentPortFamilyDecl),
    /// Required occurrence-bound spatial support.
    Support(SupportSlotDecl),
    /// Exact borrowed unknown requirement in the Component signature.
    FieldRequirement(FieldDecl),
    /// Exact nominal clock requirement in the Component signature.
    ClockRequirement(ClockRequirementDecl),
    /// Private mutable state.
    Field(FieldDecl),
    /// Simultaneous fresh initialization, owned by this occurrence.
    Initial(InitialDecl),
    /// Private exact periodic clock.
    Clock(ClockDecl),
    /// Private implicit residual group.
    Relation(RelationDecl),
    /// Private continuous Relation family over one complete exterior.
    RelationFamily(RelationFamilyDecl),
    /// Private local connection.
    Connection(ConnectionDecl),
    /// Conserving boundary-family connection, optionally pointwise-bound.
    BoundaryConnection(BoundaryConnectionDecl),
    /// Private nested component instance.
    Instance(InstanceDecl),
}

/// Model-level declaration.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Item {
    /// Continuous spatial region or one oriented boundary portion.
    Domain(DomainDecl),
    /// Mutable model state.
    Field(FieldDecl),
    /// Simultaneous fresh initialization, separate from numerical guesses.
    Initial(InitialDecl),
    /// Revision-local design value.
    Parameter(ParameterDecl),
    /// Typed compile-time expression alias expanded before Kernel lowering.
    Let(LetDecl),
    /// Causal or conserving interface.
    Port(PortDecl),
    /// Exact periodic clock.
    Clock(ClockDecl),
    /// Implicit residual group and activation.
    Relation(RelationDecl),
    /// Signal or conserving connection net.
    Connection(ConnectionDecl),
    /// Conserving connection containing exact boundary-member selectors.
    BoundaryConnection(BoundaryConnectionDecl),
    /// Public model Port list.
    Boundary(BoundaryDecl),
    /// Compile-time component instance.
    Instance(InstanceDecl),
}
