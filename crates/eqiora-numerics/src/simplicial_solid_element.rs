//! Private dimension-generic P1 solid element operators.

use eqiora_assembly::LocalContribution;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{AffineGeometryMap, GeometryMap, QuadratureRule};

use crate::affine_fem::physical_gradient;
use crate::discrete_space::{DiscreteSpace, SimplexP1Space};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct P1SolidElementMatrices {
    local_size: usize,
    mass: Vec<f64>,
    stiffness: Vec<f64>,
}

impl P1SolidElementMatrices {
    pub(crate) const fn local_size(&self) -> usize {
        self.local_size
    }

    pub(crate) fn mass(&self) -> &[f64] {
        &self.mass
    }

    pub(crate) fn stiffness(&self) -> &[f64] {
        &self.stiffness
    }
}

pub(crate) fn p1_solid_element_matrices<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    density: f64,
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> Result<P1SolidElementMatrices, Diagnostic> {
    require_contract::<D>(
        geometry,
        quadrature,
        density,
        shear_modulus,
        first_lame_parameter,
    )?;
    let basis_count = D + 1;
    let local_size = basis_count * D;
    let inverse = geometry.inverse_jacobian()?;
    let space = SimplexP1Space::new(D)?;
    let mut mass = vec![0.0; local_size * local_size];
    let mut stiffness = vec![0.0; local_size * local_size];

    for point in quadrature.points() {
        let basis = space.tabulate(&point.coordinates)?;
        let gradients = (0..basis_count)
            .map(|index| {
                physical_gradient(
                    basis.gradient(index).expect("accepted P1 basis index"),
                    &inverse,
                    D,
                )
            })
            .collect::<Vec<_>>();
        let scale = point.weight * geometry.measure_scale();
        for row_basis in 0..basis_count {
            for row_component in 0..D {
                let row = local_dimension::<D>(row_basis, row_component);
                for column_basis in 0..basis_count {
                    for column_component in 0..D {
                        let column = local_dimension::<D>(column_basis, column_component);
                        if row_component == column_component {
                            mass[row * local_size + column] += scale
                                * density
                                * basis.values()[row_basis]
                                * basis.values()[column_basis];
                        }
                        stiffness[row * local_size + column] += scale
                            * elasticity_entry(
                                &gradients[row_basis],
                                row_component,
                                &gradients[column_basis],
                                column_component,
                                shear_modulus,
                                first_lame_parameter,
                            );
                    }
                }
            }
        }
    }

    Ok(P1SolidElementMatrices {
        local_size,
        mass,
        stiffness,
    })
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)]
pub(crate) fn p1_solid_backward_euler_velocity<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    density: f64,
    shear_modulus: f64,
    first_lame_parameter: f64,
    time_step: f64,
    previous_vertex_velocity: &[[f64; D]],
    previous_vertex_displacement: &[[f64; D]],
    velocity_scale: f64,
    power_scale: f64,
) -> Result<LocalContribution, Diagnostic> {
    require_contract::<D>(
        geometry,
        quadrature,
        density,
        shear_modulus,
        first_lame_parameter,
    )?;
    let basis_count = D + 1;
    if previous_vertex_velocity.len() != basis_count
        || previous_vertex_displacement.len() != basis_count
        || !time_step.is_finite()
        || time_step <= 0.0
        || !velocity_scale.is_finite()
        || velocity_scale <= 0.0
        || !power_scale.is_finite()
        || power_scale <= 0.0
        || previous_vertex_velocity
            .iter()
            .chain(previous_vertex_displacement)
            .flatten()
            .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "P1 solid backward-Euler action requires complete finite local state and positive finite time/scale values",
        ));
    }
    let local_size = basis_count * D;
    let inverse = geometry.inverse_jacobian()?;
    let space = SimplexP1Space::new(D)?;
    let mut matrix = vec![0.0; local_size * local_size];
    let mut rhs = vec![0.0; local_size];
    for point in quadrature.points() {
        let basis = space.tabulate(&point.coordinates)?;
        let gradients = (0..basis_count)
            .map(|index| {
                physical_gradient(
                    basis.gradient(index).expect("accepted P1 basis index"),
                    &inverse,
                    D,
                )
            })
            .collect::<Vec<_>>();
        let scale = point.weight * geometry.measure_scale();
        let mut previous_velocity = [0.0; D];
        for (local, vertex_velocity) in previous_vertex_velocity.iter().enumerate() {
            for (component, value) in previous_velocity.iter_mut().enumerate() {
                *value += basis.values()[local] * vertex_velocity[component];
            }
        }
        for row_basis in 0..basis_count {
            for (row_component, previous_component) in previous_velocity.iter().enumerate() {
                let row = local_dimension::<D>(row_basis, row_component);
                rhs[row] += scale * density / time_step
                    * basis.values()[row_basis]
                    * previous_component
                    * velocity_scale
                    / power_scale;
                for column_basis in 0..basis_count {
                    for column_component in 0..D {
                        let column = local_dimension::<D>(column_basis, column_component);
                        let mass = if row_component == column_component {
                            density / time_step
                                * basis.values()[row_basis]
                                * basis.values()[column_basis]
                        } else {
                            0.0
                        };
                        let stiffness = elasticity_entry(
                            &gradients[row_basis],
                            row_component,
                            &gradients[column_basis],
                            column_component,
                            shear_modulus,
                            first_lame_parameter,
                        );
                        matrix[row * local_size + column] += scale
                            * (mass + time_step * stiffness)
                            * velocity_scale
                            * velocity_scale
                            / power_scale;
                        rhs[row] -= scale
                            * stiffness
                            * previous_vertex_displacement[column_basis][column_component]
                            * velocity_scale
                            / power_scale;
                    }
                }
            }
        }
    }
    LocalContribution::new(local_size, local_size, matrix, rhs)
}

pub(crate) const fn local_dimension<const D: usize>(basis: usize, component: usize) -> usize {
    basis * D + component
}

fn elasticity_entry(
    row_gradient: &[f64],
    row_component: usize,
    column_gradient: &[f64],
    column_component: usize,
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> f64 {
    let diagonal = if row_component == column_component {
        dot(row_gradient, column_gradient)
    } else {
        0.0
    };
    let crossed = row_gradient[column_component] * column_gradient[row_component];
    shear_modulus * (diagonal + crossed)
        + first_lame_parameter * row_gradient[row_component] * column_gradient[column_component]
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn require_contract<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    density: f64,
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> Result<(), Diagnostic> {
    if !matches!(D, 2 | 3)
        || geometry.reference_cell() != quadrature.reference_cell()
        || geometry.reference_cell().dimension() != D
        || geometry.physical_dimension() != D
        || quadrature.polynomial_exactness().unwrap_or(0) < 2
    {
        return Err(invalid(
            "P1 solid element requires matching intrinsic 2D/3D affine-simplex geometry and degree-two quadrature",
        ));
    }
    if !density.is_finite()
        || density <= 0.0
        || !shear_modulus.is_finite()
        || shear_modulus <= 0.0
        || !first_lame_parameter.is_finite()
    {
        return Err(invalid(
            "P1 solid element material coefficients must be finite with positive density and shear modulus",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}
