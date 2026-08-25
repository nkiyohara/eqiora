//! **eqiora-geometry** — geometry identity, geometry-to-mesh correspondence,
//! and kernel-neutral CAD contracts at the L2 boundary.
//!
//! The Semantic Kernel says *which* Domains exist. A geometry revision says
//! which exact geometric entities realize those Domains, and a mesh revision
//! supplies numerical entities. This crate owns geometry revision identity,
//! the pure typed correspondence between those identities, and bounded CAD
//! design, observation, and adapter contracts that contain no kernel objects
//! or entity-enumeration indices.
//!
//! It deliberately does not own a CAD kernel, STEP parsing, concrete B-rep or
//! modeling operations, artifact encoding, physics, transfer operators, or
//! generic topology naming. The one public authored result-topology owner
//! projects only accepted construction lineage into opaque dimension-carrying
//! handles; its atomic naming seam remains crate-private. Concrete CAD
//! responsibilities remain isolated in adapters such as `eqiora-cad-truck`.
//! In particular, [`ParentOutward`] is
//! geometric meaning derived relative to a parent body; it is not a normal
//! sign and is unrelated to
//! [`eqiora_meshing::OrientationCode`], which encodes a local permutation.

mod association;
mod cad;
mod cad_authored_build;
mod cad_authored_cut;
mod cad_authored_face_mesh;
mod cad_authored_graph;
mod cad_authored_result_topology;
mod cad_authored_selection;
mod cad_authored_sketch;
mod cad_authored_swept_mesh;
mod canonical;
mod circular_hole;
mod construction_geometry_v2;
mod correspondence;
mod identity;
mod region;

pub use association::{
    BodyAssociationCandidate, RetainedBodyPair, RetainedBoundaryPair, RetainedGeometryAssociation,
    RetentionRejection,
};
pub use cad::{
    AxisAlignedBox3, CadAdapterIdentityV1, CadBoxDesignV1, CadBoxObservationV1,
    CadBoxRealizationV1, CadKernelAdapter, CadRepairDispositionV1, ConstrainedRectangleV1,
    StepLengthUnitV1, StepSourceDigest,
};
pub use cad_authored_build::CadAuthoredBuild;
pub use cad_authored_face_mesh::CadAuthoredFaceMesh;
pub use cad_authored_graph::CadAuthoredGraph;
pub use cad_authored_result_topology::{
    CadAuthoredResultEdgeHandle, CadAuthoredResultFaceHandle, CadAuthoredResultTopology,
    CadAuthoredResultTopologyHandle,
};
pub use cad_authored_selection::CadAuthoredFaceHandle;
pub use cad_authored_sketch::CadAuthoredSketch;
pub use cad_authored_swept_mesh::CadAuthoredSweptMesh;
pub use canonical::{CanonicalGeometryLimits, CanonicalGeometryRef, CanonicalGeometryV1};
pub use construction_geometry_v2::CanonicalGeometryV2;
pub use correspondence::{
    CartesianBodyAssignment, CartesianBoundaryAssignment, GeometryCorrespondenceError,
    GeometryMeshCorrespondence,
};
pub use identity::{
    GeometryEntity, GeometryRevisionReference, GeometryRevisionTopology, ParentOutward,
};
pub use region::{
    EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarFace, PlanarLoop, PlanarRegion,
    VERTEX_DIMENSION,
};
