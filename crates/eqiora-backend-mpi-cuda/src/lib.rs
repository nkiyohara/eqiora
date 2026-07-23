//! **eqiora-backend-mpi-cuda** — explicit host-staged composition of the MPI
//! distributed solver with one resident CUDA CSR action per rank.
//!
//! Neither parent adapter depends on the other. This L3 crate alone owns their
//! joint lifecycle and exposes no MPI handle, CUDA pointer, or fallback path.

#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "runtime")]
pub use runtime::*;

/// Exact Eqiora composition-adapter package version compiled into this binary.
pub const MPI_CUDA_ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");
