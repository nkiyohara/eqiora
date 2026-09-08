//! Typed definitions for the closed Semantic Kernel (RFC-0001).

mod boundary_physical;
mod definition;
mod expression;
mod finite_space;
pub use finite_space::FiniteSpaceDef;
mod index_set;
pub use index_set::IndexSetDef;
pub mod physical_closure;
pub mod pure_operator;
pub mod scalar_connection;
mod time;
pub mod typing;

pub use boundary_physical::{
    BoundaryPairing, BoundaryPhysicalConnectionViolation, BoundaryPhysicalConnector,
    BoundaryPhysicalPortContract, BoundaryPhysicalViolation, BoundaryQuantityRole,
    CartesianBoundaryEmbedding, CartesianPeriodicBoundaryIdentification,
    SpatialPeriodicBoundaryViolation, validate_boundary_physical_connection,
    validate_spatial_periodic_boundary_connection,
};
pub use definition::{
    ActivationDef, ActivationKind, AxisBounds, BoundarySide, CartesianAxisDefinition,
    CartesianCoordinateSource, ClockDomainDef, ClockKind, ConnectionDef, ConnectionSemantics,
    DomainDef, DomainKind, EventDirection, FieldDef, FieldRole, GeometryDigest, KernelNode,
    ParameterDef, PortDef, PortPayload, RelationDef, RepresentationDef, RepresentationKind,
    SignalDirection,
};
pub use expression::{
    ComparisonOp, ExprDag, ExprDagBuilder, ExprId, ExprNode, PureOperatorApplication, SymbolRef,
    UnaryMathFunction,
};
pub use time::RationalTime;
