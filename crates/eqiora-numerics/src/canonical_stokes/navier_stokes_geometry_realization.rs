//! Crate-private exact-source non-box transient reference binding.
#![cfg_attr(not(test), allow(dead_code))]

mod boundary;
mod execution_owner;

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::AcceptedCircularHoleChordalRealizationV1;
use eqiora_assembly::AssemblyBackend;
use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, RawId};
use eqiora_graph::EdgeKind;
use eqiora_meshing::{MeshTopology, SimplicialMesh};
use eqiora_realization::{
    NonlinearSolvePlan, ResolvedTransientFieldwiseRealization, TransientFieldwiseRealizationPlan,
    TransientFieldwiseRealizationRequirements,
};
use eqiora_schema::kernel::{DomainKind, ExprDag, ExprId, ExprNode, KernelNode, SymbolRef};
use eqiora_sem::KernelProgram;
use eqiora_solver::{LinearSolverBackend, SolverPlan};

use self::boundary::CorrespondenceBoundary2d;
use super::IncompressibleFlowScaleProfile2d;
use super::boundary as semantic_boundary;
use super::expression::{IncompressibleStressForm, load_definition_root};
use super::inertial::parameters_referenced_by;
use super::navier_stokes::{
    TransientIncompressibleNavierStokesModel2d, lower_dfg_transient_volume, lower_transient_volume,
    require_boundary_volume,
};
use super::navier_stokes_realization::{
    require_exact_transient_plan, transient_navier_stokes_fieldwise_requirements_for_2d,
    transient_navier_stokes_mini_plan_for_2d,
};
use super::recognize::unique_circular_hole_domain;
use super::support::{lowering_error, model_lowering_error, relation_expression, unique_root};
use crate::simplicial_elliptic::SimplicialP1Field;
use crate::simplicial_navier_stokes::{
    SimplicialMiniNavierStokesState2d, SimplicialMiniNavierStokesTrajectory2d,
    advance_dfg_simplicial_mini_navier_stokes_2d_with_assembly,
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
        Self::new_with_stress(
            program,
            accepted,
            IncompressibleStressForm::SymmetricNewtonian,
        )
    }

    pub(crate) fn new_dfg(
        program: &KernelProgram,
        accepted: AcceptedCircularHoleChordalRealizationV1,
    ) -> Result<Self, Diagnostic> {
        Self::new_with_stress(program, accepted, IncompressibleStressForm::DfgNonsymmetric)
    }

    fn new_with_stress(
        program: &KernelProgram,
        accepted: AcceptedCircularHoleChordalRealizationV1,
        stress_form: IncompressibleStressForm,
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
        let volume = match stress_form {
            IncompressibleStressForm::SymmetricNewtonian => {
                lower_transient_volume::<DIMENSION>(program, domain)?
            }
            IncompressibleStressForm::DfgNonsymmetric => {
                lower_dfg_transient_volume::<DIMENSION>(program, domain)?
            }
        };
        debug_assert_eq!(volume.stress_form, stress_form);
        let lowered_boundary = semantic_boundary::lower_named_with_stress(
            program,
            domain,
            volume.velocity,
            volume.pressure,
            &volume.dynamic_viscosity,
            boundary_domains.clone(),
            stress_form,
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
            stress_form,
        };
        if stress_form == IncompressibleStressForm::DfgNonsymmetric {
            require_exact_dfg_tuple(program, &model, &lowered_boundary)?;
        }
        require_closed_model(program, &model, volume.representation, &lowered_boundary)?;
        boundary.require_dispositions(&model, &boundary_domains, stress_form)?;
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
        let (trajectory, _) = self.advance_with_stress(
            program,
            resolved,
            initial,
            steps,
            assembly,
            solver,
            IncompressibleStressForm::SymmetricNewtonian,
        )?;
        Ok(trajectory)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn advance_dfg_with_assembly(
        &self,
        program: &KernelProgram,
        resolved: &ResolvedTransientFieldwiseRealization,
        initial: SimplicialMiniNavierStokesState2d,
        steps: NonZeroStepCount,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
    ) -> Result<SimplicialMiniNavierStokesTrajectory2d, Diagnostic> {
        let execution = execution_owner::execute_dfg_with_assembly(
            self, program, resolved, initial, steps, assembly, solver,
        )?;
        Ok(execution.into_trajectory())
    }

    #[allow(clippy::too_many_arguments)]
    fn advance_with_stress(
        &self,
        program: &KernelProgram,
        resolved: &ResolvedTransientFieldwiseRealization,
        initial: SimplicialMiniNavierStokesState2d,
        steps: NonZeroStepCount,
        assembly: &dyn AssemblyBackend,
        solver: &dyn LinearSolverBackend,
        stress_form: IncompressibleStressForm,
    ) -> Result<
        (
            SimplicialMiniNavierStokesTrajectory2d,
            IncompressibleFlowScaleProfile2d,
        ),
        Diagnostic,
    > {
        if self.model.stress_form != stress_form {
            return Err(invalid(
                "transient numerical stress selection differs from the exact semantic binding",
            ));
        }
        if unique_source_digest(program) != Some(self.source_digest)
            || program.model() != resolved.model()
            || program.revision().0 != resolved.semantic_revision().get()
        {
            return Err(invalid(
                "resolved transient Model or exact Geometry source differs from this binding",
            ));
        }
        self.accepted.revalidate()?;
        let replay = Self::new_with_stress(program, self.accepted.clone(), stress_form)?;
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
        let prepared_boundary = self.boundary.numerical_boundary(
            &self.model,
            &self.boundary_domains,
            physical_mesh,
            &normalized,
            scales.velocity_value(),
        )?;
        let boundary = prepared_boundary.boundary();
        let numerical_initial = remesh_state(initial, normalized.clone())?;
        let block_system = super::block::transient_navier_stokes_block_system(
            program,
            &self.model,
            self.mesh_reference,
            &normalized,
            boundary,
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
        let cell_quadrature = eqiora_meshing::triangle_duffy_gauss_legendre(DUFFY_POINTS_PER_AXIS)?;
        let facet_quadrature = eqiora_meshing::simplex_duffy_gauss_legendre(DIMENSION - 1, 2)?;
        if stress_form == IncompressibleStressForm::DfgNonsymmetric {
            require_dfg_abstract_work(
                physical_mesh,
                &self.boundary,
                steps,
                numerical_plan,
                &cell_quadrature,
                &facet_quadrature,
            )?;
        }
        let essential_velocity =
            |coordinate| prepared_boundary.essential_velocity(&normalized, coordinate);
        let advance = match stress_form {
            IncompressibleStressForm::SymmetricNewtonian => {
                advance_simplicial_mini_navier_stokes_2d_with_assembly
            }
            IncompressibleStressForm::DfgNonsymmetric => {
                advance_dfg_simplicial_mini_navier_stokes_2d_with_assembly
            }
        };
        let trajectory = advance(
            &normalized,
            boundary,
            &essential_velocity,
            &body_force,
            numerical_initial,
            steps,
            numerical_plan,
            &cell_quadrature,
            &facet_quadrature,
            &checked,
            solver,
        )?;
        if checked.validated_materialization_count() == 0 {
            return Err(invalid(
                "transient execution returned without a validated block materialization",
            ));
        }
        Ok((trajectory, scales))
    }
}

fn require_exact_dfg_tuple(
    program: &KernelProgram,
    model: &TransientIncompressibleNavierStokesModel2d,
    boundary: &semantic_boundary::LoweredNamedStokesBoundary2d,
) -> Result<(), Diagnostic> {
    if model.bounds != [[0.0, 2.2], [0.0, 0.41]]
        || model.mass_density() != 1.0
        || model.dynamic_viscosity() != 0.001
        || model.force_potential_expression.constant_value() != Some(0.0)
    {
        return Err(lowering_error(
            model.domain,
            "private DFG binding requires the exact rho=1, mu=0.001, zero-force 2.2 by 0.41 tuple",
        ));
    }
    if boundary.normal_velocity_coefficients.len() != 1 {
        return Err(lowering_error(
            model.domain,
            "private DFG binding requires exactly one named inlet-profile coefficient",
        ));
    }
    let Some((profile, definition)) = boundary.normal_velocity_coefficients.get("inlet") else {
        return Err(lowering_error(
            model.domain,
            "private DFG binding requires its sole prescribed profile on `inlet`",
        ));
    };
    let expression = relation_expression(program, *definition)?;
    let root = unique_root(expression, *definition)?;
    let source = load_definition_root(expression, root, *profile).ok_or_else(|| {
        lowering_error(
            *definition,
            "DFG inlet profile requires one exact scalar definition",
        )
    })?;
    let (speed, height) = exact_dfg_profile_parameters(expression, source).ok_or_else(|| {
        lowering_error(
            *definition,
            "DFG inlet profile must be exactly `4 * Umax * y * (H - y) / H ^ 2`",
        )
    })?;
    let speed = parameter_value(program, expression, speed, *definition)?;
    let height = parameter_value(program, expression, height, *definition)?;
    if speed != 0.3 || height != 0.41 {
        return Err(lowering_error(
            *definition,
            "private DFG inlet requires exact Umax=0.3 and H=0.41 Parameter values",
        ));
    }
    let prescribed = boundary
        .normal_velocity_expressions
        .get("inlet")
        .ok_or_else(|| {
            lowering_error(
                model.domain,
                "private DFG binding requires its sole prescribed trace on `inlet`",
            )
        })?;
    // The boundary lowering admits both `trace(u) + normal(lift(s)) = 0`
    // (coefficient `-s`, so `u = -s n_parent`) and the sign-reversed
    // `trace(u) - normal(lift(s)) = 0` (coefficient `+s`, so `u = +s n_parent`).
    // The DFG inlet is exactly `u = -s n_parent = (s, 0)` on the parent-outward
    // normal `(-1, 0)`: interior-directed, positive-x. Require the lowered
    // coefficient to be exactly the negated profile; the reversed orientation
    // is a different physical law, not an admissible variant.
    let interior_directed =
        crate::spatial_expression::lower(program, expression, source, *definition, DIMENSION)?
            .multiply(
                crate::spatial_expression::ScalarSpatialExpression::constant(DIMENSION, -1.0),
            );
    if *prescribed != interior_directed {
        return Err(lowering_error(
            *definition,
            "private DFG inlet requires the exact interior-directed identity `trace(velocity) = -inlet_profile * parent_normal`",
        ));
    }
    Ok(())
}

fn exact_dfg_profile_parameters(expression: &ExprDag, source: ExprId) -> Option<(ExprId, ExprId)> {
    let ExprNode::Div(numerator, denominator) = expression.node(source)? else {
        return None;
    };
    let ExprNode::PowI(height_squared, 2) = expression.node(*denominator)? else {
        return None;
    };
    let height = parameter(expression, *height_squared)?;
    let mut factors = Vec::new();
    flatten_product(expression, *numerator, &mut factors);
    if factors.len() != 4 {
        return None;
    }
    let four = factors.iter().position(|factor| {
        matches!(
            expression.node(*factor),
            Some(ExprNode::Constant(value))
                if value.value() == 4.0 && value.dim() == DimExponents::DIMENSIONLESS
        )
    })?;
    let coordinate = factors.iter().position(|factor| {
        matches!(
            expression.node(*factor),
            Some(ExprNode::SpatialCoordinate(1))
        )
    })?;
    let difference = factors.iter().position(|factor| {
        let Some(ExprNode::Sub(left, right)) = expression.node(*factor) else {
            return false;
        };
        parameter(expression, *left) == Some(height)
            && matches!(
                expression.node(*right),
                Some(ExprNode::SpatialCoordinate(1))
            )
    })?;
    let speed_index = (0..factors.len()).find(|index| {
        ![four, coordinate, difference].contains(index)
            && parameter(expression, factors[*index]).is_some()
    })?;
    let speed = parameter(expression, factors[speed_index])?;
    (speed != height).then_some((factors[speed_index], *height_squared))
}

fn flatten_product(expression: &ExprDag, value: ExprId, factors: &mut Vec<ExprId>) {
    if let Some(ExprNode::Mul(left, right)) = expression.node(value) {
        flatten_product(expression, *left, factors);
        flatten_product(expression, *right, factors);
    } else {
        factors.push(value);
    }
}

fn parameter(expression: &ExprDag, value: ExprId) -> Option<RawId> {
    match expression.node(value) {
        Some(ExprNode::Symbol(SymbolRef::Parameter(parameter))) => Some(parameter.erase()),
        _ => None,
    }
}

fn parameter_value(
    program: &KernelProgram,
    expression: &ExprDag,
    value: ExprId,
    owner: RawId,
) -> Result<f64, Diagnostic> {
    crate::spatial_expression::lower(program, expression, value, owner, DIMENSION)?
        .constant_value()
        .ok_or_else(|| lowering_error(owner, "DFG profile Parameter is not a finite constant"))
}

fn require_dfg_abstract_work(
    mesh: &SimplicialMesh,
    boundary: &CorrespondenceBoundary2d,
    steps: NonZeroStepCount,
    plan: crate::simplicial_navier_stokes::MiniNavierStokesStepPlan2d,
    cell_quadrature: &eqiora_meshing::QuadratureRule,
    facet_quadrature: &eqiora_meshing::QuadratureRule,
) -> Result<(), Diagnostic> {
    let number = |value: usize, name: &str| {
        u64::try_from(value)
            .map_err(|_| invalid(format!("DFG {name} exceeds the abstract-work counter")))
    };
    let add = |left: u64, right: u64, name: &str| {
        left.checked_add(right)
            .ok_or_else(|| invalid(format!("DFG {name} overflows abstract work")))
    };
    let multiply = |left: u64, right: u64, name: &str| {
        left.checked_mul(right)
            .ok_or_else(|| invalid(format!("DFG {name} overflows abstract work")))
    };
    let vertices = number(mesh.vertices().len(), "vertex count")?;
    let cells = number(mesh.cells().len(), "cell count")?;
    let boundary_facets = number(boundary.boundary_facet_count(), "boundary-facet count")?;
    let outlet_facets = number(boundary.outlet_facet_count(), "outlet-facet count")?;
    let unknowns = add(
        multiply(3, vertices, "vertex coefficient count")?,
        multiply(2, cells, "bubble coefficient count")?,
        "coefficient count",
    )?;
    let _packets = add(cells, outlet_facets, "packet count")?;
    let audit = multiply(2, unknowns, "centered audit")?;
    let sparse_nnz = multiply(unknowns, unknowns, "structural nonzero bound")?;
    let line_trials = add(
        number(plan.maximum_line_search_steps(), "line-search count")?,
        1,
        "line-search trial count",
    )?;
    let nonlinear = multiply(
        number(plan.maximum_newton_iterations().get(), "Newton count")?,
        line_trials,
        "nonlinear factor",
    )?;
    let iteration_factor = add(
        nonlinear,
        add(audit, 1, "audit factor")?,
        "iteration factor",
    )?;
    let cell_work = multiply(
        cells,
        number(cell_quadrature.points().len(), "cell quadrature count")?,
        "cell work",
    )?;
    let facet_work = multiply(
        boundary_facets,
        number(facet_quadrature.points().len(), "facet quadrature count")?,
        "facet work",
    )?;
    let linear_work = multiply(
        number(
            plan.linear_solver().maximum_iterations().get(),
            "linear iteration count",
        )?,
        sparse_nnz,
        "linear work",
    )?;
    let spatial_work = add(
        add(cell_work, facet_work, "quadrature work")?,
        linear_work,
        "spatial work",
    )?;
    let _work = multiply(
        number(steps.get(), "step count")?,
        multiply(iteration_factor, spatial_work, "step work")?,
        "campaign work",
    )?;
    Ok(())
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
