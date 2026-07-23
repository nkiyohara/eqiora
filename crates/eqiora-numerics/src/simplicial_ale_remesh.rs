//! Conservative topology-changing transfer for the bounded 2D ALE FSI state.
//!
//! Remeshing is a zero-time numerical transition.  This module projects the
//! absolute material displacement first, derives the target harmonic geometry,
//! and only then transfers current-chart fluid fields together with the
//! reference-chart solid velocity.  Geometry overlap remains owned by
//! `eqiora-meshing`; this module supplies only finite-element meaning and
//! independently replayed numerical evidence.

mod contract;
mod integration;
mod projection;

pub use contract::{AcceptedAleFsiRemeshProjection2d, AleFsiRemeshProjectionEvidence2d};
pub use projection::project_simplicial_ale_fsi_remesh_2d;

fn invalid(message: impl Into<String>) -> eqiora_core::Diagnostic {
    eqiora_core::Diagnostic::error(
        eqiora_core::diagnostic::codes::INVALID_DISCRETIZATION,
        message,
    )
}

#[cfg(test)]
mod tests;
