use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyTarget,
    IndexedAssemblyWork, LocalContribution, LocalUnknown, TargetAssemblyMap,
};
use eqiora_core::Diagnostic;
use eqiora_meshing::{MeshEntity, MeshGeometry, MeshTopology, QuadratureRule, SimplicialMesh};
use eqiora_solver::{CanonicalCsrSystemView, LinearOperatorProperties};

use super::api::{MiniNavierStokesStepPlan2d, SimplicialMiniNavierStokesState2d};
use super::element::MiniNavierStokesCell;
use super::{COMPONENTS, DIMENSION, invalid};
use crate::simplicial_stokes::boundary::PressureReferenceKind2d;
use crate::simplicial_stokes::constraint::MiniPressureMeanConstraintCell;
use crate::simplicial_stokes::facet::MiniConstantTractionFacet;
use crate::simplicial_stokes::layout::MixedLayout;
use crate::{
    AssembledLinearizedRelation, LocalOperator, SimplicialMiniStokesBoundary2d,
    SimplicialMiniStokesPressureReference2d, SimplicialMiniVelocityField2d, SimplicialP1Field,
};

pub(super) struct StepAssembly {
    pub(super) relation: AssembledLinearizedRelation,
    pub(super) full_system: eqiora_assembly::LinearSystem,
    pub(super) residual: Vec<f64>,
    pub(super) full_residual: Vec<f64>,
    pub(super) layout: MixedLayout,
    pub(super) velocity: SimplicialMiniVelocityField2d,
    pub(super) pressure: SimplicialP1Field,
    pub(super) pressure_reference: SimplicialMiniStokesPressureReference2d,
    pub(super) gauge_multiplier: Option<f64>,
    pub(super) assembly_report: AssemblyReport,
}

impl StepAssembly {
    pub(super) fn residual_norm(&self) -> Result<f64, Diagnostic> {
        Ok(self
            .residual
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt())
    }

    pub(super) fn momentum_residual_norm(&self) -> f64 {
        self.residual[..self.layout.reduced_velocity_end()]
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt()
    }

    pub(super) fn residual(&self) -> &[f64] {
        &self.residual
    }

    pub(super) fn algebraic_values(&self) -> &[f64] {
        self.relation.accepted_unknowns()
    }
}

struct EvaluatedStepPacket {
    assembly: AssemblyPacket,
    residual: Vec<f64>,
}

pub(super) fn initial_point<B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    state: &SimplicialMiniNavierStokesState2d,
) -> Result<Vec<f64>, Diagnostic>
where
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    require_same_mesh(mesh, state)?;
    let prepared = boundary.prepare(mesh, essential_velocity)?;
    let with_gauge = prepared.pressure_reference == PressureReferenceKind2d::ZeroIntegral;
    require_pressure_policy(state, with_gauge)?;
    let layout = MixedLayout::new(mesh, &prepared.fixed_velocity, with_gauge)?;
    layout.initial_point(
        &prepared.fixed_velocity,
        state.velocity().vertex_values(),
        state.velocity().cell_bubble_values(),
        state.pressure().vertex_values(),
        state.pressure_reference().gauge_multiplier(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_step_linearization<F, B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    body_force: &F,
    previous: &SimplicialMiniNavierStokesState2d,
    candidate: &[f64],
    plan: MiniNavierStokesStepPlan2d,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
) -> Result<StepAssembly, Diagnostic>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    require_same_mesh(mesh, previous)?;
    let prepared = boundary.prepare(mesh, essential_velocity)?;
    let with_gauge = prepared.pressure_reference == PressureReferenceKind2d::ZeroIntegral;
    require_pressure_policy(previous, with_gauge)?;
    let layout = MixedLayout::new(mesh, &prepared.fixed_velocity, with_gauge)?;
    if candidate.len() != layout.reduced_size || candidate.iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "MINI Navier--Stokes candidate must be finite and match the exact mixed layout",
        ));
    }
    let (vertex_values, bubble_values, pressure_values, gauge_multiplier) =
        layout.reconstruct(candidate, &prepared.fixed_velocity)?;
    let velocity = SimplicialMiniVelocityField2d::new(mesh.clone(), vertex_values, bubble_values)?;
    let pressure = SimplicialP1Field::new(mesh.clone(), pressure_values)?;

    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("2D simplex mesh owns cells");
    let constraint_count = if with_gauge { cell_count } else { 0 };
    let constraint_end = cell_count
        .checked_add(constraint_count)
        .ok_or_else(|| invalid("transient MINI constraint packet count overflows usize"))?;
    let packet_count = constraint_end
        .checked_add(prepared.traction_facets.len())
        .ok_or_else(|| invalid("transient MINI packet count overflows usize"))?;
    let assembly_plan = AssemblyPlan::new(vec![
        AssemblyTarget::new(layout.reduced_size)?,
        AssemblyTarget::new(layout.full_size)?,
    ])?;
    let reduced_target = assembly_plan
        .target_id(0)
        .expect("two-target plan owns reduced target");
    let full_target = assembly_plan
        .target_id(1)
        .expect("two-target plan owns full target");
    let evaluate_packet = |packet| {
        if packet < cell_count {
            let cell = MeshEntity::new(DIMENSION, packet);
            let geometry = mesh
                .geometry_map(cell)
                .expect("accepted simplex cell owns geometry");
            let vertices = mesh
                .entity_vertices(cell)
                .expect("accepted simplex cell owns vertices");
            let linearization = MiniNavierStokesCell {
                cell: packet,
                vertices: &vertices,
                density: plan.density(),
                viscosity: plan.viscosity(),
                time_step: plan.time_step(),
                previous_velocity: previous.velocity(),
                candidate_velocity: &velocity,
                candidate_pressure: pressure.vertex_values(),
                body_force,
            }
            .linearize(&geometry, cell_quadrature)?;
            let residual = linearization.residual().to_vec();
            let local = linearization.into_linear_contribution()?;
            let reduced = layout.reduced_cell_map(packet, &vertices, &prepared.fixed_velocity)?;
            let full = layout.full_cell_map(packet, &vertices)?;
            Ok(EvaluatedStepPacket {
                assembly: AssemblyPacket::new(
                    local,
                    vec![
                        TargetAssemblyMap::new(reduced_target, reduced),
                        TargetAssemblyMap::new(full_target, full),
                    ],
                )?,
                residual,
            })
        } else if packet < constraint_end {
            let cell_index = packet - cell_count;
            let cell = MeshEntity::new(DIMENSION, cell_index);
            let geometry = mesh
                .geometry_map(cell)
                .expect("accepted simplex cell owns geometry");
            let vertices = mesh
                .entity_vertices(cell)
                .expect("accepted simplex cell owns vertices");
            let local = MiniPressureMeanConstraintCell.evaluate(&geometry, cell_quadrature)?;
            let reduced = layout.reduced_constraint_map(&vertices)?;
            let residual = evaluate_linear_residual(&local, &reduced, candidate)?;
            let full = layout.full_constraint_map(&vertices)?;
            Ok(EvaluatedStepPacket {
                assembly: AssemblyPacket::new(
                    local,
                    vec![
                        TargetAssemblyMap::new(reduced_target, reduced),
                        TargetAssemblyMap::new(full_target, full),
                    ],
                )?,
                residual,
            })
        } else {
            let facet = prepared.traction_facets[packet - constraint_end];
            let geometry = mesh
                .geometry_map(facet.facet)
                .expect("validated traction facet owns geometry");
            let vertices = mesh
                .entity_vertices(facet.facet)
                .expect("accepted boundary facet owns vertices");
            let local = MiniConstantTractionFacet {
                traction: facet.value,
            }
            .evaluate(&geometry, facet_quadrature)?;
            let reduced = layout.reduced_facet_map(&vertices, &prepared.fixed_velocity)?;
            let residual = evaluate_linear_residual(&local, &reduced, candidate)?;
            let full = layout.full_facet_map(&vertices)?;
            Ok(EvaluatedStepPacket {
                assembly: AssemblyPacket::new(
                    local,
                    vec![
                        TargetAssemblyMap::new(reduced_target, reduced),
                        TargetAssemblyMap::new(full_target, full),
                    ],
                )?,
                residual,
            })
        }
    };
    let work = IndexedAssemblyWork::new(packet_count, |packet| {
        evaluate_packet(packet).map(|evaluated: EvaluatedStepPacket| evaluated.assembly)
    });
    let (systems, assembly_report) = assembly.assemble(&assembly_plan, &work)?.into_parts();
    let mut residual = vec![0.0; layout.reduced_size];
    let mut full_residual = vec![0.0; layout.full_size];
    for packet in 0..packet_count {
        let evaluated = evaluate_packet(packet)?;
        for mapping in evaluated.assembly.mappings() {
            let output = match mapping.target().index() {
                0 => &mut residual,
                1 => &mut full_residual,
                _ => {
                    return Err(invalid(
                        "transient residual assembly encountered an unknown target",
                    ));
                }
            };
            scatter_residual(output, mapping.map(), &evaluated.residual)?;
        }
    }
    if residual
        .iter()
        .chain(&full_residual)
        .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "direct transient residual assembly produced a non-finite value",
        ));
    }
    let [linear_system, full_system]: [eqiora_assembly::LinearSystem; 2] =
        systems.try_into().map_err(|systems: Vec<_>| {
            invalid(format!(
                "two-target transient MINI assembly returned {} systems",
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
    let pressure_reference = if with_gauge {
        SimplicialMiniStokesPressureReference2d::ZeroIntegral {
            multiplier: gauge_multiplier.expect("gauged layout reconstructs a multiplier"),
        }
    } else {
        SimplicialMiniStokesPressureReference2d::BoundaryTraction
    };
    Ok(StepAssembly {
        relation,
        full_system,
        residual,
        full_residual,
        layout,
        velocity,
        pressure,
        pressure_reference,
        gauge_multiplier,
        assembly_report,
    })
}

fn evaluate_linear_residual(
    local: &LocalContribution,
    map: &AssemblyMap,
    global_point: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let local_point = map
        .unknowns()
        .iter()
        .map(|unknown| match unknown {
            LocalUnknown::Free(dof) => global_point.get(dof.index()).copied().ok_or_else(|| {
                invalid("local residual map references an unknown outside the candidate point")
            }),
            LocalUnknown::Fixed(value) => Ok(*value),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(local
        .matrix()
        .chunks_exact(local.columns())
        .zip(local.rhs())
        .map(|(row, rhs)| {
            row.iter()
                .zip(&local_point)
                .map(|(entry, value)| entry * value)
                .sum::<f64>()
                - rhs
        })
        .collect())
}

fn scatter_residual(
    output: &mut [f64],
    map: &AssemblyMap,
    local_residual: &[f64],
) -> Result<(), Diagnostic> {
    if map.equations().len() != local_residual.len() {
        return Err(invalid(
            "direct transient residual shape differs from its assembly map",
        ));
    }
    for (equation, value) in map.equations().iter().zip(local_residual) {
        if let Some(equation) = equation {
            let destination = output.get_mut(equation.index()).ok_or_else(|| {
                invalid("direct transient residual equation is outside its target")
            })?;
            *destination += value;
        }
    }
    Ok(())
}

fn require_same_mesh(
    mesh: &SimplicialMesh,
    previous: &SimplicialMiniNavierStokesState2d,
) -> Result<(), Diagnostic> {
    if previous.velocity().mesh() != mesh || previous.pressure().mesh() != mesh {
        return Err(invalid(
            "MINI Navier--Stokes fixed-domain step rejects stale or moving mesh state",
        ));
    }
    Ok(())
}

fn require_pressure_policy(
    previous: &SimplicialMiniNavierStokesState2d,
    with_gauge: bool,
) -> Result<(), Diagnostic> {
    let expected_gauge = matches!(
        previous.pressure_reference(),
        SimplicialMiniStokesPressureReference2d::ZeroIntegral { .. }
    );
    if expected_gauge != with_gauge {
        return Err(invalid(
            "MINI Navier--Stokes pressure closure differs from the shaped initial state",
        ));
    }
    Ok(())
}
