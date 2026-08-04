//! Failure-atomic publication of one prescribed dynamic-solid State and Run.

use std::num::{NonZeroU32, NonZeroUsize};

use eqiora_artifact::{
    DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, FieldSnapshotEnvelopeV1,
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1, ModelEnvelope,
    PrescribedDynamicSolidRealizationEnvelopeV1, RunManifestV2, SimplicialMeshEnvelopeV1,
    SpatialStateEnvelopeV1,
};
use eqiora_assembly::AssemblyBackend;
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, Id};
use eqiora_meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshQualityGate,
    SimplicialMesh, VertexId,
};
use eqiora_numerics::solid::{
    AcceptedPrescribedDynamicSolidStep3d, PrescribedDynamicSolidReference3d,
    lower_isotropic_elastodynamics_cartesian_3d,
};
use eqiora_realization::RealizationRevision;
use eqiora_schema::kernel::BoundarySide;
use eqiora_solver::{
    ExecutionReport, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    REFERENCE_SOLVER_PROVIDER, ReductionPolicy, SERIAL_EXECUTION_PROVIDER, SolverPlan,
};

use crate::ModelDocument;

#[cfg(test)]
mod tests;

const ZERO: u64 = 0;
const PRIOR_DISPLACEMENT_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
    [0x3f747ae147ae147b, ZERO, ZERO],
];
const PRIOR_VELOCITY_BITS: [[u64; 3]; 9] = [
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [ZERO, ZERO, ZERO],
    [0x3f947ae147ae147b, ZERO, ZERO],
    [0x3f847ae147ae147b, ZERO, ZERO],
];
const VERTICES: [[f64; 3]; 9] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.5, 0.5, 0.5],
];
const CELLS: [[usize; 4]; 12] = [
    [8, 0, 6, 2],
    [8, 0, 4, 6],
    [8, 1, 7, 5],
    [8, 1, 3, 7],
    [8, 0, 5, 4],
    [8, 0, 1, 5],
    [8, 2, 7, 3],
    [8, 2, 6, 7],
    [8, 0, 3, 1],
    [8, 0, 2, 3],
    [8, 4, 7, 6],
    [8, 4, 5, 7],
];
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

/// Complete owned lineage for the exact accepted prescribed-solid occurrence.
///
/// Construction is the only public join between nonforgeable accepted
/// numerical evidence and the durable Realization, State, and Run artifacts.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidStateRun3d {
    model: ModelEnvelope,
    geometry: GeometryIdentityEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    realization: PrescribedDynamicSolidRealizationEnvelopeV1,
    accepted: AcceptedPrescribedDynamicSolidStep3d,
    pub(super) prior_displacement_block: DiscreteFieldEnvelopeV1,
    pub(super) prior_velocity_block: DiscreteFieldEnvelopeV1,
    pub(super) accepted_displacement_block: DiscreteFieldEnvelopeV1,
    pub(super) accepted_velocity_block: DiscreteFieldEnvelopeV1,
    pub(super) prior_displacement_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) prior_velocity_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) accepted_displacement_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) accepted_velocity_snapshot: FieldSnapshotEnvelopeV1,
    pub(super) prior_state: SpatialStateEnvelopeV1,
    pub(super) accepted_state: SpatialStateEnvelopeV1,
    pub(super) run: RunManifestV2,
}

impl PrescribedDynamicSolidStateRun3d {
    /// Execute and atomically publish the exact accepted reference occurrence.
    ///
    /// # Errors
    /// Returns one diagnostic for changed Model meaning, resources, science,
    /// backend evidence, numerical acceptance, or durable lineage. No partial
    /// owner, State, or Run is returned.
    pub fn solve_reference(
        document: &ModelDocument,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<PrescribedDynamicSolidStateRun3d, Diagnostic> {
        let model = ModelEnvelope::from_program(document.program())?;
        let canonical = lower_isotropic_elastodynamics_cartesian_3d(document.program())?;
        let body = canonical
            .domain()
            .downcast::<kinds::Domain>()
            .ok_or_else(|| invalid("prescribed dynamic-solid body changed entity kind"))?;
        let driven_boundary = exact_boundary(&canonical, 0, BoundarySide::Upper)?;
        let mesh = reference_mesh()?;
        let geometry = GeometryIdentityEnvelopeV1::new(&model, [body], 1.0e-12)?;
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh)?;
        let prior_displacement = tagged(PRIOR_DISPLACEMENT_BITS);
        let prior_velocity = tagged(PRIOR_VELOCITY_BITS);
        let candidate = [1, 3, 5, 7]
            .into_iter()
            .map(|vertex| (VertexId::new(vertex), [0.015, 0.0, 0.0]))
            .collect::<Vec<_>>();
        let mut reference = PrescribedDynamicSolidReference3d::new(
            &model,
            &geometry,
            &mesh,
            &correspondence,
            DynQuantity::new(0.25, TIME),
            &prior_displacement,
            &prior_velocity,
            driven_boundary,
        )?;
        let accepted = reference.accept_candidate(0, &candidate, assembly, solver)?;
        let realization = PrescribedDynamicSolidRealizationEnvelopeV1::new(
            &model,
            &geometry,
            &correspondence,
            &mesh,
            RealizationRevision::new(1),
            &candidate,
        )?;
        let prior_displacement_block = block(&mesh, &prior_displacement)?;
        let prior_velocity_block = block(&mesh, &prior_velocity)?;
        let accepted_displacement_block = block(&mesh, accepted.displacement())?;
        let accepted_velocity_block = block(&mesh, accepted.velocity())?;
        let prior_displacement_snapshot = snapshot(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.displacement_field(),
            &prior_displacement_block,
        )?;
        let prior_velocity_snapshot = snapshot(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.velocity_field(),
            &prior_velocity_block,
        )?;
        let accepted_displacement_snapshot = snapshot(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.displacement_field(),
            &accepted_displacement_block,
        )?;
        let accepted_velocity_snapshot = snapshot(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            realization.velocity_field(),
            &accepted_velocity_block,
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
        let accepted_state = SpatialStateEnvelopeV1::new_prescribed_dynamic_solid(
            &model,
            &realization,
            &geometry,
            &correspondence,
            &mesh,
            1,
            0.25,
            &[
                accepted_displacement_snapshot.clone(),
                accepted_velocity_snapshot.clone(),
            ],
        )?;
        let execution = ExecutionProvenanceV1::from_provider_releases(
            accepted.solve_report().solver_provider(),
            SERIAL_EXECUTION_PROVIDER,
            ExecutionTopologyV1::Host {
                workers: NonZeroUsize::MIN,
            },
            ReductionPolicy::Reproducible,
            std::iter::empty::<(&str, &str)>(),
        )?;
        let run =
            RunManifestV2::new(&realization, execution)?.with_output(accepted_state.digest()?);
        let owner = Self {
            model,
            geometry,
            mesh,
            correspondence,
            realization,
            accepted,
            prior_displacement_block,
            prior_velocity_block,
            accepted_displacement_block,
            accepted_velocity_block,
            prior_displacement_snapshot,
            prior_velocity_snapshot,
            accepted_displacement_snapshot,
            accepted_velocity_snapshot,
            prior_state,
            accepted_state,
            run,
        };
        owner.revalidate()?;
        Ok(owner)
    }

    /// Exact caller-owned current Model.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Exact Geometry identity.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact Geometry-to-Mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Exact imported affine tetrahedron mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }

    /// Exact standalone prescribed-solid Realization.
    #[must_use]
    pub const fn realization(&self) -> &PrescribedDynamicSolidRealizationEnvelopeV1 {
        &self.realization
    }

    /// Nonforgeable accepted in-memory numerical result.
    #[must_use]
    pub const fn accepted(&self) -> &AcceptedPrescribedDynamicSolidStep3d {
        &self.accepted
    }

    /// Exact retained prior displacement coefficients.
    #[must_use]
    pub const fn prior_displacement_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.prior_displacement_block
    }

    /// Exact retained prior velocity coefficients.
    #[must_use]
    pub const fn prior_velocity_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.prior_velocity_block
    }

    /// Exact accepted displacement coefficients.
    #[must_use]
    pub const fn accepted_displacement_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.accepted_displacement_block
    }

    /// Exact accepted velocity coefficients.
    #[must_use]
    pub const fn accepted_velocity_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.accepted_velocity_block
    }

    /// Exact retained prior displacement snapshot.
    #[must_use]
    pub const fn prior_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.prior_displacement_snapshot
    }

    /// Exact retained prior velocity snapshot.
    #[must_use]
    pub const fn prior_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.prior_velocity_snapshot
    }

    /// Exact accepted displacement snapshot.
    #[must_use]
    pub const fn accepted_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.accepted_displacement_snapshot
    }

    /// Exact accepted velocity snapshot.
    #[must_use]
    pub const fn accepted_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.accepted_velocity_snapshot
    }

    /// Exact prior State at `(0, 0.0)`.
    #[must_use]
    pub const fn prior_state(&self) -> &SpatialStateEnvelopeV1 {
        &self.prior_state
    }

    /// Exact accepted-next State at `(1, 0.25)`.
    #[must_use]
    pub const fn accepted_state(&self) -> &SpatialStateEnvelopeV1 {
        &self.accepted_state
    }

    /// Run whose sole output is the accepted-next State.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Replay the complete owner and reject any role or evidence substitution.
    ///
    /// # Errors
    /// Returns `EQ0901` for any Model, Realization, block, snapshot, State,
    /// Run, provider, generation, or accepted numerical leaf drift.
    pub fn revalidate(&self) -> Result<(), Diagnostic> {
        self.realization.validate_against(
            &self.model,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
        )?;
        for block in [
            &self.prior_displacement_block,
            &self.prior_velocity_block,
            &self.accepted_displacement_block,
            &self.accepted_velocity_block,
        ] {
            block.validate_mesh_artifact(&self.mesh)?;
        }
        require_flat_bits(
            self.prior_displacement_block.values(),
            &PRIOR_DISPLACEMENT_BITS,
            "prior displacement",
        )?;
        require_flat_bits(
            self.prior_velocity_block.values(),
            &PRIOR_VELOCITY_BITS,
            "prior velocity",
        )?;
        require_accepted_block(
            &self.accepted_displacement_block,
            self.accepted.displacement(),
            "accepted displacement",
        )?;
        require_accepted_block(
            &self.accepted_velocity_block,
            self.accepted.velocity(),
            "accepted velocity",
        )?;
        validate_snapshot(
            self,
            &self.prior_displacement_snapshot,
            &self.prior_displacement_block,
        )?;
        validate_snapshot(
            self,
            &self.prior_velocity_snapshot,
            &self.prior_velocity_block,
        )?;
        validate_snapshot(
            self,
            &self.accepted_displacement_snapshot,
            &self.accepted_displacement_block,
        )?;
        validate_snapshot(
            self,
            &self.accepted_velocity_snapshot,
            &self.accepted_velocity_block,
        )?;
        self.prior_state.validate_against_prescribed_dynamic_solid(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            &[
                self.prior_displacement_snapshot.clone(),
                self.prior_velocity_snapshot.clone(),
            ],
        )?;
        self.accepted_state
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                &[
                    self.accepted_displacement_snapshot.clone(),
                    self.accepted_velocity_snapshot.clone(),
                ],
            )?;
        if self.prior_state.step() != 0
            || self.prior_state.time_s().to_bits() != 0.0f64.to_bits()
            || self.accepted_state.step() != 1
            || self.accepted_state.time_s().to_bits() != 0.25f64.to_bits()
            || self.accepted.generation() != 1
            || self.run.outputs() != vec![self.accepted_state.digest()?]
        {
            return Err(invalid(
                "prescribed dynamic-solid prior/accepted State roles or Run singleton differ",
            ));
        }
        self.run.validate_against(&self.realization)?;
        require_execution_evidence(&self.accepted, &self.run)?;
        for (candidate, expected) in self
            .realization
            .driven_total_displacement()
            .iter()
            .zip([1, 3, 5, 7])
        {
            if candidate.0.index() != expected
                || self.accepted.displacement()[expected] != *candidate
            {
                return Err(invalid(
                    "accepted displacement differs from the exact driven candidate",
                ));
            }
        }
        Ok(())
    }
}

fn exact_boundary(
    canonical: &eqiora_numerics::solid::IsotropicElastodynamicsCartesianModel3d,
    axis: usize,
    side: BoundarySide,
) -> Result<Id<kinds::Domain>, Diagnostic> {
    canonical
        .boundary_inventory()
        .boundary(axis, side)
        .and_then(|entry| entry.boundary().downcast())
        .ok_or_else(|| invalid("prescribed dynamic-solid boundary identity changed kind"))
}

fn reference_mesh() -> Result<SimplicialMeshEnvelopeV1, Diagnostic> {
    SimplicialMeshEnvelopeV1::from_mesh(&SimplicialMesh::new(
        3,
        VERTICES.iter().map(|value| value.to_vec()).collect(),
        CELLS.iter().map(|value| value.to_vec()).collect(),
        MeshQualityGate::new(0.1)?,
    )?)
}

fn tagged(bits: [[u64; 3]; 9]) -> Vec<(VertexId, [f64; 3])> {
    bits.into_iter()
        .enumerate()
        .map(|(vertex, value)| (VertexId::new(vertex), value.map(f64::from_bits)))
        .collect()
}

fn block(
    mesh: &SimplicialMeshEnvelopeV1,
    values: &[(VertexId, [f64; 3])],
) -> Result<DiscreteFieldEnvelopeV1, Diagnostic> {
    if values
        .iter()
        .enumerate()
        .any(|(index, (vertex, _))| vertex.index() != index)
    {
        return Err(invalid(
            "prescribed dynamic-solid coefficients are outside canonical vertex order",
        ));
    }
    let payload = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Vector {
            components: NonZeroU32::new(3).expect("three is nonzero"),
        },
        values
            .iter()
            .flat_map(|(_, value)| value)
            .copied()
            .collect(),
    )?;
    DiscreteFieldEnvelopeV1::from_payload(mesh, &payload)
}

#[allow(clippy::too_many_arguments)]
fn snapshot(
    model: &ModelEnvelope,
    realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
    geometry: &GeometryIdentityEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    field: Id<kinds::Field>,
    block: &DiscreteFieldEnvelopeV1,
) -> Result<FieldSnapshotEnvelopeV1, Diagnostic> {
    FieldSnapshotEnvelopeV1::new_prescribed_dynamic_solid(
        model,
        realization,
        geometry,
        correspondence,
        mesh,
        field,
        std::slice::from_ref(block),
    )
}

fn validate_snapshot(
    owner: &PrescribedDynamicSolidStateRun3d,
    snapshot: &FieldSnapshotEnvelopeV1,
    block: &DiscreteFieldEnvelopeV1,
) -> Result<(), Diagnostic> {
    snapshot.validate_against_prescribed_dynamic_solid(
        &owner.model,
        &owner.realization,
        &owner.geometry,
        &owner.correspondence,
        &owner.mesh,
        std::slice::from_ref(block),
    )
}

fn require_flat_bits(
    actual: &[f64],
    expected: &[[u64; 3]; 9],
    label: &str,
) -> Result<(), Diagnostic> {
    if actual
        .iter()
        .map(|value| value.to_bits())
        .ne(expected.iter().flatten().copied())
    {
        return Err(invalid(format!(
            "prescribed dynamic-solid {label} leaves differ bit for bit"
        )));
    }
    Ok(())
}

fn require_accepted_block(
    block: &DiscreteFieldEnvelopeV1,
    accepted: &[(VertexId, [f64; 3])],
    label: &str,
) -> Result<(), Diagnostic> {
    if accepted
        .iter()
        .enumerate()
        .any(|(index, (vertex, _))| vertex.index() != index)
        || block
            .values()
            .iter()
            .map(|value| value.to_bits())
            .ne(accepted
                .iter()
                .flat_map(|(_, value)| value)
                .map(|value| value.to_bits()))
    {
        return Err(invalid(format!(
            "prescribed dynamic-solid {label} differs from accepted in-memory evidence"
        )));
    }
    Ok(())
}

fn require_execution_evidence(
    accepted: &AcceptedPrescribedDynamicSolidStep3d,
    run: &RunManifestV2,
) -> Result<(), Diagnostic> {
    let report = accepted.solve_report();
    if accepted.assembly_report().execution() != ExecutionReport::host_serial()
        || report.solver_provider() != REFERENCE_SOLVER_PROVIDER
        || report.solver_plan() != reference_solver_plan()?
        || report.execution_provider() != SERIAL_EXECUTION_PROVIDER
        || report.execution() != ExecutionReport::host_serial()
        || report.verification_provider() != SERIAL_EXECUTION_PROVIDER
        || report.verification() != ExecutionReport::host_serial()
    {
        return Err(invalid(
            "prescribed dynamic-solid accepted backend evidence differs from exact serial reference",
        ));
    }
    let expected = ExecutionProvenanceV1::from_provider_releases(
        REFERENCE_SOLVER_PROVIDER,
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
        std::iter::empty::<(&str, &str)>(),
    )?;
    if run.execution() != expected {
        return Err(invalid(
            "prescribed dynamic-solid Run execution provenance differs",
        ));
    }
    Ok(())
}

fn reference_solver_plan() -> Result<SolverPlan, Diagnostic> {
    Ok(SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-13,
        1.0e-15,
        NonZeroUsize::new(500).expect("positive iteration budget"),
    )?
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Reproducible))
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
