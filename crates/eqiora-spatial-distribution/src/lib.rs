//! **eqiora-spatial-distribution** — exact mesh ownership and owner-routed assembly.
//!
//! This L2 seam composes accepted mesh topology, canonical local assembly, and
//! distributed algebra without moving any of those meanings into another
//! crate. Cell ownership is the sole spatial partition input. Lower entity
//! residency, assembly-row ownership, and solver-vector halos remain three
//! distinct derived contracts.

mod assembly;
mod mesh;

pub use assembly::{
    AdmittedRowOwnership, AdmittedRowOwnershipIdentityV1, AssemblyBoundDistributedLinearSystem,
    AssemblyRowOwnership, AssemblyRowRouteDescriptorV1, AssemblyRowRouteIdentityV1,
    AssemblyRowRouteV1, CollectiveRowOwnerCandidatesV1, DistributedAssemblyEvidence,
    DistributedAssemblyPlanIdentityV1, DistributedAssemblyReceiptIdentityV1,
    DistributedAssemblyReceiptV1, DistributedAssemblyRoutePlanV1,
    DistributedAssemblySystemIdentityV1, LOOPBACK_SPATIAL_ASSEMBLY_EXECUTION,
    LocalAssemblyProjection, LocalRouteAdmissionIdentityV1, LocalRouteAdmissionV1,
    LoopbackSpatialAssemblyBackend, OwnedRowAssemblyResult, reconstruct_distributed_assembly,
};

pub use mesh::{
    CellOwnershipClaim, DistributedMeshLayout, DistributedMeshLayoutIdentityV1, EntityExchange,
    MeshRevisionIdentityV1,
};
