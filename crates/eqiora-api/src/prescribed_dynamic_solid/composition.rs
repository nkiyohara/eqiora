//! Private construction shared by the direct and connected-provider owners.

use super::*;

/// Exact pre-solve resources and prior State, built before provider Field exchange.
pub(super) struct PreparedPrescribedDynamicSolid3d {
    pub(super) model: ModelEnvelope,
    pub(super) geometry: GeometryIdentityEnvelopeV1,
    pub(super) correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    pub(super) mesh: SimplicialMeshEnvelopeV1,
    pub(super) realization: PrescribedDynamicSolidRealizationEnvelopeV1,
    pub(super) prior_displacement: Vec<(VertexId, [f64; 3])>,
    pub(super) prior_velocity: Vec<(VertexId, [f64; 3])>,
    pub(super) prior_displacement_block: DiscreteFieldEnvelopeV1,
    pub(super) prior_velocity_block: DiscreteFieldEnvelopeV1,
    pub(super) prior_displacement_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) prior_velocity_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) prior_state: SpatialStateEnvelopeV1,
}

impl PreparedPrescribedDynamicSolid3d {
    pub(super) fn new(document: &ModelDocument) -> Result<Self, Diagnostic> {
        let model = ModelEnvelope::from_program(document.program())?;
        let canonical = lower_isotropic_elastodynamics_cartesian_3d(document.program())?;
        let body = canonical.domain().downcast().ok_or_else(|| {
            invalid("prescribed dynamic-solid body does not retain a typed Domain identity")
        })?;
        let geometry = GeometryIdentityEnvelopeV1::new(&model, [body], 1.0e-12)?;
        let mesh = exact_mesh()?;
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh)?;
        let prior_displacement = VERTICES
            .iter()
            .enumerate()
            .map(|(index, coordinates)| (VertexId::new(index), [coordinates[0] / 100.0, 0.0, 0.0]))
            .collect::<Vec<_>>();
        let prior_velocity = VERTICES
            .iter()
            .enumerate()
            .map(|(index, coordinates)| (VertexId::new(index), [coordinates[0] / 50.0, 0.0, 0.0]))
            .collect::<Vec<_>>();
        let realization = PrescribedDynamicSolidRealizationEnvelopeV1::new(
            &model,
            &geometry,
            &correspondence,
            &mesh,
            RealizationRevision::new(1),
            &exact_candidate(),
        )?;
        let prior_displacement_block = vector_block(&mesh, &prior_displacement)?;
        let prior_velocity_block = vector_block(&mesh, &prior_velocity)?;
        let prior_displacement_snapshot = FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.displacement_field(),
            std::slice::from_ref(&prior_displacement_block),
        )?;
        let prior_velocity_snapshot = FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.velocity_field(),
            std::slice::from_ref(&prior_velocity_block),
        )?;
        let prior_state = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            0,
            0.0,
            &[
                prior_displacement_snapshot.clone(),
                prior_velocity_snapshot.clone(),
            ],
        )?;
        Ok(Self {
            model,
            geometry,
            correspondence,
            mesh,
            realization,
            prior_displacement,
            prior_velocity,
            prior_displacement_block,
            prior_velocity_block,
            prior_displacement_snapshot,
            prior_velocity_snapshot,
            prior_state,
        })
    }

    pub(super) fn accept(
        &self,
        document: &ModelDocument,
        candidate: &[(VertexId, [f64; 3])],
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<PrescribedDynamicSolidStateRun3d, Diagnostic> {
        let accepted = self.solve_candidate(document, candidate, assembly, solver)?;
        self.compose_accepted(accepted)
    }

    pub(super) fn solve_candidate(
        &self,
        document: &ModelDocument,
        candidate: &[(VertexId, [f64; 3])],
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<AcceptedPrescribedDynamicSolidStep3d, Diagnostic> {
        if !candidate_matches_exact(candidate) {
            return Err(invalid(
                "prescribed dynamic-solid provider candidate differs from the frozen affine predictor",
            ));
        }
        let canonical = lower_isotropic_elastodynamics_cartesian_3d(document.program())?;
        let driven_boundary = canonical
            .boundary_inventory()
            .boundary(0, eqiora_schema::kernel::BoundarySide::Upper)
            .and_then(|entry| entry.boundary().downcast())
            .ok_or_else(|| {
                invalid(
                    "prescribed dynamic-solid x-upper boundary does not retain a typed Domain identity",
                )
            })?;
        if driven_boundary != self.realization.driven_boundary() {
            return Err(invalid(
                "prescribed dynamic-solid provider boundary differs from the prepared Realization",
            ));
        }
        let solver_provider = solver.provider();
        let mut reference = PrescribedDynamicSolidReference3d::new(
            &self.model,
            &self.geometry,
            &self.mesh,
            &self.correspondence,
            DynQuantity::new(0.25, TIME),
            &self.prior_displacement,
            &self.prior_velocity,
            driven_boundary,
        )?;
        let accepted = reference.accept_candidate(0, candidate, assembly, solver)?;
        if accepted.solve_report().solver_provider() != solver_provider {
            return Err(invalid(
                "accepted prescribed dynamic-solid evidence differs from the injected solver provider",
            ));
        }
        Ok(accepted)
    }

    pub(super) fn compose_accepted(
        &self,
        accepted: AcceptedPrescribedDynamicSolidStep3d,
    ) -> Result<PrescribedDynamicSolidStateRun3d, Diagnostic> {
        let accepted_displacement_block = vector_block(&self.mesh, accepted.displacement())?;
        let accepted_velocity_block = vector_block(&self.mesh, accepted.velocity())?;
        let accepted_displacement_snapshot = FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            self.realization.displacement_field(),
            std::slice::from_ref(&accepted_displacement_block),
        )?;
        let accepted_velocity_snapshot = FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            self.realization.velocity_field(),
            std::slice::from_ref(&accepted_velocity_block),
        )?;
        let accepted_state = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            1,
            0.25,
            &[
                accepted_displacement_snapshot.clone(),
                accepted_velocity_snapshot.clone(),
            ],
        )?;
        let run = RunManifestV2::new(&self.realization, exact_execution(&accepted)?)?
            .with_output(accepted_state.digest()?);
        let value = PrescribedDynamicSolidStateRun3d {
            model: self.model.clone(),
            geometry: self.geometry.clone(),
            correspondence: self.correspondence.clone(),
            mesh: self.mesh.clone(),
            realization: self.realization.clone(),
            accepted,
            prior_displacement_block: self.prior_displacement_block.clone(),
            prior_velocity_block: self.prior_velocity_block.clone(),
            accepted_displacement_block,
            accepted_velocity_block,
            prior_displacement_snapshot: self.prior_displacement_snapshot.clone(),
            prior_velocity_snapshot: self.prior_velocity_snapshot.clone(),
            accepted_displacement_snapshot,
            accepted_velocity_snapshot,
            prior_state: self.prior_state.clone(),
            accepted_state,
            run,
        };
        value.revalidate()?;
        Ok(value)
    }
}

pub(super) fn exact_candidate() -> Vec<(VertexId, [f64; 3])> {
    DRIVEN_VERTICES
        .into_iter()
        .map(|index| (VertexId::new(index), [0.015, 0.0, 0.0]))
        .collect()
}

pub(super) fn candidate_matches_exact(candidate: &[(VertexId, [f64; 3])]) -> bool {
    candidate.len() == DRIVEN_VERTICES.len()
        && candidate.iter().zip(exact_candidate()).all(
            |((vertex, value), (expected_vertex, expected))| {
                *vertex == expected_vertex && value.map(f64::to_bits) == expected.map(f64::to_bits)
            },
        )
}
