//! Atomic publication of one prescribed dynamic-solid State and Run lineage.

use std::num::{NonZeroU32, NonZeroUsize};

use eqiora_artifact::{
    DiscreteFieldEnvelopeV1, ExecutionProvenanceV1, ExecutionTopologyV1, FieldSnapshotEnvelopeV1,
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1, ModelEnvelope,
    PrescribedDynamicSolidRealizationEnvelopeV1, RunManifestV2, SimplicialMeshEnvelopeV1,
    SpatialStateEnvelopeV1,
};
use eqiora_assembly::AssemblyBackend;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity};
use eqiora_meshing::{
    DiscreteFieldAssociation, DiscreteFieldPayload, DiscreteFieldShape, MeshQualityGate,
    SimplicialMesh, VertexId,
};
use eqiora_numerics::solid::{
    AcceptedPrescribedDynamicSolidStep3d, PrescribedDynamicSolidReference3d,
    lower_isotropic_elastodynamics_cartesian_3d,
};
use eqiora_realization::RealizationRevision;
use eqiora_solver::{
    ExecutionReport, LinearSolver, LinearSolverBackend, PreconditionerPolicy, ReductionPolicy,
    SERIAL_EXECUTION_PROVIDER, SolverPlan,
};

use crate::ModelDocument;

mod composition;
mod provider;

pub use provider::PrescribedDynamicSolidExternalProviderStateRun3d;

#[cfg(test)]
mod tests;

const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};
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
const DRIVEN_VERTICES: [usize; 4] = [1, 3, 5, 7];

/// Complete accepted owner for the exact prescribed dynamic-solid occurrence.
///
/// This in-process value is not another durable Result schema. It retains all
/// resources and numerical evidence needed to revalidate the two existing
/// State artifacts and the singleton-output Run as one failure-atomic unit.
#[derive(Debug, Clone, PartialEq)]
pub struct PrescribedDynamicSolidStateRun3d {
    model: ModelEnvelope,
    geometry: GeometryIdentityEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    realization: PrescribedDynamicSolidRealizationEnvelopeV1,
    accepted: AcceptedPrescribedDynamicSolidStep3d,
    prior_displacement_block: DiscreteFieldEnvelopeV1,
    prior_velocity_block: DiscreteFieldEnvelopeV1,
    accepted_displacement_block: DiscreteFieldEnvelopeV1,
    accepted_velocity_block: DiscreteFieldEnvelopeV1,
    prior_displacement_snapshot: FieldSnapshotEnvelopeV1,
    prior_velocity_snapshot: FieldSnapshotEnvelopeV1,
    accepted_displacement_snapshot: FieldSnapshotEnvelopeV1,
    accepted_velocity_snapshot: FieldSnapshotEnvelopeV1,
    prior_state: SpatialStateEnvelopeV1,
    accepted_state: SpatialStateEnvelopeV1,
    run: RunManifestV2,
}

impl PrescribedDynamicSolidStateRun3d {
    /// Execute and atomically publish the exact accepted reference occurrence.
    ///
    /// # Errors
    /// Returns a structured diagnostic for foreign Model meaning, assembly or
    /// solve rejection, evidence drift, or any incomplete artifact lineage.
    pub fn solve_reference(
        document: &ModelDocument,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        let prepared = composition::PreparedPrescribedDynamicSolid3d::new(document)?;
        prepared.accept(document, &composition::exact_candidate(), assembly, solver)
    }

    /// Exact current Model used by this occurrence.
    #[must_use]
    pub const fn model(&self) -> &ModelEnvelope {
        &self.model
    }

    /// Exact Geometry identity used by this occurrence.
    #[must_use]
    pub const fn geometry(&self) -> &GeometryIdentityEnvelopeV1 {
        &self.geometry
    }

    /// Exact Geometry-to-Mesh correspondence.
    #[must_use]
    pub const fn correspondence(&self) -> &GeometryMeshCorrespondenceEnvelopeV1 {
        &self.correspondence
    }

    /// Exact immutable reference mesh artifact.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMeshEnvelopeV1 {
        &self.mesh
    }

    /// Exact standalone prescribed-solid Realization.
    #[must_use]
    pub const fn realization(&self) -> &PrescribedDynamicSolidRealizationEnvelopeV1 {
        &self.realization
    }

    /// Nonforgeable accepted numerical evidence retained by this occurrence.
    #[must_use]
    pub const fn accepted(&self) -> &AcceptedPrescribedDynamicSolidStep3d {
        &self.accepted
    }

    /// Prior displacement coefficient block.
    #[must_use]
    pub const fn prior_displacement_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.prior_displacement_block
    }

    /// Prior velocity coefficient block.
    #[must_use]
    pub const fn prior_velocity_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.prior_velocity_block
    }

    /// Accepted-next displacement coefficient block.
    #[must_use]
    pub const fn accepted_displacement_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.accepted_displacement_block
    }

    /// Accepted-next velocity coefficient block.
    #[must_use]
    pub const fn accepted_velocity_block(&self) -> &DiscreteFieldEnvelopeV1 {
        &self.accepted_velocity_block
    }

    /// Prior displacement Field snapshot.
    #[must_use]
    pub const fn prior_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.prior_displacement_snapshot
    }

    /// Prior velocity Field snapshot.
    #[must_use]
    pub const fn prior_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.prior_velocity_snapshot
    }

    /// Accepted-next displacement Field snapshot.
    #[must_use]
    pub const fn accepted_displacement_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.accepted_displacement_snapshot
    }

    /// Accepted-next velocity Field snapshot.
    #[must_use]
    pub const fn accepted_velocity_snapshot(&self) -> &FieldSnapshotEnvelopeV1 {
        &self.accepted_velocity_snapshot
    }

    /// Exact retained prior State input observation.
    #[must_use]
    pub const fn prior_state(&self) -> &SpatialStateEnvelopeV1 {
        &self.prior_state
    }

    /// Exact accepted-next State output.
    #[must_use]
    pub const fn accepted_state(&self) -> &SpatialStateEnvelopeV1 {
        &self.accepted_state
    }

    /// Singleton-output Run for the accepted-next State.
    #[must_use]
    pub const fn run(&self) -> &RunManifestV2 {
        &self.run
    }

    /// Revalidate every retained resource, role edge, numerical leaf, and Run edge.
    ///
    /// # Errors
    /// Returns `EQ0901` for any resource, role, content, generation, evidence,
    /// or singleton-output substitution.
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

        let expected_prior_displacement = VERTICES
            .iter()
            .enumerate()
            .map(|(index, point)| (VertexId::new(index), [point[0] / 100.0, 0.0, 0.0]))
            .collect::<Vec<_>>();
        let expected_prior_velocity = VERTICES
            .iter()
            .enumerate()
            .map(|(index, point)| (VertexId::new(index), [point[0] / 50.0, 0.0, 0.0]))
            .collect::<Vec<_>>();
        if self.prior_displacement_block != vector_block(&self.mesh, &expected_prior_displacement)?
            || self.prior_velocity_block != vector_block(&self.mesh, &expected_prior_velocity)?
        {
            return Err(invalid(
                "prescribed dynamic-solid prior numerical leaves differ from the frozen inputs",
            ));
        }

        self.prior_displacement_snapshot
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                std::slice::from_ref(&self.prior_displacement_block),
            )?;
        self.prior_velocity_snapshot
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                std::slice::from_ref(&self.prior_velocity_block),
            )?;
        self.accepted_displacement_snapshot
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                std::slice::from_ref(&self.accepted_displacement_block),
            )?;
        self.accepted_velocity_snapshot
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                std::slice::from_ref(&self.accepted_velocity_block),
            )?;

        let prior_snapshots = [
            self.prior_displacement_snapshot.clone(),
            self.prior_velocity_snapshot.clone(),
        ];
        let accepted_snapshots = [
            self.accepted_displacement_snapshot.clone(),
            self.accepted_velocity_snapshot.clone(),
        ];
        if self.prior_state.step() != 0
            || self.prior_state.time_s().to_bits() != 0.0_f64.to_bits()
            || self.accepted_state.step() != 1
            || self.accepted_state.time_s().to_bits() != 0.25_f64.to_bits()
        {
            return Err(invalid(
                "prescribed dynamic-solid retained States differ from their exact occurrence roles",
            ));
        }
        self.prior_state.validate_against_prescribed_dynamic_solid(
            &self.model,
            &self.realization,
            &self.geometry,
            &self.correspondence,
            &self.mesh,
            &prior_snapshots,
        )?;
        self.accepted_state
            .validate_against_prescribed_dynamic_solid(
                &self.model,
                &self.realization,
                &self.geometry,
                &self.correspondence,
                &self.mesh,
                &accepted_snapshots,
            )?;

        if self.accepted.generation() != 1
            || !values_match_block(
                self.accepted.displacement(),
                &self.accepted_displacement_block,
            )
            || !values_match_block(self.accepted.velocity(), &self.accepted_velocity_block)
            || !accepted_candidate_matches(&self.accepted, &self.realization)
        {
            return Err(invalid(
                "prescribed dynamic-solid accepted generation, candidate, or numerical leaves differ",
            ));
        }
        validate_accepted_evidence(&self.accepted)?;

        self.run.validate_against(&self.realization)?;
        let expected_execution = exact_execution(&self.accepted)?;
        if self.run.execution() != expected_execution
            || self.run.outputs() != vec![self.accepted_state.digest()?]
        {
            return Err(invalid(
                "prescribed dynamic-solid Run execution or singleton accepted-State output differs",
            ));
        }
        Ok(())
    }
}

fn exact_mesh() -> Result<SimplicialMeshEnvelopeV1, Diagnostic> {
    let mesh = SimplicialMesh::new(
        3,
        VERTICES.iter().map(|point| point.to_vec()).collect(),
        CELLS.iter().map(|cell| cell.to_vec()).collect(),
        MeshQualityGate::new(0.1)?,
    )?;
    SimplicialMeshEnvelopeV1::from_mesh(&mesh)
}

fn vector_block(
    mesh: &SimplicialMeshEnvelopeV1,
    values: &[(VertexId, [f64; 3])],
) -> Result<DiscreteFieldEnvelopeV1, Diagnostic> {
    if values.len() != mesh.mesh().vertices().len()
        || values
            .iter()
            .enumerate()
            .any(|(index, (vertex, _))| vertex.index() != index)
    {
        return Err(invalid(
            "prescribed dynamic-solid coefficient values are not in complete canonical vertex order",
        ));
    }
    let payload = DiscreteFieldPayload::new(
        mesh.mesh(),
        DiscreteFieldAssociation::Vertex,
        DiscreteFieldShape::Vector {
            components: NonZeroU32::new(3).expect("three components are positive"),
        },
        values
            .iter()
            .flat_map(|(_, value)| value.iter().copied())
            .collect(),
    )?;
    DiscreteFieldEnvelopeV1::from_payload(mesh, &payload)
}

fn values_match_block(values: &[(VertexId, [f64; 3])], block: &DiscreteFieldEnvelopeV1) -> bool {
    values.len() == VERTICES.len()
        && values
            .iter()
            .enumerate()
            .all(|(index, (vertex, _))| vertex.index() == index)
        && values
            .iter()
            .flat_map(|(_, value)| value)
            .map(|value| value.to_bits())
            .eq(block.values().iter().map(|value| value.to_bits()))
}

fn accepted_candidate_matches(
    accepted: &AcceptedPrescribedDynamicSolidStep3d,
    realization: &PrescribedDynamicSolidRealizationEnvelopeV1,
) -> bool {
    realization
        .driven_total_displacement()
        .iter()
        .all(|(vertex, expected)| {
            accepted
                .displacement()
                .get(vertex.index())
                .is_some_and(|(actual_vertex, actual)| {
                    actual_vertex == vertex
                        && actual.map(f64::to_bits) == expected.map(f64::to_bits)
                })
        })
}

fn validate_accepted_evidence(
    accepted: &AcceptedPrescribedDynamicSolidStep3d,
) -> Result<(), Diagnostic> {
    let assembly = accepted.assembly_report();
    let solve = accepted.solve_report();
    if assembly.execution() != ExecutionReport::host_serial()
        || assembly.packet_count() != CELLS.len()
        || assembly.target_count() != 2
        || solve.solver_plan() != exact_solver_plan()?
        || solve.execution_provider() != SERIAL_EXECUTION_PROVIDER
        || solve.execution() != ExecutionReport::host_serial()
        || solve.verification_provider() != SERIAL_EXECUTION_PROVIDER
        || solve.verification() != ExecutionReport::host_serial()
    {
        return Err(invalid(
            "prescribed dynamic-solid accepted assembly, solver, execution, or verification evidence differs",
        ));
    }
    Ok(())
}

fn exact_solver_plan() -> Result<SolverPlan, Diagnostic> {
    Ok(SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-13,
        1.0e-15,
        NonZeroUsize::new(500).expect("the frozen iteration budget is positive"),
    )?
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Reproducible))
}

fn exact_execution(
    accepted: &AcceptedPrescribedDynamicSolidStep3d,
) -> Result<ExecutionProvenanceV1, Diagnostic> {
    ExecutionProvenanceV1::from_provider_releases(
        accepted.solve_report().solver_provider(),
        SERIAL_EXECUTION_PROVIDER,
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
        std::iter::empty::<(&str, &str)>(),
    )
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
