//! Damped Newton execution for the bounded fixed-topology ALE FSI slice.

use std::ops::ControlFlow;

use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{QuadratureRule, SimplicialMesh};
use eqiora_solver::{LinearOperatorProperties, LinearProblem, LinearSolverBackend, ScalarType};

use super::acceptance::{NewtonEvidence, accept_step_prepared};
use super::api::{AleFsiStepEvidence, AleFsiTrajectory, AleFsiTrajectory2d, AleFsiTrajectory3d};
use super::assembly::{
    PreparedAleFsiAction, PreparedAleFsiStructure, assemble_step_linearization_with_structure,
    prepare_ale_fsi_structure,
};
use super::boundary_step::{PreparedAleFsiBoundaryRun, PreparedAleFsiBoundaryStep};
use super::contract::{
    AleFsiBoundary, AleFsiBoundary2d, AleFsiBoundary3d, AleFsiState, AleFsiState2d, AleFsiState3d,
    AleFsiStepPlan, AleFsiStepPlan2d, AleFsiStepPlan3d,
};
use super::{
    P1HarmonicMeshMotionAction, P1HarmonicMeshMotionAction2d, P1HarmonicMeshMotionAction3d,
};
use crate::prepared_execution::advance_prepared_actions;
use crate::simplicial_fsi::{
    FixedReferenceFsiPartition, FixedReferenceFsiPartition2d, FixedReferenceFsiPartition3d,
};
use crate::step_count::NonZeroStepCount;

/// Advance one fixed-topology reference through accepted monolithic ALE steps.
///
/// # Errors
/// Fails closed on stale reference/state data, invalid quadrature, unsupported
/// solver policy, analytic linearization failure, Krylov nonconvergence,
/// unsuccessful globalization, or any acceptance falsifier. A failed step does
/// not mutate the last accepted trajectory state.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_ale_fsi_2d(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition2d,
    boundary: &AleFsiBoundary2d,
    motion: &P1HarmonicMeshMotionAction2d,
    initial: AleFsiState2d,
    step_count: NonZeroStepCount,
    plan: AleFsiStepPlan2d,
    quadrature: &QuadratureRule,
    solver: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory2d, Diagnostic> {
    advance_simplicial_ale_fsi_2d_with_assembly(
        reference,
        partition,
        boundary,
        motion,
        initial,
        step_count,
        plan,
        quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Advance through an explicit assembly adapter and common linear backend.
///
/// # Errors
/// Preserves all ALE, assembly, Newton, Krylov, and independent acceptance
/// diagnostics without fallback.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_ale_fsi_2d_with_assembly(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition2d,
    boundary: &AleFsiBoundary2d,
    motion: &P1HarmonicMeshMotionAction2d,
    initial: AleFsiState2d,
    step_count: NonZeroStepCount,
    plan: AleFsiStepPlan2d,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory2d, Diagnostic> {
    advance_simplicial_ale_fsi_with_assembly::<2>(
        reference, partition, boundary, motion, initial, step_count, plan, quadrature, assembly,
        solver,
    )
}

/// Advance one tetrahedral fixed-topology reference through accepted ALE FSI steps.
///
/// # Errors
/// Preserves the same fail-closed geometry, nonlinear, linearization, solver,
/// and independent-acceptance boundary as the established two-dimensional
/// entry point.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_ale_fsi_3d(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition3d,
    boundary: &AleFsiBoundary3d,
    motion: &P1HarmonicMeshMotionAction3d,
    initial: AleFsiState3d,
    step_count: NonZeroStepCount,
    plan: AleFsiStepPlan3d,
    quadrature: &QuadratureRule,
    solver: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory3d, Diagnostic> {
    advance_simplicial_ale_fsi_3d_with_assembly(
        reference,
        partition,
        boundary,
        motion,
        initial,
        step_count,
        plan,
        quadrature,
        &REFERENCE_ASSEMBLY_BACKEND,
        solver,
    )
}

/// Advance tetrahedral ALE FSI through an explicit assembly adapter.
///
/// # Errors
/// Preserves all typed tetrahedral, degree-eleven quadrature, serial-host,
/// assembly, Newton, Krylov, and independent acceptance diagnostics without
/// fallback.
#[allow(clippy::too_many_arguments)]
pub fn advance_simplicial_ale_fsi_3d_with_assembly(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition3d,
    boundary: &AleFsiBoundary3d,
    motion: &P1HarmonicMeshMotionAction3d,
    initial: AleFsiState3d,
    step_count: NonZeroStepCount,
    plan: AleFsiStepPlan3d,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory3d, Diagnostic> {
    advance_simplicial_ale_fsi_with_assembly::<3>(
        reference, partition, boundary, motion, initial, step_count, plan, quadrature, assembly,
        solver,
    )
}

struct PreparedAleFsiRun<'a, const D: usize> {
    reference: &'a SimplicialMesh,
    partition: &'a FixedReferenceFsiPartition<D>,
    boundary: PreparedAleFsiBoundaryRun<D>,
    structure: PreparedAleFsiStructure<D>,
    motion: &'a P1HarmonicMeshMotionAction<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &'a QuadratureRule,
    assembly: &'a dyn AssemblyBackend,
    solver: &'a dyn LinearSolverBackend,
    #[cfg(test)]
    phases: AleFsiRunPhaseCounts,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AleFsiRunPhaseCounts {
    authentication: usize,
    normalization: usize,
    boundary: usize,
    layout: usize,
    maps: usize,
    quadrature: usize,
    sparsity: usize,
}

impl<'a, const D: usize> PreparedAleFsiRun<'a, D> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        reference: &'a SimplicialMesh,
        partition: &'a FixedReferenceFsiPartition<D>,
        boundary: &'a AleFsiBoundary<D>,
        motion: &'a P1HarmonicMeshMotionAction<D>,
        initial: &AleFsiState<D>,
        plan: AleFsiStepPlan<D>,
        quadrature: &'a QuadratureRule,
        assembly: &'a dyn AssemblyBackend,
        solver: &'a dyn LinearSolverBackend,
    ) -> Result<Self, Diagnostic> {
        solver.capabilities().require_problem(
            plan.linear_solver(),
            ScalarType::F64,
            LinearOperatorProperties::General,
        )?;
        let boundary = PreparedAleFsiBoundaryRun::new(reference, boundary, initial, plan)?;
        let structure = prepare_ale_fsi_structure(
            reference,
            partition,
            boundary.template(),
            motion,
            initial,
            plan,
            quadrature,
        )?;
        #[cfg(test)]
        let phases = {
            let structural = structure.phase_counts();
            AleFsiRunPhaseCounts {
                authentication: structural.authentication,
                normalization: boundary.normalization_count(),
                boundary: 1,
                layout: structural.layout,
                maps: structural.maps,
                quadrature: structural.quadrature,
                sparsity: structural.sparsity,
            }
        };
        Ok(Self {
            reference,
            partition,
            boundary,
            structure,
            motion,
            plan,
            quadrature,
            assembly,
            solver,
            #[cfg(test)]
            phases,
        })
    }

    fn advance(
        &self,
        previous: &AleFsiState<D>,
    ) -> Result<(AleFsiState<D>, AleFsiStepEvidence<D>), Diagnostic> {
        let boundary = self.boundary.action(previous, self.plan)?;
        let action = self.structure.prepare_action(
            self.reference,
            self.partition,
            boundary,
            previous,
            self.plan,
        )?;
        solve_one_step_prepared(
            self.reference,
            self.partition,
            &self.structure,
            &action,
            self.motion,
            previous,
            self.plan,
            self.quadrature,
            self.assembly,
            self.solver,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn advance_simplicial_ale_fsi_with_assembly<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotionAction<D>,
    initial: AleFsiState<D>,
    step_count: NonZeroStepCount,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<AleFsiTrajectory<D>, Diagnostic> {
    match advance_prepared_actions(
        AleFsiTrajectory::<D>::new(initial),
        step_count.get(),
        |trajectory| {
            PreparedAleFsiRun::new(
                reference,
                partition,
                boundary,
                motion,
                trajectory
                    .states()
                    .last()
                    .expect("ALE FSI trajectory owns its initial state"),
                plan,
                quadrature,
                assembly,
                solver,
            )
        },
        |prepared, trajectory| {
            prepared.advance(
                trajectory
                    .states()
                    .last()
                    .expect("ALE FSI trajectory owns its initial state"),
            )
        },
        |trajectory, _, (next, evidence)| trajectory.push(next, evidence),
        |_, _| None::<()>,
    )? {
        ControlFlow::Continue(trajectory) => Ok(trajectory),
        ControlFlow::Break(()) => unreachable!("ALE FSI run has no early boundary"),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_one_step<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotionAction<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    assembly_backend: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<(AleFsiState<D>, AleFsiStepEvidence<D>), Diagnostic> {
    let prepared = match PreparedAleFsiBoundaryStep::from_boundary(boundary) {
        Some(prepared) => prepared,
        None => PreparedAleFsiBoundaryStep::homogeneous(
            reference,
            boundary,
            previous.time(),
            previous.time() + plan.time_step(),
            plan.scale().velocity(),
        )?,
    };
    let structure = prepare_ale_fsi_structure(
        reference, partition, &prepared, motion, previous, plan, quadrature,
    )?;
    let action = structure.prepare_action(reference, partition, prepared, previous, plan)?;
    solve_one_step_prepared(
        reference,
        partition,
        &structure,
        &action,
        motion,
        previous,
        plan,
        quadrature,
        assembly_backend,
        solver,
    )
}

#[allow(clippy::too_many_arguments)]
fn solve_one_step_prepared<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    structure: &PreparedAleFsiStructure<D>,
    action: &PreparedAleFsiAction<D>,
    motion: &P1HarmonicMeshMotionAction<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    assembly_backend: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<(AleFsiState<D>, AleFsiStepEvidence<D>), Diagnostic> {
    let mut point = structure.initial_point(action, previous, plan)?;
    let mut current = assemble_step_linearization_with_structure(
        reference,
        partition,
        structure,
        action,
        motion,
        previous,
        &point,
        plan,
        quadrature,
        assembly_backend,
    )?;
    let initial_residual_norm = current.residual_norm()?;
    let residual_target = nonlinear_target::<D>(plan, initial_residual_norm)?;
    if initial_residual_norm <= residual_target {
        return accept_step_prepared::<D>(
            reference,
            partition,
            structure,
            action,
            motion,
            previous,
            plan,
            quadrature,
            assembly_backend,
            current,
            NewtonEvidence {
                iterations: 0,
                initial_residual_norm,
                linear_solves: Vec::new(),
            },
        );
    }

    let maximum_iterations = plan.nonlinear().maximum_iterations().get();
    let mut reports = Vec::new();
    reports
        .try_reserve_exact(maximum_iterations)
        .map_err(|_| solve_failed("ALE FSI Newton report allocation failed"))?;
    for iteration in 1..=maximum_iterations {
        let linear_problem = LinearProblem::new(
            current.relation.state_jacobian(),
            current.relation.right_hand_side(),
            LinearOperatorProperties::General,
        )?
        .with_initial_guess(&point)?;
        let solution = solver.solve(&linear_problem, plan.linear_solver())?;
        let (proposed, report) = solution.into_parts();
        reports.push(report);
        let previous_norm = current.residual_norm()?;
        let mut accepted = None;
        let mut scale = 1.0;
        for _ in 0..=plan.nonlinear().maximum_line_search_steps() {
            let candidate = point
                .iter()
                .zip(&proposed)
                .map(|(point, proposed)| point + scale * (proposed - point))
                .collect::<Vec<_>>();
            match assemble_step_linearization_with_structure(
                reference,
                partition,
                structure,
                action,
                motion,
                previous,
                &candidate,
                plan,
                quadrature,
                assembly_backend,
            ) {
                Ok(assembled) => {
                    let norm = assembled.residual_norm()?;
                    if norm <= residual_target || norm < previous_norm {
                        accepted = Some((candidate, assembled, norm));
                        break;
                    }
                }
                Err(diagnostic) if diagnostic.code() == codes::INVALID_MESH => {
                    // Geometry is a trial-dependent admissibility constraint.
                    // All other diagnostics describe contract or execution
                    // failure and therefore propagate without globalization.
                }
                Err(diagnostic) => return Err(diagnostic),
            }
            scale *= 0.5;
        }
        let Some((candidate, assembled, norm)) = accepted else {
            return Err(solve_failed(
                "ALE FSI Newton line search found no residual-decreasing admissible geometry",
            ));
        };
        point = candidate;
        current = assembled;
        if norm <= residual_target {
            return accept_step_prepared::<D>(
                reference,
                partition,
                structure,
                action,
                motion,
                previous,
                plan,
                quadrature,
                assembly_backend,
                current,
                NewtonEvidence {
                    iterations: iteration,
                    initial_residual_norm,
                    linear_solves: reports,
                },
            );
        }
    }
    Err(solve_failed(format!(
        "ALE FSI Newton solve reached {maximum_iterations} iterations above target {residual_target:e}"
    )))
}

fn nonlinear_target<const D: usize>(
    plan: AleFsiStepPlan<D>,
    initial_norm: f64,
) -> Result<f64, Diagnostic> {
    if !initial_norm.is_finite() || initial_norm < 0.0 {
        return Err(solve_failed(
            "ALE FSI initial nonlinear residual must be finite and non-negative",
        ));
    }
    let nonlinear = plan.nonlinear();
    let target = nonlinear
        .absolute_tolerance()
        .max(nonlinear.relative_tolerance() * initial_norm);
    if target.is_finite() {
        Ok(target)
    } else {
        Err(solve_failed("ALE FSI nonlinear residual target overflowed"))
    }
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

#[cfg(test)]
mod tests;
