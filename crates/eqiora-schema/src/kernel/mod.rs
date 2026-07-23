//! Typed definitions for the closed Semantic Kernel (RFC-0001).

mod boundary_physical;
mod definition;
mod expression;
pub mod physical_closure;
pub mod pure_operator;
pub mod scalar_connection;
mod time;
pub mod typing;
mod value;

pub use boundary_physical::{
    BoundaryPairing, BoundaryPhysicalConnectionViolation, BoundaryPhysicalConnector,
    BoundaryPhysicalPortContract, BoundaryPhysicalViolation, BoundaryQuantityRole,
    CartesianBoundaryEmbedding, CartesianPeriodicBoundaryIdentification,
    SpatialPeriodicBoundaryViolation, validate_boundary_physical_connection,
    validate_spatial_periodic_boundary_connection,
};
pub use definition::{
    ActivationDef, ActivationKind, AxisBounds, BoundarySide, ClockDomainDef, ClockKind,
    ConnectionDef, ConnectionSemantics, DomainDef, DomainKind, EventDirection, FieldDef,
    KernelNode, ParameterDef, PortDef, PortPayload, RelationDef, RepresentationDef,
    RepresentationKind, SignalDirection,
};
pub use expression::{
    ExprDag, ExprDagBuilder, ExprId, ExprNode, PureOperatorApplication, SymbolRef,
    UnaryMathFunction,
};
pub use time::RationalTime;
pub use value::ValueFrame;
