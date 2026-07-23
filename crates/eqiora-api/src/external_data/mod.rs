//! Application-owned replay of bounded external-data adapters.
//!
//! Format syntax remains isolated in L3. This module owns the L4 composition
//! with shared mesh, field, provenance, and artifact contracts.

#[cfg(feature = "vtu")]
mod vtu;
#[cfg(feature = "xdmf")]
mod xdmf;

#[cfg(feature = "vtu")]
pub use vtu::*;
#[cfg(feature = "xdmf")]
pub use xdmf::*;
