//! Shared element-local MINI/P1 transient incompressible-fluid relation.
//!
//! This private kernel owns one dimension-parametric weak relation over an
//! affine simplex.  Mesh ownership, global numbering, assembly, nonlinear
//! policy, and FSI coupling remain with their respective realizations.  State
//! storage is slice based because stable Rust cannot yet express the required
//! `D + 1` and `D + 2` array lengths as generic constants.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{
    AffineGeometryLinearization, AffineGeometryMap, FixedTopologyCellGeometryAction, GeometryMap,
    QuadratureRule,
};

use crate::affine_fem::physical_gradient;
use crate::continuum_kinematics::symmetric_gradient_bilinear_entry;
use crate::discrete_space::{DiscreteSpace, SimplexP1BubbleSpace, SimplexP1Space};

/// Transport identity carried by the transient fluid relation.
///
/// `Disabled` is the linear transient relation and contains no convective or
/// ALE datum to inspect or accidentally apply. `SkewStationary` is the
/// fixed-domain nonlinear Navier--Stokes specialization and therefore carries
/// no fictitious mesh-motion history. `SkewRelativeGcl` carries the single
/// sealed geometry action from which relative transport and metric correction
/// are derived.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MiniTransport<'a, const D: usize> {
    Disabled,
    SkewStationary,
    SkewRelativeGcl(&'a FixedTopologyCellGeometryAction<D>),
}

impl<const D: usize> MiniTransport<'_, D> {
    pub(crate) const fn required_quadrature_exactness(self) -> usize {
        match self {
            Self::Disabled => 2 * (D + 1),
            Self::SkewStationary | Self::SkewRelativeGcl(_) => 3 * D + 2,
        }
    }

    fn at_primal_point<'a>(
        self,
        reference: &[f64],
        velocity: &'a [f64; D],
        velocity_gradient: &'a [[f64; D]; D],
    ) -> Result<PrimalConvectionPoint<'a, D>, Diagnostic> {
        match self {
            Self::Disabled => Ok(PrimalConvectionPoint::Disabled),
            Self::SkewStationary => Ok(PrimalConvectionPoint::Stationary {
                velocity,
                velocity_gradient,
            }),
            Self::SkewRelativeGcl(action) => {
                let mesh_velocity = action.mesh_velocity(reference)?;
                Ok(PrimalConvectionPoint::Ale {
                    relative_velocity: std::array::from_fn(|axis| {
                        velocity[axis] - mesh_velocity[axis]
                    }),
                    velocity,
                    velocity_gradient,
                    mesh_divergence: action.current_velocity_divergence(),
                })
            }
        }
    }
}

/// Geometry direction at the current affine endpoint.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MiniGeometryDirection<'a> {
    #[cfg(test)]
    Zero,
    Endpoint(&'a AffineGeometryLinearization),
}

/// Primal coefficients for one affine MINI/P1 fluid cell.
pub(crate) struct MiniTransientCell<'a, const D: usize> {
    pub(crate) geometry: &'a AffineGeometryMap,
    pub(crate) transport: MiniTransport<'a, D>,
    pub(crate) density: f64,
    pub(crate) viscosity: f64,
    pub(crate) time_step: f64,
    pub(crate) previous_velocity: &'a [[f64; D]],
    pub(crate) current_velocity: &'a [[f64; D]],
    pub(crate) current_pressure: &'a [f64],
}

/// Exact direction through current state and affine geometry.
pub(crate) struct MiniTransientDirection<'a, const D: usize> {
    pub(crate) current_velocity: &'a [[f64; D]],
    pub(crate) current_pressure: &'a [f64],
    pub(crate) current_geometry: MiniGeometryDirection<'a>,
}

/// Residual and analytic JVP evaluated at one identical primal point.
#[derive(Debug)]
pub(crate) struct MiniTransientEvaluation {
    residual: Vec<f64>,
    jvp: Vec<f64>,
}

impl MiniTransientEvaluation {
    pub(crate) fn into_parts(self) -> (Vec<f64>, Vec<f64>) {
        (self.residual, self.jvp)
    }
}

/// Dense state linearization with geometry and spatial source held fixed.
///
/// The projection differentiates only current MINI velocity and pressure. Its
/// body-force callback is sampled once per physical quadrature point for the
/// primal residual and is deliberately not interpreted as a geometry-tangent
/// callback. A moving-geometry source requires a future explicit source JVP
/// contract rather than an implicit derivative through this projection.
#[derive(Debug)]
pub(crate) struct MiniFixedGeometryStateLinearization {
    jacobian: Vec<f64>,
    residual: Vec<f64>,
}

impl MiniFixedGeometryStateLinearization {
    pub(crate) fn into_parts(self) -> (Vec<f64>, Vec<f64>) {
        (self.jacobian, self.residual)
    }
}

/// Congruence scales for a dimensionless affine projection.
///
/// Velocity and pressure are trial/test field scales. `power` is the common
/// action-rate normalization. Applying these factors inside each quadrature
/// accumulation preserves the exact algebra selected by the realization; the
/// projection never rescales an already integrated operator.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MiniAffineScales {
    velocity: f64,
    pressure: f64,
    power: f64,
}

impl MiniAffineScales {
    pub(crate) fn new(velocity: f64, pressure: f64, power: f64) -> Result<Self, Diagnostic> {
        if [velocity, pressure, power]
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(invalid(
                "MINI affine projection scales must be finite and positive",
            ));
        }
        Ok(Self {
            velocity,
            pressure,
            power,
        })
    }
}

/// Linear transient MINI/P1 cell projected directly to scaled `(A, b)` form.
///
/// This projection exists alongside the residual/JVP projection because
/// fixed-reference monolithic assembly owns an affine solve. Direct assembly
/// retains quadrature addition order and never recovers `b` from `A x - R`.
pub(crate) struct MiniScaledAffineCell<'a, const D: usize> {
    pub(crate) geometry: &'a AffineGeometryMap,
    pub(crate) density: f64,
    pub(crate) viscosity: f64,
    pub(crate) time_step: f64,
    pub(crate) previous_velocity: &'a [[f64; D]],
    pub(crate) scales: MiniAffineScales,
}

/// Dense element-local scaled affine projection in MINI/P1 ordering.
#[derive(Debug)]
pub(crate) struct MiniScaledAffineProjection {
    local_size: usize,
    matrix: Vec<f64>,
    rhs: Vec<f64>,
}

impl MiniScaledAffineProjection {
    pub(crate) fn into_parts(self) -> (usize, Vec<f64>, Vec<f64>) {
        (self.local_size, self.matrix, self.rhs)
    }
}

impl<const D: usize> MiniScaledAffineCell<'_, D> {
    /// Integrate the disabled-transport relation directly as scaled `(A, b)`.
    pub(crate) fn project(
        &self,
        quadrature: &QuadratureRule,
    ) -> Result<MiniScaledAffineProjection, Diagnostic> {
        self.validate(quadrature)?;

        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        let local_size = velocity_basis_count * D + p1_basis_count;
        let inverse = self.geometry.inverse_jacobian()?;
        let velocity_space = SimplexP1BubbleSpace::new(D)?;
        let pressure_space = SimplexP1Space::new(D)?;
        let mut matrix = vec![0.0; local_size * local_size];
        let mut rhs = vec![0.0; local_size];

        for point in quadrature.points() {
            let velocity = velocity_space.tabulate(&point.coordinates)?;
            let pressure = pressure_space.tabulate(&point.coordinates)?;
            let gradients = (0..velocity_basis_count)
                .map(|basis| {
                    physical_gradient(
                        velocity.gradient(basis).expect("accepted MINI basis index"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let measure = point.weight * self.geometry.measure_scale();
            let (previous_value, _) =
                evaluate_velocity(self.previous_velocity, velocity.values(), &gradients);

            for row_basis in 0..velocity_basis_count {
                for (row_component, previous_component) in previous_value.iter().enumerate() {
                    let row = local_velocity::<D>(row_basis, row_component);
                    rhs[row] += measure * self.density / self.time_step
                        * velocity.values()[row_basis]
                        * previous_component
                        * self.scales.velocity
                        / self.scales.power;
                    for column_basis in 0..velocity_basis_count {
                        for column_component in 0..D {
                            let column = local_velocity::<D>(column_basis, column_component);
                            let mass = if row_component == column_component {
                                self.density / self.time_step
                                    * velocity.values()[row_basis]
                                    * velocity.values()[column_basis]
                            } else {
                                0.0
                            };
                            matrix[row * local_size + column] += measure
                                * (mass
                                    + self.viscosity
                                        * symmetric_gradient_bilinear_entry(
                                            &gradients[row_basis],
                                            row_component,
                                            &gradients[column_basis],
                                            column_component,
                                        ))
                                * self.scales.velocity
                                * self.scales.velocity
                                / self.scales.power;
                        }
                    }
                    for pressure_basis in 0..p1_basis_count {
                        let pressure_dof = velocity_basis_count * D + pressure_basis;
                        let coupling = -measure
                            * pressure.values()[pressure_basis]
                            * gradients[row_basis][row_component]
                            * self.scales.velocity
                            * self.scales.pressure
                            / self.scales.power;
                        matrix[row * local_size + pressure_dof] += coupling;
                        matrix[pressure_dof * local_size + row] += coupling;
                    }
                }
            }
        }

        if matrix.iter().chain(&rhs).any(|value| !value.is_finite()) {
            return Err(invalid(
                "MINI scaled affine projection produced a non-finite coefficient",
            ));
        }
        Ok(MiniScaledAffineProjection {
            local_size,
            matrix,
            rhs,
        })
    }

    fn validate(&self, quadrature: &QuadratureRule) -> Result<(), Diagnostic> {
        if !matches!(D, 2 | 3) {
            return Err(invalid(
                "MINI scaled affine projection admits dimensions two and three",
            ));
        }
        let velocity_basis_count = D + 2;
        if self.previous_velocity.len() != velocity_basis_count {
            return Err(invalid(format!(
                "{D}D MINI affine projection requires {velocity_basis_count} previous velocity coefficients",
            )));
        }
        if !self.density.is_finite()
            || self.density <= 0.0
            || !self.viscosity.is_finite()
            || self.viscosity <= 0.0
            || !self.time_step.is_finite()
            || self.time_step <= 0.0
            || self
                .previous_velocity
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "MINI affine projection requires finite physical data",
            ));
        }
        if self.geometry.reference_cell().dimension() != D
            || self.geometry.physical_dimension() != D
            || quadrature.reference_cell() != self.geometry.reference_cell()
        {
            return Err(invalid(format!(
                "MINI affine projection requires one affine {D}D simplex and matching quadrature",
            )));
        }
        let required_exactness = MiniTransport::<D>::Disabled.required_quadrature_exactness();
        if quadrature.polynomial_exactness().unwrap_or(0) < required_exactness {
            return Err(invalid(format!(
                "{D}D MINI affine projection requires quadrature exactness at least {required_exactness}, received {}",
                quadrature.polynomial_exactness().unwrap_or(0),
            )));
        }
        Ok(())
    }
}

impl<const D: usize> MiniTransientCell<'_, D> {
    /// Evaluate the fixed-domain skew-transport residual without constructing
    /// or traversing state-Jacobian entries.
    pub(crate) fn residual_fixed_geometry_state<F>(
        &self,
        body_force: &F,
        quadrature: &QuadratureRule,
    ) -> Result<Vec<f64>, Diagnostic>
    where
        F: Fn([f64; D]) -> Result<[f64; D], Diagnostic> + Sync,
    {
        self.project_fixed_geometry_state(body_force, quadrature, None)
    }

    /// Evaluate only the primal transient relation for stationary or ALE transport.
    pub(crate) fn residual(&self, quadrature: &QuadratureRule) -> Result<Vec<f64>, Diagnostic> {
        self.validate_primal(quadrature)?;

        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        let pressure_offset = velocity_basis_count * D;
        let local_dof_count = pressure_offset + p1_basis_count;
        let inverse = self.geometry.inverse_jacobian()?;
        let velocity_space = SimplexP1BubbleSpace::new(D)?;
        let pressure_space = SimplexP1Space::new(D)?;
        let mut residual = vec![0.0; local_dof_count];

        for point in quadrature.points() {
            let velocity_basis = velocity_space.tabulate(&point.coordinates)?;
            let pressure_basis = pressure_space.tabulate(&point.coordinates)?;
            let gradients = (0..velocity_basis_count)
                .map(|basis| {
                    physical_gradient(
                        velocity_basis
                            .gradient(basis)
                            .expect("accepted MINI basis index"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let (velocity, velocity_gradient) =
                evaluate_velocity(self.current_velocity, velocity_basis.values(), &gradients);
            let (previous_velocity, _) =
                evaluate_velocity(self.previous_velocity, velocity_basis.values(), &gradients);
            let pressure = dot(self.current_pressure, pressure_basis.values());
            let measure = point.weight * self.geometry.measure_scale();
            let primal = MiniPrimalPoint {
                density: self.density,
                viscosity: self.viscosity,
                time_step: self.time_step,
                velocity: &velocity,
                previous_velocity: &previous_velocity,
                velocity_gradient: &velocity_gradient,
                pressure,
            };

            for pressure_test in 0..p1_basis_count {
                let row = pressure_offset + pressure_test;
                residual[row] +=
                    measure * primal.continuity_action(pressure_basis.values()[pressure_test]);
            }

            let convection = self.transport.at_primal_point(
                &point.coordinates,
                &velocity,
                &velocity_gradient,
            )?;
            for (row_basis, test_gradient) in gradients.iter().enumerate() {
                let test = velocity_basis.values()[row_basis];
                for row_component in 0..D {
                    let row = local_velocity::<D>(row_basis, row_component);
                    let convective =
                        convection.action(self.density, test, test_gradient, row_component);
                    residual[row] += measure
                        * primal.momentum_action(test, test_gradient, row_component, convective);
                }
            }
        }

        if residual.iter().any(|value| !value.is_finite()) {
            return Err(invalid("MINI transient fluid residual is non-finite"));
        }
        Ok(residual)
    }

    /// Project fixed-domain skew transport to its dense state Jacobian.
    ///
    /// Geometry and the spatial body-force field are parameters of this
    /// projection. The callback is evaluated once per physical quadrature
    /// point and contributes only to the primal residual. Quadrature, row,
    /// column, and accumulation order intentionally match the established 2D
    /// CPU reference projection.
    pub(crate) fn linearize_fixed_geometry_state<F>(
        &self,
        body_force: &F,
        quadrature: &QuadratureRule,
    ) -> Result<MiniFixedGeometryStateLinearization, Diagnostic>
    where
        F: Fn([f64; D]) -> Result<[f64; D], Diagnostic> + Sync,
    {
        let local_dof_count = (D + 2) * D + D + 1;
        let mut jacobian = vec![0.0; local_dof_count * local_dof_count];
        let residual =
            self.project_fixed_geometry_state(body_force, quadrature, Some(&mut jacobian))?;
        Ok(MiniFixedGeometryStateLinearization { jacobian, residual })
    }

    fn project_fixed_geometry_state<F>(
        &self,
        body_force: &F,
        quadrature: &QuadratureRule,
        mut jacobian: Option<&mut [f64]>,
    ) -> Result<Vec<f64>, Diagnostic>
    where
        F: Fn([f64; D]) -> Result<[f64; D], Diagnostic> + Sync,
    {
        if !matches!(self.transport, MiniTransport::SkewStationary) {
            return Err(invalid(
                "fixed-geometry MINI state projection requires stationary skew transport",
            ));
        }
        self.validate_primal(quadrature)?;

        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        let pressure_offset = velocity_basis_count * D;
        let local_dof_count = pressure_offset + p1_basis_count;
        let inverse = self.geometry.inverse_jacobian()?;
        let velocity_space = SimplexP1BubbleSpace::new(D)?;
        let pressure_space = SimplexP1Space::new(D)?;
        if jacobian
            .as_ref()
            .is_some_and(|entries| entries.len() != local_dof_count * local_dof_count)
        {
            return Err(invalid(
                "fixed-geometry MINI state Jacobian sink has the wrong shape",
            ));
        }
        let mut residual = vec![0.0; local_dof_count];

        for point in quadrature.points() {
            let velocity_basis = velocity_space.tabulate(&point.coordinates)?;
            let pressure_basis = pressure_space.tabulate(&point.coordinates)?;
            let gradients = (0..velocity_basis_count)
                .map(|basis| {
                    physical_gradient(
                        velocity_basis
                            .gradient(basis)
                            .expect("accepted MINI basis index"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let (velocity, velocity_gradient) =
                evaluate_velocity(self.current_velocity, velocity_basis.values(), &gradients);
            let (previous_velocity, _) =
                evaluate_velocity(self.previous_velocity, velocity_basis.values(), &gradients);
            let pressure = self
                .current_pressure
                .iter()
                .zip(pressure_basis.values())
                .map(|(coefficient, basis)| coefficient * basis)
                .sum::<f64>();
            let mut coordinates = [0.0; D];
            self.geometry
                .map_point(&point.coordinates, &mut coordinates)?;
            let force = body_force(coordinates)?;
            if force.iter().any(|value| !value.is_finite()) {
                return Err(invalid("MINI Navier--Stokes body force is non-finite"));
            }
            let scale = point.weight * self.geometry.measure_scale();

            let divergence = (0..D)
                .map(|axis| velocity_gradient[axis][axis])
                .sum::<f64>();
            for pressure_test in 0..p1_basis_count {
                residual[pressure_offset + pressure_test] -=
                    scale * pressure_basis.values()[pressure_test] * divergence;
            }

            for row_basis in 0..velocity_basis_count {
                let velocity_dot_row_gradient = dot(&velocity, &gradients[row_basis]);
                for row_component in 0..D {
                    let row = local_velocity::<D>(row_basis, row_component);
                    let test = velocity_basis.values()[row_basis];
                    let time_residual = self.density / self.time_step
                        * test
                        * (velocity[row_component] - previous_velocity[row_component]);
                    let viscous_residual = self.viscosity
                        * projected_symmetric_gradient_test(
                            &velocity_gradient,
                            &gradients[row_basis],
                            row_component,
                        );
                    let pressure_residual = -pressure * gradients[row_basis][row_component];
                    let convective_residual = 0.5
                        * self.density
                        * (dot(&velocity, &velocity_gradient[row_component]) * test
                            - velocity_dot_row_gradient * velocity[row_component]);
                    residual[row] += scale
                        * (time_residual
                            + viscous_residual
                            + pressure_residual
                            + convective_residual
                            - force[row_component] * test);

                    if let Some(jacobian) = jacobian.as_deref_mut() {
                        for column_basis in 0..velocity_basis_count {
                            for column_component in 0..D {
                                let column = local_velocity::<D>(column_basis, column_component);
                                let trial = velocity_basis.values()[column_basis];
                                let mass = if row_component == column_component {
                                    self.density / self.time_step * test * trial
                                } else {
                                    0.0
                                };
                                let viscous = self.viscosity
                                    * symmetric_gradient_bilinear_entry(
                                        &gradients[row_basis],
                                        row_component,
                                        &gradients[column_basis],
                                        column_component,
                                    );
                                let convective = ProjectedConvectiveLinearization {
                                    density: self.density,
                                    velocity: &velocity,
                                    velocity_gradient: &velocity_gradient,
                                    basis: velocity_basis.values(),
                                    gradients: &gradients,
                                }
                                .entry(
                                    row_basis,
                                    row_component,
                                    column_basis,
                                    column_component,
                                );
                                jacobian[row * local_dof_count + column] +=
                                    scale * (mass + viscous + convective);
                            }
                        }
                        for pressure_basis_index in 0..p1_basis_count {
                            let column = pressure_offset + pressure_basis_index;
                            let coupling = -scale
                                * pressure_basis.values()[pressure_basis_index]
                                * gradients[row_basis][row_component];
                            jacobian[row * local_dof_count + column] += coupling;
                            jacobian[column * local_dof_count + row] += coupling;
                        }
                    }
                }
            }
        }

        if residual.iter().any(|value| !value.is_finite())
            || jacobian
                .as_deref()
                .is_some_and(|entries| entries.iter().any(|value| !value.is_finite()))
        {
            return Err(invalid(
                "fixed-geometry MINI state projection produced a non-finite value",
            ));
        }
        Ok(residual)
    }

    /// Evaluate the local weak residual and exact directional action.
    pub(crate) fn evaluate(
        &self,
        direction: MiniTransientDirection<'_, D>,
        quadrature: &QuadratureRule,
    ) -> Result<MiniTransientEvaluation, Diagnostic> {
        self.validate(&direction, quadrature)?;

        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        let pressure_offset = velocity_basis_count * D;
        let local_dof_count = pressure_offset + p1_basis_count;
        let inverse = self.geometry.inverse_jacobian()?;
        let geometry_tangent = GeometryTangent::new(self.geometry, direction.current_geometry)?;
        let velocity_space = SimplexP1BubbleSpace::new(D)?;
        let pressure_space = SimplexP1Space::new(D)?;
        let transport_tangent =
            TransportTangent::new(self.transport, &inverse, &geometry_tangent, self.time_step)?;
        let mut residual = vec![0.0; local_dof_count];
        let mut jvp = vec![0.0; local_dof_count];

        for point in quadrature.points() {
            let velocity_basis = velocity_space.tabulate(&point.coordinates)?;
            let pressure_basis = pressure_space.tabulate(&point.coordinates)?;
            let gradients = (0..velocity_basis_count)
                .map(|basis| {
                    physical_gradient(
                        velocity_basis
                            .gradient(basis)
                            .expect("accepted MINI basis index"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let gradient_tangents = (0..velocity_basis_count)
                .map(|basis| {
                    physical_gradient(
                        velocity_basis
                            .gradient(basis)
                            .expect("accepted MINI basis index"),
                        &geometry_tangent.inverse_jacobian,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let (velocity, velocity_gradient) =
                evaluate_velocity(self.current_velocity, velocity_basis.values(), &gradients);
            let (velocity_tangent, velocity_gradient_tangent) = evaluate_velocity_tangent(
                self.current_velocity,
                direction.current_velocity,
                velocity_basis.values(),
                &gradients,
                &gradient_tangents,
            );
            let (previous_velocity, _) =
                evaluate_velocity(self.previous_velocity, velocity_basis.values(), &gradients);
            let pressure = dot(self.current_pressure, pressure_basis.values());
            let pressure_tangent = dot(direction.current_pressure, pressure_basis.values());
            let divergence_tangent = trace(&velocity_gradient_tangent);
            let measure = point.weight * self.geometry.measure_scale();
            let measure_tangent = point.weight * geometry_tangent.measure_scale;
            let primal = MiniPrimalPoint {
                density: self.density,
                viscosity: self.viscosity,
                time_step: self.time_step,
                velocity: &velocity,
                previous_velocity: &previous_velocity,
                velocity_gradient: &velocity_gradient,
                pressure,
            };

            for pressure_test in 0..p1_basis_count {
                let row = pressure_offset + pressure_test;
                let integrand = primal.continuity_action(pressure_basis.values()[pressure_test]);
                let integrand_tangent =
                    -pressure_basis.values()[pressure_test] * divergence_tangent;
                accumulate(
                    &mut residual[row],
                    &mut jvp[row],
                    measure,
                    measure_tangent,
                    integrand,
                    integrand_tangent,
                );
            }

            let point_state = PointState {
                velocity: &velocity,
                velocity_tangent: &velocity_tangent,
                velocity_gradient: &velocity_gradient,
                velocity_gradient_tangent: &velocity_gradient_tangent,
            };
            let convection = transport_tangent.at_point(
                &point.coordinates,
                point_state,
                &geometry_tangent,
                self.time_step,
            )?;
            for row_basis in 0..velocity_basis_count {
                let test = velocity_basis.values()[row_basis];
                let test_gradient = &gradients[row_basis];
                let test_gradient_tangent = &gradient_tangents[row_basis];

                for row_component in 0..D {
                    let row = local_velocity::<D>(row_basis, row_component);
                    let time_tangent =
                        self.density / self.time_step * test * velocity_tangent[row_component];
                    let viscous_tangent = self.viscosity
                        * symmetric_gradient_test_tangent(
                            &velocity_gradient,
                            &velocity_gradient_tangent,
                            test_gradient,
                            test_gradient_tangent,
                            row_component,
                        );
                    let pressure_action_tangent = -pressure_tangent * test_gradient[row_component]
                        - pressure * test_gradient_tangent[row_component];
                    let (convective, convective_tangent) = convection.action(
                        self.density,
                        test,
                        test_gradient,
                        test_gradient_tangent,
                        row_component,
                    );
                    accumulate(
                        &mut residual[row],
                        &mut jvp[row],
                        measure,
                        measure_tangent,
                        primal.momentum_action(test, test_gradient, row_component, convective),
                        time_tangent
                            + viscous_tangent
                            + pressure_action_tangent
                            + convective_tangent,
                    );
                }
            }
        }

        if residual.iter().chain(&jvp).any(|value| !value.is_finite()) {
            return Err(invalid(
                "MINI transient fluid residual or analytic JVP is non-finite",
            ));
        }
        Ok(MiniTransientEvaluation { residual, jvp })
    }

    fn validate(
        &self,
        direction: &MiniTransientDirection<'_, D>,
        quadrature: &QuadratureRule,
    ) -> Result<(), Diagnostic> {
        self.validate_primal(quadrature)?;
        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        if direction.current_velocity.len() != velocity_basis_count
            || direction.current_pressure.len() != p1_basis_count
        {
            return Err(invalid(format!(
                "{D}D MINI transient fluid direction requires {velocity_basis_count} velocity and {p1_basis_count} pressure coefficients",
            )));
        }
        if direction
            .current_velocity
            .iter()
            .flatten()
            .chain(direction.current_pressure)
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "MINI transient fluid relation requires finite direction data",
            ));
        }
        if matches!(
            direction.current_geometry,
            MiniGeometryDirection::Endpoint(linearization) if linearization.map() != self.geometry
        ) {
            return Err(invalid(
                "MINI geometry direction must linearize the exact current affine map",
            ));
        }
        Ok(())
    }

    fn validate_primal(&self, quadrature: &QuadratureRule) -> Result<(), Diagnostic> {
        if !matches!(D, 2 | 3) {
            return Err(invalid(
                "MINI transient fluid relation admits dimensions two and three",
            ));
        }
        let p1_basis_count = D + 1;
        let velocity_basis_count = D + 2;
        if self.previous_velocity.len() != velocity_basis_count
            || self.current_velocity.len() != velocity_basis_count
            || self.current_pressure.len() != p1_basis_count
        {
            return Err(invalid(format!(
                "{D}D MINI transient fluid state requires {velocity_basis_count} velocity and {p1_basis_count} pressure coefficients",
            )));
        }
        if !self.density.is_finite()
            || self.density <= 0.0
            || !self.viscosity.is_finite()
            || self.viscosity <= 0.0
            || !self.time_step.is_finite()
            || self.time_step <= 0.0
            || self
                .previous_velocity
                .iter()
                .chain(self.current_velocity)
                .flatten()
                .chain(self.current_pressure)
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "MINI transient fluid relation requires finite physical state data",
            ));
        }
        if self.geometry.reference_cell().dimension() != D
            || self.geometry.physical_dimension() != D
            || quadrature.reference_cell() != self.geometry.reference_cell()
        {
            return Err(invalid(format!(
                "MINI transient fluid relation requires one affine {D}D simplex and matching quadrature",
            )));
        }
        if matches!(
            self.transport,
            MiniTransport::SkewRelativeGcl(action) if action.current_map() != self.geometry
        ) {
            return Err(invalid(
                "ALE MINI transport requires the exact current geometry carried by its sealed action",
            ));
        }
        let required_exactness = self.transport.required_quadrature_exactness();
        if quadrature.polynomial_exactness().unwrap_or(0) < required_exactness {
            return Err(invalid(format!(
                "{D}D MINI transient fluid transport requires quadrature exactness at least {required_exactness}, received {}",
                quadrature.polynomial_exactness().unwrap_or(0),
            )));
        }
        Ok(())
    }
}

struct MiniPrimalPoint<'a, const D: usize> {
    density: f64,
    viscosity: f64,
    time_step: f64,
    velocity: &'a [f64; D],
    previous_velocity: &'a [f64; D],
    velocity_gradient: &'a [[f64; D]; D],
    pressure: f64,
}

impl<const D: usize> MiniPrimalPoint<'_, D> {
    fn continuity_action(&self, pressure_test: f64) -> f64 {
        -pressure_test * trace(self.velocity_gradient)
    }

    fn momentum_action(
        &self,
        test: f64,
        test_gradient: &[f64],
        component: usize,
        convection: f64,
    ) -> f64 {
        let time = self.density / self.time_step
            * test
            * (self.velocity[component] - self.previous_velocity[component]);
        let viscous = self.viscosity
            * symmetric_gradient_test(self.velocity_gradient, test_gradient, component);
        let pressure = -self.pressure * test_gradient[component];
        time + viscous + pressure + convection
    }
}

struct GeometryTangent {
    inverse_jacobian: Vec<f64>,
    origin: Vec<f64>,
    jacobian: Vec<f64>,
    measure_scale: f64,
}

impl GeometryTangent {
    fn new(
        _geometry: &AffineGeometryMap,
        direction: MiniGeometryDirection<'_>,
    ) -> Result<Self, Diagnostic> {
        match direction {
            #[cfg(test)]
            MiniGeometryDirection::Zero => Ok(Self {
                inverse_jacobian: vec![0.0; _geometry.jacobian().len()],
                origin: vec![0.0; _geometry.physical_dimension()],
                jacobian: vec![0.0; _geometry.jacobian().len()],
                measure_scale: 0.0,
            }),
            MiniGeometryDirection::Endpoint(linearization) => Ok(Self {
                inverse_jacobian: linearization.inverse_jacobian_tangent()?,
                origin: linearization.origin_tangent().to_vec(),
                jacobian: linearization.jacobian_tangent().to_vec(),
                measure_scale: linearization.measure_scale_tangent(),
            }),
        }
    }

    fn map_point<const D: usize>(&self, reference: &[f64]) -> [f64; D] {
        std::array::from_fn(|row| {
            self.origin[row]
                + reference
                    .iter()
                    .enumerate()
                    .map(|(column, coordinate)| self.jacobian[row * D + column] * coordinate)
                    .sum::<f64>()
        })
    }
}

enum PrimalConvectionPoint<'a, const D: usize> {
    Disabled,
    Stationary {
        velocity: &'a [f64; D],
        velocity_gradient: &'a [[f64; D]; D],
    },
    Ale {
        relative_velocity: [f64; D],
        velocity: &'a [f64; D],
        velocity_gradient: &'a [[f64; D]; D],
        mesh_divergence: f64,
    },
}

impl<const D: usize> PrimalConvectionPoint<'_, D> {
    fn action(&self, density: f64, test: f64, test_gradient: &[f64], component: usize) -> f64 {
        match self {
            Self::Disabled => 0.0,
            Self::Stationary {
                velocity,
                velocity_gradient,
            } => stationary_convection_action(
                density,
                velocity,
                velocity_gradient,
                test,
                test_gradient,
                component,
            ),
            Self::Ale {
                relative_velocity,
                velocity,
                velocity_gradient,
                mesh_divergence,
            } => ale_convection_action(
                density,
                relative_velocity,
                velocity,
                velocity_gradient,
                *mesh_divergence,
                test,
                test_gradient,
                component,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stationary_convection_action<const D: usize>(
    density: f64,
    velocity: &[f64; D],
    velocity_gradient: &[[f64; D]; D],
    test: f64,
    test_gradient: &[f64],
    component: usize,
) -> f64 {
    0.5 * density
        * (dot(velocity, &velocity_gradient[component]) * test
            - dot(velocity, test_gradient) * velocity[component])
}

#[allow(clippy::too_many_arguments)]
fn ale_convection_action<const D: usize>(
    density: f64,
    relative_velocity: &[f64; D],
    velocity: &[f64; D],
    velocity_gradient: &[[f64; D]; D],
    mesh_divergence: f64,
    test: f64,
    test_gradient: &[f64],
    component: usize,
) -> f64 {
    0.5 * density
        * (dot(relative_velocity, &velocity_gradient[component]) * test
            - dot(relative_velocity, test_gradient) * velocity[component]
            + mesh_divergence * velocity[component] * test)
}

enum TransportTangent<'a, const D: usize> {
    Disabled,
    SkewStationary,
    SkewRelativeGcl {
        action: &'a FixedTopologyCellGeometryAction<D>,
        mesh_divergence_tangent: f64,
    },
}

impl<'a, const D: usize> TransportTangent<'a, D> {
    fn new(
        transport: MiniTransport<'a, D>,
        inverse: &[f64],
        geometry_tangent: &GeometryTangent,
        time_step: f64,
    ) -> Result<Self, Diagnostic> {
        match transport {
            MiniTransport::Disabled => Ok(Self::Disabled),
            MiniTransport::SkewStationary => Ok(Self::SkewStationary),
            MiniTransport::SkewRelativeGcl(action) => {
                let reference_tangent = geometry_tangent
                    .jacobian
                    .iter()
                    .map(|value| value / time_step)
                    .collect::<Vec<_>>();
                let gradient_tangent = multiply_linearization::<D>(
                    &reference_tangent,
                    inverse,
                    action.reference_velocity_gradient(),
                    &geometry_tangent.inverse_jacobian,
                )?;
                Ok(Self::SkewRelativeGcl {
                    action,
                    mesh_divergence_tangent: trace_flat::<D>(&gradient_tangent),
                })
            }
        }
    }

    fn at_point(
        &'a self,
        reference: &[f64],
        state: PointState<'a, D>,
        geometry_tangent: &GeometryTangent,
        time_step: f64,
    ) -> Result<ConvectionPoint<'a, D>, Diagnostic> {
        match self {
            Self::Disabled => Ok(ConvectionPoint::Disabled),
            Self::SkewStationary => Ok(ConvectionPoint::Stationary {
                velocity: state.velocity,
                velocity_tangent: state.velocity_tangent,
                velocity_gradient: state.velocity_gradient,
                velocity_gradient_tangent: state.velocity_gradient_tangent,
            }),
            Self::SkewRelativeGcl {
                action,
                mesh_divergence_tangent,
            } => {
                let mesh_velocity = action.mesh_velocity(reference)?;
                let mesh_velocity_tangent = geometry_tangent
                    .map_point::<D>(reference)
                    .map(|value| value / time_step);
                Ok(ConvectionPoint::Ale {
                    relative_velocity: std::array::from_fn(|axis| {
                        state.velocity[axis] - mesh_velocity[axis]
                    }),
                    relative_velocity_tangent: std::array::from_fn(|axis| {
                        state.velocity_tangent[axis] - mesh_velocity_tangent[axis]
                    }),
                    velocity: state.velocity,
                    velocity_tangent: state.velocity_tangent,
                    velocity_gradient: state.velocity_gradient,
                    velocity_gradient_tangent: state.velocity_gradient_tangent,
                    mesh_divergence: action.current_velocity_divergence(),
                    mesh_divergence_tangent: *mesh_divergence_tangent,
                })
            }
        }
    }
}

struct PointState<'a, const D: usize> {
    velocity: &'a [f64; D],
    velocity_tangent: &'a [f64; D],
    velocity_gradient: &'a [[f64; D]; D],
    velocity_gradient_tangent: &'a [[f64; D]; D],
}

enum ConvectionPoint<'a, const D: usize> {
    Disabled,
    Stationary {
        velocity: &'a [f64; D],
        velocity_tangent: &'a [f64; D],
        velocity_gradient: &'a [[f64; D]; D],
        velocity_gradient_tangent: &'a [[f64; D]; D],
    },
    Ale {
        relative_velocity: [f64; D],
        relative_velocity_tangent: [f64; D],
        velocity: &'a [f64; D],
        velocity_tangent: &'a [f64; D],
        velocity_gradient: &'a [[f64; D]; D],
        velocity_gradient_tangent: &'a [[f64; D]; D],
        mesh_divergence: f64,
        mesh_divergence_tangent: f64,
    },
}

impl<const D: usize> ConvectionPoint<'_, D> {
    fn action(
        &self,
        density: f64,
        test: f64,
        test_gradient: &[f64],
        test_gradient_tangent: &[f64],
        component: usize,
    ) -> (f64, f64) {
        match self {
            Self::Disabled => (0.0, 0.0),
            Self::Stationary {
                velocity,
                velocity_tangent,
                velocity_gradient,
                velocity_gradient_tangent,
            } => {
                let velocity_dot_test_gradient = dot(*velocity, test_gradient);
                let velocity_dot_test_gradient_tangent =
                    dot(*velocity_tangent, test_gradient) + dot(*velocity, test_gradient_tangent);
                let velocity_dot_velocity_gradient_tangent =
                    dot(*velocity_tangent, &velocity_gradient[component])
                        + dot(*velocity, &velocity_gradient_tangent[component]);
                (
                    stationary_convection_action(
                        density,
                        velocity,
                        velocity_gradient,
                        test,
                        test_gradient,
                        component,
                    ),
                    0.5 * density
                        * (velocity_dot_velocity_gradient_tangent * test
                            - velocity_dot_test_gradient_tangent * velocity[component]
                            - velocity_dot_test_gradient * velocity_tangent[component]),
                )
            }
            Self::Ale {
                relative_velocity,
                relative_velocity_tangent,
                velocity,
                velocity_tangent,
                velocity_gradient,
                velocity_gradient_tangent,
                mesh_divergence,
                mesh_divergence_tangent,
            } => {
                let relative_dot_test_gradient = dot(relative_velocity, test_gradient);
                let relative_dot_test_gradient_tangent =
                    dot(relative_velocity_tangent, test_gradient)
                        + dot(relative_velocity, test_gradient_tangent);
                let relative_dot_velocity_gradient_tangent =
                    dot(relative_velocity_tangent, &velocity_gradient[component])
                        + dot(relative_velocity, &velocity_gradient_tangent[component]);
                (
                    ale_convection_action(
                        density,
                        relative_velocity,
                        velocity,
                        velocity_gradient,
                        *mesh_divergence,
                        test,
                        test_gradient,
                        component,
                    ),
                    0.5 * density
                        * (relative_dot_velocity_gradient_tangent * test
                            - relative_dot_test_gradient_tangent * velocity[component]
                            - relative_dot_test_gradient * velocity_tangent[component]
                            + mesh_divergence_tangent * velocity[component] * test
                            + mesh_divergence * velocity_tangent[component] * test),
                )
            }
        }
    }
}

fn evaluate_velocity<const D: usize>(
    coefficients: &[[f64; D]],
    basis: &[f64],
    gradients: &[Vec<f64>],
) -> ([f64; D], [[f64; D]; D]) {
    let mut value = [0.0; D];
    let mut gradient = [[0.0; D]; D];
    for local in 0..coefficients.len() {
        for component in 0..D {
            value[component] += coefficients[local][component] * basis[local];
            for axis in 0..D {
                gradient[component][axis] +=
                    coefficients[local][component] * gradients[local][axis];
            }
        }
    }
    (value, gradient)
}

fn evaluate_velocity_tangent<const D: usize>(
    coefficients: &[[f64; D]],
    coefficient_tangents: &[[f64; D]],
    basis: &[f64],
    gradients: &[Vec<f64>],
    gradient_tangents: &[Vec<f64>],
) -> ([f64; D], [[f64; D]; D]) {
    let mut value = [0.0; D];
    let mut gradient = [[0.0; D]; D];
    for local in 0..coefficients.len() {
        for component in 0..D {
            value[component] += coefficient_tangents[local][component] * basis[local];
            for axis in 0..D {
                gradient[component][axis] += coefficient_tangents[local][component]
                    * gradients[local][axis]
                    + coefficients[local][component] * gradient_tangents[local][axis];
            }
        }
    }
    (value, gradient)
}

fn multiply_linearization<const D: usize>(
    left_tangent: &[f64],
    right: &[f64],
    left: &[f64],
    right_tangent: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let matrix_entries = D
        .checked_mul(D)
        .ok_or_else(|| invalid("MINI transport matrix shape overflows usize"))?;
    if left_tangent.len() != matrix_entries
        || right.len() != matrix_entries
        || left.len() != matrix_entries
        || right_tangent.len() != matrix_entries
    {
        return Err(invalid(
            "MINI transport linearization requires four square matrices",
        ));
    }
    Ok((0..matrix_entries)
        .map(|entry| {
            let row = entry / D;
            let column = entry % D;
            (0..D)
                .map(|axis| {
                    left_tangent[row * D + axis] * right[axis * D + column]
                        + left[row * D + axis] * right_tangent[axis * D + column]
                })
                .sum()
        })
        .collect())
}

fn symmetric_gradient_test<const D: usize>(
    gradient: &[[f64; D]; D],
    test_gradient: &[f64],
    test_component: usize,
) -> f64 {
    (0..D)
        .map(|axis| {
            (gradient[test_component][axis] + gradient[axis][test_component]) * test_gradient[axis]
        })
        .sum()
}

fn symmetric_gradient_test_tangent<const D: usize>(
    gradient: &[[f64; D]; D],
    gradient_tangent: &[[f64; D]; D],
    test_gradient: &[f64],
    test_gradient_tangent: &[f64],
    test_component: usize,
) -> f64 {
    (0..D)
        .map(|axis| {
            (gradient_tangent[test_component][axis] + gradient_tangent[axis][test_component])
                * test_gradient[axis]
                + (gradient[test_component][axis] + gradient[axis][test_component])
                    * test_gradient_tangent[axis]
        })
        .sum()
}

/// State-projection form with the established per-axis operation order.
fn projected_symmetric_gradient_test<const D: usize>(
    gradient: &[[f64; D]; D],
    test_gradient: &[f64],
    test_component: usize,
) -> f64 {
    (0..D)
        .map(|axis| {
            gradient[test_component][axis] * test_gradient[axis]
                + gradient[axis][test_component] * test_gradient[axis]
        })
        .sum()
}

struct ProjectedConvectiveLinearization<'a, const D: usize> {
    density: f64,
    velocity: &'a [f64; D],
    velocity_gradient: &'a [[f64; D]; D],
    basis: &'a [f64],
    gradients: &'a [Vec<f64>],
}

impl<const D: usize> ProjectedConvectiveLinearization<'_, D> {
    fn entry(
        &self,
        row_basis: usize,
        row_component: usize,
        column_basis: usize,
        column_component: usize,
    ) -> f64 {
        let row_value = self.basis[row_basis];
        let column_value = self.basis[column_basis];
        let diagonal = usize::from(row_component == column_component) as f64;
        0.5 * self.density
            * (column_value * self.velocity_gradient[row_component][column_component] * row_value
                + diagonal * dot(self.velocity, &self.gradients[column_basis]) * row_value
                - column_value
                    * self.gradients[row_basis][column_component]
                    * self.velocity[row_component]
                - diagonal * dot(self.velocity, &self.gradients[row_basis]) * column_value)
    }
}

fn accumulate(
    residual: &mut f64,
    jvp: &mut f64,
    measure: f64,
    measure_tangent: f64,
    integrand: f64,
    integrand_tangent: f64,
) {
    *residual += measure * integrand;
    *jvp += measure_tangent * integrand + measure * integrand_tangent;
}

fn local_velocity<const D: usize>(basis: usize, component: usize) -> usize {
    basis * D + component
}

fn trace<const D: usize>(matrix: &[[f64; D]; D]) -> f64 {
    let mut value = matrix[0][0];
    for (axis, row) in matrix.iter().enumerate().skip(1) {
        value += row[axis];
    }
    value
}

fn trace_flat<const D: usize>(matrix: &[f64]) -> f64 {
    let mut value = matrix[0];
    for axis in 1..D {
        value += matrix[axis * D + axis];
    }
    value
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_meshing::{
        FixedTopologyGeometryAction, FixedTopologyGeometryState, MeshQualityGate, SimplicialMesh,
        simplex_duffy_gauss_legendre,
    };

    #[test]
    fn transport_exactness_is_dimension_and_identity_specific() {
        assert_eq!(
            MiniTransport::<2>::Disabled.required_quadrature_exactness(),
            6
        );
        assert_eq!(
            MiniTransport::<3>::Disabled.required_quadrature_exactness(),
            8
        );
        assert_eq!(
            MiniTransport::<2>::SkewStationary.required_quadrature_exactness(),
            8
        );
        assert_eq!(
            MiniTransport::<3>::SkewStationary.required_quadrature_exactness(),
            11
        );

        let mesh = tetrahedron();
        let state = FixedTopologyGeometryState::<3>::reference(&mesh).unwrap();
        let action = FixedTopologyGeometryAction::<3>::new(&mesh, &state, &state, 0.25).unwrap();
        assert_eq!(
            MiniTransport::SkewRelativeGcl(action.cell(0).unwrap()).required_quadrature_exactness(),
            11
        );
    }

    #[test]
    fn stationary_dense_jacobian_is_the_direct_state_jvp() {
        let geometry = stationary_triangle();
        let previous = [[0.17, -0.08], [0.11, 0.06], [-0.04, 0.13], [0.025, -0.035]];
        let current = [[0.21, -0.02], [0.09, 0.075], [-0.055, 0.16], [0.04, -0.015]];
        let pressure = [0.14, -0.065, 0.035];
        let velocity_direction = [[0.03, -0.01], [-0.02, 0.04], [0.015, 0.025], [-0.01, 0.02]];
        let pressure_direction = [-0.03, 0.02, 0.01];
        let quadrature = simplex_duffy_gauss_legendre(2, 5).unwrap();
        let cell = MiniTransientCell::<2> {
            geometry: &geometry,
            transport: MiniTransport::SkewStationary,
            density: 1.35,
            viscosity: 0.07,
            time_step: 0.18,
            previous_velocity: &previous,
            current_velocity: &current,
            current_pressure: &pressure,
        };
        let (jacobian, residual) = cell
            .linearize_fixed_geometry_state(&|_| Ok([0.0; 2]), &quadrature)
            .unwrap()
            .into_parts();
        let (direct_residual, direct_jvp) = cell
            .evaluate(
                MiniTransientDirection {
                    current_velocity: &velocity_direction,
                    current_pressure: &pressure_direction,
                    current_geometry: MiniGeometryDirection::Zero,
                },
                &quadrature,
            )
            .unwrap()
            .into_parts();
        assert_eq!(
            residual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            direct_residual
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        let direction = velocity_direction
            .iter()
            .flatten()
            .copied()
            .chain(pressure_direction)
            .collect::<Vec<_>>();
        let projected = jacobian
            .chunks_exact(direction.len())
            .map(|row| {
                row.iter()
                    .zip(&direction)
                    .map(|(entry, direction)| entry * direction)
                    .sum::<f64>()
            })
            .collect::<Vec<_>>();
        for (row, (projected, direct)) in projected.iter().zip(&direct_jvp).enumerate() {
            let tolerance = 4096.0 * f64::EPSILON * projected.abs().max(direct.abs()).max(1.0);
            assert!(
                (projected - direct).abs() <= tolerance,
                "stationary state row {row}: {projected:e} versus {direct:e}",
            );
        }
    }

    #[test]
    fn stationary_projection_fails_closed_on_exactness_and_body_force() {
        let geometry = stationary_triangle();
        let velocity = [[0.1, -0.05]; 4];
        let pressure = [0.0; 3];
        let cell = MiniTransientCell::<2> {
            geometry: &geometry,
            transport: MiniTransport::SkewStationary,
            density: 1.0,
            viscosity: 0.1,
            time_step: 0.25,
            previous_velocity: &velocity,
            current_velocity: &velocity,
            current_pressure: &pressure,
        };
        let low_rule = simplex_duffy_gauss_legendre(2, 4).unwrap();
        let error = cell
            .linearize_fixed_geometry_state(&|_| Ok([0.0; 2]), &low_rule)
            .unwrap_err();
        assert!(error.message().contains("at least 8"));

        let quadrature = simplex_duffy_gauss_legendre(2, 5).unwrap();
        let callback_error = cell
            .linearize_fixed_geometry_state(
                &|_| {
                    Err(Diagnostic::error(
                        codes::INVALID_DISCRETIZATION,
                        "source sentinel",
                    ))
                },
                &quadrature,
            )
            .unwrap_err();
        assert_eq!(callback_error.message(), "source sentinel");
        let non_finite = cell
            .linearize_fixed_geometry_state(&|_| Ok([f64::INFINITY, 0.0]), &quadrature)
            .unwrap_err();
        assert!(non_finite.message().contains("body force is non-finite"));
    }

    #[test]
    fn scaled_affine_projection_is_the_disabled_relation_in_two_and_three_dimensions() {
        let triangle = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.1, -0.2],
            vec![1.3, 0.1],
            vec![-0.2, 0.9],
        ])
        .unwrap();
        assert_scaled_affine_identity(
            &triangle,
            &simplex_duffy_gauss_legendre(2, 4).unwrap(),
            &[[0.17, -0.08], [0.11, 0.04], [-0.06, 0.13], [0.07, -0.03]],
            &[[0.12, -0.03], [0.08, 0.09], [-0.02, 0.11], [0.05, -0.04]],
            &[0.2, -0.07, 0.03],
        );

        let tetrahedron = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.1, -0.1, 0.05],
            vec![1.3, 0.1, -0.05],
            vec![0.2, 1.1, 0.15],
            vec![-0.1, 0.2, 1.2],
        ])
        .unwrap();
        assert_scaled_affine_identity(
            &tetrahedron,
            &simplex_duffy_gauss_legendre(3, 6).unwrap(),
            &[
                [0.17, -0.08, 0.03],
                [0.11, 0.04, -0.02],
                [-0.06, 0.13, 0.05],
                [0.09, -0.02, 0.07],
                [0.07, -0.03, 0.02],
            ],
            &[
                [0.12, -0.03, 0.04],
                [0.08, 0.09, -0.02],
                [-0.02, 0.11, 0.06],
                [0.03, -0.05, 0.08],
                [0.05, -0.04, 0.01],
            ],
            &[0.2, -0.07, 0.03, -0.04],
        );
    }

    #[test]
    fn three_dimensional_ale_rejects_the_current_degree_nine_rule() {
        let mesh = tetrahedron();
        let previous = FixedTopologyGeometryState::<3>::reference(&mesh).unwrap();
        let current = FixedTopologyGeometryState::<3>::new(
            &mesh,
            vec![
                vec![0.01, -0.02, 0.00],
                vec![1.04, 0.01, 0.02],
                vec![0.02, 0.97, 0.01],
                vec![-0.01, 0.02, 1.03],
            ],
        )
        .unwrap();
        let action =
            FixedTopologyGeometryAction::<3>::new(&mesh, &previous, &current, 0.25).unwrap();
        let cell = action.cell(0).unwrap();
        let geometry_direction = AffineGeometryLinearization::new(
            cell.current_map().clone(),
            vec![0.01, -0.02, 0.03],
            vec![0.02, -0.01, 0.00, 0.01, 0.03, -0.02, -0.01, 0.02, 0.01],
        )
        .unwrap();
        let previous_velocity = vec![[0.1, -0.1, 0.05]; 5];
        let current_velocity = vec![
            [0.12, -0.08, 0.04],
            [0.09, -0.04, 0.03],
            [0.08, -0.05, 0.06],
            [0.11, -0.06, 0.02],
            [0.02, 0.01, -0.01],
        ];
        let velocity_direction = vec![[0.01, -0.02, 0.03]; 5];
        let pressure = [0.1, -0.03, 0.02, -0.04];
        let pressure_direction = [-0.01, 0.02, 0.03, -0.02];
        let error = MiniTransientCell::<3> {
            geometry: cell.current_map(),
            transport: MiniTransport::SkewRelativeGcl(cell),
            density: 1.2,
            viscosity: 0.04,
            time_step: 0.25,
            previous_velocity: &previous_velocity,
            current_velocity: &current_velocity,
            current_pressure: &pressure,
        }
        .evaluate(
            MiniTransientDirection {
                current_velocity: &velocity_direction,
                current_pressure: &pressure_direction,
                current_geometry: MiniGeometryDirection::Endpoint(&geometry_direction),
            },
            &simplex_duffy_gauss_legendre(3, 6).unwrap(),
        )
        .unwrap_err();
        assert!(error.message().contains("at least 11"));
    }

    #[test]
    fn three_dimensional_moving_ale_jvp_matches_centered_reassembly() {
        const STEP: f64 = 0.25;
        let mesh = tetrahedron();
        let previous = FixedTopologyGeometryState::<3>::reference(&mesh).unwrap();
        let current_coordinates = vec![
            vec![0.01, -0.02, 0.00],
            vec![1.04, 0.01, 0.02],
            vec![0.02, 0.97, 0.01],
            vec![-0.01, 0.02, 1.03],
        ];
        let coordinate_direction = [
            [0.01, -0.02, 0.03],
            [0.03, -0.01, 0.01],
            [0.00, 0.02, -0.01],
            [-0.02, 0.01, 0.04],
        ];
        let current =
            FixedTopologyGeometryState::<3>::new(&mesh, current_coordinates.clone()).unwrap();
        let action =
            FixedTopologyGeometryAction::<3>::new(&mesh, &previous, &current, STEP).unwrap();
        let geometry_direction = tetrahedron_geometry_direction(
            action.cell(0).unwrap().current_map(),
            &coordinate_direction,
        );
        let previous_velocity = vec![[0.1, -0.1, 0.05]; 5];
        let current_velocity = vec![
            [0.12, -0.08, 0.04],
            [0.09, -0.04, 0.03],
            [0.08, -0.05, 0.06],
            [0.11, -0.06, 0.02],
            [0.02, 0.01, -0.01],
        ];
        let velocity_direction = vec![
            [0.01, -0.02, 0.03],
            [-0.02, 0.01, 0.02],
            [0.03, 0.00, -0.01],
            [0.01, 0.02, -0.02],
            [-0.01, 0.03, 0.01],
        ];
        let pressure = [0.1, -0.03, 0.02, -0.04];
        let pressure_direction = [-0.01, 0.02, 0.03, -0.02];
        let quadrature = simplex_duffy_gauss_legendre(3, 7).unwrap();
        let evaluated = MiniTransientCell::<3> {
            geometry: action.cell(0).unwrap().current_map(),
            transport: MiniTransport::SkewRelativeGcl(action.cell(0).unwrap()),
            density: 1.2,
            viscosity: 0.04,
            time_step: STEP,
            previous_velocity: &previous_velocity,
            current_velocity: &current_velocity,
            current_pressure: &pressure,
        }
        .evaluate(
            MiniTransientDirection {
                current_velocity: &velocity_direction,
                current_pressure: &pressure_direction,
                current_geometry: MiniGeometryDirection::Endpoint(&geometry_direction),
            },
            &quadrature,
        )
        .unwrap();
        let (_, analytic) = evaluated.into_parts();

        let epsilon = f64::EPSILON.cbrt();
        let perturbed = |sign: f64| {
            let coordinates = current_coordinates
                .iter()
                .zip(coordinate_direction)
                .map(|(coordinate, direction)| {
                    coordinate
                        .iter()
                        .zip(direction)
                        .map(|(value, direction)| value + sign * epsilon * direction)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let geometry = FixedTopologyGeometryState::<3>::new(&mesh, coordinates).unwrap();
            let action =
                FixedTopologyGeometryAction::<3>::new(&mesh, &previous, &geometry, STEP).unwrap();
            let velocity = current_velocity
                .iter()
                .zip(&velocity_direction)
                .map(|(velocity, direction)| {
                    std::array::from_fn(|axis| velocity[axis] + sign * epsilon * direction[axis])
                })
                .collect::<Vec<_>>();
            let pressure = std::array::from_fn::<_, 4, _>(|basis| {
                pressure[basis] + sign * epsilon * pressure_direction[basis]
            });
            let zero_velocity = vec![[0.0; 3]; 5];
            let zero_pressure = [0.0; 4];
            MiniTransientCell::<3> {
                geometry: action.cell(0).unwrap().current_map(),
                transport: MiniTransport::SkewRelativeGcl(action.cell(0).unwrap()),
                density: 1.2,
                viscosity: 0.04,
                time_step: STEP,
                previous_velocity: &previous_velocity,
                current_velocity: &velocity,
                current_pressure: &pressure,
            }
            .evaluate(
                MiniTransientDirection {
                    current_velocity: &zero_velocity,
                    current_pressure: &zero_pressure,
                    current_geometry: MiniGeometryDirection::Zero,
                },
                &quadrature,
            )
            .unwrap()
            .into_parts()
            .0
        };
        let plus = perturbed(1.0);
        let minus = perturbed(-1.0);
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
        assert!(error < 5.0e-7 * (1.0 + scale), "{error:e} versus {scale:e}");
        assert!(analytic.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn coefficient_shapes_fail_closed_before_evaluation() {
        let mesh = tetrahedron();
        let state = FixedTopologyGeometryState::<3>::reference(&mesh).unwrap();
        let action = FixedTopologyGeometryAction::<3>::new(&mesh, &state, &state, 0.25).unwrap();
        let cell = action.cell(0).unwrap();
        let short_velocity = vec![[0.0; 3]; 4];
        let pressure = [0.0; 4];
        let direction_velocity = vec![[0.0; 3]; 5];
        let error = MiniTransientCell::<3> {
            geometry: cell.current_map(),
            transport: MiniTransport::Disabled,
            density: 1.0,
            viscosity: 1.0,
            time_step: 0.25,
            previous_velocity: &short_velocity,
            current_velocity: &short_velocity,
            current_pressure: &pressure,
        }
        .evaluate(
            MiniTransientDirection {
                current_velocity: &direction_velocity,
                current_pressure: &pressure,
                current_geometry: MiniGeometryDirection::Zero,
            },
            &simplex_duffy_gauss_legendre(3, 6).unwrap(),
        )
        .unwrap_err();
        assert!(error.message().contains("5 velocity"));
    }

    #[test]
    fn disabled_transport_accepts_linear_exactness_while_skew_transport_rejects_it() {
        let mesh = tetrahedron();
        let state = FixedTopologyGeometryState::<3>::reference(&mesh).unwrap();
        let action = FixedTopologyGeometryAction::<3>::new(&mesh, &state, &state, 0.25).unwrap();
        let cell = action.cell(0).unwrap();
        let velocity = vec![[0.1, -0.05, 0.02]; 5];
        let direction = vec![[0.0; 3]; 5];
        let pressure = [0.0; 4];
        let source_rule = simplex_duffy_gauss_legendre(3, 6).unwrap();
        let linear_rule = QuadratureRule::new(
            source_rule.reference_cell(),
            Some(8),
            source_rule.points().to_vec(),
        )
        .unwrap();
        let evaluate = |transport| {
            MiniTransientCell::<3> {
                geometry: cell.current_map(),
                transport,
                density: 1.0,
                viscosity: 0.1,
                time_step: 0.25,
                previous_velocity: &velocity,
                current_velocity: &velocity,
                current_pressure: &pressure,
            }
            .evaluate(
                MiniTransientDirection {
                    current_velocity: &direction,
                    current_pressure: &pressure,
                    current_geometry: MiniGeometryDirection::Zero,
                },
                &linear_rule,
            )
        };

        evaluate(MiniTransport::Disabled).unwrap();
        let error = evaluate(MiniTransport::SkewRelativeGcl(cell)).unwrap_err();
        assert!(error.message().contains("at least 11"));
    }

    fn tetrahedron() -> SimplicialMesh {
        SimplicialMesh::new(
            3,
            vec![
                vec![0.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
            ],
            vec![vec![0, 1, 2, 3]],
            MeshQualityGate::new(0.01).unwrap(),
        )
        .unwrap()
    }

    fn stationary_triangle() -> AffineGeometryMap {
        AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.2, -0.3],
            vec![1.4, 0.1],
            vec![-0.15, 1.25],
        ])
        .unwrap()
    }

    fn assert_scaled_affine_identity<const D: usize>(
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
        previous_velocity: &[[f64; D]],
        current_velocity: &[[f64; D]],
        current_pressure: &[f64],
    ) {
        let density = 1.7;
        let viscosity = 0.23;
        let time_step = 0.17;
        let scales = MiniAffineScales::new(2.3, 4.1, 7.9).unwrap();
        let zero_velocity = vec![[0.0; D]; D + 2];
        let zero_pressure = vec![0.0; D + 1];
        let residual = MiniTransientCell::<D> {
            geometry,
            transport: MiniTransport::Disabled,
            density,
            viscosity,
            time_step,
            previous_velocity,
            current_velocity,
            current_pressure,
        }
        .evaluate(
            MiniTransientDirection {
                current_velocity: &zero_velocity,
                current_pressure: &zero_pressure,
                current_geometry: MiniGeometryDirection::Zero,
            },
            quadrature,
        )
        .unwrap()
        .into_parts()
        .0;
        let (local_size, matrix, rhs) = MiniScaledAffineCell::<D> {
            geometry,
            density,
            viscosity,
            time_step,
            previous_velocity,
            scales,
        }
        .project(quadrature)
        .unwrap()
        .into_parts();
        let mut point = current_velocity
            .iter()
            .flat_map(|value| value.iter().map(|value| value / scales.velocity))
            .collect::<Vec<_>>();
        point.extend(current_pressure.iter().map(|value| value / scales.pressure));
        assert_eq!(point.len(), local_size);
        for row in 0..local_size {
            let affine = matrix[row * local_size..(row + 1) * local_size]
                .iter()
                .zip(&point)
                .map(|(entry, point)| entry * point)
                .sum::<f64>()
                - rhs[row];
            let row_scale = if row < (D + 2) * D {
                scales.velocity
            } else {
                scales.pressure
            };
            let expected = residual[row] * row_scale / scales.power;
            let tolerance = 8_192.0 * f64::EPSILON * affine.abs().max(expected.abs()).max(1.0);
            assert!(
                (affine - expected).abs() <= tolerance,
                "row {row}: {affine:e} versus {expected:e}",
            );
        }
    }

    fn tetrahedron_geometry_direction(
        map: &AffineGeometryMap,
        vertices: &[[f64; 3]; 4],
    ) -> AffineGeometryLinearization {
        let mut jacobian = vec![0.0; 9];
        for row in 0..3 {
            for column in 0..3 {
                jacobian[row * 3 + column] = vertices[column + 1][row] - vertices[0][row];
            }
        }
        AffineGeometryLinearization::new(map.clone(), vertices[0].to_vec(), jacobian).unwrap()
    }
}
