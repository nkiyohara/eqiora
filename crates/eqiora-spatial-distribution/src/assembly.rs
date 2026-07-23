mod backend;
mod codec;
mod ownership;
mod route;
mod shard;

pub use backend::{LOOPBACK_SPATIAL_ASSEMBLY_EXECUTION, LoopbackSpatialAssemblyBackend};
pub use ownership::{
    AdmittedRowOwnership, AdmittedRowOwnershipIdentityV1, AssemblyRowOwnership,
    CollectiveRowOwnerCandidatesV1, LocalAssemblyProjection,
};
pub use route::{
    AssemblyRowRouteDescriptorV1, AssemblyRowRouteIdentityV1, AssemblyRowRouteV1,
    DistributedAssemblyPlanIdentityV1, DistributedAssemblyRoutePlanV1,
    LocalRouteAdmissionIdentityV1, LocalRouteAdmissionV1,
};
pub use shard::{
    AssemblyBoundDistributedLinearSystem, DistributedAssemblyEvidence,
    DistributedAssemblyReceiptIdentityV1, DistributedAssemblyReceiptV1,
    DistributedAssemblySystemIdentityV1, OwnedRowAssemblyResult, reconstruct_distributed_assembly,
};

#[cfg(test)]
mod tests;
