//! One exact in-memory prescribed-displacement dynamic-solid reference step.

mod acceptance;
mod assembly;
mod contract;

use eqiora_artifact::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
};
use eqiora_assembly::AssemblyBackend;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DynQuantity, Id};
use eqiora_meshing::VertexId;
use eqiora_solver::LinearSolverBackend;

pub use acceptance::AcceptedPrescribedDynamicSolidStep3d;
use acceptance::solve_and_accept;
use assembly::assemble_physical_operators;
use contract::{PrescribedDynamicSolidContract, invalid};

/// Immutable reference context for one exact serial-host 3D dynamic-solid step.
///
/// The context owns its fixed numerical policy and generation counter. It
/// publishes no durable State or Run artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidReference3d {
    contract: PrescribedDynamicSolidContract,
    accepted_generation: u64,
}

impl PrescribedDynamicSolidReference3d {
    /// Bind the exact canonical Model, artifact lineage, reference mesh,
    /// prior fields, time step, and live driven boundary.
    ///
    /// # Errors
    /// Rejects semantic, artifact, topology, boundary, time, or field drift.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &impl ReplayableCanonicalModelArtifact,
        geometry: &GeometryIdentityEnvelopeV1,
        mesh: &SimplicialMeshEnvelopeV1,
        correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
        time_step: DynQuantity,
        prior_displacement: &[(VertexId, [f64; 3])],
        prior_velocity: &[(VertexId, [f64; 3])],
        driven_boundary: Id<kinds::Domain>,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            contract: PrescribedDynamicSolidContract::new(
                model,
                geometry,
                mesh,
                correspondence,
                time_step,
                prior_displacement,
                prior_velocity,
                driven_boundary,
            )?,
            accepted_generation: 0,
        })
    }

    /// Latest successfully accepted generation.
    #[must_use]
    pub const fn accepted_generation(&self) -> u64 {
        self.accepted_generation
    }

    /// Canonically ordered complete driven-boundary vertex inventory.
    #[must_use]
    pub fn driven_vertices(&self) -> &[VertexId] {
        self.contract.driven_vertices()
    }

    /// Project prior displacement and velocity on the exact driven surface.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn project_driven_surface(
        &self,
    ) -> (u64, Vec<(VertexId, [f64; 3])>, Vec<(VertexId, [f64; 3])>) {
        let displacement = self
            .contract
            .driven_vertices()
            .iter()
            .map(|vertex| self.contract.prior_displacement()[vertex.index()])
            .collect();
        let velocity = self
            .contract
            .driven_vertices()
            .iter()
            .map(|vertex| self.contract.prior_velocity()[vertex.index()])
            .collect();
        (self.accepted_generation, displacement, velocity)
    }

    /// Validate, assemble, solve, and atomically accept one driven total
    /// displacement candidate.
    ///
    /// Candidate values are total displacement at the next time, never an
    /// increment or velocity. The driven velocity is derived inside this
    /// boundary. Any error leaves [`Self::accepted_generation`] unchanged.
    ///
    /// # Errors
    /// Rejects stale/invalid candidates, backend failure, solve failure, or
    /// post-solve physical-residual failure.
    pub fn accept_candidate(
        &mut self,
        generation: u64,
        driven_total_displacement: &[(VertexId, [f64; 3])],
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<AcceptedPrescribedDynamicSolidStep3d, Diagnostic> {
        if generation != self.accepted_generation {
            return Err(invalid(format!(
                "prescribed dynamic-solid candidate generation {generation} differs from accepted generation {}",
                self.accepted_generation
            )));
        }
        self.contract
            .validate_candidate(driven_total_displacement)?;
        let assembled = assemble_physical_operators(&self.contract, assembly)?;
        let accepted = solve_and_accept(
            &self.contract,
            generation,
            driven_total_displacement,
            assembled,
            solver,
        )?;
        self.accepted_generation = accepted.generation();
        Ok(accepted)
    }
}
