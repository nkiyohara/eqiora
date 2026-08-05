//! Crate-private exact-source non-box transient reference binding.
#![cfg_attr(not(test), allow(dead_code))]

mod boundary;

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora_assembly::AssemblyBackend;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshTopology, SimplicialMesh};
use eqiora_realization::{
    NonlinearSolvePlan, ResolvedTransientFieldwiseRealization, TransientFieldwiseRealizationPlan,
    TransientFieldwiseRealizationRequirements,
};
use eqiora_schema::kernel::{DomainKind, KernelNode};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearSolverBackend, SolverPlan};

use self::boundary::CorrespondenceBoundary2d;
use super::IncompressibleFlowScaleProfile2d;
use super::boundary as semantic_boundary;
use super::inertial::parameters_referenced_by;
use super::navier_stokes::{
    TransientIncompressibleNavierStokesModel2d, lower_transient_volume, require_boundary_volume,
};
use super::navier_stokes_realization::{
    require_exact_transient_plan, transient_navier_stokes_fieldwise_requirements_for_2d,
    transient_navier_stokes_mini_plan_for_2d,
};
use super::recognize::unique_circular_hole_domain;
use super::support::{lowering_error, model_lowering_error};
use crate::simplicial_elliptic::SimplicialP1Field;
use crate::simplicial_navier_stokes::{
    SimplicialMiniNavierStokesState2d, SimplicialMiniNavierStokesTrajectory2d,
    advance_simplicial_mini_navier_stokes_2d_with_assembly,
};
use crate::simplicial_stokes::SimplicialMiniVelocityField2d;
use crate::step_count::NonZeroStepCount;

const DIMENSION: usize = 2;
const DUFFY_POINTS_PER_AXIS: usize = 5;

/// Exact source, semantic roles, named correspondence, and Mesh identity.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TransientNavierStokesGeometryBinding2d {
    model: TransientIncompressibleNavierStokesModel2d,
    accepted: AcceptedCircularHoleChordalRealizationV1,
    boundary_domains: BTreeMap<String, eqiora_core::RawId>,
    boundary: CorrespondenceBoundary2d,
    source_digest: [u8; 32],
    mesh_reference: eqiora_realization::MeshArtifactReference,
}

impl TransientNavierStokesGeometryBinding2d {
    pub(crate) fn new(
        program: &KernelProgram,
        accepted: AcceptedCircularHoleChordalRealizationV1,
    ) -> Result<Self, Diagnostic> {
        accepted.revalidate()?;
        let source = accepted.source();
        let source_digest = source.digest_bytes();
        let mesh_reference = accepted.mesh().artifact_reference()?;
        let boundary = CorrespondenceBoundary2d::new(&accepted)?;
        let (domain, boundary_domains) = unique_circular_hole_domain(program, source)?;
        let bounds = *source.circular_hole_bounds().ok_or_else(|| {
            lowering_error(
                domain,
                "geometry-backed transient flow requires exact circular-hole geometry",
            )
        })?;
        let volume = lower_transient_volume::<DIMENSION>(program, domain)?;
        let lowered_boundary = semantic_boundary::lower_named(
            program,
            domain,
            volume.velocity,
            volume.pressure,
            &volume.dynamic_viscosity,
            boundary_domains.clone(),
        )?;
        require_boundary_volume(
            &volume,
            &lowered_boundary.normal_velocity_fields,
            &lowered_boundary.normal_velocity_definitions,
        )?;
        let model = TransientIncompressibleNavierStokesModel2d {
            domain,
            velocity: volume.velocity,
            pressure: volume.pressure,
            force_potential: volume.force_potential,
            bounds,
            mass_density: volume.mass_density,
            dynamic_viscosity: volume.dynamic_viscosity,
            force_potential_expression: volume.force_potential_expression,
            force_potential_definition: volume.force_potential_definition,
            momentum_relation: volume.momentum_relation,
            incompressibility_relation: volume.incompressibility_relation,
            boundary_dispositions: lowered_boundary
                .entries
                .values()
                .map(|entry| (entry.boundary(), entry.disposition()))
                .collect(),
            boundary_relations: lowered_boundary.boundary_relations.clone(),
            normal_velocity_expressions: lowered_boundary
                .normal_velocity_expressions
                .iter()
                .filter_map(|(name, expression)| {
                    lowered_boundary
                        .entries
                        .get(name)
                        .map(|entry| (entry.boundary(), expression.clone()))
                })
                .collect(),
        };
        require_closed_model(program, &model, volume.representation, &lowered_boundary)?;
        boundary.require_dispositions(&model, &boundary_domains)?;
        Ok(Self {
            model,
            accepted,
            boundary_domains,
            boundary,
            source_digest,
            mesh_reference,
        })
    }

    pub(crate) fn fieldwise_requirements(&self) -> TransientFieldwiseRealizationRequirements {
        transient_navier_stokes_fieldwise_requirements_for_2d(&self.model)
    }

    pub(crate) fn mini_plan(
        &self,
        scales: IncompressibleFlowScaleProfile2d,
        time_step: DynQuantity,
        nonlinear: NonlinearSolvePlan,
        solver: SolverPlan,
    ) -> Result<TransientFieldwiseRealizationPlan, Diagnostic> {
        transient_navier_stokes_mini_plan_for_2d(
            &self.model,
            self.mesh_reference,
            scales,
            time_step,
            nonlinear,
            solver,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn advance_with_assembly(
        &self,
        program: &KernelProgram,
        resolved: &ResolvedTransientFieldwiseRealization,
        initial: SimplicialMiniNavierStokesState2d,
        steps: NonZeroStepCount,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<SimplicialMiniNavierStokesTrajectory2d, Diagnostic> {
        if unique_source_digest(program) != Some(self.source_digest)
            || program.model() != resolved.model()
            || program.revision().0 != resolved.semantic_revision().get()
        {
            return Err(invalid(
                "resolved transient Model or exact Geometry source differs from this binding",
            ));
        }
        self.accepted.revalidate()?;
        let replay = Self::new(program, self.accepted.clone())?;
        if replay.model != self.model || replay.mesh_reference != self.mesh_reference {
            return Err(invalid(
                "transient semantic or accepted Geometry--Mesh binding changed before execution",
            ));
        }
        let graph = resolved.portable_graph()?;
        let (scales, numerical_plan) =
            require_exact_transient_plan(&self.model, resolved, &graph, self.mesh_reference)?;
        let physical_mesh = self.accepted.mesh().mesh();
        if initial.velocity().mesh() != physical_mesh || initial.pressure().mesh() != physical_mesh
        {
            return Err(invalid(
                "transient initial state is stale for the bound exact Mesh",
            ));
        }
        let normalized = normalize_mesh(&self.model.bounds, physical_mesh, scales.length_value())?;
        let boundary =
            self.boundary
                .numerical_boundary(&self.model, &self.boundary_domains, &normalized)?;
        let numerical_initial = remesh_state(initial, normalized.clone())?;
        let block_system = super::block::transient_navier_stokes_block_system(
            program,
            &self.model,
            self.mesh_reference,
            &normalized,
            &boundary,
            resolved,
            scales,
        )?;
        let checked = block_system.checked_backend(assembly);
        let lower = [self.model.bounds[0][0], self.model.bounds[1][0]];
        let length = scales.length_value();
        let pressure = scales.pressure_value();
        let body_force = |coordinate_hat: [f64; DIMENSION]| {
            let physical = [
                lower[0] + length * coordinate_hat[0],
                lower[1] + length * coordinate_hat[1],
            ];
            let force = self.model.conservative_body_force(&physical)?;
            Ok([length * force[0] / pressure, length * force[1] / pressure])
        };
        let trajectory = advance_simplicial_mini_navier_stokes_2d_with_assembly(
            &normalized,
            &boundary,
            &|_| Ok([0.0; DIMENSION]),
            &body_force,
            numerical_initial,
            steps,
            numerical_plan,
            &eqiora_meshing::triangle_duffy_gauss_legendre(DUFFY_POINTS_PER_AXIS)?,
            &eqiora_meshing::simplex_duffy_gauss_legendre(DIMENSION - 1, 2)?,
            &checked,
            solver,
        )?;
        if checked.validated_materialization_count() == 0 {
            return Err(invalid(
                "transient execution returned without a validated block materialization",
            ));
        }
        Ok(trajectory)
    }
}

fn require_closed_model(
    program: &KernelProgram,
    model: &TransientIncompressibleNavierStokesModel2d,
    representation: eqiora_core::RawId,
    boundary: &semantic_boundary::LoweredNamedStokesBoundary2d,
) -> Result<(), Diagnostic> {
    let mut domains = BTreeSet::from([model.domain]);
    domains.extend(model.boundary_dispositions.keys().copied());
    domains.extend(boundary.connector_domains.iter().copied());
    let mut relations = BTreeSet::from([
        model.force_potential_definition,
        model.momentum_relation,
        model.incompressibility_relation,
    ]);
    relations.extend(
        model
            .boundary_relations
            .iter()
            .map(|entry| entry.relation()),
    );
    relations.extend(boundary.normal_velocity_definitions.iter().copied());
    let activations = program
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::Activates && relations.contains(&edge.to()))
        .map(|edge| edge.from())
        .collect::<BTreeSet<_>>();
    let parameters = parameters_referenced_by(program, &relations);
    let mut fields = BTreeSet::from([model.velocity, model.pressure, model.force_potential]);
    fields.extend(boundary.normal_velocity_fields.iter().copied());
    for node in program.nodes() {
        let admitted = match node {
            KernelNode::Domain(value) => domains.contains(&value.id().erase()),
            KernelNode::Representation(value) => value.id().erase() == representation,
            KernelNode::Field(value) => fields.contains(&value.id().erase()),
            KernelNode::Parameter(value) => parameters.contains(&value.id().erase()),
            KernelNode::Relation(value) => relations.contains(&value.id().erase()),
            KernelNode::Activation(value) => activations.contains(&value.id().erase()),
            KernelNode::Port(value) => boundary.ports.contains(&value.id().erase()),
            KernelNode::Connection(value) => boundary.connections.contains(&value.id().erase()),
            _ => false,
        };
        if !admitted {
            return Err(model_lowering_error(
                program,
                format!(
                    "closed geometry-backed transient lowering would ignore unexpected {:?} node {}",
                    node.kind(),
                    node.id()
                ),
            ));
        }
    }
    Ok(())
}

fn unique_source_digest(program: &KernelProgram) -> Option<[u8; 32]> {
    let mut digests = program.nodes().filter_map(|node| match node {
        KernelNode::Domain(domain) => match domain.kind() {
            DomainKind::GeometryRegion { geometry, .. } => Some(geometry.bytes()),
            _ => None,
        },
        _ => None,
    });
    let digest = digests.next()?;
    digests.next().is_none().then_some(digest)
}

fn normalize_mesh(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
    length: f64,
) -> Result<SimplicialMesh, Diagnostic> {
    if mesh.topological_dimension() != DIMENSION {
        return Err(invalid(
            "non-box transient execution requires an intrinsic 2D Mesh",
        ));
    }
    let vertices = mesh
        .vertices()
        .iter()
        .map(|coordinate| {
            if coordinate.len() != DIMENSION
                || coordinate
                    .iter()
                    .enumerate()
                    .any(|(axis, value)| *value < bounds[axis][0] || *value > bounds[axis][1])
            {
                return Err(invalid(
                    "non-box transient Mesh has a vertex outside the exact source bounds",
                ));
            }
            Ok(vec![
                (coordinate[0] - bounds[0][0]) / length,
                (coordinate[1] - bounds[1][0]) / length,
            ])
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    SimplicialMesh::new(
        DIMENSION,
        vertices,
        mesh.cells().to_vec(),
        mesh.quality_gate(),
    )
    .map_err(|error| invalid(error.message()))
}

fn remesh_state(
    state: SimplicialMiniNavierStokesState2d,
    mesh: SimplicialMesh,
) -> Result<SimplicialMiniNavierStokesState2d, Diagnostic> {
    SimplicialMiniNavierStokesState2d::new(
        state.time(),
        SimplicialMiniVelocityField2d::new(
            mesh.clone(),
            state.velocity().vertex_values().to_vec(),
            state.velocity().cell_bubble_values().to_vec(),
        )?,
        SimplicialP1Field::new(mesh, state.pressure().vertex_values().to_vec())?,
        state.pressure_reference(),
    )
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests;
