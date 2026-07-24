use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::Diagnostic;
use eqiora_ir::{LinearizedRelation, RelationTangent};
use eqiora_meshing::{QuadratureRule, SimplicialMesh};
use eqiora_solver::{LinearProblem, LinearSolverBackend};

use super::acceptance::{NewtonEvidence, accept_step, require_consistent_initial_state};
use super::api::{
    MiniNavierStokesStepPlan2d, SimplicialMiniNavierStokesState2d,
    SimplicialMiniNavierStokesTrajectory2d,
};
use super::assembly::{
    assemble_step_linearization, assemble_step_residual, build_step_jacobian_pattern,
};
use super::{COMPONENTS, DIMENSION, solve_failed};
use crate::jacobian_audit::{
    CenteredJacobianAuditEvidence, StructuralJacobianPattern, audit_centered_jacobian,
};
use crate::simplicial_stokes::SimplicialMiniStokesBoundary2d;
use crate::step_count::NonZeroStepCount;

/// Advance a fixed mesh through one or more accepted implicit steps with the
/// deterministic reference assembler.
///
/// # Errors
/// Fails closed on initial-state/mesh/pressure drift, insufficient quadrature,
/// analytic linearization failure, Krylov nonconvergence, unsuccessful
/// globalization, or non-monotone model time. A failed step does not mutate
/// the last accepted state.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_mini_navier_stokes_2d<F, B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    body_force: &F,
    initial: SimplicialMiniNavierStokesState2d,
    step_count: NonZeroStepCount,
    plan: MiniNavierStokesStepPlan2d,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    solver: &dyn LinearSolverBackend,
) -> Result<SimplicialMiniNavierStokesTrajectory2d, Diagnostic>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    advance_simplicial_mini_navier_stokes_2d_with_assembly(
        mesh,
        boundary,
        essential_velocity,
        body_force,
        initial,
        step_count,
        plan,
        cell_quadrature,
        facet_quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Advance through an explicit assembly adapter and common linear backend.
///
/// # Errors
/// Preserves all fixed-domain, assembly, Newton, Krylov, and acceptance
/// diagnostics without fallback.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_mini_navier_stokes_2d_with_assembly<F, B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    body_force: &F,
    initial: SimplicialMiniNavierStokesState2d,
    step_count: NonZeroStepCount,
    plan: MiniNavierStokesStepPlan2d,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<SimplicialMiniNavierStokesTrajectory2d, Diagnostic>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    let jacobian_pattern = build_step_jacobian_pattern(mesh, boundary, essential_velocity)?;
    let mut trajectory = SimplicialMiniNavierStokesTrajectory2d::new(initial);
    for _ in 0..step_count.get() {
        let previous = trajectory
            .states()
            .last()
            .expect("trajectory owns its initial state");
        let (next, evidence) = solve_one_step(
            mesh,
            boundary,
            essential_velocity,
            body_force,
            previous,
            plan,
            cell_quadrature,
            facet_quadrature,
            &jacobian_pattern,
            assembly,
            solver,
        )?;
        trajectory.push(next, evidence)?;
    }
    Ok(trajectory)
}

#[allow(clippy::too_many_arguments)]
fn solve_one_step<F, B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    body_force: &F,
    previous: &SimplicialMiniNavierStokesState2d,
    plan: MiniNavierStokesStepPlan2d,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    jacobian_pattern: &StructuralJacobianPattern,
    assembly_backend: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<
    (
        SimplicialMiniNavierStokesState2d,
        super::api::SimplicialMiniNavierStokesStepEvidence2d,
    ),
    Diagnostic,
>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    let mut point = super::assembly::initial_point(mesh, boundary, essential_velocity, previous)?;
    require_consistent_initial_state(mesh, cell_quadrature, previous, plan)?;
    let mut current = assemble_step_linearization(
        mesh,
        boundary,
        essential_velocity,
        body_force,
        previous,
        &point,
        plan,
        cell_quadrature,
        facet_quadrature,
        assembly_backend,
    )?;
    let initial_residual_norm = current.residual_norm()?;
    let residual_target = plan.nonlinear_target(initial_residual_norm)?;
    if initial_residual_norm <= residual_target {
        let jacobian_audit = verify_analytic_jacobian(
            mesh,
            boundary,
            essential_velocity,
            body_force,
            previous,
            &current,
            plan,
            cell_quadrature,
            facet_quadrature,
            jacobian_pattern,
        )?;
        return accept_step(
            mesh,
            previous,
            plan,
            cell_quadrature,
            current,
            NewtonEvidence {
                iterations: 0,
                initial_residual_norm,
                residual_target,
                jacobian_audit,
                linear_solves: Vec::new(),
            },
        );
    }

    let mut reports = Vec::new();
    for iteration in 1..=plan.maximum_newton_iterations().get() {
        let linear_problem = LinearProblem::new(
            current.relation.state_jacobian(),
            current.relation.right_hand_side(),
            eqiora_solver::LinearOperatorProperties::General,
        )?
        .with_initial_guess(&point)?;
        let solution = solver.solve(&linear_problem, plan.linear_solver())?;
        reports.push(solution.report().clone());
        let proposed = solution.values();
        let previous_norm = current.residual_norm()?;
        let mut accepted = None;
        let mut scale = 1.0;
        for _ in 0..=plan.maximum_line_search_steps() {
            let candidate = point
                .iter()
                .zip(proposed)
                .map(|(point, proposed)| point + scale * (proposed - point))
                .collect::<Vec<_>>();
            let assembled = assemble_step_linearization(
                mesh,
                boundary,
                essential_velocity,
                body_force,
                previous,
                &candidate,
                plan,
                cell_quadrature,
                facet_quadrature,
                assembly_backend,
            )?;
            let norm = assembled.residual_norm()?;
            if norm <= residual_target || norm < previous_norm {
                accepted = Some((candidate, assembled, norm));
                break;
            }
            scale *= 0.5;
        }
        let Some((candidate, assembled, norm)) = accepted else {
            return Err(solve_failed(
                "MINI Navier--Stokes Newton line search failed to decrease the residual",
            ));
        };
        point = candidate;
        current = assembled;
        if norm <= residual_target {
            let jacobian_audit = verify_analytic_jacobian(
                mesh,
                boundary,
                essential_velocity,
                body_force,
                previous,
                &current,
                plan,
                cell_quadrature,
                facet_quadrature,
                jacobian_pattern,
            )?;
            return accept_step(
                mesh,
                previous,
                plan,
                cell_quadrature,
                current,
                NewtonEvidence {
                    iterations: iteration,
                    initial_residual_norm,
                    residual_target,
                    jacobian_audit,
                    linear_solves: reports,
                },
            );
        }
    }
    Err(solve_failed(format!(
        "MINI Navier--Stokes Newton solve reached {} iterations above target {residual_target:e}",
        plan.maximum_newton_iterations()
    )))
}

#[allow(clippy::too_many_arguments)]
fn verify_analytic_jacobian<F, B>(
    mesh: &SimplicialMesh,
    boundary: &SimplicialMiniStokesBoundary2d,
    essential_velocity: &B,
    body_force: &F,
    previous: &SimplicialMiniNavierStokesState2d,
    accepted: &super::assembly::StepAssembly,
    plan: MiniNavierStokesStepPlan2d,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
    jacobian_pattern: &StructuralJacobianPattern,
) -> Result<CenteredJacobianAuditEvidence, Diagnostic>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    let point = accepted.algebraic_values();
    audit_centered_jacobian(
        point,
        jacobian_pattern,
        8.0e-6,
        "transient MINI",
        |candidate| {
            assemble_step_residual(
                mesh,
                boundary,
                essential_velocity,
                body_force,
                previous,
                candidate,
                plan,
                cell_quadrature,
                facet_quadrature,
            )
        },
        |column, analytic| {
            let mut direction = vec![0.0; point.len()];
            direction[column] = 1.0;
            accepted
                .relation
                .jvp(RelationTangent::Unknown(&direction), analytic)
        },
    )
}
