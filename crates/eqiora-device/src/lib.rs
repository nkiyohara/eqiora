//! **eqiora-device** — backend-neutral device execution contracts.
//!
//! This L2 crate describes identity, capability, typed residency, ordered
//! submission, completion, transfer, and timing evidence. CUDA/ROCm handles,
//! pointers, streams, events, and errors remain private to L3 adapters.
//! Device completions are deliberately unrelated to Semantic Model events.

mod buffer;
mod evidence;
mod identity;
mod queue;
mod runtime;

pub use buffer::{
    BufferId, DeviceBuffer, DeviceBufferDescriptor, DeviceElement, DeviceElementType,
    HostBufferDescriptor, MemoryRegion, TransferDirection, TransferPlan,
};
pub use evidence::{DeviceExecutionTimings, TransferEvidence};
pub use identity::{DeviceId, RuntimeId};
pub use queue::{
    CommandQueue, Completion, Fence, QueueId, QueueSlot, QueueTimeline, SubmissionId,
    WaitedCompletion,
};
pub use runtime::{
    DeviceCapability, DeviceDescriptor, DeviceRuntime, SparseActionPolicy, SparseActionTolerance,
};
