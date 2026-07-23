//! Monolithic reduced residual and exact ALE state Jacobian assembly.
//!
//! The algebraic candidate uses the unchanged dimensionless FSI quotient
//! layout.  Solid displacement and current geometry are not independent
//! unknowns: backward Euler derives the former from shared solid velocity and
//! the sealed harmonic action derives the latter.  Every Jacobian column
//! follows that same composition analytically.

use eqiora_core::Diagnostic;
use eqiora_meshing::FixedTopologyGeometryAction;
use eqiora_solver::{CanonicalCsrSystemView, LinearOperatorProperties};

use crate::simplicial_fsi::{
    element::solid_local, layout::FsiLayout, partition::CellMaterial, validate_problem,
};
use crate::{
    AffineGeometryLinearization, AssembledLinearizedRelation, AssemblyBackend, AssemblyMap,
    AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyTarget, DofId,
    FixedReferenceFsiPartition, FixedReferenceFsiState, IndexedAssemblyWork, LocalContribution,
    LocalUnknown, MeshEntity, MeshGeometry, MeshTopology, QuadratureRule, SimplicialMesh,
    TargetAssemblyMap,
};

use super::contract::{AleFsiBoundary, AleFsiState, AleFsiStepPlan};
use super::element::{AleMiniFluidCell, AleMiniFluidDirection};
use super::{P1HarmonicMeshMotion, invalid};

/// One assembled Newton point and the independently evaluated physical split.
pub(super) struct StepAssembly<const D: usize> {
    pub(super) relation: AssembledLinearizedRelation,
    pub(super) current: AleFsiState<D>,
    pub(super) geometry_action: FixedTopologyGeometryAction<D>,
    pub(super) residual: Vec<f64>,
    pub(super) full_fluid_residual: Vec<f64>,
    pub(super) full_solid_residual: Vec<f64>,
    pub(super) layout: FsiLayout<D>,
    pub(super) assembly_report: AssemblyReport,
}

impl<const D: usize> StepAssembly<D> {
    pub(super) fn residual_norm(&self) -> Result<f64, Diagnostic> {
        finite_norm(&self.residual, "ALE FSI reduced residual")
    }

    pub(super) fn residual(&self) -> &[f64] {
        &self.residual
    }

    pub(super) fn algebraic_values(&self) -> &[f64] {
        self.relation.accepted_unknowns()
    }

    pub(super) const fn current_state(&self) -> &AleFsiState<D> {
        &self.current
    }

    pub(super) const fn geometry_action(&self) -> &FixedTopologyGeometryAction<D> {
        &self.geometry_action
    }

    pub(super) fn full_fluid_residual(&self) -> &[f64] {
        &self.full_fluid_residual
    }

    pub(super) fn full_solid_residual(&self) -> &[f64] {
        &self.full_solid_residual
    }

    pub(super) const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }
}

/// Map one accepted state to the unchanged dimensionless quotient coordinates.
pub(super) fn initial_point<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
) -> Result<Vec<f64>, Diagnostic> {
    validate_inputs(
        reference, partition, boundary, motion, previous, plan, quadrature,
    )?;
    let layout = FsiLayout::new(reference, partition, boundary)?;
    let velocity_scale = plan.scale().velocity();
    let pressure_scale = plan.scale().pressure();
    let vertex_velocity = previous
        .vertex_velocity()
        .iter()
        .map(|value| value.map(|component| component / velocity_scale))
        .collect::<Vec<_>>();
    let bubbles = previous
        .fluid_cell_bubble_velocity()
        .iter()
        .map(|value| value.map(|component| component / velocity_scale))
        .collect::<Vec<_>>();
    let pressure = previous
        .fluid_pressure()
        .iter()
        .map(|value| value / pressure_scale)
        .collect::<Vec<_>>();
    layout.reduce(&vertex_velocity, &bubbles, &pressure)
}

/// Assemble the exact reduced Newton action at one dimensionless candidate.
///
/// Each cell packet owns its physical residual rows and a rectangular block
/// containing every reduced state column.  The assembly backend therefore
/// sees the same general sparse action used by the solver, while direct
/// residual assembly remains independent of `A x - b` reconstruction.
#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_step_linearization<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    candidate: &[f64],
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
) -> Result<StepAssembly<D>, Diagnostic> {
    let prepared = prepare_step(
        reference, partition, boundary, motion, previous, plan, quadrature, candidate,
    )?;
    let directions = build_directions(partition, motion, plan, &prepared.layout)?;
    let assembly_plan =
        AssemblyPlan::new(vec![AssemblyTarget::new(prepared.layout.reduced_size())?])?;
    let reduced_target = assembly_plan
        .target_id(0)
        .expect("one-target ALE FSI plan owns its reduced target");
    let evaluate = |cell_index| {
        evaluate_cell(
            cell_index,
            reference,
            partition,
            quadrature,
            previous,
            &prepared.previous_reference,
            &prepared.current,
            &prepared.geometry_action,
            plan,
            candidate,
            &directions,
            &prepared.layout,
            reduced_target,
        )
    };
    let work = IndexedAssemblyWork::new(prepared.cell_count, |cell_index| {
        evaluate(cell_index).map(|evaluated| evaluated.packet)
    });
    let (systems, assembly_report) = assembly.assemble(&assembly_plan, &work)?.into_parts();
    if assembly_report.packet_count() != prepared.cell_count || assembly_report.target_count() != 1
    {
        return Err(invalid(
            "ALE FSI assembly evidence differs from its exact cell and target inventory",
        ));
    }

    let direct = assemble_direct_residuals(
        reference, partition, previous, candidate, plan, quadrature, &prepared,
    )?;

    let [linear_system]: [crate::LinearSystem; 1] =
        systems.try_into().map_err(|systems: Vec<_>| {
            invalid(format!(
                "one-target ALE FSI assembly returned {} systems",
                systems.len()
            ))
        })?;
    let canonical = CanonicalCsrSystemView::new(&linear_system, LinearOperatorProperties::General)?;
    let relation = AssembledLinearizedRelation::from_canonical(
        canonical,
        candidate.to_vec(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )?;
    let mut reconstructed = zeroed(direct.reduced.len(), "captured relation residual")?;
    eqiora_ir::LinearizedRelation::primal(&relation, &mut reconstructed)?;
    require_same_residual(&direct.reduced, &reconstructed)?;

    Ok(StepAssembly {
        relation,
        current: prepared.current,
        geometry_action: prepared.geometry_action,
        residual: direct.reduced,
        full_fluid_residual: direct.full_fluid,
        full_solid_residual: direct.full_solid,
        layout: prepared.layout,
        assembly_report,
    })
}

/// Reassemble only the direct reduced residual at one dimensionless point.
///
/// This is the finite-difference path used to verify the analytic ALE state
/// Jacobian. It deliberately performs the identical validation, state and
/// geometry reconstruction, cell traversal, local primal evaluation, and
/// scatter as assemble_step_linearization, without constructing dense
/// directions, a Jacobian, assembly packets, or an assembly backend request.
#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_step_residual<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    candidate: &[f64],
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
) -> Result<Vec<f64>, Diagnostic> {
    let prepared = prepare_step(
        reference, partition, boundary, motion, previous, plan, quadrature, candidate,
    )?;
    Ok(assemble_direct_residuals(
        reference, partition, previous, candidate, plan, quadrature, &prepared,
    )?
    .reduced)
}

struct PreparedStep<const D: usize> {
    current: AleFsiState<D>,
    geometry_action: FixedTopologyGeometryAction<D>,
    previous_reference: FixedReferenceFsiState<D>,
    layout: FsiLayout<D>,
    cell_count: usize,
}

#[allow(clippy::too_many_arguments)]
fn prepare_step<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    candidate: &[f64],
) -> Result<PreparedStep<D>, Diagnostic> {
    validate_inputs(
        reference, partition, boundary, motion, previous, plan, quadrature,
    )?;
    let layout = FsiLayout::new(reference, partition, boundary)?;
    if candidate.len() != layout.reduced_size() || candidate.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(
            "ALE FSI candidate must be finite and match the exact reduced quotient layout",
        ));
    }
    let current = reconstruct_current_state(
        reference, partition, motion, previous, candidate, plan, &layout,
    )?;
    let geometry_action = plan.geometry_action(reference, partition, motion, previous, &current)?;
    let previous_reference = previous.to_fixed_reference_state(reference, partition)?;
    let cell_count = reference.entity_count(D).ok_or_else(|| {
        invalid(format!(
            "ALE FSI reference mesh omits its {D}D cell stratum"
        ))
    })?;
    if cell_count != partition.cell_count() {
        return Err(invalid(
            "ALE FSI material partition differs from the reference cell inventory",
        ));
    }
    Ok(PreparedStep {
        current,
        geometry_action,
        previous_reference,
        layout,
        cell_count,
    })
}

struct DirectResiduals {
    reduced: Vec<f64>,
    full_fluid: Vec<f64>,
    full_solid: Vec<f64>,
}

#[allow(clippy::too_many_arguments)]
fn assemble_direct_residuals<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    previous: &AleFsiState<D>,
    candidate: &[f64],
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    prepared: &PreparedStep<D>,
) -> Result<DirectResiduals, Diagnostic> {
    let mut reduced = zeroed(prepared.layout.reduced_size(), "reduced residual")?;
    let mut full_fluid = zeroed(prepared.layout.full_size(), "full fluid residual")?;
    let mut full_solid = zeroed(prepared.layout.full_size(), "full solid residual")?;
    for cell_index in 0..prepared.cell_count {
        let evaluated = evaluate_cell_residual(
            cell_index,
            reference,
            partition,
            quadrature,
            previous,
            &prepared.previous_reference,
            &prepared.current,
            &prepared.geometry_action,
            plan,
            candidate,
            &prepared.layout,
        )?;
        scatter_residual(
            &mut reduced,
            evaluated.reduced_map.equations(),
            &evaluated.residual,
        )?;
        let full = match evaluated.material {
            CellMaterial::Fluid => &mut full_fluid,
            CellMaterial::Solid => &mut full_solid,
            CellMaterial::Unassigned => {
                return Err(invalid(format!(
                    "ALE FSI cell {cell_index} has no material assignment"
                )));
            }
        };
        scatter_residual(full, evaluated.full_map.equations(), &evaluated.residual)?;
    }
    if reduced
        .iter()
        .chain(&full_fluid)
        .chain(&full_solid)
        .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "direct ALE FSI residual assembly produced a non-finite value",
        ));
    }
    Ok(DirectResiduals {
        reduced,
        full_fluid,
        full_solid,
    })
}

#[derive(Debug)]
struct AlgebraicDirection<const D: usize> {
    vertex_velocity: Vec<[f64; D]>,
    fluid_bubbles: Vec<[f64; D]>,
    pressure: Vec<f64>,
    coordinate: Vec<[f64; D]>,
}

struct EvaluatedCell {
    packet: AssemblyPacket,
}

struct EvaluatedCellResidual {
    residual: Vec<f64>,
    reduced_map: AssemblyMap,
    full_map: AssemblyMap,
    material: CellMaterial,
    source: CellResidualSource,
}

enum CellResidualSource {
    Fluid { fluid_position: usize },
    Solid(LocalContribution),
}

#[allow(clippy::too_many_arguments)]
fn evaluate_cell<const D: usize>(
    cell_index: usize,
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    quadrature: &QuadratureRule,
    previous: &AleFsiState<D>,
    previous_reference: &FixedReferenceFsiState<D>,
    current: &AleFsiState<D>,
    geometry_action: &FixedTopologyGeometryAction<D>,
    plan: AleFsiStepPlan<D>,
    candidate: &[f64],
    directions: &[AlgebraicDirection<D>],
    layout: &FsiLayout<D>,
    reduced_target: crate::AssemblyTargetId,
) -> Result<EvaluatedCell, Diagnostic> {
    let evaluated = evaluate_cell_residual(
        cell_index,
        reference,
        partition,
        quadrature,
        previous,
        previous_reference,
        current,
        geometry_action,
        plan,
        candidate,
        layout,
    )?;
    let matrix = match &evaluated.source {
        CellResidualSource::Fluid { fluid_position } => evaluate_fluid_jacobian(
            cell_index,
            *fluid_position,
            reference,
            partition,
            quadrature,
            previous,
            current,
            geometry_action,
            plan,
            candidate.len(),
            directions,
        )?,
        CellResidualSource::Solid(local) => {
            embed_solid_jacobian(local, &evaluated.reduced_map, candidate.len())?
        }
    };
    let rhs = affine_rhs_from_residual(&matrix, candidate, &evaluated.residual)?;
    let dense_map = AssemblyMap::new(
        evaluated.reduced_map.equations().to_vec(),
        (0..candidate.len())
            .map(|index| LocalUnknown::Free(DofId::new(index)))
            .collect(),
    )?;
    Ok(EvaluatedCell {
        packet: AssemblyPacket::new(
            LocalContribution::new(evaluated.residual.len(), candidate.len(), matrix, rhs)?,
            vec![TargetAssemblyMap::new(reduced_target, dense_map)],
        )?,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_cell_residual<const D: usize>(
    cell_index: usize,
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    quadrature: &QuadratureRule,
    previous: &AleFsiState<D>,
    previous_reference: &FixedReferenceFsiState<D>,
    current: &AleFsiState<D>,
    geometry_action: &FixedTopologyGeometryAction<D>,
    plan: AleFsiStepPlan<D>,
    candidate: &[f64],
    layout: &FsiLayout<D>,
) -> Result<EvaluatedCellResidual, Diagnostic> {
    let cell = MeshEntity::new(D, cell_index);
    let vertices = reference.entity_vertices(cell).ok_or_else(|| {
        invalid(format!(
            "ALE FSI cell packet {cell_index} has no reference vertex closure"
        ))
    })?;
    require_simplex_closure::<D>(&vertices, reference.vertices().len())?;
    let material = partition.material(cell_index);
    let (residual, reduced_map, full_map, source) = match material {
        CellMaterial::Fluid => evaluate_fluid_residual(
            cell_index,
            &vertices,
            partition,
            quadrature,
            previous,
            current,
            geometry_action,
            plan,
            layout,
        )?,
        CellMaterial::Solid => evaluate_solid_residual(
            cell_index,
            cell,
            &vertices,
            reference,
            partition,
            quadrature,
            previous_reference,
            plan,
            candidate,
            layout,
        )?,
        CellMaterial::Unassigned => {
            return Err(invalid(format!(
                "ALE FSI cell packet {cell_index} has no material assignment"
            )));
        }
    };
    if residual.len() != reduced_map.equations().len()
        || residual.len() != full_map.equations().len()
        || residual.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(format!(
            "{D}D ALE FSI cell residual differs from its exact equation closures"
        )));
    }
    Ok(EvaluatedCellResidual {
        residual,
        reduced_map,
        full_map,
        material,
        source,
    })
}

struct PreparedFluidCell<'a, const D: usize> {
    geometry: &'a eqiora_meshing::FixedTopologyCellGeometryAction<D>,
    previous_velocity: Vec<[f64; D]>,
    current_velocity: Vec<[f64; D]>,
    current_pressure: Vec<f64>,
}

impl<const D: usize> PreparedFluidCell<'_, D> {
    fn operator(&self, plan: AleFsiStepPlan<D>) -> AleMiniFluidCell<'_, D> {
        AleMiniFluidCell::<D> {
            geometry: self.geometry,
            density: plan.material().fluid_density(),
            viscosity: plan.material().fluid_dynamic_viscosity(),
            time_step: plan.time_step(),
            previous_velocity: &self.previous_velocity,
            current_velocity: &self.current_velocity,
            current_pressure: &self.current_pressure,
        }
    }
}

fn prepare_fluid_cell<'a, const D: usize>(
    cell_index: usize,
    vertices: &[MeshEntity],
    partition: &FixedReferenceFsiPartition<D>,
    previous: &AleFsiState<D>,
    current: &AleFsiState<D>,
    geometry_action: &'a FixedTopologyGeometryAction<D>,
) -> Result<(usize, PreparedFluidCell<'a, D>), Diagnostic> {
    let fluid_position = partition.fluid_position(cell_index).ok_or_else(|| {
        invalid(format!(
            "ALE FSI fluid cell {cell_index} has no canonical bubble position"
        ))
    })?;
    let previous_bubble = previous
        .fluid_cell_bubble_velocity()
        .get(fluid_position)
        .copied()
        .ok_or_else(|| invalid("ALE FSI previous fluid bubble inventory is incomplete"))?;
    let current_bubble = current
        .fluid_cell_bubble_velocity()
        .get(fluid_position)
        .copied()
        .ok_or_else(|| invalid("ALE FSI current fluid bubble inventory is incomplete"))?;
    let previous_velocity =
        local_velocity_coefficients(vertices, previous.vertex_velocity(), previous_bubble)?;
    let current_velocity =
        local_velocity_coefficients(vertices, current.vertex_velocity(), current_bubble)?;
    let current_pressure =
        local_pressure_coefficients(vertices, partition, current.fluid_pressure())?;
    let geometry = geometry_action.cell(cell_index).ok_or_else(|| {
        invalid(format!(
            "ALE FSI geometry action omits fluid cell {cell_index}"
        ))
    })?;
    Ok((
        fluid_position,
        PreparedFluidCell {
            geometry,
            previous_velocity,
            current_velocity,
            current_pressure,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_fluid_residual<const D: usize>(
    cell_index: usize,
    vertices: &[MeshEntity],
    partition: &FixedReferenceFsiPartition<D>,
    quadrature: &QuadratureRule,
    previous: &AleFsiState<D>,
    current: &AleFsiState<D>,
    geometry_action: &FixedTopologyGeometryAction<D>,
    plan: AleFsiStepPlan<D>,
    layout: &FsiLayout<D>,
) -> Result<(Vec<f64>, AssemblyMap, AssemblyMap, CellResidualSource), Diagnostic> {
    let (fluid_position, prepared) = prepare_fluid_cell(
        cell_index,
        vertices,
        partition,
        previous,
        current,
        geometry_action,
    )?;
    let stationary =
        AffineGeometryLinearization::stationary(prepared.geometry.current_map().clone())?;
    let zero_velocity = vec![[0.0; D]; D + 2];
    let zero_pressure = vec![0.0; D + 1];
    let primal = prepared.operator(plan).evaluate(
        AleMiniFluidDirection::<D> {
            current_velocity: &zero_velocity,
            current_pressure: &zero_pressure,
            current_geometry: &stationary,
        },
        quadrature,
    )?;
    let row_scales = fluid_row_scales(plan);
    if primal.residual().len() != row_scales.len() {
        return Err(invalid(format!(
            "{D}D ALE FSI fluid residual differs from its typed row-scale inventory"
        )));
    }
    let residual = primal
        .residual()
        .iter()
        .zip(row_scales.iter().copied())
        .map(|(value, scale)| value * scale)
        .collect::<Vec<_>>();
    if residual.iter().any(|value| !value.is_finite()) {
        return Err(invalid(format!(
            "{D}D ALE FSI fluid residual is non-finite after typed row scaling"
        )));
    }
    Ok((
        residual,
        layout.fluid_map(fluid_position, vertices, true)?,
        layout.fluid_map(fluid_position, vertices, false)?,
        CellResidualSource::Fluid { fluid_position },
    ))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_fluid_jacobian<const D: usize>(
    cell_index: usize,
    expected_fluid_position: usize,
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    quadrature: &QuadratureRule,
    previous: &AleFsiState<D>,
    current: &AleFsiState<D>,
    geometry_action: &FixedTopologyGeometryAction<D>,
    plan: AleFsiStepPlan<D>,
    candidate_width: usize,
    directions: &[AlgebraicDirection<D>],
) -> Result<Vec<f64>, Diagnostic> {
    if directions.len() != candidate_width {
        return Err(invalid(
            "ALE FSI analytic direction inventory differs from the candidate width",
        ));
    }
    let cell = MeshEntity::new(D, cell_index);
    let vertices = reference.entity_vertices(cell).ok_or_else(|| {
        invalid(format!(
            "ALE FSI fluid cell {cell_index} has no reference vertex closure"
        ))
    })?;
    require_simplex_closure::<D>(&vertices, reference.vertices().len())?;
    let (fluid_position, prepared) = prepare_fluid_cell(
        cell_index,
        &vertices,
        partition,
        previous,
        current,
        geometry_action,
    )?;
    if fluid_position != expected_fluid_position {
        return Err(invalid(
            "ALE FSI fluid position changed between residual and Jacobian evaluation",
        ));
    }
    let cell_operator = prepared.operator(plan);
    let row_scales = fluid_row_scales(plan);
    let entry_count = fluid_local_size::<D>()
        .checked_mul(candidate_width)
        .ok_or_else(|| invalid("ALE FSI fluid cell Jacobian shape overflows usize"))?;
    let mut matrix = zeroed(entry_count, "fluid cell Jacobian")?;
    for (column, direction) in directions.iter().enumerate() {
        let bubble_direction = direction
            .fluid_bubbles
            .get(fluid_position)
            .copied()
            .ok_or_else(|| invalid("ALE FSI fluid direction bubble inventory is incomplete"))?;
        let velocity_direction =
            local_velocity_coefficients(&vertices, &direction.vertex_velocity, bubble_direction)?;
        let pressure_direction =
            local_pressure_coefficients(&vertices, partition, &direction.pressure)?;
        if direction.coordinate.len() != reference.vertices().len() {
            return Err(invalid(
                "ALE FSI coordinate direction differs from the mesh vertex inventory",
            ));
        }
        let coordinate_direction = direction
            .coordinate
            .iter()
            .map(|value| value.to_vec())
            .collect::<Vec<_>>();
        let geometry_direction = geometry_action
            .current_mesh()
            .linearized_geometry_map(MeshEntity::new(D, cell_index), &coordinate_direction)?;
        let evaluated = cell_operator.evaluate(
            AleMiniFluidDirection::<D> {
                current_velocity: &velocity_direction,
                current_pressure: &pressure_direction,
                current_geometry: &geometry_direction,
            },
            quadrature,
        )?;
        if evaluated.jvp().len() != row_scales.len() {
            return Err(invalid(format!(
                "{D}D ALE FSI fluid JVP differs from its typed row-scale inventory"
            )));
        }
        for (row, (&value, scale)) in evaluated
            .jvp()
            .iter()
            .zip(row_scales.iter().copied())
            .enumerate()
        {
            matrix[row * candidate_width + column] = value * scale;
        }
    }
    if matrix.iter().any(|value| !value.is_finite()) {
        return Err(invalid("ALE FSI fluid Jacobian is non-finite"));
    }
    Ok(matrix)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_solid_residual<const D: usize>(
    cell_index: usize,
    cell: MeshEntity,
    vertices: &[MeshEntity],
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    quadrature: &QuadratureRule,
    previous: &FixedReferenceFsiState<D>,
    plan: AleFsiStepPlan<D>,
    candidate: &[f64],
    layout: &FsiLayout<D>,
) -> Result<(Vec<f64>, AssemblyMap, AssemblyMap, CellResidualSource), Diagnostic> {
    if partition.material(cell_index) != CellMaterial::Solid {
        return Err(invalid(
            "ALE FSI solid residual received a non-solid material packet",
        ));
    }
    let geometry = reference.geometry_map(cell).ok_or_else(|| {
        invalid(format!(
            "ALE FSI solid cell {cell_index} has no reference affine geometry"
        ))
    })?;
    let local = solid_local(
        &geometry,
        quadrature,
        plan.fixed_reference_config(),
        vertices,
        previous,
    )?;
    let reduced_map = layout.solid_map(vertices, true)?;
    let local_point = local_point(&reduced_map, candidate)?;
    let residual = evaluate_affine_residual(&local, &local_point)?;
    let full_map = layout.solid_map(vertices, false)?;
    Ok((
        residual,
        reduced_map,
        full_map,
        CellResidualSource::Solid(local),
    ))
}

fn embed_solid_jacobian(
    local: &LocalContribution,
    reduced_map: &AssemblyMap,
    candidate_width: usize,
) -> Result<Vec<f64>, Diagnostic> {
    if reduced_map.unknowns().len() != local.columns() {
        return Err(invalid(
            "ALE FSI solid local unknown closure differs from its matrix columns",
        ));
    }
    let entry_count = local
        .rows()
        .checked_mul(candidate_width)
        .ok_or_else(|| invalid("ALE FSI solid cell Jacobian shape overflows usize"))?;
    let mut matrix = zeroed(entry_count, "solid cell Jacobian")?;
    for (local_column, unknown) in reduced_map.unknowns().iter().enumerate() {
        if let LocalUnknown::Free(dof) = unknown {
            if dof.index() >= candidate_width {
                return Err(invalid(
                    "ALE FSI solid local unknown lies outside the candidate width",
                ));
            }
            for row in 0..local.rows() {
                matrix[row * candidate_width + dof.index()] +=
                    local.matrix()[row * local.columns() + local_column];
            }
        }
    }
    if matrix.iter().any(|value| !value.is_finite()) {
        return Err(invalid("ALE FSI solid Jacobian is non-finite"));
    }
    Ok(matrix)
}

fn reconstruct_current_state<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    candidate: &[f64],
    plan: AleFsiStepPlan<D>,
    layout: &FsiLayout<D>,
) -> Result<AleFsiState<D>, Diagnostic> {
    let (vertex_hat, bubbles_hat, pressure_hat) =
        layout.reconstruct(candidate, partition.fluid_cells().len())?;
    let velocity_scale = plan.scale().velocity();
    let pressure_scale = plan.scale().pressure();
    let vertex_velocity = vertex_hat
        .iter()
        .map(|value| value.map(|component| component * velocity_scale))
        .collect::<Vec<_>>();
    let bubbles = bubbles_hat
        .iter()
        .map(|value| value.map(|component| component * velocity_scale))
        .collect::<Vec<_>>();
    let pressure = pressure_hat
        .iter()
        .map(|value| value * pressure_scale)
        .collect::<Vec<_>>();
    let mut displacement = previous.solid_displacement().to_vec();
    for vertex in partition.solid_vertices() {
        for component in 0..D {
            displacement[vertex.index()][component] +=
                plan.time_step() * vertex_velocity[vertex.index()][component];
        }
    }
    AleFsiState::<D>::new(
        previous.time() + plan.time_step(),
        reference,
        partition,
        motion,
        vertex_velocity,
        bubbles,
        pressure,
        displacement,
    )
}

fn build_directions<const D: usize>(
    partition: &FixedReferenceFsiPartition<D>,
    motion: &P1HarmonicMeshMotion<D>,
    plan: AleFsiStepPlan<D>,
    layout: &FsiLayout<D>,
) -> Result<Vec<AlgebraicDirection<D>>, Diagnostic> {
    let dimension = layout.reduced_size();
    let mut directions = Vec::new();
    directions
        .try_reserve_exact(dimension)
        .map_err(|_| invalid("ALE FSI direction inventory allocation failed"))?;
    for column in 0..dimension {
        let mut basis = zeroed(dimension, "reduced basis direction")?;
        basis[column] = 1.0;
        let (vertex_hat, bubble_hat, pressure_hat) =
            layout.reconstruct(&basis, partition.fluid_cells().len())?;
        let vertex_velocity = vertex_hat
            .iter()
            .map(|value| value.map(|component| component * plan.scale().velocity()))
            .collect::<Vec<_>>();
        let fluid_bubbles = bubble_hat
            .iter()
            .map(|value| value.map(|component| component * plan.scale().velocity()))
            .collect::<Vec<_>>();
        let pressure = pressure_hat
            .iter()
            .map(|value| value * plan.scale().pressure())
            .collect::<Vec<_>>();
        let mut displacement = vec![[0.0; D]; vertex_velocity.len()];
        for vertex in partition.solid_vertices() {
            displacement[vertex.index()] =
                vertex_velocity[vertex.index()].map(|value| plan.time_step() * value);
        }
        let coordinate = motion.apply_jvp(&displacement)?;
        directions.push(AlgebraicDirection {
            vertex_velocity,
            fluid_bubbles,
            pressure,
            coordinate,
        });
    }
    Ok(directions)
}

fn validate_inputs<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotion<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    motion.validate_reference(reference, partition)?;
    previous.validate_against(reference, partition, motion)?;
    let previous_reference = previous.to_fixed_reference_state(reference, partition)?;
    validate_problem(
        reference,
        partition,
        boundary,
        &previous_reference,
        plan.fixed_reference_config(),
        quadrature,
    )?;
    let required_exactness = 3 * D + 2;
    if quadrature.polynomial_exactness().unwrap_or(0) < required_exactness {
        return Err(invalid(format!(
            "{D}D ALE FSI fluid action requires simplex quadrature exactness at least {required_exactness}"
        )));
    }
    Ok(())
}

fn local_velocity_coefficients<const D: usize>(
    vertices: &[MeshEntity],
    vertex_values: &[[f64; D]],
    bubble: [f64; D],
) -> Result<Vec<[f64; D]>, Diagnostic> {
    if vertices.len() != D + 1 {
        return Err(invalid(format!(
            "{D}D ALE FSI velocity closure must contain exactly {} vertices",
            D + 1
        )));
    }
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(D + 2)
        .map_err(|_| invalid("ALE FSI local velocity coefficient allocation failed"))?;
    for vertex in vertices {
        coefficients.push(
            vertex_values.get(vertex.index()).copied().ok_or_else(|| {
                invalid("ALE FSI velocity closure references a missing mesh vertex")
            })?,
        );
    }
    coefficients.push(bubble);
    Ok(coefficients)
}

fn local_pressure_coefficients<const D: usize>(
    vertices: &[MeshEntity],
    partition: &FixedReferenceFsiPartition<D>,
    pressure: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let values = vertices
        .iter()
        .map(|vertex| {
            let position = partition
                .fluid_vertices()
                .binary_search_by_key(&vertex.index(), |candidate| candidate.index())
                .map_err(|_| {
                    invalid("ALE FSI fluid cell vertex has no canonical pressure position")
                })?;
            pressure.get(position).copied().ok_or_else(|| {
                invalid("ALE FSI pressure field differs from its partition ordering")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != D + 1 {
        return Err(invalid(format!(
            "{D}D ALE FSI simplex pressure closure must contain {} vertices",
            D + 1
        )));
    }
    Ok(values)
}

fn fluid_row_scales<const D: usize>(plan: AleFsiStepPlan<D>) -> Vec<f64> {
    let scale = plan.scale();
    let power = scale.power();
    (0..fluid_local_size::<D>())
        .map(|row| {
            if row < fluid_pressure_offset::<D>() {
                scale.velocity() / power
            } else {
                scale.pressure() / power
            }
        })
        .collect()
}

const fn fluid_pressure_offset<const D: usize>() -> usize {
    (D + 2) * D
}

const fn fluid_local_size<const D: usize>() -> usize {
    fluid_pressure_offset::<D>() + D + 1
}

fn local_point(map: &AssemblyMap, candidate: &[f64]) -> Result<Vec<f64>, Diagnostic> {
    map.unknowns()
        .iter()
        .map(|unknown| match unknown {
            LocalUnknown::Free(dof) => candidate.get(dof.index()).copied().ok_or_else(|| {
                invalid("ALE FSI local map references an unknown outside the candidate")
            }),
            LocalUnknown::Fixed(value) => Ok(*value),
        })
        .collect()
}

fn require_simplex_closure<const D: usize>(
    vertices: &[MeshEntity],
    vertex_count: usize,
) -> Result<(), Diagnostic> {
    if vertices.len() != D + 1
        || vertices
            .iter()
            .any(|vertex| vertex.dimension() != 0 || vertex.index() >= vertex_count)
    {
        return Err(invalid(format!(
            "{D}D ALE FSI cell requires one exact {}-vertex simplex closure",
            D + 1
        )));
    }
    Ok(())
}

fn evaluate_affine_residual(
    local: &LocalContribution,
    point: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let entry_count = local
        .rows()
        .checked_mul(local.columns())
        .ok_or_else(|| invalid("ALE FSI affine local matrix shape overflows usize"))?;
    if local.columns() != point.len() || local.matrix().len() != entry_count {
        return Err(invalid(
            "ALE FSI affine residual point differs from its local matrix shape",
        ));
    }
    let residual = local
        .matrix()
        .chunks_exact(local.columns())
        .zip(local.rhs())
        .map(|(row, rhs)| {
            row.iter()
                .zip(point)
                .map(|(entry, value)| entry * value)
                .sum::<f64>()
                - rhs
        })
        .collect::<Vec<_>>();
    if residual.len() != local.rows() || residual.iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "ALE FSI affine local residual is non-finite or differs from its row closure",
        ));
    }
    Ok(residual)
}

fn affine_rhs_from_residual(
    matrix: &[f64],
    point: &[f64],
    residual: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let entry_count = residual
        .len()
        .checked_mul(point.len())
        .ok_or_else(|| invalid("ALE FSI dense local matrix shape overflows usize"))?;
    if matrix.len() != entry_count {
        return Err(invalid(
            "ALE FSI dense Jacobian differs from its residual and candidate shape",
        ));
    }
    let rhs = matrix
        .chunks_exact(point.len())
        .zip(residual)
        .map(|(row, residual)| {
            row.iter()
                .zip(point)
                .map(|(entry, value)| entry * value)
                .sum::<f64>()
                - residual
        })
        .collect::<Vec<_>>();
    if rhs.len() != residual.len() || rhs.iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "ALE FSI captured local right-hand side is non-finite or has the wrong row shape",
        ));
    }
    Ok(rhs)
}

fn scatter_residual(
    output: &mut [f64],
    equations: &[Option<DofId>],
    local: &[f64],
) -> Result<(), Diagnostic> {
    if equations.len() != local.len() {
        return Err(invalid(
            "ALE FSI direct residual shape differs from its equation map",
        ));
    }
    for (equation, value) in equations.iter().zip(local) {
        if let Some(equation) = equation {
            let destination = output.get_mut(equation.index()).ok_or_else(|| {
                invalid("ALE FSI residual equation is outside its assembly target")
            })?;
            *destination += value;
        }
    }
    Ok(())
}

fn require_same_residual(direct: &[f64], reconstructed: &[f64]) -> Result<(), Diagnostic> {
    if direct.len() != reconstructed.len() {
        return Err(invalid(
            "captured ALE FSI relation residual shape differs from direct assembly",
        ));
    }
    let scale = direct
        .iter()
        .chain(reconstructed)
        .fold(1.0_f64, |scale, value| scale.max(value.abs()));
    let defect = direct
        .iter()
        .zip(reconstructed)
        .fold(0.0_f64, |defect, (direct, reconstructed)| {
            defect.max((direct - reconstructed).abs())
        });
    if defect > 65_536.0 * f64::EPSILON * scale {
        return Err(invalid(
            "captured ALE FSI relation does not reproduce its independently assembled residual",
        ));
    }
    Ok(())
}

fn finite_norm(values: &[f64], name: &'static str) -> Result<f64, Diagnostic> {
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !norm.is_finite() {
        return Err(invalid(format!("{name} norm is non-finite")));
    }
    Ok(norm)
}

fn zeroed(length: usize, name: &'static str) -> Result<Vec<f64>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| invalid(format!("ALE FSI {name} allocation failed")))?;
    values.resize(length, 0.0);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_ir::{LinearizedRelation, RelationTangent};
    use eqiora_realization::{NonlinearSolvePlan, Target};
    use eqiora_solver::{
        LinearSolveRequest, LinearSolver, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
        ReductionPolicy, SolverPlan,
    };

    use crate::{
        AleFsiBoundary2d, AleFsiBoundary3d, AleFsiState2d, AleFsiState3d, AleFsiStepPlan2d,
        AleFsiStepPlan3d, CellId, FacetId, FixedReferenceFsiLoad2d, FixedReferenceFsiLoad3d,
        FixedReferenceFsiMaterial2d, FixedReferenceFsiMaterial3d, FixedReferenceFsiPartition2d,
        FixedReferenceFsiPartition3d, FixedReferenceFsiScale2d, FixedReferenceFsiScale3d,
        MeshQualityGate, P1HarmonicMeshMotion2d, P1HarmonicMeshMotion3d,
        REFERENCE_ASSEMBLY_BACKEND, simplex_duffy_gauss_legendre, triangle_duffy_gauss_legendre,
    };

    use super::*;

    const COMPONENTS: usize = 2;

    struct Fixture {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition2d,
        boundary: AleFsiBoundary2d,
        motion: P1HarmonicMeshMotion2d,
        previous: AleFsiState2d,
        plan: AleFsiStepPlan2d,
    }

    struct Fixture3d {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition3d,
        boundary: AleFsiBoundary3d,
        motion: P1HarmonicMeshMotion3d,
        previous: AleFsiState3d,
        plan: AleFsiStepPlan3d,
    }

    #[test]
    fn analytic_global_jvp_matches_centered_full_reassembly() {
        let fixture = fixture();
        let quadrature = triangle_duffy_gauss_legendre(5).unwrap();
        let mut point = initial_point(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            fixture.plan,
            &quadrature,
        )
        .unwrap();
        for (index, value) in point.iter_mut().enumerate() {
            *value = 2.0e-3 * ((index % 7) as f64 - 3.0);
        }
        let direction = (0..point.len())
            .map(|index| 0.1 * ((index % 5) as f64 - 2.0))
            .collect::<Vec<_>>();
        let assembled = assemble(&fixture, &point, &quadrature);
        let residual_only = residual(&fixture, &point, &quadrature);
        assert_eq!(
            residual_only
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            assembled
                .residual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        let mut captured_primal = vec![0.0; point.len()];
        assembled.relation.primal(&mut captured_primal).unwrap();
        let primal_defect = captured_primal
            .iter()
            .zip(&assembled.residual)
            .map(|(captured, direct)| (captured - direct).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(primal_defect < 1.0e-12, "{primal_defect:e}");
        let mut analytic = vec![0.0; point.len()];
        assembled
            .relation
            .jvp(RelationTangent::Unknown(&direction), &mut analytic)
            .unwrap();

        let epsilon = f64::EPSILON.cbrt();
        let shifted = |sign: f64| {
            point
                .iter()
                .zip(&direction)
                .map(|(point, direction)| point + sign * epsilon * direction)
                .collect::<Vec<_>>()
        };
        let plus = residual(&fixture, &shifted(1.0), &quadrature);
        let minus = residual(&fixture, &shifted(-1.0), &quadrature);
        let centered = plus
            .iter()
            .zip(minus)
            .map(|(plus, minus)| (plus - minus) / (2.0 * epsilon))
            .collect::<Vec<_>>();
        let error = centered
            .iter()
            .zip(&analytic)
            .map(|(centered, analytic)| (centered - analytic).powi(2))
            .sum::<f64>()
            .sqrt();
        let scale = analytic
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        assert!(error < 2.0e-6 * (1.0 + scale), "{error:e} versus {scale:e}");
        assert_eq!(
            assembled.assembly_report.packet_count(),
            fixture.partition.cell_count()
        );
        assert_eq!(assembled.assembly_report.target_count(), 1);
    }

    #[test]
    fn zero_solid_update_produces_an_exact_static_geometry_action() {
        let fixture = fixture();
        let quadrature = triangle_duffy_gauss_legendre(5).unwrap();
        let point = initial_point(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            fixture.plan,
            &quadrature,
        )
        .unwrap();
        let assembled = assemble(&fixture, &point, &quadrature);
        assert_eq!(
            assembled.current_state().geometry(),
            fixture.previous.geometry()
        );
        assert!(
            assembled
                .geometry_action()
                .vertex_velocities()
                .iter()
                .flatten()
                .all(|value| *value == 0.0)
        );
        for cell in assembled.geometry_action().cells() {
            assert_eq!(cell.previous_map(), cell.current_map());
            assert_eq!(cell.current_velocity_divergence(), 0.0);
            assert_eq!(cell.skew_gcl_correction(), 0.0);
            assert_eq!(cell.endpoint_metric_rate(), 0.0);
        }
    }

    #[test]
    fn degree_six_rule_is_rejected_before_ale_assembly() {
        let fixture = fixture();
        assert!(
            initial_point(
                &fixture.mesh,
                &fixture.partition,
                &fixture.boundary,
                &fixture.motion,
                &fixture.previous,
                fixture.plan,
                &triangle_duffy_gauss_legendre(4).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn residual_only_rejects_nonfinite_and_wrong_shape_candidates() {
        let fixture = fixture();
        let quadrature = triangle_duffy_gauss_legendre(5).unwrap();
        let point = initial_point(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            fixture.plan,
            &quadrature,
        )
        .unwrap();
        let mut short = point.clone();
        short.pop();
        let shape_error = assemble_step_residual(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            &short,
            fixture.plan,
            &quadrature,
        )
        .unwrap_err();
        assert!(
            shape_error
                .message()
                .contains("exact reduced quotient layout")
        );

        let mut nonfinite = point;
        nonfinite[0] = f64::NAN;
        let finite_error = assemble_step_residual(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            &nonfinite,
            fixture.plan,
            &quadrature,
        )
        .unwrap_err();
        assert!(finite_error.message().contains("finite"));
    }

    #[test]
    fn tetrahedral_assembly_has_typed_power_exactness_and_centered_jvp() {
        let fixture = fixture_3d();
        let degree_nine = simplex_duffy_gauss_legendre(3, 6).unwrap();
        let rejected = initial_point(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            fixture.plan,
            &degree_nine,
        )
        .unwrap_err();
        assert!(rejected.message().contains("at least 11"));

        let quadrature = simplex_duffy_gauss_legendre(3, 7).unwrap();
        let mut point = initial_point(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            fixture.plan,
            &quadrature,
        )
        .unwrap();
        for (index, value) in point.iter_mut().enumerate() {
            *value = 2.0e-4 * ((index % 7) as f64 - 3.0);
        }
        let assembled = assemble_step_linearization(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            &point,
            fixture.plan,
            &quadrature,
            &REFERENCE_ASSEMBLY_BACKEND,
        )
        .unwrap();
        let residual_only = assemble_step_residual(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            &point,
            fixture.plan,
            &quadrature,
        )
        .unwrap();
        assert_eq!(
            residual_only
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            assembled
                .residual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        let direction = (0..point.len())
            .map(|index| 0.01 * ((index % 5) as f64 - 2.0))
            .collect::<Vec<_>>();
        let mut jvp = vec![0.0; point.len()];
        assembled
            .relation
            .jvp(RelationTangent::Unknown(&direction), &mut jvp)
            .unwrap();
        assert!(jvp.iter().all(|value| value.is_finite()));
        assert!(jvp.iter().any(|value| *value != 0.0));
        assert!(
            assembled
                .geometry_action()
                .vertex_velocities()
                .iter()
                .flatten()
                .any(|value| *value != 0.0)
        );

        let epsilon = f64::EPSILON.cbrt();
        let shifted_residual = |sign: f64| {
            let shifted = point
                .iter()
                .zip(&direction)
                .map(|(point, direction)| point + sign * epsilon * direction)
                .collect::<Vec<_>>();
            assemble_step_residual(
                &fixture.mesh,
                &fixture.partition,
                &fixture.boundary,
                &fixture.motion,
                &fixture.previous,
                &shifted,
                fixture.plan,
                &quadrature,
            )
            .unwrap()
        };
        let plus = shifted_residual(1.0);
        let minus = shifted_residual(-1.0);
        let centered = plus
            .iter()
            .zip(minus)
            .map(|(plus, minus)| (plus - minus) / (2.0 * epsilon))
            .collect::<Vec<_>>();
        let error = centered
            .iter()
            .zip(&jvp)
            .map(|(centered, analytic)| (centered - analytic).powi(2))
            .sum::<f64>()
            .sqrt();
        let scale = jvp.iter().map(|value| value * value).sum::<f64>().sqrt();
        assert!(error < 5.0e-6 * (1.0 + scale), "{error:e} versus {scale:e}");
        assert_eq!(
            assembled.assembly_report.packet_count(),
            fixture.partition.cell_count()
        );
        assert_eq!(
            assembled.geometry_action().cells().len(),
            fixture.mesh.cells().len()
        );

        let row_scales = fluid_row_scales(fixture.plan);
        assert_eq!(fixture.plan.scale().power(), 60.0);
        assert_eq!(row_scales.len(), 19);
        assert!(row_scales[..15].iter().all(|value| *value == 5.0 / 60.0));
        assert!(row_scales[15..].iter().all(|value| *value == 3.0 / 60.0));
    }

    fn assemble(fixture: &Fixture, point: &[f64], quadrature: &QuadratureRule) -> StepAssembly<2> {
        assemble_step_linearization(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            point,
            fixture.plan,
            quadrature,
            &REFERENCE_ASSEMBLY_BACKEND,
        )
        .unwrap()
    }

    fn residual(fixture: &Fixture, point: &[f64], quadrature: &QuadratureRule) -> Vec<f64> {
        assemble_step_residual(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            &fixture.previous,
            point,
            fixture.plan,
            quadrature,
        )
        .unwrap()
    }

    fn fixture() -> Fixture {
        let mesh = two_domain_mesh();
        let (fluid, solid, interface) = inventories(&mesh);
        let partition = FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
        let boundary = AleFsiBoundary2d::homogeneous_exterior(&mesh).unwrap();
        let motion = P1HarmonicMeshMotion2d::new(&mesh, &partition, harmonic_solver()).unwrap();
        let previous = AleFsiState2d::new(
            0.0,
            &mesh,
            &partition,
            &motion,
            vec![[0.0; COMPONENTS]; mesh.vertices().len()],
            vec![[0.0; COMPONENTS]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            vec![[0.0; COMPONENTS]; mesh.vertices().len()],
        )
        .unwrap();
        Fixture {
            mesh,
            partition,
            boundary,
            motion,
            previous,
            plan: step_plan(),
        }
    }

    fn fixture_3d() -> Fixture3d {
        let (mesh, fluid, solid, interface) = tetrahedral_problem();
        let partition = FixedReferenceFsiPartition3d::new(&mesh, fluid, solid, interface).unwrap();
        let boundary = AleFsiBoundary3d::homogeneous_exterior(&mesh).unwrap();
        let motion = P1HarmonicMeshMotion3d::new(&mesh, &partition, harmonic_solver()).unwrap();
        let previous = AleFsiState3d::new(
            0.0,
            &mesh,
            &partition,
            &motion,
            vec![[0.0; 3]; mesh.vertices().len()],
            vec![[0.0; 3]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            vec![[0.0; 3]; mesh.vertices().len()],
        )
        .unwrap();
        Fixture3d {
            mesh,
            partition,
            boundary,
            motion,
            previous,
            plan: step_plan_3d(),
        }
    }

    fn tetrahedral_problem() -> (SimplicialMesh, Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
        // A, B, C span the material interface. I is a genuine fluid-interior
        // vertex and Q is a genuine interface-interior vertex. The subdivision
        // is the smallest conforming patch that exercises both harmonic
        // extension and nonzero shared-interface motion.
        let vertices = vec![
            vec![0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![-1.0, 0.0, 0.0],
            vec![-0.25, 0.25, 0.25],
            vec![0.0, 1.0 / 3.0, 1.0 / 3.0],
            vec![1.0, 0.0, 0.0],
        ];
        let mut cells = vec![
            vec![4, 5, 0, 1],
            vec![4, 5, 1, 2],
            vec![4, 5, 2, 0],
            vec![4, 3, 1, 2],
            vec![4, 3, 2, 0],
            vec![4, 3, 0, 1],
            vec![6, 5, 0, 1],
            vec![6, 5, 1, 2],
            vec![6, 5, 2, 0],
        ];
        for cell in &mut cells {
            if signed_tetrahedron_measure(&vertices, cell) < 0.0 {
                cell.swap(1, 2);
            }
        }
        let fluid = (0..6).map(CellId::new).collect();
        let solid = (6..9).map(CellId::new).collect();
        let mesh =
            SimplicialMesh::new(3, vertices, cells, MeshQualityGate::new(0.005).unwrap()).unwrap();
        let interface = (0..mesh.entity_count(2).unwrap())
            .filter(|&facet| {
                mesh.entity_vertices(MeshEntity::new(2, facet))
                    .unwrap()
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 0.0)
            })
            .map(FacetId::new)
            .collect();
        (mesh, fluid, solid, interface)
    }

    fn signed_tetrahedron_measure(vertices: &[Vec<f64>], cell: &[usize]) -> f64 {
        let origin = &vertices[cell[0]];
        let column = |vertex: usize, axis: usize| vertices[cell[vertex]][axis] - origin[axis];
        column(1, 0) * (column(2, 1) * column(3, 2) - column(3, 1) * column(2, 2))
            - column(2, 0) * (column(1, 1) * column(3, 2) - column(3, 1) * column(1, 2))
            + column(3, 0) * (column(1, 1) * column(2, 2) - column(2, 1) * column(1, 2))
    }

    fn two_domain_mesh() -> SimplicialMesh {
        let x_coordinates = [0.0, 0.5, 1.0, 1.5, 2.0];
        let mut vertices = Vec::new();
        for y in [0.0, 0.5, 1.0] {
            for x in x_coordinates {
                vertices.push(vec![x, y]);
            }
        }
        let width = x_coordinates.len();
        let mut cells = Vec::new();
        for row in 0..2 {
            for column in 0..width - 1 {
                let lower_left = row * width + column;
                let lower_right = lower_left + 1;
                let upper_left = lower_left + width;
                let upper_right = upper_left + 1;
                cells.push(vec![lower_left, lower_right, upper_right]);
                cells.push(vec![lower_left, upper_right, upper_left]);
            }
        }
        SimplicialMesh::new(2, vertices, cells, MeshQualityGate::new(0.3).unwrap()).unwrap()
    }

    fn inventories(mesh: &SimplicialMesh) -> (Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
        let mut fluid = Vec::new();
        let mut solid = Vec::new();
        for (index, cell) in mesh.cells().iter().enumerate() {
            let centroid_x = cell
                .iter()
                .map(|vertex| mesh.vertices()[*vertex][0])
                .sum::<f64>()
                / 3.0;
            if centroid_x < 1.0 {
                fluid.push(CellId::new(index));
            } else {
                solid.push(CellId::new(index));
            }
        }
        let interface = (0..mesh.entity_count(1).unwrap())
            .filter(|&facet| {
                mesh.entity_vertices(MeshEntity::new(1, facet))
                    .unwrap()
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
            })
            .map(FacetId::new)
            .collect();
        (fluid, solid, interface)
    }

    fn step_plan() -> AleFsiStepPlan2d {
        let nonlinear =
            NonlinearSolvePlan::new(1.0e-9, 1.0e-12, NonZeroUsize::new(20).unwrap(), 12).unwrap();
        let linear = SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Fast);
        AleFsiStepPlan2d::new(
            0.05,
            FixedReferenceFsiMaterial2d::new(1.0, 0.1, 1.0, 2.0, 1.0).unwrap(),
            FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap(),
            FixedReferenceFsiLoad2d::Zero,
            nonlinear,
            linear,
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap()
    }

    fn step_plan_3d() -> AleFsiStepPlan3d {
        let nonlinear =
            NonlinearSolvePlan::new(1.0e-9, 1.0e-12, NonZeroUsize::new(20).unwrap(), 12).unwrap();
        let linear = SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Fast);
        AleFsiStepPlan3d::new(
            0.05,
            FixedReferenceFsiMaterial3d::new(1.0, 0.1, 1.0, 2.0, 1.0).unwrap(),
            FixedReferenceFsiScale3d::new(2.0, 5.0, 3.0).unwrap(),
            FixedReferenceFsiLoad3d::Zero,
            nonlinear,
            linear,
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap()
    }

    fn harmonic_solver() -> LinearSolveRequest<'static> {
        let plan = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap();
        LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, plan)
    }
}
