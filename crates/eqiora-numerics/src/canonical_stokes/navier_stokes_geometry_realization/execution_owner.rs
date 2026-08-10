use eqiora_assembly::AssemblyBackend;
use eqiora_core::Diagnostic;
use eqiora_realization::ResolvedTransientFieldwiseRealization;
use eqiora_sem::KernelProgram;
use eqiora_solver::LinearSolverBackend;

use super::super::IncompressibleFlowScaleProfile2d;
use super::super::expression::IncompressibleStressForm;
use super::TransientNavierStokesGeometryBinding2d;
use crate::simplicial_navier_stokes::{
    SimplicialMiniNavierStokesState2d, SimplicialMiniNavierStokesTrajectory2d,
};
use crate::step_count::NonZeroStepCount;

// The accepted serialized Wave C consumes the sealed accessors after Wave A review.
#[allow(dead_code)]
pub(super) struct AcceptedDfgExecution<'owner> {
    binding: &'owner TransientNavierStokesGeometryBinding2d,
    program: &'owner KernelProgram,
    resolved: &'owner ResolvedTransientFieldwiseRealization,
    scales: IncompressibleFlowScaleProfile2d,
    trajectory: SimplicialMiniNavierStokesTrajectory2d,
}

#[allow(dead_code)]
pub(super) struct DfgCurrentState<'execution, 'owner> {
    execution: &'execution AcceptedDfgExecution<'owner>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_dfg_with_assembly<'owner>(
    binding: &'owner TransientNavierStokesGeometryBinding2d,
    program: &'owner KernelProgram,
    resolved: &'owner ResolvedTransientFieldwiseRealization,
    initial: SimplicialMiniNavierStokesState2d,
    steps: NonZeroStepCount,
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<AcceptedDfgExecution<'owner>, Diagnostic> {
    let (trajectory, scales) = binding.advance_with_stress(
        program,
        resolved,
        initial,
        steps,
        assembly,
        solver,
        IncompressibleStressForm::DfgNonsymmetric,
    )?;
    Ok(AcceptedDfgExecution {
        binding,
        program,
        resolved,
        scales,
        trajectory,
    })
}

#[allow(dead_code)]
impl<'owner> AcceptedDfgExecution<'owner> {
    pub(super) fn binding(&self) -> &TransientNavierStokesGeometryBinding2d {
        self.binding
    }

    pub(super) fn program(&self) -> &KernelProgram {
        self.program
    }

    pub(super) fn resolved(&self) -> &ResolvedTransientFieldwiseRealization {
        self.resolved
    }

    pub(super) fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scales
    }

    pub(super) fn current_state<'execution>(
        &'execution self,
    ) -> Result<DfgCurrentState<'execution, 'owner>, Diagnostic> {
        self.trajectory
            .states()
            .last()
            .ok_or_else(|| super::invalid("accepted DFG execution has no current state"))?;
        Ok(DfgCurrentState { execution: self })
    }

    pub(super) fn into_trajectory(self) -> SimplicialMiniNavierStokesTrajectory2d {
        self.trajectory
    }
}

#[allow(dead_code)]
impl<'execution, 'owner> DfgCurrentState<'execution, 'owner> {
    pub(super) fn execution(&self) -> &'execution AcceptedDfgExecution<'owner> {
        self.execution
    }

    pub(super) fn state(&self) -> &'execution SimplicialMiniNavierStokesState2d {
        self.execution
            .trajectory
            .states()
            .last()
            .expect("a DFG current-state token is issued only for a nonempty trajectory")
    }
}
