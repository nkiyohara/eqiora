//! **eqiora-geometry** — exact geometry identity at the L2 boundary.
//!
//! The Semantic Kernel says *which* Domains exist. A geometry revision says
//! which exact geometric entities realize those Domains, and a mesh revision
//! supplies numerical entities. This crate closes only the pure, typed
//! correspondence between those three identities.
//!
//! It deliberately does not own a CAD kernel, source-file import, artifact
//! encoding, physics, transfer operators, or topology naming. In particular,
//! [`ParentOutward`] is geometric meaning derived relative to a parent body;
//! it is not a normal sign and is unrelated to
//! [`eqiora_meshing::OrientationCode`], which encodes a local permutation.

mod association;
mod cad;
mod correspondence;
mod identity;

pub use association::{
    BodyAssociationCandidate, RetainedBodyPair, RetainedBoundaryPair, RetainedGeometryAssociation,
    RetentionRejection,
};
pub use cad::{
    AxisAlignedBox3, CadAdapterIdentityV1, CadBoxDesignV1, CadBoxObservationV1,
    CadBoxRealizationV1, CadKernelAdapter, CadRepairDispositionV1, ConstrainedRectangleV1,
    StepLengthUnitV1, StepSourceDigest,
};
pub use correspondence::{
    CartesianBodyAssignment, CartesianBoundaryAssignment, GeometryCorrespondenceError,
    GeometryMeshCorrespondence,
};
pub use identity::{
    GeometryEntity, GeometryRevisionReference, GeometryRevisionTopology, ParentOutward,
};
