//! Failure-atomic solve and immutable accepted evidence.

use std::num::NonZeroUsize;

use eqiora_assembly::{AssemblyReport, CsrMatrix, LinearSystem};
use eqiora_core::Diagnostic;
use eqiora_meshing::VertexId;
use eqiora_solver::{
    CanonicalCsrSystemView, LinearOperatorProperties, LinearSolver, LinearSolverBackend,
    PreconditionerPolicy, ReductionPolicy, SolveReport, SolverPlan,
};

use super::assembly::AssembledPhysicalOperators;
use super::contract::{PrescribedDynamicSolidContract, invalid};

const DIMENSION: usize = 3;

/// One immutable accepted in-memory prescribed-displacement dynamic-solid step.
///
/// This value is numerical evidence only. It is not a durable State, Run, or
/// standalone-solid Realization artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedPrescribedDynamicSolidStep3d {
    generation: u64,
    displacement: Vec<(VertexId, [f64; DIMENSION])>,
    velocity: Vec<(VertexId, [f64; DIMENSION])>,
    acceleration: Vec<(VertexId, [f64; DIMENSION])>,
    constraint_reactions: Vec<(VertexId, [f64; DIMENSION])>,
    mass_operator: CsrMatrix,
    stiffness_operator: CsrMatrix,
    reduced_system: CanonicalCsrSystemView,
    free_momentum_residual_norm: f64,
    kinematic_residual_norm: f64,
    assembly_report: AssemblyReport,
    solve_report: SolveReport,
}

impl AcceptedPrescribedDynamicSolidStep3d {
    /// Generation accepted by this step.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Complete next total displacement in canonical vertex order.
    #[must_use]
    pub fn displacement(&self) -> &[(VertexId, [f64; DIMENSION])] {
        &self.displacement
    }

    /// Complete next velocity in canonical vertex order.
    #[must_use]
    pub fn velocity(&self) -> &[(VertexId, [f64; DIMENSION])] {
        &self.velocity
    }

    /// Complete next backward-Euler acceleration in canonical vertex order.
    #[must_use]
    pub fn acceleration(&self) -> &[(VertexId, [f64; DIMENSION])] {
        &self.acceleration
    }

    /// Complete vertex-aligned constraint-on-body reactions.
    ///
    /// Unconstrained components are represented by exact zero.
    #[must_use]
    pub fn constraint_reactions(&self) -> &[(VertexId, [f64; DIMENSION])] {
        &self.constraint_reactions
    }

    /// Density-inclusive physical consistent-mass operator.
    #[must_use]
    pub const fn mass_operator(&self) -> &CsrMatrix {
        &self.mass_operator
    }

    /// Physical infinitesimal-strain isotropic stiffness operator.
    #[must_use]
    pub const fn stiffness_operator(&self) -> &CsrMatrix {
        &self.stiffness_operator
    }

    /// Essential-constraint-eliminated backward-Euler displacement system.
    #[must_use]
    pub const fn reduced_system(&self) -> &CanonicalCsrSystemView {
        &self.reduced_system
    }

    /// Euclidean norm of the physical momentum residual on free components.
    #[must_use]
    pub const fn free_momentum_residual_norm(&self) -> f64 {
        self.free_momentum_residual_norm
    }

    /// Euclidean norm of the complete backward-Euler kinematic residual.
    #[must_use]
    pub const fn kinematic_residual_norm(&self) -> f64 {
        self.kinematic_residual_norm
    }

    /// Complete physical-operator assembly evidence.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    /// Complete serial-host linear-solve evidence.
    #[must_use]
    pub const fn solve_report(&self) -> &SolveReport {
        &self.solve_report
    }
}

pub(super) fn solve_and_accept(
    contract: &PrescribedDynamicSolidContract,
    generation: u64,
    driven_total_displacement: &[(VertexId, [f64; DIMENSION])],
    assembled: AssembledPhysicalOperators,
    solver: &dyn LinearSolverBackend,
) -> Result<AcceptedPrescribedDynamicSolidStep3d, Diagnostic> {
    let accepted_generation = generation
        .checked_add(1)
        .ok_or_else(|| invalid("prescribed dynamic-solid accepted generation overflows u64"))?;
    let time_step = contract.time_step();
    let inverse_time = 1.0 / time_step;
    let inverse_time_squared = inverse_time * inverse_time;
    let vertex_count = contract.mesh().vertices().len();
    let full_size = vertex_count * DIMENSION;

    let mut constrained_vertex = vec![false; vertex_count];
    for vertex in contract
        .fixed_vertices()
        .iter()
        .chain(contract.driven_vertices())
    {
        constrained_vertex[vertex.index()] = true;
    }

    let mut next_displacement = contract
        .prior_displacement()
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    for ((vertex, value), expected) in driven_total_displacement
        .iter()
        .zip(contract.driven_vertices())
    {
        debug_assert_eq!(vertex, expected);
        next_displacement[vertex.index()] = *value;
    }

    let previous_displacement = flatten(contract.prior_displacement());
    let previous_velocity = flatten(contract.prior_velocity());
    let inertial_state = previous_displacement
        .iter()
        .zip(&previous_velocity)
        .map(|(displacement, velocity)| {
            inverse_time_squared * displacement + inverse_time * velocity
        })
        .collect::<Vec<_>>();
    let full_rhs = assembled.mass().multiply(&inertial_state)?;

    let mut fixed_values = vec![None; full_size];
    let mut free_position = vec![None; full_size];
    let mut free_dofs = Vec::new();
    for vertex in 0..vertex_count {
        for (component, constrained_value) in next_displacement[vertex].iter().copied().enumerate()
        {
            let dof = vertex * DIMENSION + component;
            if constrained_vertex[vertex] {
                fixed_values[dof] = Some(constrained_value);
            } else {
                free_position[dof] = Some(free_dofs.len());
                free_dofs.push(dof);
            }
        }
    }
    if free_dofs.is_empty() {
        return Err(invalid(
            "prescribed dynamic-solid reference requires at least one free displacement component",
        ));
    }

    let reduced_linear_system = reduce_backward_euler_system(
        assembled.mass(),
        assembled.stiffness(),
        inverse_time_squared,
        &full_rhs,
        &fixed_values,
        &free_position,
        &free_dofs,
    )?;
    let reduced_system = CanonicalCsrSystemView::new(
        &reduced_linear_system,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?;
    let solved = solver.solve(&reduced_system.linear_problem()?, reference_solver_plan()?)?;
    if solved.values().len() != free_dofs.len() {
        return Err(invalid(
            "prescribed dynamic-solid solver result differs from the reduced displacement layout",
        ));
    }
    for (value, full_dof) in solved.values().iter().zip(&free_dofs) {
        next_displacement[*full_dof / DIMENSION][*full_dof % DIMENSION] = *value;
    }
    let (algebraic_displacement, solve_report) = solved.into_parts();

    let prior_displacement_values = contract
        .prior_displacement()
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let prior_velocity_values = contract
        .prior_velocity()
        .iter()
        .map(|(_, value)| *value)
        .collect::<Vec<_>>();
    let velocity = next_displacement
        .iter()
        .zip(&prior_displacement_values)
        .map(|(next, prior)| {
            std::array::from_fn(|component| inverse_time * (next[component] - prior[component]))
        })
        .collect::<Vec<[f64; DIMENSION]>>();
    let acceleration = velocity
        .iter()
        .zip(&prior_velocity_values)
        .map(|(next, prior)| {
            std::array::from_fn(|component| inverse_time * (next[component] - prior[component]))
        })
        .collect::<Vec<[f64; DIMENSION]>>();

    let flat_next_displacement = flatten_values(&next_displacement);
    let flat_acceleration = flatten_values(&acceleration);
    let mut momentum_residual = assembled.stiffness().multiply(&flat_next_displacement)?;
    let inertia = assembled.mass().multiply(&flat_acceleration)?;
    for (residual, inertia) in momentum_residual.iter_mut().zip(inertia) {
        *residual += inertia;
    }
    let free_momentum_residual_norm = norm(
        &free_dofs
            .iter()
            .map(|dof| momentum_residual[*dof])
            .collect::<Vec<_>>(),
    );
    let algebraic_scale =
        1.0 + norm(&algebraic_displacement) + norm(reduced_system.right_hand_side());
    let physical_residual_limit =
        solve_report.residual_target() + 16_384.0 * f64::EPSILON * algebraic_scale;
    if !free_momentum_residual_norm.is_finite()
        || free_momentum_residual_norm > physical_residual_limit
    {
        return Err(invalid(format!(
            "prescribed dynamic-solid free momentum residual {free_momentum_residual_norm:e} exceeds {physical_residual_limit:e}"
        )));
    }

    let kinematic_residual_norm = velocity
        .iter()
        .zip(&next_displacement)
        .zip(&prior_displacement_values)
        .flat_map(|((velocity, next), prior)| {
            (0..DIMENSION).map(move |component| {
                velocity[component] - inverse_time * (next[component] - prior[component])
            })
        })
        .map(|residual| residual * residual)
        .sum::<f64>()
        .sqrt();
    if !kinematic_residual_norm.is_finite() {
        return Err(invalid(
            "prescribed dynamic-solid kinematic residual is non-finite",
        ));
    }

    let constraint_reactions = (0..vertex_count)
        .map(|vertex| {
            let reaction = if constrained_vertex[vertex] {
                std::array::from_fn(|component| momentum_residual[vertex * DIMENSION + component])
            } else {
                [0.0; DIMENSION]
            };
            (VertexId::new(vertex), reaction)
        })
        .collect();
    let displacement = tagged(next_displacement);
    let velocity = tagged(velocity);
    let acceleration = tagged(acceleration);
    let (mass_operator, stiffness_operator, assembly_report) = assembled.into_parts();

    Ok(AcceptedPrescribedDynamicSolidStep3d {
        generation: accepted_generation,
        displacement,
        velocity,
        acceleration,
        constraint_reactions,
        mass_operator,
        stiffness_operator,
        reduced_system,
        free_momentum_residual_norm,
        kinematic_residual_norm,
        assembly_report,
        solve_report,
    })
}

#[allow(clippy::too_many_arguments)]
fn reduce_backward_euler_system(
    mass: &CsrMatrix,
    stiffness: &CsrMatrix,
    inverse_time_squared: f64,
    full_rhs: &[f64],
    fixed_values: &[Option<f64>],
    free_position: &[Option<usize>],
    free_dofs: &[usize],
) -> Result<LinearSystem, Diagnostic> {
    let free_size = free_dofs.len();
    let full_size = mass.rows();
    if mass.columns() != full_size
        || stiffness.rows() != full_size
        || stiffness.columns() != full_size
        || full_rhs.len() != full_size
        || fixed_values.len() != full_size
        || free_position.len() != full_size
    {
        return Err(invalid(
            "prescribed dynamic-solid physical operators and elimination layout differ in shape",
        ));
    }
    let mut row_offsets = Vec::with_capacity(free_size + 1);
    let mut column_indices = Vec::new();
    let mut values = Vec::new();
    let mut rhs = Vec::with_capacity(free_size);
    row_offsets.push(0);
    for full_row in free_dofs {
        let mut row_rhs = full_rhs[*full_row];
        for full_column in 0..full_size {
            let entry = stiffness
                .entry(*full_row, full_column)
                .expect("physical stiffness row/column is in range")
                + inverse_time_squared
                    * mass
                        .entry(*full_row, full_column)
                        .expect("physical mass row/column is in range");
            if entry == 0.0 {
                continue;
            }
            if let Some(reduced_column) = free_position[full_column] {
                column_indices.push(reduced_column);
                values.push(entry);
            } else if let Some(fixed) = fixed_values[full_column] {
                row_rhs -= entry * fixed;
            } else {
                return Err(invalid(
                    "prescribed dynamic-solid displacement component is neither free nor constrained",
                ));
            }
        }
        row_offsets.push(values.len());
        rhs.push(row_rhs);
    }
    LinearSystem::new(
        CsrMatrix::from_sorted_csr(free_size, free_size, row_offsets, column_indices, values)?,
        rhs,
    )
}

fn reference_solver_plan() -> Result<SolverPlan, Diagnostic> {
    Ok(SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-13,
        1.0e-15,
        NonZeroUsize::new(500).expect("positive frozen iteration budget"),
    )?
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Reproducible))
}

fn flatten(values: &[(VertexId, [f64; DIMENSION])]) -> Vec<f64> {
    values
        .iter()
        .flat_map(|(_, value)| value.iter().copied())
        .collect()
}

fn flatten_values(values: &[[f64; DIMENSION]]) -> Vec<f64> {
    values.iter().flatten().copied().collect()
}

fn tagged(values: Vec<[f64; DIMENSION]>) -> Vec<(VertexId, [f64; DIMENSION])> {
    values
        .into_iter()
        .enumerate()
        .map(|(vertex, value)| (VertexId::new(vertex), value))
        .collect()
}

fn norm(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}
