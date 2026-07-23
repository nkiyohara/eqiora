//! Accepted state and step-policy contracts for fixed-topology ALE FSI.
//!
//! Coordinates are deliberately absent from the public state constructor.
//! One sealed harmonic-motion action maps absolute solid displacement to the
//! complete vertex displacement, from which the current geometry is rebuilt
//! against immutable reference topology.  The step plan likewise consumes the
//! common nonlinear and linear solver contracts directly; it does not create a
//! method-specific Krylov configuration.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{FixedTopologyGeometryAction, FixedTopologyGeometryState};
use eqiora_realization::{NonlinearSolvePlan, Target};
use eqiora_solver::{LinearOperatorProperties, LinearSolver, SolverPlan};

use crate::{
    FixedReferenceFsiBoundary, FixedReferenceFsiLoad, FixedReferenceFsiMaterial,
    FixedReferenceFsiPartition, FixedReferenceFsiScale, FixedReferenceFsiState,
    FixedReferenceFsiStepConfig, MeshTopology, SimplicialMesh,
};

use super::{P1HarmonicMeshMotion, invalid};

/// Homogeneous physical-velocity boundary used by the bounded ALE slice.
///
/// Mesh-motion boundary ownership remains sealed in
/// [`P1HarmonicMeshMotion`]; this alias describes only the physical velocity
/// closure and therefore reuses the fixed-reference FSI contract exactly.
pub type AleFsiBoundary<const D: usize> = FixedReferenceFsiBoundary<D>;

/// Established two-dimensional physical-velocity boundary.
pub type AleFsiBoundary2d = AleFsiBoundary<2>;

/// Three-dimensional physical-velocity boundary.
pub type AleFsiBoundary3d = AleFsiBoundary<3>;

/// One accepted or restartable state on immutable reference topology.
///
/// Velocity and displacement coefficients use reference-vertex order. MINI
/// bubble velocity uses [`FixedReferenceFsiPartition::fluid_cells`] order,
/// while pressure uses [`FixedReferenceFsiPartition::fluid_vertices`] order.
/// The stored geometry is a derived value, never independent state.
#[derive(Debug, Clone, PartialEq)]
pub struct AleFsiState<const D: usize> {
    time: f64,
    vertex_velocity: Vec<[f64; D]>,
    fluid_cell_bubble_velocity: Vec<[f64; D]>,
    fluid_pressure: Vec<f64>,
    solid_displacement: Vec<[f64; D]>,
    geometry: FixedTopologyGeometryState<D>,
}

/// Established two-dimensional ALE state API.
pub type AleFsiState2d = AleFsiState<2>;

/// Three-dimensional ALE state over immutable tetrahedral topology.
pub type AleFsiState3d = AleFsiState<3>;

impl<const D: usize> AleFsiState<D> {
    /// Derive and admit one complete moving-domain state.
    ///
    /// `solid_displacement` is the sole geometry driver. It must use reference
    /// vertex order and be exact zero outside the solid closure. The sealed
    /// harmonic action supplies interface continuity, fixed fluid-exterior
    /// values, and every fluid-interior value before coordinates are formed as
    /// `reference + absolute_displacement`.
    ///
    /// # Errors
    /// Returns `EQ0801` for non-finite/negative time, an incompatible sealed
    /// reference or partition, a non-finite or incorrectly shaped field,
    /// displacement outside the solid closure, or coordinate overflow. Mesh
    /// orientation and quality failures retain their `EQ0803` diagnostic.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time: f64,
        reference_mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        motion: &P1HarmonicMeshMotion<D>,
        vertex_velocity: Vec<[f64; D]>,
        fluid_cell_bubble_velocity: Vec<[f64; D]>,
        fluid_pressure: Vec<f64>,
        solid_displacement: Vec<[f64; D]>,
    ) -> Result<Self, Diagnostic> {
        if !time.is_finite() || time < 0.0 {
            return Err(invalid(
                "fixed-topology ALE FSI state time must be finite and non-negative",
            ));
        }
        motion.validate_reference(reference_mesh, partition)?;
        validate_state_fields(
            reference_mesh,
            partition,
            &vertex_velocity,
            &fluid_cell_bubble_velocity,
            &fluid_pressure,
            &solid_displacement,
        )?;

        let displacement = motion.apply(&solid_displacement)?;
        let coordinates = current_coordinates(reference_mesh, &displacement)?;
        let geometry = FixedTopologyGeometryState::<D>::new(reference_mesh, coordinates)?;
        let value = Self {
            time,
            vertex_velocity,
            fluid_cell_bubble_velocity,
            fluid_pressure,
            solid_displacement,
            geometry,
        };
        value.validate_against(reference_mesh, partition, motion)?;
        Ok(value)
    }

    /// Model time in coherent seconds.
    #[must_use]
    pub const fn time(&self) -> f64 {
        self.time
    }

    /// Shared fluid/solid P1 velocity in reference-vertex order.
    #[must_use]
    pub fn vertex_velocity(&self) -> &[[f64; D]] {
        &self.vertex_velocity
    }

    /// Fluid MINI bubble velocity in canonical fluid-cell order.
    #[must_use]
    pub fn fluid_cell_bubble_velocity(&self) -> &[[f64; D]] {
        &self.fluid_cell_bubble_velocity
    }

    /// Fluid P1 pressure in canonical fluid-vertex order.
    #[must_use]
    pub fn fluid_pressure(&self) -> &[f64] {
        &self.fluid_pressure
    }

    /// Absolute solid P1 displacement in reference-vertex order.
    ///
    /// Entries outside the solid closure are exact zero.
    #[must_use]
    pub fn solid_displacement(&self) -> &[[f64; D]] {
        &self.solid_displacement
    }

    /// Current coordinates and recomputed quality derived from solid motion.
    #[must_use]
    pub const fn geometry(&self) -> &FixedTopologyGeometryState<D> {
        &self.geometry
    }

    /// Revalidate this state against the exact sealed reference root.
    ///
    /// This is the restart/replay gate. In addition to field shape and support,
    /// it independently reapplies the harmonic action and requires exact
    /// equality with the stored derived geometry.
    ///
    /// # Errors
    /// Returns `EQ0801` if any state field or derived geometry cannot replay
    /// against the supplied immutable reference, partition, and motion action;
    /// mesh reconstruction failures retain their `EQ0803` diagnostic.
    pub fn validate_against(
        &self,
        reference_mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        motion: &P1HarmonicMeshMotion<D>,
    ) -> Result<(), Diagnostic> {
        if !self.time.is_finite() || self.time < 0.0 {
            return Err(invalid(
                "fixed-topology ALE FSI state time must be finite and non-negative",
            ));
        }
        motion.validate_reference(reference_mesh, partition)?;
        validate_state_fields(
            reference_mesh,
            partition,
            &self.vertex_velocity,
            &self.fluid_cell_bubble_velocity,
            &self.fluid_pressure,
            &self.solid_displacement,
        )?;
        let displacement = motion.apply(&self.solid_displacement)?;
        let coordinates = current_coordinates(reference_mesh, &displacement)?;
        let replayed = FixedTopologyGeometryState::<D>::new(reference_mesh, coordinates)?;
        if replayed != self.geometry {
            return Err(invalid(
                "fixed-topology ALE FSI geometry must equal reference coordinates plus replayed absolute harmonic motion",
            ));
        }
        self.geometry.reconstruct_mesh(reference_mesh)?;
        Ok(())
    }

    /// Exact bridge to the unchanged reference-layout velocity/displacement state.
    pub(crate) fn to_fixed_reference_state(
        &self,
        reference_mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
    ) -> Result<FixedReferenceFsiState<D>, Diagnostic> {
        FixedReferenceFsiState::<D>::new(
            reference_mesh,
            partition,
            self.vertex_velocity.clone(),
            self.fluid_cell_bubble_velocity.clone(),
            self.solid_displacement.clone(),
        )
    }
}

/// Complete bounded policy for one monolithic backward-Euler ALE FSI step.
///
/// Material, scale, and load retain their existing physical meaning. The
/// common [`NonlinearSolvePlan`] and [`SolverPlan`] remain the sole nonlinear
/// and linear controls; this type only closes their ALE-FSI compatibility and
/// serial reference placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AleFsiStepPlan<const D: usize> {
    fixed_reference: FixedReferenceFsiStepConfig<D>,
    nonlinear: NonlinearSolvePlan,
    linear_solver: SolverPlan,
    target: Target,
}

/// Established two-dimensional ALE step policy.
pub type AleFsiStepPlan2d = AleFsiStepPlan<2>;

/// Three-dimensional ALE step policy.
pub type AleFsiStepPlan3d = AleFsiStepPlan<3>;

impl<const D: usize> AleFsiStepPlan<D> {
    /// Admit the bounded serial-host nonlinear ALE FSI policy.
    ///
    /// # Errors
    /// Returns `EQ0801` for an invalid duration and `EQ0807` unless the load is
    /// the explicit zero-load slice, the common linear plan selects BiCGSTAB
    /// for the general Newton action, and placement is exactly one host worker.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_step: f64,
        material: FixedReferenceFsiMaterial<D>,
        scale: FixedReferenceFsiScale<D>,
        load: FixedReferenceFsiLoad,
        nonlinear: NonlinearSolvePlan,
        linear_solver: SolverPlan,
        target: Target,
    ) -> Result<Self, Diagnostic> {
        let fixed_reference =
            FixedReferenceFsiStepConfig::<D>::new(time_step, material, scale, load)?;
        if load != FixedReferenceFsiLoad::Zero {
            return Err(invalid_realization(
                "fixed-topology ALE FSI v1 admits only the explicit zero-load policy",
            ));
        }
        if linear_solver.algorithm() != LinearSolver::BiConjugateGradientStabilized
            || !linear_solver
                .algorithm()
                .accepts(LinearOperatorProperties::General)
        {
            return Err(invalid_realization(
                "fixed-topology ALE FSI Newton actions require the common general-operator BiCGSTAB plan",
            ));
        }
        if target
            != (Target::HostCpu {
                threads: std::num::NonZeroUsize::MIN,
            })
        {
            return Err(invalid_realization(
                "fixed-topology ALE FSI v1 requires the serial HostCpu target",
            ));
        }
        Ok(Self {
            fixed_reference,
            nonlinear,
            linear_solver,
            target,
        })
    }

    /// Backward-Euler step width.
    #[must_use]
    pub const fn time_step(self) -> f64 {
        self.fixed_reference.time_step()
    }

    /// Stable Newtonian-fluid and linear-solid material data.
    #[must_use]
    pub const fn material(self) -> FixedReferenceFsiMaterial<D> {
        self.fixed_reference.material()
    }

    /// Characteristic acceptance scales.
    #[must_use]
    pub const fn scale(self) -> FixedReferenceFsiScale<D> {
        self.fixed_reference.scale()
    }

    /// Explicit bounded load policy.
    #[must_use]
    pub const fn load(self) -> FixedReferenceFsiLoad {
        self.fixed_reference.load()
    }

    /// Common nonlinear convergence and globalization policy.
    #[must_use]
    pub const fn nonlinear(self) -> NonlinearSolvePlan {
        self.nonlinear
    }

    /// Common linear plan used for every general Newton action.
    #[must_use]
    pub const fn linear_solver(self) -> SolverPlan {
        self.linear_solver
    }

    /// Mathematical class of every admitted Newton action.
    #[must_use]
    pub const fn operator_properties(self) -> LinearOperatorProperties {
        LinearOperatorProperties::General
    }

    /// Exact one-worker host placement of the bounded reference slice.
    #[must_use]
    pub const fn target(self) -> Target {
        self.target
    }

    /// Unchanged material/scale/load bridge for reference-solid assembly.
    pub(crate) const fn fixed_reference_config(self) -> FixedReferenceFsiStepConfig<D> {
        self.fixed_reference
    }

    /// Revalidate two accepted states and derive their sole geometry action.
    ///
    /// The current time must be the exact result of adding this plan's duration
    /// to the previous time. Mesh velocity and all GCL coefficients then come
    /// only from the returned consecutive geometry action.
    pub(crate) fn geometry_action(
        self,
        reference_mesh: &SimplicialMesh,
        partition: &FixedReferenceFsiPartition<D>,
        motion: &P1HarmonicMeshMotion<D>,
        previous: &AleFsiState<D>,
        current: &AleFsiState<D>,
    ) -> Result<FixedTopologyGeometryAction<D>, Diagnostic> {
        previous.validate_against(reference_mesh, partition, motion)?;
        current.validate_against(reference_mesh, partition, motion)?;
        let expected_time = previous.time + self.time_step();
        if !expected_time.is_finite()
            || expected_time <= previous.time
            || current.time != expected_time
        {
            return Err(invalid(
                "fixed-topology ALE FSI states must advance by the exact plan duration",
            ));
        }
        FixedTopologyGeometryAction::<D>::new(
            reference_mesh,
            previous.geometry(),
            current.geometry(),
            self.time_step(),
        )
    }
}

fn validate_state_fields<const D: usize>(
    reference_mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    vertex_velocity: &[[f64; D]],
    fluid_cell_bubble_velocity: &[[f64; D]],
    fluid_pressure: &[f64],
    solid_displacement: &[[f64; D]],
) -> Result<(), Diagnostic> {
    if !matches!(D, 2 | 3)
        || reference_mesh.topological_dimension() != D
        || reference_mesh
            .vertices()
            .iter()
            .any(|coordinates| coordinates.len() != D)
        || fluid_pressure.len() != partition.fluid_vertices().len()
        || fluid_pressure.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(format!(
            "fixed-topology ALE FSI state must own finite pressure in canonical fluid-vertex order on one intrinsic {D}D reference mesh with D equal to 2 or 3"
        )));
    }
    FixedReferenceFsiState::<D>::new(
        reference_mesh,
        partition,
        vertex_velocity.to_vec(),
        fluid_cell_bubble_velocity.to_vec(),
        solid_displacement.to_vec(),
    )?;
    Ok(())
}

fn current_coordinates<const D: usize>(
    reference_mesh: &SimplicialMesh,
    displacement: &[[f64; D]],
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if displacement.len() != reference_mesh.vertices().len()
        || reference_mesh
            .vertices()
            .iter()
            .any(|coordinates| coordinates.len() != D)
    {
        return Err(invalid(format!(
            "fixed-topology ALE FSI motion must cover the exact intrinsic-{D}D reference vertex inventory"
        )));
    }
    let coordinates = reference_mesh
        .vertices()
        .iter()
        .zip(displacement)
        .map(|(reference, displacement)| {
            reference
                .iter()
                .zip(displacement)
                .map(|(reference, displacement)| reference + displacement)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if coordinates.iter().flatten().any(|value| !value.is_finite()) {
        return Err(invalid(
            "fixed-topology ALE FSI current-coordinate derivation overflowed",
        ));
    }
    Ok(coordinates)
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_solver::{
        LinearSolveRequest, PreconditionerPolicy, REFERENCE_LINEAR_SOLVER, ReductionPolicy,
    };

    use crate::{
        CellId, FacetId, FixedReferenceFsiBoundary2d, FixedReferenceFsiLoad2d,
        FixedReferenceFsiMaterial2d, FixedReferenceFsiPartition2d, FixedReferenceFsiPartition3d,
        FixedReferenceFsiScale2d, FixedTopologyGeometryState2d, MeshEntity, MeshQualityGate,
        MeshTopology, P1HarmonicMeshMotion2d, P1HarmonicMeshMotion3d,
    };

    use super::*;

    const COMPONENTS: usize = 2;

    #[test]
    fn state_derives_geometry_from_the_only_admitted_driver() {
        let fixture = fixture();
        let solid_displacement = moving_solid_displacement(&fixture);
        let state = AleFsiState2d::new(
            0.25,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors(&fixture.mesh),
            vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
            (0..fixture.partition.fluid_vertices().len())
                .map(|index| index as f64)
                .collect(),
            solid_displacement.clone(),
        )
        .unwrap();

        let all_displacement = fixture.motion.apply(&solid_displacement).unwrap();
        let expected = current_coordinates(&fixture.mesh, &all_displacement).unwrap();
        assert_eq!(state.geometry().coordinates(), expected);
        assert_eq!(state.time(), 0.25);
        assert_eq!(state.vertex_velocity().len(), fixture.mesh.vertices().len());
        assert_eq!(
            state.fluid_cell_bubble_velocity().len(),
            fixture.partition.fluid_cells().len()
        );
        assert_eq!(
            state.fluid_pressure().len(),
            fixture.partition.fluid_vertices().len()
        );
        assert_eq!(state.solid_displacement(), solid_displacement);
        let fixed = state
            .to_fixed_reference_state(&fixture.mesh, &fixture.partition)
            .unwrap();
        assert_eq!(fixed.solid_displacement(), state.solid_displacement());
        state
            .validate_against(&fixture.mesh, &fixture.partition, &fixture.motion)
            .unwrap();
    }

    #[test]
    fn state_rejects_wrong_pressure_support_and_nonfinite_fields() {
        let fixture = fixture();
        let valid_pressure = vec![0.0; fixture.partition.fluid_vertices().len()];
        let error = AleFsiState2d::new(
            0.0,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors(&fixture.mesh),
            vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
            valid_pressure[..valid_pressure.len() - 1].to_vec(),
            zero_vertex_vectors(&fixture.mesh),
        )
        .unwrap_err();
        assert!(error.message().contains("canonical fluid-vertex order"));

        let fluid_only = fixture
            .partition
            .fluid_vertices()
            .iter()
            .find(|vertex| {
                fixture
                    .partition
                    .solid_vertices()
                    .binary_search(vertex)
                    .is_err()
            })
            .unwrap();
        let mut unsupported = zero_vertex_vectors(&fixture.mesh);
        unsupported[fluid_only.index()] = [0.01, 0.0];
        assert!(
            AleFsiState2d::new(
                0.0,
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                zero_vertex_vectors(&fixture.mesh),
                vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
                valid_pressure.clone(),
                unsupported,
            )
            .is_err()
        );

        let mut velocity = zero_vertex_vectors(&fixture.mesh);
        velocity[0][0] = f64::NAN;
        assert!(
            AleFsiState2d::new(
                0.0,
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                velocity,
                vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
                valid_pressure,
                zero_vertex_vectors(&fixture.mesh),
            )
            .is_err()
        );
    }

    #[test]
    fn restart_replay_rejects_a_substituted_derived_geometry() {
        let fixture = fixture();
        let mut state = zero_state(0.0, &fixture);
        let mut substituted = fixture.mesh.vertices().to_vec();
        substituted[0][0] += 0.01;
        state.geometry = FixedTopologyGeometryState2d::new(&fixture.mesh, substituted).unwrap();
        let error = state
            .validate_against(&fixture.mesh, &fixture.partition, &fixture.motion)
            .unwrap_err();
        assert!(
            error
                .message()
                .contains("replayed absolute harmonic motion")
        );
    }

    #[test]
    fn step_plan_closes_general_solver_serial_target_and_exact_time() {
        let fixture = fixture();
        let plan = valid_plan();
        assert_eq!(
            plan.operator_properties(),
            LinearOperatorProperties::General
        );
        assert_eq!(
            plan.linear_solver().algorithm(),
            LinearSolver::BiConjugateGradientStabilized
        );
        assert_eq!(plan.material(), material());
        assert_eq!(
            plan.scale(),
            FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap()
        );
        assert_eq!(plan.load(), FixedReferenceFsiLoad2d::Zero);
        assert_eq!(plan.nonlinear(), nonlinear());
        assert_eq!(
            plan.target(),
            Target::HostCpu {
                threads: NonZeroUsize::MIN
            }
        );
        assert_eq!(plan.fixed_reference_config().time_step(), plan.time_step());
        let _boundary: AleFsiBoundary2d =
            FixedReferenceFsiBoundary2d::homogeneous_exterior(&fixture.mesh).unwrap();
        let previous = zero_state(0.0, &fixture);
        let current = zero_state(plan.time_step(), &fixture);
        let action = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &current,
            )
            .unwrap();
        assert_eq!(action.time_step(), plan.time_step());

        let wrong_time = zero_state(2.0 * plan.time_step(), &fixture);
        assert!(
            plan.geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &wrong_time,
            )
            .is_err()
        );

        let rounded_previous = zero_state(f64::MAX, &fixture);
        let rounded_current = zero_state(f64::MAX, &fixture);
        assert!(
            plan.geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &rounded_previous,
                &rounded_current,
            )
            .is_err()
        );
    }

    #[test]
    fn step_plan_rejects_symmetric_solver_and_parallel_target() {
        let material = material();
        let scale = FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap();
        let nonlinear = nonlinear();
        let minres = SolverPlan::new(
            LinearSolver::MinimumResidual,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap();
        let serial = Target::HostCpu {
            threads: NonZeroUsize::MIN,
        };
        assert!(
            AleFsiStepPlan2d::new(
                0.05,
                material,
                scale,
                FixedReferenceFsiLoad2d::Zero,
                nonlinear,
                minres,
                serial,
            )
            .is_err()
        );

        let general = general_solver();
        assert!(
            AleFsiStepPlan2d::new(
                0.05,
                material,
                scale,
                FixedReferenceFsiLoad2d::Zero,
                nonlinear,
                general,
                Target::HostCpu {
                    threads: NonZeroUsize::new(2).unwrap(),
                },
            )
            .is_err()
        );
    }

    #[test]
    fn tetrahedral_state_replays_the_only_geometry_driver_and_rejects_wrong_shape() {
        let fixture = fixture_3d();
        let solid_displacement = moving_solid_displacement_3d(&fixture);
        let state = AleFsiState3d::new(
            0.25,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors_3d(&fixture.mesh),
            vec![[0.0; 3]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            solid_displacement.clone(),
        )
        .expect("tetrahedral ALE state is admitted");
        let all_displacement = fixture
            .motion
            .apply(&solid_displacement)
            .expect("sealed motion derives the complete displacement");
        let expected = current_coordinates::<3>(&fixture.mesh, &all_displacement)
            .expect("current coordinates are finite");
        assert_eq!(state.geometry().coordinates(), expected);
        assert_eq!(state.solid_displacement(), solid_displacement);
        state
            .validate_against(&fixture.mesh, &fixture.partition, &fixture.motion)
            .expect("exact tetrahedral state replays");
        let fixed = state
            .to_fixed_reference_state(&fixture.mesh, &fixture.partition)
            .expect("fixed-reference state bridge remains exact");
        assert_eq!(fixed.vertex_velocity(), state.vertex_velocity());
        assert_eq!(fixed.solid_displacement(), state.solid_displacement());

        assert!(
            AleFsiState3d::new(
                0.0,
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                zero_vertex_vectors_3d(&fixture.mesh),
                vec![[0.0; 3]; fixture.partition.fluid_cells().len()],
                vec![0.0; fixture.partition.fluid_vertices().len() - 1],
                zero_vertex_vectors_3d(&fixture.mesh),
            )
            .is_err()
        );
        assert!(
            AleFsiState3d::new(
                0.0,
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                zero_vertex_vectors_3d(&fixture.mesh)[1..].to_vec(),
                vec![[0.0; 3]; fixture.partition.fluid_cells().len()],
                vec![0.0; fixture.partition.fluid_vertices().len()],
                zero_vertex_vectors_3d(&fixture.mesh),
            )
            .is_err()
        );
    }

    #[test]
    fn tetrahedral_restart_and_step_action_fail_closed_against_substituted_geometry() {
        let fixture = fixture_3d();
        let plan = valid_plan_3d();
        let previous = zero_state_3d(0.0, &fixture);
        let current = AleFsiState3d::new(
            plan.time_step(),
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors_3d(&fixture.mesh),
            vec![[0.0; 3]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            moving_solid_displacement_3d(&fixture),
        )
        .expect("moving tetrahedral state is admitted");
        let action = plan
            .geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &current,
            )
            .expect("consecutive states derive one geometry action");
        assert_eq!(action.time_step(), plan.time_step());
        assert_eq!(
            action.vertex_velocities().len(),
            fixture.mesh.vertices().len()
        );
        let _boundary: AleFsiBoundary3d =
            FixedReferenceFsiBoundary::<3>::homogeneous_exterior(&fixture.mesh)
                .expect("tetrahedral exterior boundary closes");

        let mut substituted = current.clone();
        let mut coordinates = substituted.geometry.coordinates().to_vec();
        coordinates[0][0] += 1.0e-3;
        substituted.geometry = FixedTopologyGeometryState::<3>::new(&fixture.mesh, coordinates)
            .expect("substituted geometry remains individually admissible");
        assert!(
            substituted
                .validate_against(&fixture.mesh, &fixture.partition, &fixture.motion)
                .is_err()
        );
        assert!(
            plan.geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &substituted,
            )
            .is_err()
        );

        let wrong_time = zero_state_3d(2.0 * plan.time_step(), &fixture);
        assert!(
            plan.geometry_action(
                &fixture.mesh,
                &fixture.partition,
                &fixture.motion,
                &previous,
                &wrong_time,
            )
            .is_err()
        );
    }

    struct Fixture {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition2d,
        motion: P1HarmonicMeshMotion2d,
    }

    struct Fixture3d {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition3d,
        motion: P1HarmonicMeshMotion3d,
    }

    fn fixture() -> Fixture {
        let mesh = two_domain_mesh_with_fluid_interior();
        let (fluid, solid, interface) = inventories(&mesh);
        let partition = FixedReferenceFsiPartition2d::new(&mesh, fluid, solid, interface).unwrap();
        let motion = P1HarmonicMeshMotion2d::new(&mesh, &partition, harmonic_solver()).unwrap();
        Fixture {
            mesh,
            partition,
            motion,
        }
    }

    fn fixture_3d() -> Fixture3d {
        let mesh = two_domain_tetrahedral_mesh_with_fluid_interior();
        let (fluid, solid, interface) = inventories_3d(&mesh);
        let partition = FixedReferenceFsiPartition3d::new(&mesh, fluid, solid, interface)
            .expect("exact tetrahedral material partition");
        let motion = P1HarmonicMeshMotion3d::new(&mesh, &partition, harmonic_solver())
            .expect("tetrahedral harmonic motion seals");
        Fixture3d {
            mesh,
            partition,
            motion,
        }
    }

    fn two_domain_mesh_with_fluid_interior() -> SimplicialMesh {
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

    fn two_domain_tetrahedral_mesh_with_fluid_interior() -> SimplicialMesh {
        let x_coordinates = [0.0, 0.5, 1.0, 2.0];
        let y_coordinates = [0.0, 0.5, 1.0];
        let z_coordinates = [0.0, 0.5, 1.0];
        let nx = x_coordinates.len();
        let ny = y_coordinates.len();
        let vertex = |x: usize, y: usize, z: usize| z * ny * nx + y * nx + x;
        let vertices = z_coordinates
            .iter()
            .flat_map(|&z| {
                y_coordinates
                    .iter()
                    .flat_map(move |&y| x_coordinates.into_iter().map(move |x| vec![x, y, z]))
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
                        cells.push(tetrahedron);
                    }
                }
            }
        }
        SimplicialMesh::new(
            3,
            vertices,
            cells,
            MeshQualityGate::new(0.02).expect("valid tetrahedral quality gate"),
        )
        .expect("valid conforming tetrahedral test mesh")
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

    fn inventories_3d(mesh: &SimplicialMesh) -> (Vec<CellId>, Vec<CellId>, Vec<FacetId>) {
        let mut fluid = Vec::new();
        let mut solid = Vec::new();
        for (index, cell) in mesh.cells().iter().enumerate() {
            let centroid_x = cell
                .iter()
                .map(|vertex| mesh.vertices()[*vertex][0])
                .sum::<f64>()
                / 4.0;
            if centroid_x < 1.0 {
                fluid.push(CellId::new(index));
            } else {
                solid.push(CellId::new(index));
            }
        }
        let interface = (0..mesh.entity_count(2).expect("3D mesh owns facets"))
            .filter(|&facet| {
                mesh.entity_vertices(MeshEntity::new(2, facet))
                    .expect("test facet owns vertices")
                    .iter()
                    .all(|vertex| mesh.vertices()[vertex.index()][0] == 1.0)
            })
            .map(FacetId::new)
            .collect();
        (fluid, solid, interface)
    }

    fn moving_solid_displacement(fixture: &Fixture) -> Vec<[f64; COMPONENTS]> {
        let mut displacement = zero_vertex_vectors(&fixture.mesh);
        for vertex in fixture.partition.solid_vertices() {
            let point = &fixture.mesh.vertices()[vertex.index()];
            displacement[vertex.index()] = [0.01 * point[1], 0.005 * point[0]];
        }
        displacement
    }

    fn moving_solid_displacement_3d(fixture: &Fixture3d) -> Vec<[f64; 3]> {
        let mut displacement = zero_vertex_vectors_3d(&fixture.mesh);
        for vertex in fixture.partition.solid_vertices() {
            let point = &fixture.mesh.vertices()[vertex.index()];
            displacement[vertex.index()] = [
                0.005 * point[1],
                0.003 * point[0] - 0.002 * point[2],
                0.004 * point[1],
            ];
        }
        displacement
    }

    fn zero_state(time: f64, fixture: &Fixture) -> AleFsiState2d {
        AleFsiState2d::new(
            time,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors(&fixture.mesh),
            vec![[0.0; COMPONENTS]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            zero_vertex_vectors(&fixture.mesh),
        )
        .unwrap()
    }

    fn zero_state_3d(time: f64, fixture: &Fixture3d) -> AleFsiState3d {
        AleFsiState3d::new(
            time,
            &fixture.mesh,
            &fixture.partition,
            &fixture.motion,
            zero_vertex_vectors_3d(&fixture.mesh),
            vec![[0.0; 3]; fixture.partition.fluid_cells().len()],
            vec![0.0; fixture.partition.fluid_vertices().len()],
            zero_vertex_vectors_3d(&fixture.mesh),
        )
        .expect("valid zero tetrahedral state")
    }

    fn zero_vertex_vectors(mesh: &SimplicialMesh) -> Vec<[f64; COMPONENTS]> {
        vec![[0.0; COMPONENTS]; mesh.vertices().len()]
    }

    fn zero_vertex_vectors_3d(mesh: &SimplicialMesh) -> Vec<[f64; 3]> {
        vec![[0.0; 3]; mesh.vertices().len()]
    }

    fn valid_plan() -> AleFsiStepPlan2d {
        AleFsiStepPlan2d::new(
            0.05,
            material(),
            FixedReferenceFsiScale2d::new(2.0, 1.0, 1.0).unwrap(),
            FixedReferenceFsiLoad2d::Zero,
            nonlinear(),
            general_solver(),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .unwrap()
    }

    fn valid_plan_3d() -> AleFsiStepPlan3d {
        AleFsiStepPlan3d::new(
            0.05,
            FixedReferenceFsiMaterial::<3>::new(1.0, 0.1, 1.0, 2.0, 1.0)
                .expect("coercive tetrahedral material"),
            FixedReferenceFsiScale::<3>::new(2.0, 1.0, 1.0).expect("finite tetrahedral scales"),
            FixedReferenceFsiLoad::Zero,
            nonlinear(),
            general_solver(),
            Target::HostCpu {
                threads: NonZeroUsize::MIN,
            },
        )
        .expect("valid tetrahedral ALE plan")
    }

    fn material() -> FixedReferenceFsiMaterial2d {
        FixedReferenceFsiMaterial2d::new(1.0, 0.1, 1.0, 2.0, 1.0).unwrap()
    }

    fn nonlinear() -> NonlinearSolvePlan {
        NonlinearSolvePlan::new(1.0e-9, 1.0e-12, NonZeroUsize::new(20).unwrap(), 12).unwrap()
    }

    fn general_solver() -> SolverPlan {
        SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(500).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Identity)
        .with_reduction(ReductionPolicy::Fast)
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
