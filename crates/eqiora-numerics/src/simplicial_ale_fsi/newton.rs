//! Damped Newton execution for the bounded fixed-topology ALE FSI slice.

use eqiora_assembly::{AssemblyBackend, REFERENCE_ASSEMBLY_BACKEND};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_ir::{LinearizedRelation, RelationTangent};
use eqiora_meshing::{QuadratureRule, SimplicialMesh};
use eqiora_solver::{LinearOperatorProperties, LinearProblem, LinearSolverBackend, ScalarType};

use super::acceptance::{NewtonEvidence, accept_step};
use super::api::{AleFsiStepEvidence, AleFsiTrajectory, AleFsiTrajectory2d, AleFsiTrajectory3d};
use super::assembly::{
    StepAssembly, assemble_step_linearization, assemble_step_residual, build_step_jacobian_pattern,
    initial_point,
};
use super::contract::{
    AleFsiBoundary, AleFsiBoundary2d, AleFsiBoundary3d, AleFsiState, AleFsiState2d, AleFsiState3d,
    AleFsiStepPlan, AleFsiStepPlan2d, AleFsiStepPlan3d,
};
use super::{
    P1HarmonicMeshMotionAction, P1HarmonicMeshMotionAction2d, P1HarmonicMeshMotionAction3d,
};
use crate::jacobian_audit::{
    CenteredJacobianAuditEvidence, StructuralJacobianPattern, audit_centered_jacobian,
};
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
    solver.capabilities().require_problem(
        plan.linear_solver(),
        ScalarType::F64,
        LinearOperatorProperties::General,
    )?;
    initial.validate_against(reference, partition, motion)?;
    let jacobian_pattern = build_step_jacobian_pattern(reference, partition, boundary, motion)?;
    let mut trajectory = AleFsiTrajectory::<D>::new(initial);
    for _ in 0..step_count.get() {
        let previous = trajectory
            .states()
            .last()
            .expect("ALE FSI trajectory owns its initial state");
        let (next, evidence) = solve_one_step::<D>(
            reference,
            partition,
            boundary,
            motion,
            previous,
            plan,
            quadrature,
            &jacobian_pattern,
            assembly,
            solver,
        )?;
        trajectory.push(next, evidence)?;
    }
    Ok(trajectory)
}

#[allow(clippy::too_many_arguments)]
fn solve_one_step<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotionAction<D>,
    previous: &AleFsiState<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    jacobian_pattern: &StructuralJacobianPattern,
    assembly_backend: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<(AleFsiState<D>, AleFsiStepEvidence<D>), Diagnostic> {
    let mut point = initial_point(
        reference, partition, boundary, motion, previous, plan, quadrature,
    )?;
    let mut current = assemble_step_linearization(
        reference,
        partition,
        boundary,
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
        let jacobian_audit = verify_analytic_jacobian::<D>(
            reference,
            partition,
            boundary,
            motion,
            previous,
            &current,
            plan,
            quadrature,
            jacobian_pattern,
        )?;
        return accept_step::<D>(
            reference,
            partition,
            boundary,
            motion,
            previous,
            plan,
            quadrature,
            assembly_backend,
            current,
            NewtonEvidence {
                iterations: 0,
                initial_residual_norm,
                jacobian_audit,
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
            match assemble_step_linearization(
                reference,
                partition,
                boundary,
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
            let jacobian_audit = verify_analytic_jacobian::<D>(
                reference,
                partition,
                boundary,
                motion,
                previous,
                &current,
                plan,
                quadrature,
                jacobian_pattern,
            )?;
            return accept_step::<D>(
                reference,
                partition,
                boundary,
                motion,
                previous,
                plan,
                quadrature,
                assembly_backend,
                current,
                NewtonEvidence {
                    iterations: iteration,
                    initial_residual_norm,
                    jacobian_audit,
                    linear_solves: reports,
                },
            );
        }
    }
    Err(solve_failed(format!(
        "ALE FSI Newton solve reached {maximum_iterations} iterations above target {residual_target:e}"
    )))
}

#[allow(clippy::too_many_arguments)]
fn verify_analytic_jacobian<const D: usize>(
    reference: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    boundary: &AleFsiBoundary<D>,
    motion: &P1HarmonicMeshMotionAction<D>,
    previous: &AleFsiState<D>,
    accepted: &StepAssembly<D>,
    plan: AleFsiStepPlan<D>,
    quadrature: &QuadratureRule,
    jacobian_pattern: &StructuralJacobianPattern,
) -> Result<CenteredJacobianAuditEvidence, Diagnostic> {
    let point = accepted.algebraic_values();
    audit_centered_jacobian(
        point,
        jacobian_pattern,
        2.0e-5,
        "ALE FSI",
        |candidate| {
            assemble_step_residual::<D>(
                reference, partition, boundary, motion, previous, candidate, plan, quadrature,
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
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_meshing::{
        CellId, FacetId, MeshEntity, MeshQualityGate, MeshTopology, VertexId,
        simplex_duffy_gauss_legendre, triangle_duffy_gauss_legendre,
    };
    use eqiora_realization::{NonlinearSolvePlan, Target};
    use eqiora_solver::{
        BackendId, ConvergenceReason, ExecutionReport, LinearOperator, LinearProblem,
        LinearSolution, LinearSolver, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
        ReductionPolicy, ReplicatedLinearExecution, ScalarType, SolverCapabilities,
        SolverCapability, SolverPlan, SolverProvider, accept_linear_solution_with_execution,
    };

    use super::*;
    use crate::simplicial_fsi::{
        FixedReferenceFsiLoad2d, FixedReferenceFsiLoad3d, FixedReferenceFsiMaterial2d,
        FixedReferenceFsiMaterial3d, FixedReferenceFsiScale2d, FixedReferenceFsiScale3d,
    };

    const INTERFACE_INTERIOR_3D: VertexId = VertexId::new(5);

    #[test]
    fn two_steps_close_the_complete_accepted_evidence_chain() {
        let fixture = fixture();
        let plan = step_plan();
        let trajectory = advance_simplicial_ale_fsi_2d(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            fixture.initial,
            NonZeroStepCount::new(NonZeroUsize::new(2).unwrap()),
            plan,
            &triangle_duffy_gauss_legendre(5).unwrap(),
            &DenseGeneralSolver,
        )
        .unwrap();

        assert_eq!(trajectory.states().len(), 3);
        assert_eq!(trajectory.steps().len(), 2);
        for (step, evidence) in trajectory.steps().iter().enumerate() {
            assert_eq!(
                evidence.accepted_time(),
                (step + 1) as f64 * plan.time_step()
            );
            assert!(evidence.final_residual_norm() <= evidence.residual_target());
            assert!(evidence.continuity_residual_norm() <= evidence.residual_target() + 1.0e-8);
            assert!(evidence.kinematic_residual_norm() < 1.0e-12);
            assert_eq!(evidence.interface_velocity_jump_norm(), 0.0);
            assert!(evidence.interface_action_imbalance_norm() < 1.0e-6);
            assert!(evidence.interface_power_imbalance() < 1.0e-6);
            assert!(evidence.maximum_affine_metric_identity_defect() < 1.0e-10);
            assert!(evidence.minimum_current_mean_ratio() > 0.3);
            assert!(evidence.minimum_current_signed_jacobian() > 0.0);
            assert!(evidence.minimum_path_signed_jacobian() > 0.0);
            assert!(evidence.maximum_analytic_jvp_verification_error() < 1.0e-3);
            assert!(evidence.probed_moving_fluid_cell_count() > 0);
            assert!(evidence.gcl_active_moving_fluid_cell_count() > 0);
            assert!(evidence.compatible_constant_free_stream_residual_norm() < 1.0e-12);
            assert!(evidence.omitted_gcl_witness_norm() > 1.0e-8);
            assert_eq!(
                evidence.nonlinear_linear_solves().len(),
                evidence.nonlinear_iterations()
            );
        }
        assert!(
            trajectory
                .states()
                .windows(2)
                .all(|states| states[1].time() > states[0].time())
        );
    }

    #[test]
    fn one_tetrahedral_step_closes_every_three_dimensional_evidence_link() {
        let fixture = fixture_3d();
        let plan = step_plan_3d();
        let degree_nine = simplex_duffy_gauss_legendre(3, 6).unwrap();
        let rejected = advance_simplicial_ale_fsi_3d(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            fixture.initial.clone(),
            NonZeroStepCount::new(NonZeroUsize::MIN),
            plan,
            &degree_nine,
            &DenseGeneralSolver,
        )
        .expect_err("degree-nine tetrahedral quadrature must fail before publication");
        assert!(rejected.message().contains("at least 11"));

        let quadrature = simplex_duffy_gauss_legendre(3, 7).unwrap();
        assert_eq!(quadrature.polynomial_exactness(), Some(11));
        let initial_third_displacement =
            fixture.initial.solid_displacement()[INTERFACE_INTERIOR_3D.index()][2];
        let trajectory = advance_simplicial_ale_fsi_3d(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            fixture.initial,
            NonZeroStepCount::new(NonZeroUsize::MIN),
            plan,
            &quadrature,
            &DenseGeneralSolver,
        )
        .unwrap();

        assert_eq!(trajectory.states().len(), 2);
        assert_eq!(trajectory.steps().len(), 1);
        let final_state = trajectory.final_state();
        let evidence = &trajectory.steps()[0];
        assert_eq!(final_state.time(), plan.time_step());
        assert_eq!(evidence.accepted_time(), final_state.time());
        assert!(evidence.nonlinear_iterations() > 0);
        assert!(evidence.final_residual_norm() <= evidence.residual_target());
        assert!(evidence.continuity_residual_norm() <= evidence.residual_target() + 1.0e-8);
        assert!(evidence.kinematic_residual_norm() < 1.0e-12);
        assert_eq!(evidence.interface_velocity_jump_norm(), 0.0);
        assert!(evidence.interface_action_imbalance_norm() < 1.0e-6);
        assert!(evidence.interface_power_imbalance() < 1.0e-6);
        assert!(evidence.maximum_affine_metric_identity_defect() < 1.0e-10);
        assert!(evidence.minimum_current_mean_ratio() > 0.0);
        assert!(evidence.minimum_current_signed_jacobian() > 0.0);
        assert!(evidence.minimum_path_signed_jacobian() > 0.0);
        assert!(evidence.maximum_analytic_jvp_verification_error() < 1.0e-3);
        assert!(evidence.probed_moving_fluid_cell_count() > 0);
        assert!(evidence.gcl_active_moving_fluid_cell_count() > 0);
        assert!(evidence.compatible_constant_free_stream_residual_norm() < 1.0e-12);
        assert!(evidence.omitted_gcl_witness_norm() > 1.0e-10);
        assert_eq!(
            evidence.nonlinear_linear_solves().len(),
            evidence.nonlinear_iterations()
        );
        assert!(evidence.nonlinear_linear_solves().iter().all(|report| {
            report.execution() == ExecutionReport::host_serial()
                && report.verification() == ExecutionReport::host_serial()
        }));
        assert_eq!(
            evidence.assembly_report().execution(),
            ExecutionReport::host_serial()
        );
        assert_ne!(
            final_state.solid_displacement()[INTERFACE_INTERIOR_3D.index()][2],
            initial_third_displacement
        );
        assert_ne!(
            final_state.vertex_velocity()[INTERFACE_INTERIOR_3D.index()][2],
            0.0
        );
        assert!(evidence.interface_actions().iter().any(|action| {
            action.vertex() == INTERFACE_INTERIOR_3D
                && (action.fluid()[2] != 0.0 || action.solid()[2] != 0.0)
        }));
    }

    #[test]
    fn unsupported_general_solver_fails_before_a_step_is_published() {
        let fixture = fixture();
        let error = advance_simplicial_ale_fsi_2d(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
            &fixture.motion,
            fixture.initial,
            NonZeroStepCount::new(NonZeroUsize::MIN),
            step_plan(),
            &triangle_duffy_gauss_legendre(5).unwrap(),
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::INVALID_REALIZATION);
    }

    struct Fixture {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition2d,
        boundary: AleFsiBoundary2d,
        motion: P1HarmonicMeshMotionAction2d,
        initial: AleFsiState2d,
    }

    struct Fixture3d {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition3d,
        boundary: AleFsiBoundary3d,
        motion: P1HarmonicMeshMotionAction3d,
        initial: AleFsiState3d,
    }

    fn fixture() -> Fixture {
        let mesh = two_domain_mesh();
        let (fluid, solid, interface) = inventories(&mesh);
        let partition = FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
        let boundary = AleFsiBoundary2d::homogeneous_exterior(&mesh).unwrap();
        let motion_plan = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap();
        let motion = P1HarmonicMeshMotionAction2d::new(
            &mesh,
            &partition,
            eqiora_solver::LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, motion_plan),
        )
        .unwrap();
        let mut solid_displacement = vec![[0.0; 2]; mesh.vertices().len()];
        let displaced = find_vertex(&mesh, [1.5, 0.5]);
        assert!(
            partition
                .solid_vertices()
                .contains(&VertexId::new(displaced))
        );
        solid_displacement[displaced] = [0.0, 0.002];
        let initial = AleFsiState2d::new(
            0.0,
            &mesh,
            &partition,
            &motion,
            vec![[0.0; 2]; mesh.vertices().len()],
            vec![[0.0; 2]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            solid_displacement,
        )
        .unwrap();
        Fixture {
            mesh,
            partition,
            boundary,
            motion,
            initial,
        }
    }

    fn fixture_3d() -> Fixture3d {
        let (mesh, fluid, solid, interface) = tetrahedral_problem();
        let partition = FixedReferenceFsiPartition3d::new(&mesh, fluid, solid, interface).unwrap();
        let boundary = AleFsiBoundary3d::homogeneous_exterior(&mesh).unwrap();
        let motion_plan = SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-12,
            1.0e-14,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap();
        let motion = P1HarmonicMeshMotionAction3d::new(
            &mesh,
            &partition,
            eqiora_solver::LinearSolveRequest::new(&REFERENCE_LINEAR_SOLVER, motion_plan),
        )
        .unwrap();
        let mut solid_displacement = vec![[0.0; 3]; mesh.vertices().len()];
        solid_displacement[INTERFACE_INTERIOR_3D.index()][2] = 2.0e-4;
        let initial = AleFsiState3d::new(
            0.0,
            &mesh,
            &partition,
            &motion,
            vec![[0.0; 3]; mesh.vertices().len()],
            vec![[0.0; 3]; partition.fluid_cells().len()],
            vec![0.0; partition.fluid_vertices().len()],
            solid_displacement,
        )
        .unwrap();
        Fixture3d {
            mesh,
            partition,
            boundary,
            motion,
            initial,
        }
    }

    fn step_plan() -> AleFsiStepPlan2d {
        AleFsiStepPlan2d::new(
            0.02,
            FixedReferenceFsiMaterial2d::new(1.0, 0.2, 1.0, 2.0, 1.0).unwrap(),
            FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap(),
            FixedReferenceFsiLoad2d::Zero,
            NonlinearSolvePlan::new(1.0e-7, 1.0e-10, NonZeroUsize::new(20).unwrap(), 16).unwrap(),
            SolverPlan::new(
                LinearSolver::BiConjugateGradientStabilized,
                1.0e-9,
                1.0e-11,
                NonZeroUsize::new(500).unwrap(),
            )
            .unwrap()
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Fast),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap()
    }

    fn step_plan_3d() -> AleFsiStepPlan3d {
        AleFsiStepPlan3d::new(
            0.02,
            FixedReferenceFsiMaterial3d::new(1.0, 0.2, 1.0, 2.0, 1.0).unwrap(),
            FixedReferenceFsiScale3d::new(2.0, 1.0, 1.0).unwrap(),
            FixedReferenceFsiLoad3d::Zero,
            NonlinearSolvePlan::new(1.0e-7, 1.0e-10, NonZeroUsize::new(20).unwrap(), 16).unwrap(),
            SolverPlan::new(
                LinearSolver::BiConjugateGradientStabilized,
                1.0e-9,
                1.0e-11,
                NonZeroUsize::new(500).unwrap(),
            )
            .unwrap()
            .with_preconditioner(PreconditionerPolicy::Identity)
            .with_reduction(ReductionPolicy::Fast),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap()
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

    fn tetrahedral_problem() -> (SimplicialMesh, Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
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

    fn find_vertex(mesh: &SimplicialMesh, target: [f64; 2]) -> usize {
        mesh.vertices()
            .iter()
            .position(|coordinates| coordinates.as_slice() == target)
            .unwrap()
    }

    #[derive(Debug)]
    struct DenseGeneralSolver;

    impl LinearSolverBackend for DenseGeneralSolver {
        fn provider(&self) -> SolverProvider {
            SolverProvider::new(
                BackendId::new("eqiora.test.dense-general"),
                env!("CARGO_PKG_VERSION"),
                &[],
            )
        }

        fn capabilities(&self) -> SolverCapabilities {
            SolverCapabilities::exact([SolverCapability {
                algorithm: LinearSolver::BiConjugateGradientStabilized,
                operator_properties: LinearOperatorProperties::General,
                preconditioner: PreconditionerPolicy::Identity,
                reduction: ReductionPolicy::Fast,
                scalar_type: ScalarType::F64,
            }])
            .unwrap()
        }

        fn solve_with_execution(
            &self,
            problem: &LinearProblem<'_>,
            plan: SolverPlan,
            execution: &dyn ReplicatedLinearExecution,
        ) -> Result<LinearSolution, Diagnostic> {
            self.capabilities()
                .require_problem(plan, ScalarType::F64, problem.properties())?;
            if execution.report() != ExecutionReport::host_serial() {
                return Err(Diagnostic::error(
                    codes::INVALID_REALIZATION,
                    "dense test solver requires serial-host execution",
                ));
            }
            let dimension = problem.operator().columns();
            let mut matrix = vec![0.0; dimension * dimension];
            for column in 0..dimension {
                let mut basis = vec![0.0; dimension];
                basis[column] = 1.0;
                let mut action = vec![0.0; dimension];
                LinearOperator::apply(problem.operator(), &basis, &mut action)?;
                for (row, value) in action.into_iter().enumerate() {
                    matrix[row * dimension + column] = value;
                }
            }
            let values = solve_dense(matrix, problem.right_hand_side().to_vec())?;
            accept_linear_solution_with_execution(
                problem,
                plan,
                self.provider(),
                ConvergenceReason::ResidualToleranceSatisfied,
                1,
                0.0,
                values,
                execution,
            )
        }
    }

    fn solve_dense(mut matrix: Vec<f64>, mut rhs: Vec<f64>) -> Result<Vec<f64>, Diagnostic> {
        let dimension = rhs.len();
        for pivot in 0..dimension {
            let selected = (pivot..dimension)
                .max_by(|left, right| {
                    matrix[*left * dimension + pivot]
                        .abs()
                        .total_cmp(&matrix[*right * dimension + pivot].abs())
                })
                .expect("nonempty pivot suffix");
            let pivot_value = matrix[selected * dimension + pivot];
            if !pivot_value.is_finite() || pivot_value.abs() <= f64::MIN_POSITIVE {
                return Err(solve_failed(
                    "dense test solver encountered a singular pivot",
                ));
            }
            if selected != pivot {
                for column in 0..dimension {
                    matrix.swap(pivot * dimension + column, selected * dimension + column);
                }
                rhs.swap(pivot, selected);
            }
            let diagonal = matrix[pivot * dimension + pivot];
            for row in pivot + 1..dimension {
                let factor = matrix[row * dimension + pivot] / diagonal;
                matrix[row * dimension + pivot] = 0.0;
                for column in pivot + 1..dimension {
                    matrix[row * dimension + column] -= factor * matrix[pivot * dimension + column];
                }
                rhs[row] -= factor * rhs[pivot];
            }
        }
        let mut solution = vec![0.0; dimension];
        for row in (0..dimension).rev() {
            let remainder = (row + 1..dimension)
                .map(|column| matrix[row * dimension + column] * solution[column])
                .sum::<f64>();
            solution[row] = (rhs[row] - remainder) / matrix[row * dimension + row];
        }
        if solution.iter().all(|value| value.is_finite()) {
            Ok(solution)
        } else {
            Err(solve_failed(
                "dense test solver produced a non-finite solution",
            ))
        }
    }
}
