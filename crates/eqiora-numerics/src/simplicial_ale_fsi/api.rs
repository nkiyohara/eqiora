//! Accepted outputs and falsifying evidence for fixed-topology ALE FSI.
//!
//! This surface records independently checkable residual, interface,
//! geometry, and linearization evidence. It deliberately makes no fixed-domain
//! energy-equality claim: moving-volume energetics require a separate theorem
//! and acceptance contract.

use eqiora_assembly::AssemblyReport;
use eqiora_core::Diagnostic;
use eqiora_meshing::FixedTopologyGeometryAction;
use eqiora_meshing::VertexId;
use eqiora_solver::{ExecutionTopology, SolveReport};

use super::{AleFsiState, AleFsiStepPlan, invalid};
use crate::jacobian_audit::CenteredJacobianAuditEvidence;

/// Independently recovered fluid and solid actions on one interface vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AleFsiInterfaceAction<const D: usize> {
    vertex: VertexId,
    fluid: [f64; D],
    solid: [f64; D],
}

/// Established two-dimensional interface-action API.
pub type AleFsiInterfaceAction2d = AleFsiInterfaceAction<2>;

/// Three-dimensional interface action on a shared tetrahedral trace vertex.
pub type AleFsiInterfaceAction3d = AleFsiInterfaceAction<3>;

impl<const D: usize> AleFsiInterfaceAction<D> {
    /// Admit one finite pair of physical nodal actions.
    pub(super) fn new(
        vertex: VertexId,
        fluid: [f64; D],
        solid: [f64; D],
    ) -> Result<Self, Diagnostic> {
        if !matches!(D, 2 | 3) {
            return Err(invalid(
                "fixed-topology ALE FSI interface actions admit dimensions two and three",
            ));
        }
        if fluid.iter().chain(&solid).any(|value| !value.is_finite()) {
            return Err(invalid(
                "fixed-topology ALE FSI interface actions must be finite",
            ));
        }
        Ok(Self {
            vertex,
            fluid,
            solid,
        })
    }

    /// Shared interface vertex in immutable reference order.
    #[must_use]
    pub const fn vertex(self) -> VertexId {
        self.vertex
    }

    /// Fluid-side physical nodal action.
    #[must_use]
    pub const fn fluid(self) -> [f64; D] {
        self.fluid
    }

    /// Solid-side physical nodal action after the explicit configuration bridge.
    #[must_use]
    pub const fn solid(self) -> [f64; D] {
        self.solid
    }

    /// Fluid-plus-solid action which vanishes on an unconstrained shared trace.
    #[must_use]
    pub fn imbalance(self) -> [f64; D] {
        std::array::from_fn(|component| self.fluid[component] + self.solid[component])
    }

    /// Euclidean magnitude of [`Self::imbalance`].
    #[must_use]
    pub fn imbalance_norm(self) -> f64 {
        self.imbalance()
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt()
    }

    /// Fluid-side power at one finite shared physical interface velocity.
    ///
    /// # Errors
    /// Returns `EQ0801` when `shared_velocity` is non-finite or the dot product
    /// overflows.
    pub fn fluid_power(self, shared_velocity: [f64; D]) -> Result<f64, Diagnostic> {
        finite_power(self.fluid, shared_velocity)
    }

    /// Solid-side power at one finite shared physical interface velocity.
    ///
    /// # Errors
    /// Returns `EQ0801` when `shared_velocity` is non-finite or the dot product
    /// overflows.
    pub fn solid_power(self, shared_velocity: [f64; D]) -> Result<f64, Diagnostic> {
        finite_power(self.solid, shared_velocity)
    }

    /// Signed fluid-plus-solid interface power defect.
    ///
    /// # Errors
    /// Returns `EQ0801` when either side's evaluation is non-finite.
    pub fn power_imbalance(self, shared_velocity: [f64; D]) -> Result<f64, Diagnostic> {
        let value = self.fluid_power(shared_velocity)? + self.solid_power(shared_velocity)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid(
                "fixed-topology ALE FSI interface-power imbalance overflowed",
            ))
        }
    }
}

/// Named internal measurements submitted to the acceptance boundary.
///
/// Geometry quality, metric identity, action imbalance, power imbalance, and
/// the nonlinear target are intentionally absent: the constructor derives
/// those from the bound geometry action, accepted state, interface actions,
/// and common step plan.
pub(super) struct AleFsiStepEvidenceInput<const D: usize> {
    pub(super) nonlinear_iterations: usize,
    pub(super) initial_residual_norm: f64,
    pub(super) final_residual_norm: f64,
    pub(super) continuity_residual_norm: f64,
    pub(super) kinematic_residual_norm: f64,
    pub(super) interface_velocity_jump_norm: f64,
    pub(super) interface_actions: Vec<AleFsiInterfaceAction<D>>,
    pub(super) jacobian_audit: CenteredJacobianAuditEvidence,
    pub(super) probed_moving_fluid_cell_count: usize,
    pub(super) gcl_active_moving_fluid_cell_count: usize,
    pub(super) compatible_constant_free_stream_residual_norm: f64,
    pub(super) omitted_gcl_witness_norm: f64,
    pub(super) assembly_report: AssemblyReport,
    pub(super) nonlinear_linear_solves: Vec<SolveReport>,
}

#[cfg(test)]
pub(super) type AleFsiStepEvidenceInput2d = AleFsiStepEvidenceInput<2>;

/// Independently accepted evidence for one monolithic ALE FSI step.
#[derive(Debug, Clone, PartialEq)]
pub struct AleFsiStepEvidence<const D: usize> {
    accepted_time: f64,
    nonlinear_iterations: usize,
    initial_residual_norm: f64,
    residual_target: f64,
    final_residual_norm: f64,
    continuity_residual_norm: f64,
    kinematic_residual_norm: f64,
    interface_velocity_jump_norm: f64,
    interface_actions: Vec<AleFsiInterfaceAction<D>>,
    interface_action_imbalance_norm: f64,
    interface_power_imbalance: f64,
    maximum_affine_metric_identity_defect: f64,
    minimum_current_mean_ratio: f64,
    minimum_current_signed_jacobian: f64,
    minimum_path_signed_jacobian: f64,
    jacobian_audit: CenteredJacobianAuditEvidence,
    probed_moving_fluid_cell_count: usize,
    gcl_active_moving_fluid_cell_count: usize,
    compatible_constant_free_stream_residual_norm: f64,
    omitted_gcl_witness_norm: f64,
    assembly_report: AssemblyReport,
    nonlinear_linear_solves: Vec<SolveReport>,
}

/// Established two-dimensional step-evidence API.
pub type AleFsiStepEvidence2d = AleFsiStepEvidence<2>;

/// Three-dimensional accepted step evidence.
pub type AleFsiStepEvidence3d = AleFsiStepEvidence<3>;

impl<const D: usize> AleFsiStepEvidence<D> {
    /// Bind independently measured residuals to one exact accepted geometry.
    ///
    /// # Errors
    /// Returns `EQ0801` for non-finite or sign-invalid evidence, a stale state
    /// or geometry action, non-canonical interface actions, an unaccepted
    /// nonlinear residual, inconsistent Newton/Krylov counts or plans, or
    /// execution evidence outside the serial-host reference boundary.
    pub(super) fn new(
        plan: AleFsiStepPlan<D>,
        geometry: &FixedTopologyGeometryAction<D>,
        accepted: &AleFsiState<D>,
        input: AleFsiStepEvidenceInput<D>,
    ) -> Result<Self, Diagnostic> {
        if geometry.time_step() != plan.time_step() || geometry.current() != accepted.geometry() {
            return Err(invalid(
                "fixed-topology ALE FSI evidence must bind the accepted state and exact step geometry",
            ));
        }

        let scalar_norms = [
            input.initial_residual_norm,
            input.final_residual_norm,
            input.continuity_residual_norm,
            input.kinematic_residual_norm,
            input.interface_velocity_jump_norm,
            input.jacobian_audit.maximum_error(),
            input.compatible_constant_free_stream_residual_norm,
            input.omitted_gcl_witness_norm,
        ];
        if scalar_norms
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(invalid(
                "fixed-topology ALE FSI residual and verification norms must be finite and non-negative",
            ));
        }
        let free_stream_tolerance =
            65_536.0 * f64::EPSILON * (1.0 + input.omitted_gcl_witness_norm);
        if input.gcl_active_moving_fluid_cell_count > input.probed_moving_fluid_cell_count
            || (input.probed_moving_fluid_cell_count == 0
                && (input.compatible_constant_free_stream_residual_norm != 0.0
                    || input.omitted_gcl_witness_norm != 0.0))
            || (input.gcl_active_moving_fluid_cell_count > 0
                && input.omitted_gcl_witness_norm == 0.0)
            || input.compatible_constant_free_stream_residual_norm > free_stream_tolerance
        {
            return Err(invalid(
                "fixed-topology ALE FSI constant-free-stream probe is inconsistent with its moving-cell and omitted-GCL witness",
            ));
        }

        let nonlinear = plan.nonlinear();
        let residual_target = nonlinear
            .absolute_tolerance()
            .max(nonlinear.relative_tolerance() * input.initial_residual_norm);
        if !residual_target.is_finite()
            || input.final_residual_norm > residual_target
            || input.final_residual_norm > input.initial_residual_norm
            || input.nonlinear_iterations > nonlinear.maximum_iterations().get()
            || input.nonlinear_linear_solves.len() != input.nonlinear_iterations
            || (input.nonlinear_iterations == 0
                && (input.initial_residual_norm > residual_target
                    || input.final_residual_norm != input.initial_residual_norm))
            || (input.nonlinear_iterations > 0 && input.initial_residual_norm <= residual_target)
        {
            return Err(invalid(
                "fixed-topology ALE FSI nonlinear evidence is inconsistent with the common acceptance plan",
            ));
        }
        if input.nonlinear_linear_solves.iter().any(|report| {
            report.solver_plan() != plan.linear_solver()
                || !serial_host(report.execution().topology())
                || !serial_host(report.verification().topology())
        }) {
            return Err(invalid(
                "fixed-topology ALE FSI Krylov evidence must use the exact step plan and serial-host execution",
            ));
        }
        if input.assembly_report.packet_count() == 0
            || input.assembly_report.target_count() == 0
            || !serial_host(input.assembly_report.execution().topology())
        {
            return Err(invalid(
                "fixed-topology ALE FSI final assembly evidence must be non-empty and serial-host",
            ));
        }
        validate_interface_order(&input.interface_actions)?;

        let interface_action_imbalance_norm = input
            .interface_actions
            .iter()
            .flat_map(|action| action.imbalance())
            .map(|value| value * value)
            .sum::<f64>()
            .sqrt();
        let signed_power_imbalance = input.interface_actions.iter().try_fold(
            0.0,
            |sum, action| -> Result<f64, Diagnostic> {
                let velocity = accepted
                    .vertex_velocity()
                    .get(action.vertex().index())
                    .copied()
                    .ok_or_else(|| {
                        invalid(
                            "fixed-topology ALE FSI interface action is outside the accepted vertex inventory",
                        )
                    })?;
                let next = sum + action.power_imbalance(velocity)?;
                if next.is_finite() {
                    Ok(next)
                } else {
                    Err(invalid(
                        "fixed-topology ALE FSI global interface-power imbalance overflowed",
                    ))
                }
            },
        )?;
        let interface_power_imbalance = signed_power_imbalance.abs();
        if !interface_action_imbalance_norm.is_finite() || !interface_power_imbalance.is_finite() {
            return Err(invalid(
                "fixed-topology ALE FSI interface balance evidence must be finite",
            ));
        }

        let maximum_affine_metric_identity_defect = geometry
            .cells()
            .iter()
            .map(|cell| cell.metric_identity_defect().abs())
            .fold(0.0_f64, f64::max);
        let quality = geometry.current().quality_report();
        let minimum_current_mean_ratio = quality.minimum_mean_ratio();
        let minimum_current_signed_jacobian = quality.minimum_signed_measure_scale();
        let minimum_path_signed_jacobian = geometry.minimum_path_signed_measure_scale();
        if !maximum_affine_metric_identity_defect.is_finite()
            || maximum_affine_metric_identity_defect < 0.0
            || !minimum_current_mean_ratio.is_finite()
            || !(0.0..=1.0).contains(&minimum_current_mean_ratio)
            || !minimum_current_signed_jacobian.is_finite()
            || minimum_current_signed_jacobian <= 0.0
            || !minimum_path_signed_jacobian.is_finite()
            || minimum_path_signed_jacobian <= 0.0
        {
            return Err(invalid(
                "fixed-topology ALE FSI geometry evidence must be finite, positively oriented, and quality-admitted",
            ));
        }

        Ok(Self {
            accepted_time: accepted.time(),
            nonlinear_iterations: input.nonlinear_iterations,
            initial_residual_norm: input.initial_residual_norm,
            residual_target,
            final_residual_norm: input.final_residual_norm,
            continuity_residual_norm: input.continuity_residual_norm,
            kinematic_residual_norm: input.kinematic_residual_norm,
            interface_velocity_jump_norm: input.interface_velocity_jump_norm,
            interface_actions: input.interface_actions,
            interface_action_imbalance_norm,
            interface_power_imbalance,
            maximum_affine_metric_identity_defect,
            minimum_current_mean_ratio,
            minimum_current_signed_jacobian,
            minimum_path_signed_jacobian,
            jacobian_audit: input.jacobian_audit,
            probed_moving_fluid_cell_count: input.probed_moving_fluid_cell_count,
            gcl_active_moving_fluid_cell_count: input.gcl_active_moving_fluid_cell_count,
            compatible_constant_free_stream_residual_norm: input
                .compatible_constant_free_stream_residual_norm,
            omitted_gcl_witness_norm: input.omitted_gcl_witness_norm,
            assembly_report: input.assembly_report,
            nonlinear_linear_solves: input.nonlinear_linear_solves,
        })
    }

    /// Accepted state time to which this evidence is bound.
    #[must_use]
    pub const fn accepted_time(&self) -> f64 {
        self.accepted_time
    }

    /// Accepted damped-Newton updates.
    #[must_use]
    pub const fn nonlinear_iterations(&self) -> usize {
        self.nonlinear_iterations
    }

    /// Residual norm at the previous-state warm start.
    #[must_use]
    pub const fn initial_residual_norm(&self) -> f64 {
        self.initial_residual_norm
    }

    /// Frozen nonlinear acceptance threshold derived from the common plan.
    #[must_use]
    pub const fn residual_target(&self) -> f64 {
        self.residual_target
    }

    /// Independently reassembled final nonlinear residual norm.
    #[must_use]
    pub const fn final_residual_norm(&self) -> f64 {
        self.final_residual_norm
    }

    /// Weak fluid incompressibility residual norm.
    #[must_use]
    pub const fn continuity_residual_norm(&self) -> f64 {
        self.continuity_residual_norm
    }

    /// Solid backward-Euler kinematic residual norm.
    #[must_use]
    pub const fn kinematic_residual_norm(&self) -> f64 {
        self.kinematic_residual_norm
    }

    /// Physical velocity jump across the shared interface trace.
    #[must_use]
    pub const fn interface_velocity_jump_norm(&self) -> f64 {
        self.interface_velocity_jump_norm
    }

    /// Independently recovered interface actions in strict vertex order.
    #[must_use]
    pub fn interface_actions(&self) -> &[AleFsiInterfaceAction<D>] {
        &self.interface_actions
    }

    /// Euclidean norm of all fluid-plus-solid interface action components.
    #[must_use]
    pub const fn interface_action_imbalance_norm(&self) -> f64 {
        self.interface_action_imbalance_norm
    }

    /// Absolute global fluid-plus-solid interface power defect.
    #[must_use]
    pub const fn interface_power_imbalance(&self) -> f64 {
        self.interface_power_imbalance
    }

    /// Maximum absolute affine `dJ/dt - J div(w)` defect over all cells.
    #[must_use]
    pub const fn maximum_affine_metric_identity_defect(&self) -> f64 {
        self.maximum_affine_metric_identity_defect
    }

    /// Minimum current-cell mean-ratio quality.
    #[must_use]
    pub const fn minimum_current_mean_ratio(&self) -> f64 {
        self.minimum_current_mean_ratio
    }

    /// Minimum positive current-cell signed Jacobian.
    #[must_use]
    pub const fn minimum_current_signed_jacobian(&self) -> f64 {
        self.minimum_current_signed_jacobian
    }

    /// Minimum positive signed Jacobian over every complete affine path.
    #[must_use]
    pub const fn minimum_path_signed_jacobian(&self) -> f64 {
        self.minimum_path_signed_jacobian
    }

    /// Maximum independent analytic-JVP versus centered-difference error.
    #[must_use]
    pub const fn maximum_analytic_jvp_verification_error(&self) -> f64 {
        self.jacobian_audit.maximum_error()
    }

    /// Number of analytic columns independently reconstructed by the audit.
    #[must_use]
    pub fn jacobian_audited_column_count(&self) -> usize {
        self.jacobian_audit.column_count()
    }

    /// Number of conservative structural colors.
    #[must_use]
    pub const fn jacobian_color_count(&self) -> usize {
        self.jacobian_audit.color_count()
    }

    /// Globally coupled columns retained as conservative singleton colors.
    #[must_use]
    pub const fn jacobian_global_singleton_count(&self) -> usize {
        self.jacobian_audit.globally_coupled_singleton_count()
    }

    /// Complete residual assemblies used by the centered audit.
    #[must_use]
    pub const fn jacobian_residual_assembly_count(&self) -> usize {
        self.jacobian_audit.residual_assembly_count()
    }

    /// Moving fluid cells tested with the compatible constant-stream probe.
    ///
    /// Momentum is tested only with the cell bubble whose trace vanishes.
    /// The value therefore does not claim that a nonzero constant velocity is
    /// admissible under the model's homogeneous exterior boundary condition.
    #[must_use]
    pub const fn probed_moving_fluid_cell_count(&self) -> usize {
        self.probed_moving_fluid_cell_count
    }

    /// Probed cells whose nonzero mesh divergence activates the GCL term.
    #[must_use]
    pub const fn gcl_active_moving_fluid_cell_count(&self) -> usize {
        self.gcl_active_moving_fluid_cell_count
    }

    /// Dimensionless residual norm of the compatible constant-stream probe.
    #[must_use]
    pub const fn compatible_constant_free_stream_residual_norm(&self) -> f64 {
        self.compatible_constant_free_stream_residual_norm
    }

    /// Norm that would remain on the same probe if the GCL correction vanished.
    ///
    /// This witness may be zero for static or exactly isochoric grid motion;
    /// it must be nonzero whenever a probed cell has nonzero mesh divergence.
    #[must_use]
    pub const fn omitted_gcl_witness_norm(&self) -> f64 {
        self.omitted_gcl_witness_norm
    }

    /// Accepted placement and packet shape of final independent reassembly.
    #[must_use]
    pub const fn assembly_report(&self) -> &AssemblyReport {
        &self.assembly_report
    }

    /// Common-contract Krylov reports in nonlinear-update order.
    ///
    /// Harmonic influence-column reports remain on
    /// `P1HarmonicMeshMotionAction<D>`; they are not duplicated here.
    #[must_use]
    pub fn nonlinear_linear_solves(&self) -> &[SolveReport] {
        &self.nonlinear_linear_solves
    }
}

/// Initial state followed by accepted moving states and one evidence record per step.
#[derive(Debug, Clone, PartialEq)]
pub struct AleFsiTrajectory<const D: usize> {
    states: Vec<AleFsiState<D>>,
    steps: Vec<AleFsiStepEvidence<D>>,
}

/// Established two-dimensional trajectory API.
pub type AleFsiTrajectory2d = AleFsiTrajectory<2>;

/// Three-dimensional accepted ALE FSI trajectory.
pub type AleFsiTrajectory3d = AleFsiTrajectory<3>;

impl<const D: usize> AleFsiTrajectory<D> {
    pub(crate) fn new(initial: AleFsiState<D>) -> Self {
        Self {
            states: vec![initial],
            steps: Vec::new(),
        }
    }

    /// Append one already accepted state/evidence pair atomically.
    ///
    /// Exact agreement with the step duration is checked by the solver before
    /// evidence construction. This container additionally requires strict time
    /// increase and the evidence's exact accepted-state time.
    pub(crate) fn push(
        &mut self,
        state: AleFsiState<D>,
        evidence: AleFsiStepEvidence<D>,
    ) -> Result<(), Diagnostic> {
        let previous = self
            .states
            .last()
            .expect("ALE FSI trajectory owns its initial state");
        if state.time() <= previous.time() || evidence.accepted_time() != state.time() {
            return Err(invalid(
                "accepted fixed-topology ALE FSI states must increase strictly and match their evidence time",
            ));
        }
        self.states
            .try_reserve(1)
            .map_err(|_| invalid("fixed-topology ALE FSI trajectory state allocation failed"))?;
        self.steps
            .try_reserve(1)
            .map_err(|_| invalid("fixed-topology ALE FSI trajectory evidence allocation failed"))?;
        self.states.push(state);
        self.steps.push(evidence);
        Ok(())
    }

    /// Initial state followed by accepted states in strict model-time order.
    #[must_use]
    pub fn states(&self) -> &[AleFsiState<D>] {
        &self.states
    }

    /// One evidence record per transition between adjacent states.
    #[must_use]
    pub fn steps(&self) -> &[AleFsiStepEvidence<D>] {
        &self.steps
    }

    /// Initial state of the trajectory.
    #[must_use]
    pub fn initial_state(&self) -> &AleFsiState<D> {
        self.states
            .first()
            .expect("ALE FSI trajectory owns its initial state")
    }

    /// Most recently accepted state, or the initial state before any step.
    #[must_use]
    pub fn final_state(&self) -> &AleFsiState<D> {
        self.states
            .last()
            .expect("ALE FSI trajectory owns its initial state")
    }
}

fn finite_power<const D: usize>(
    action: [f64; D],
    shared_velocity: [f64; D],
) -> Result<f64, Diagnostic> {
    if shared_velocity.iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "fixed-topology ALE FSI interface velocity must be finite",
        ));
    }
    let power = action
        .iter()
        .zip(shared_velocity)
        .map(|(action, velocity)| action * velocity)
        .sum::<f64>();
    if power.is_finite() {
        Ok(power)
    } else {
        Err(invalid(
            "fixed-topology ALE FSI interface-power evaluation overflowed",
        ))
    }
}

fn validate_interface_order<const D: usize>(
    actions: &[AleFsiInterfaceAction<D>],
) -> Result<(), Diagnostic> {
    if actions.is_empty()
        || actions
            .windows(2)
            .any(|pair| pair[0].vertex().index() >= pair[1].vertex().index())
    {
        return Err(invalid(
            "fixed-topology ALE FSI evidence requires non-empty interface actions in strict vertex order",
        ));
    }
    Ok(())
}

fn serial_host(topology: ExecutionTopology) -> bool {
    matches!(
        topology,
        ExecutionTopology::Host { workers } if workers == std::num::NonZeroUsize::MIN
    )
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_assembly::{AssemblyPlan, AssemblyResult, AssemblyTarget, CsrMatrix, LinearSystem};
    use eqiora_meshing::{
        CellId, FacetId, MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh,
    };
    use eqiora_realization::{NonlinearSolvePlan, Target};
    use eqiora_solver::{
        BackendId, ConvergenceReason, ExecutionReport, LinearOperatorOrientation,
        LinearSolveRequest, LinearSolver, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER,
        ReductionPolicy, SERIAL_EXECUTION_PROVIDER, SolverPlan, SolverProvider,
    };

    use super::super::{
        AleFsiState2d, AleFsiState3d, AleFsiStepPlan2d, AleFsiStepPlan3d,
        P1HarmonicMeshMotionAction2d, P1HarmonicMeshMotionAction3d,
    };
    use super::*;
    use crate::simplicial_fsi::{
        FixedReferenceFsiLoad2d, FixedReferenceFsiLoad3d, FixedReferenceFsiMaterial2d,
        FixedReferenceFsiMaterial3d, FixedReferenceFsiPartition2d, FixedReferenceFsiPartition3d,
        FixedReferenceFsiScale2d, FixedReferenceFsiScale3d,
    };

    const COMPONENTS: usize = 2;
    const COMPONENTS_3D: usize = 3;
    const TEST_ALE_FSI_SOLVER_PROVIDER: SolverProvider = SolverProvider::new(
        BackendId::new("eqiora.test.ale-fsi"),
        env!("CARGO_PKG_VERSION"),
        &[],
    );

    #[test]
    fn interface_action_exposes_finite_balance_and_power_helpers() {
        let action =
            AleFsiInterfaceAction2d::new(VertexId::new(3), [2.0, -1.0], [-2.0, 1.0]).unwrap();
        assert_eq!(action.vertex(), VertexId::new(3));
        assert_eq!(action.fluid(), [2.0, -1.0]);
        assert_eq!(action.solid(), [-2.0, 1.0]);
        assert_eq!(action.imbalance(), [0.0, 0.0]);
        assert_eq!(action.imbalance_norm(), 0.0);
        assert_eq!(action.fluid_power([3.0, 4.0]).unwrap(), 2.0);
        assert_eq!(action.solid_power([3.0, 4.0]).unwrap(), -2.0);
        assert_eq!(action.power_imbalance([3.0, 4.0]).unwrap(), 0.0);
        assert!(action.power_imbalance([f64::NAN, 0.0]).is_err());
        assert!(
            AleFsiInterfaceAction2d::new(VertexId::new(3), [f64::INFINITY, 0.0], [0.0; 2]).is_err()
        );
    }

    #[test]
    fn three_dimensional_interface_action_is_typed_and_fails_closed() {
        let action =
            AleFsiInterfaceAction3d::new(VertexId::new(7), [2.0, -1.0, 0.5], [-2.0, 1.0, -0.5])
                .unwrap();
        assert_eq!(action.vertex(), VertexId::new(7));
        assert_eq!(action.imbalance(), [0.0; 3]);
        assert_eq!(action.fluid_power([3.0, 4.0, 2.0]).unwrap(), 3.0);
        assert_eq!(action.solid_power([3.0, 4.0, 2.0]).unwrap(), -3.0);
        assert_eq!(action.power_imbalance([3.0, 4.0, 2.0]).unwrap(), 0.0);
        assert!(action.fluid_power([0.0, f64::NAN, 0.0]).is_err());
        assert!(
            AleFsiInterfaceAction3d::new(VertexId::new(7), [f64::INFINITY, 0.0, 0.0], [0.0; 3],)
                .is_err()
        );
        assert!(AleFsiInterfaceAction::<1>::new(VertexId::new(0), [0.0], [0.0]).is_err());

        let ordered = [
            AleFsiInterfaceAction3d::new(VertexId::new(2), [0.0; 3], [0.0; 3]).unwrap(),
            AleFsiInterfaceAction3d::new(VertexId::new(4), [0.0; 3], [0.0; 3]).unwrap(),
        ];
        assert!(validate_interface_order(&ordered).is_ok());
        assert!(validate_interface_order(&ordered.into_iter().rev().collect::<Vec<_>>()).is_err());
    }

    #[test]
    fn three_dimensional_evidence_and_trajectory_are_exercised_and_fail_closed() {
        let fixture = fixture_3d();
        let plan = step_plan_3d();
        let interface_vertex = fixture.partition.interface_vertices()[0];
        let initial = state_3d(0.0, &fixture, interface_vertex);
        let current = state_3d(plan.time_step(), &fixture, interface_vertex);
        let geometry = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &initial,
                &current,
            )
            .unwrap();
        let evidence = AleFsiStepEvidence3d::new(
            plan,
            &geometry,
            &current,
            accepted_input_3d(plan, interface_vertex),
        )
        .unwrap();
        assert_eq!(evidence.interface_actions()[0].imbalance(), [0.0; 3]);
        assert_eq!(evidence.interface_power_imbalance(), 0.0);

        let mut power_overflow = accepted_input_3d(plan, interface_vertex);
        power_overflow.interface_actions = vec![
            AleFsiInterfaceAction3d::new(
                interface_vertex,
                [1.0, -2.0, f64::MAX],
                [-1.0, 2.0, -3.0],
            )
            .unwrap(),
        ];
        let error = AleFsiStepEvidence3d::new(plan, &geometry, &current, power_overflow)
            .expect_err("third-component interface-power overflow must fail closed");
        assert!(
            error
                .message()
                .contains("interface-power evaluation overflowed")
        );

        let mut trajectory = AleFsiTrajectory3d::new(initial);
        trajectory.push(current, evidence.clone()).unwrap();
        assert_eq!(trajectory.states().len(), 2);
        assert_eq!(trajectory.steps(), std::slice::from_ref(&evidence));

        let later = state_3d(2.0 * plan.time_step(), &fixture, interface_vertex);
        let state_count = trajectory.states().len();
        let step_count = trajectory.steps().len();
        let error = trajectory
            .push(later, evidence)
            .expect_err("trajectory evidence time must bind the appended state");
        assert!(error.message().contains("match their evidence time"));
        assert_eq!(trajectory.states().len(), state_count);
        assert_eq!(trajectory.steps().len(), step_count);
    }

    #[test]
    fn evidence_derives_geometry_interface_and_nonlinear_acceptance() {
        let fixture = fixture();
        let plan = step_plan();
        let previous = state(0.0, &fixture);
        let current = state(plan.time_step(), &fixture);
        let geometry = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &current,
            )
            .unwrap();
        let interface_vertex = fixture.partition.interface_vertices()[0];
        let evidence = AleFsiStepEvidence2d::new(
            plan,
            &geometry,
            &current,
            accepted_input(plan, interface_vertex),
        )
        .unwrap();

        assert_eq!(evidence.accepted_time(), current.time());
        assert_eq!(evidence.nonlinear_iterations(), 1);
        assert_eq!(evidence.initial_residual_norm(), 1.0);
        assert_eq!(evidence.residual_target(), 1.0e-9);
        assert_eq!(evidence.final_residual_norm(), 1.0e-10);
        assert_eq!(evidence.continuity_residual_norm(), 1.0e-11);
        assert_eq!(evidence.kinematic_residual_norm(), 1.0e-12);
        assert_eq!(evidence.interface_velocity_jump_norm(), 0.0);
        assert_eq!(evidence.interface_actions().len(), 1);
        assert_eq!(evidence.interface_action_imbalance_norm(), 0.0);
        assert_eq!(evidence.interface_power_imbalance(), 0.0);
        assert!(evidence.maximum_affine_metric_identity_defect() >= 0.0);
        assert!(evidence.minimum_current_mean_ratio() > 0.0);
        assert!(evidence.minimum_current_signed_jacobian() > 0.0);
        assert!(evidence.minimum_path_signed_jacobian() > 0.0);
        assert_eq!(evidence.maximum_analytic_jvp_verification_error(), 1.0e-12);
        assert_eq!(evidence.jacobian_audited_column_count(), 1);
        assert_eq!(evidence.jacobian_color_count(), 1);
        assert_eq!(evidence.jacobian_residual_assembly_count(), 2);
        assert_eq!(evidence.probed_moving_fluid_cell_count(), 0);
        assert_eq!(evidence.gcl_active_moving_fluid_cell_count(), 0);
        assert_eq!(
            evidence.compatible_constant_free_stream_residual_norm(),
            0.0
        );
        assert_eq!(evidence.omitted_gcl_witness_norm(), 0.0);
        assert_eq!(evidence.assembly_report().packet_count(), 1);
        assert_eq!(evidence.nonlinear_linear_solves().len(), 1);
    }

    #[test]
    fn evidence_rejects_iteration_mismatch_and_stale_geometry() {
        let fixture = fixture();
        let plan = step_plan();
        let previous = state(0.0, &fixture);
        let current = state(plan.time_step(), &fixture);
        let geometry = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &current,
            )
            .unwrap();
        let interface_vertex = fixture.partition.interface_vertices()[0];

        let mut mismatched = accepted_input(plan, interface_vertex);
        mismatched.nonlinear_linear_solves.clear();
        assert!(AleFsiStepEvidence2d::new(plan, &geometry, &current, mismatched).is_err());

        let mut missing_gcl_witness = accepted_input(plan, interface_vertex);
        missing_gcl_witness.probed_moving_fluid_cell_count = 1;
        missing_gcl_witness.gcl_active_moving_fluid_cell_count = 1;
        assert!(AleFsiStepEvidence2d::new(plan, &geometry, &current, missing_gcl_witness).is_err());

        let mut nonzero_static_probe = accepted_input(plan, interface_vertex);
        nonzero_static_probe.compatible_constant_free_stream_residual_norm = 1.0e-6;
        assert!(
            AleFsiStepEvidence2d::new(plan, &geometry, &current, nonzero_static_probe).is_err()
        );

        let mut displacement = vec![[0.0; COMPONENTS]; fixture.mesh.vertices().len()];
        for vertex in fixture.partition.solid_vertices() {
            displacement[vertex.index()] = [0.002, 0.0];
        }
        let moved = AleFsiState2d::new(
            current.time(),
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            vec![[0.0; COMPONENTS]; fixture.mesh.vertices().len()],
            vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            displacement,
        )
        .unwrap();
        assert!(
            AleFsiStepEvidence2d::new(
                plan,
                &geometry,
                &moved,
                accepted_input(plan, interface_vertex),
            )
            .is_err()
        );
    }

    #[test]
    fn trajectory_push_is_atomic_and_time_bound() {
        let fixture = fixture();
        let plan = step_plan();
        let initial = state(0.0, &fixture);
        let current = state(plan.time_step(), &fixture);
        let geometry = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &initial,
                &current,
            )
            .unwrap();
        let evidence = AleFsiStepEvidence2d::new(
            plan,
            &geometry,
            &current,
            accepted_input(plan, fixture.partition.interface_vertices()[0]),
        )
        .unwrap();
        let mut trajectory = AleFsiTrajectory2d::new(initial);
        trajectory.push(current.clone(), evidence.clone()).unwrap();
        assert_eq!(trajectory.states().len(), 2);
        assert_eq!(trajectory.steps().len(), 1);
        assert_eq!(trajectory.initial_state().time(), 0.0);
        assert_eq!(trajectory.final_state().time(), plan.time_step());

        let state_count = trajectory.states().len();
        let step_count = trajectory.steps().len();
        assert!(trajectory.push(current, evidence).is_err());
        assert_eq!(trajectory.states().len(), state_count);
        assert_eq!(trajectory.steps().len(), step_count);
    }

    struct Fixture {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition2d,
        motion: P1HarmonicMeshMotionAction2d,
    }

    struct Fixture3d {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition3d,
        motion: P1HarmonicMeshMotionAction3d,
    }

    fn fixture() -> Fixture {
        let mesh = two_domain_mesh();
        let (fluid, solid, interface) = inventories(&mesh);
        let partition = FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
        let motion =
            P1HarmonicMeshMotionAction2d::new(&mesh, &partition, harmonic_solver()).unwrap();
        Fixture {
            mesh,
            partition,
            motion,
        }
    }

    fn fixture_3d() -> Fixture3d {
        let (mesh, partition) =
            partitioned_block_3d(&[0.0, 0.5, 1.0, 2.0], &[0.0, 0.5, 1.0], &[0.0, 0.5, 1.0]);
        let motion =
            P1HarmonicMeshMotionAction3d::new(&mesh, &partition, harmonic_solver()).unwrap();
        Fixture3d {
            mesh,
            partition,
            motion,
        }
    }

    fn state(time: f64, fixture: &Fixture) -> AleFsiState2d {
        AleFsiState2d::new(
            time,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            vec![[0.0; COMPONENTS]; fixture.mesh.vertices().len()],
            vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            vec![[0.0; COMPONENTS]; fixture.mesh.vertices().len()],
        )
        .unwrap()
    }

    fn state_3d(time: f64, fixture: &Fixture3d, interface_vertex: VertexId) -> AleFsiState3d {
        let mut vertex_velocity = vec![[0.0; COMPONENTS_3D]; fixture.mesh.vertices().len()];
        vertex_velocity[interface_vertex.index()] = [0.0, 0.0, 2.0];
        AleFsiState3d::new(
            time,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            vertex_velocity,
            vec![[0.0; COMPONENTS_3D]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            vec![[0.0; COMPONENTS_3D]; fixture.mesh.vertices().len()],
        )
        .unwrap()
    }

    fn accepted_input(
        plan: AleFsiStepPlan2d,
        interface_vertex: VertexId,
    ) -> AleFsiStepEvidenceInput2d {
        AleFsiStepEvidenceInput2d {
            nonlinear_iterations: 1,
            initial_residual_norm: 1.0,
            final_residual_norm: 1.0e-10,
            continuity_residual_norm: 1.0e-11,
            kinematic_residual_norm: 1.0e-12,
            interface_velocity_jump_norm: 0.0,
            interface_actions: vec![
                AleFsiInterfaceAction2d::new(interface_vertex, [1.0, -2.0], [-1.0, 2.0]).unwrap(),
            ],
            jacobian_audit: CenteredJacobianAuditEvidence::new(vec![vec![0]], 0, 1.0e-12).unwrap(),
            probed_moving_fluid_cell_count: 0,
            gcl_active_moving_fluid_cell_count: 0,
            compatible_constant_free_stream_residual_norm: 0.0,
            omitted_gcl_witness_norm: 0.0,
            assembly_report: assembly_report(),
            nonlinear_linear_solves: vec![linear_report(plan)],
        }
    }

    fn accepted_input_3d(
        plan: AleFsiStepPlan3d,
        interface_vertex: VertexId,
    ) -> AleFsiStepEvidenceInput<3> {
        AleFsiStepEvidenceInput {
            nonlinear_iterations: 1,
            initial_residual_norm: 1.0,
            final_residual_norm: 1.0e-10,
            continuity_residual_norm: 1.0e-11,
            kinematic_residual_norm: 1.0e-12,
            interface_velocity_jump_norm: 0.0,
            interface_actions: vec![
                AleFsiInterfaceAction3d::new(interface_vertex, [1.0, -2.0, 3.0], [-1.0, 2.0, -3.0])
                    .unwrap(),
            ],
            jacobian_audit: CenteredJacobianAuditEvidence::new(vec![vec![0]], 0, 1.0e-12).unwrap(),
            probed_moving_fluid_cell_count: 0,
            gcl_active_moving_fluid_cell_count: 0,
            compatible_constant_free_stream_residual_norm: 0.0,
            omitted_gcl_witness_norm: 0.0,
            assembly_report: assembly_report(),
            nonlinear_linear_solves: vec![linear_report(plan)],
        }
    }

    fn linear_report<const D: usize>(plan: AleFsiStepPlan<D>) -> SolveReport {
        SolveReport::accepted(
            TEST_ALE_FSI_SOLVER_PROVIDER,
            SERIAL_EXECUTION_PROVIDER,
            ExecutionReport::host_serial(),
            LinearOperatorOrientation::Normal,
            plan.linear_solver(),
            ConvergenceReason::ResidualToleranceSatisfied,
            1,
            1.0,
            1.0e-12,
            1.0e-12,
            1.0e-10,
        )
        .unwrap()
    }

    fn assembly_report() -> AssemblyReport {
        let plan = AssemblyPlan::new(vec![AssemblyTarget::new(1).unwrap()]).unwrap();
        let matrix = CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![0], vec![1.0]).unwrap();
        let system = LinearSystem::new(matrix, vec![0.0]).unwrap();
        *AssemblyResult::from_complete_systems(
            &plan,
            vec![system],
            1,
            ExecutionReport::host_serial(),
        )
        .unwrap()
        .report()
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
            FixedReferenceFsiScale3d::new(2.0, 1.0, 1.0).unwrap(),
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

    fn partitioned_block_3d(
        x_coordinates: &[f64],
        y_coordinates: &[f64],
        z_coordinates: &[f64],
    ) -> (SimplicialMesh, FixedReferenceFsiPartition3d) {
        let nx = x_coordinates.len();
        let ny = y_coordinates.len();
        let vertex = |x: usize, y: usize, z: usize| z * ny * nx + y * nx + x;
        let vertices = z_coordinates
            .iter()
            .flat_map(|&z| {
                y_coordinates
                    .iter()
                    .flat_map(move |&y| x_coordinates.iter().map(move |&x| vec![x, y, z]))
            })
            .collect::<Vec<_>>();
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut cells = Vec::new();
        let mut fluid_cells = Vec::new();
        let mut solid_cells = Vec::new();
        for x in 0..nx - 1 {
            for y in 0..ny - 1 {
                for z in 0..z_coordinates.len() - 1 {
                    for permutation in permutations {
                        let mut offset = [0, 0, 0];
                        let mut tetrahedron = vec![vertex(x, y, z)];
                        for axis in permutation {
                            offset[axis] = 1;
                            tetrahedron.push(vertex(x + offset[0], y + offset[1], z + offset[2]));
                        }
                        if signed_tetrahedron_measure(&vertices, &tetrahedron) < 0.0 {
                            tetrahedron.swap(1, 2);
                        }
                        let id = CellId::new(cells.len());
                        if x_coordinates[x + 1] <= 1.0 {
                            fluid_cells.push(id);
                        } else {
                            solid_cells.push(id);
                        }
                        cells.push(tetrahedron);
                    }
                }
            }
        }
        let mesh =
            SimplicialMesh::new(3, vertices, cells, MeshQualityGate::new(0.02).unwrap()).unwrap();
        let interface_facets = (0..mesh.entity_count(2).unwrap())
            .filter_map(|facet| {
                let vertices = mesh.entity_vertices(MeshEntity::new(2, facet)).unwrap();
                vertices
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
                    .then_some(FacetId::new(facet))
            })
            .collect::<Vec<_>>();
        let partition =
            FixedReferenceFsiPartition3d::new(&mesh, fluid_cells, solid_cells, interface_facets)
                .unwrap();
        (mesh, partition)
    }

    fn signed_tetrahedron_measure(vertices: &[Vec<f64>], cell: &[usize]) -> f64 {
        let origin = &vertices[cell[0]];
        let column = |vertex: usize, axis: usize| vertices[cell[vertex]][axis] - origin[axis];
        column(1, 0) * (column(2, 1) * column(3, 2) - column(3, 1) * column(2, 2))
            - column(2, 0) * (column(1, 1) * column(3, 2) - column(3, 1) * column(1, 2))
            + column(3, 0) * (column(1, 1) * column(2, 2) - column(2, 1) * column(1, 2))
    }
}
