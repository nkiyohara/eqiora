//! Backend-neutral local-contribution, assembly, and sparse algebra contracts.
//!
//! Spatial realizations produce anonymous local contributions. Reference,
//! threaded, distributed, and device assemblers consume this L2 vocabulary
//! without acquiring model or physics semantics.

mod action;
mod execution;
mod local;
mod projection;
mod sparse;

pub use action::{PacketLinearOperator, PacketLinearSystem};
pub use execution::{
    AssemblyAccumulator, AssemblyBackend, AssemblyPacket, AssemblyPacketSetIdentityV1,
    AssemblyPlan, AssemblyReport, AssemblyResult, AssemblyTarget, AssemblyTargetId, AssemblyWork,
    IndexedAssemblyWork, REFERENCE_ASSEMBLY_BACKEND, ReferenceAssemblyBackend, TargetAssemblyDelta,
    TargetAssemblyMap,
};
pub use local::LocalContribution;
pub use projection::{AssemblyDelta, AssemblyRowDelta};
pub use sparse::{AssemblyMap, CooAssembler, CsrMatrix, DofId, LinearSystem, LocalUnknown};
