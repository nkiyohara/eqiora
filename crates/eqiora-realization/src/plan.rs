use std::num::{NonZeroU16, NonZeroUsize};

use eqiora_core::Diagnostic;
use eqiora_solver::{LinearSolver, SolverPlan};

use crate::{
    Discretization, DiscretizationMethod, ExecutionSchedule, MeshPolicy, QuadraturePolicy, Space,
    SpaceFamily, Target, invalid_realization,
};

/// Complete pure payload for one Realization Graph selection.
#[derive(Debug, Clone, PartialEq)]
pub struct RealizationPlan {
    space: Space,
    discretization: Discretization,
    solver: SolverPlan,
    target: Target,
    schedule: ExecutionSchedule,
}

impl RealizationPlan {
    /// Construct and cross-validate independently owned realization components.
    ///
    /// # Errors
    /// Returns `EQ0807` when a method, space, quadrature, or deployment choice
    /// contradicts another component.
    pub fn new(
        space: Space,
        discretization: Discretization,
        solver: SolverPlan,
        target: Target,
        schedule: ExecutionSchedule,
    ) -> Result<Self, Diagnostic> {
        let plan = Self {
            space,
            discretization,
            solver,
            target,
            schedule,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Discrete-space choice.
    #[must_use]
    pub const fn space(&self) -> Space {
        self.space
    }

    /// Method, mesh, and quadrature choice.
    #[must_use]
    pub const fn discretization(&self) -> Discretization {
        self.discretization
    }

    /// Solver choice.
    #[must_use]
    pub const fn solver(&self) -> SolverPlan {
        self.solver
    }

    /// Deployment target.
    #[must_use]
    pub const fn target(&self) -> Target {
        self.target
    }

    /// Deployment schedule.
    #[must_use]
    pub const fn schedule(&self) -> ExecutionSchedule {
        self.schedule
    }

    pub(crate) fn validate(&self) -> Result<(), Diagnostic> {
        match (
            self.discretization.method(),
            self.space.family(),
            self.discretization.mesh(),
            self.discretization.quadrature(),
        ) {
            (
                DiscretizationMethod::ContinuousGalerkin,
                SpaceFamily::ContinuousLagrange { .. },
                MeshPolicy::GeneratedUniform { .. },
                QuadraturePolicy::GaussLegendre { .. },
            )
            | (
                DiscretizationMethod::CellCenteredFiniteVolume,
                SpaceFamily::CellConstant,
                MeshPolicy::GeneratedUniform { .. } | MeshPolicy::SuppliedCartesian { .. },
                QuadraturePolicy::CellCentroid,
            ) => {}
            (
                DiscretizationMethod::ContinuousGalerkin,
                SpaceFamily::ContinuousLagrange { order },
                MeshPolicy::ImportedSimplicial { .. },
                QuadraturePolicy::SimplexCentroid,
            ) if order == NonZeroU16::MIN => {}
            (DiscretizationMethod::ContinuousGalerkin, _, _, _) => {
                return Err(invalid_realization(
                    "continuous Galerkin requires generated Cartesian/Gauss-Legendre or imported affine-simplex/P1-centroid contracts in v0",
                ));
            }
            (DiscretizationMethod::CellCenteredFiniteVolume, _, _, _) => {
                return Err(invalid_realization(
                    "cell-centered finite volume requires a generated or supplied Cartesian mesh, cell-constant space, and centroid quadrature in v0",
                ));
            }
        }
        if matches!(self.schedule, ExecutionSchedule::RealTime { .. })
            && matches!(self.target, Target::CudaGpu { .. })
        {
            return Err(invalid_realization(
                "the v0 CUDA target has no declared real-time scheduling contract",
            ));
        }
        Ok(())
    }
}

/// Frozen definition of default policy v0: generated P1 FEM on one host CPU.
///
/// # Errors
/// Returns `EQ0807` only if the internally declared policy becomes inconsistent.
pub fn default_plan_v0() -> Result<RealizationPlan, Diagnostic> {
    let one = NonZeroUsize::MIN;
    RealizationPlan::new(
        Space::continuous_lagrange(NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::GeneratedUniform {
                cells_per_axis: NonZeroUsize::new(16).expect("16 is non-zero"),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("2 is non-zero"),
            },
        ),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-13,
            1.0e-14,
            NonZeroUsize::new(512).expect("512 is non-zero"),
        )?,
        Target::HostCpu { threads: one },
        ExecutionSchedule::Offline,
    )
}
