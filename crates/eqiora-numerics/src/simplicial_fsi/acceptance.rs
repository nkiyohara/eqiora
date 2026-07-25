//! Independent residual, interface-action, pressure, and energy acceptance.

use eqiora_assembly::{CsrMatrix, LinearSystem, LocalContribution};
use eqiora_core::Diagnostic;
use eqiora_meshing::{MeshEntity, MeshGeometry, QuadratureRule, SimplicialMesh};
use eqiora_solver::CanonicalCsrSystemView;

use super::api::FixedReferenceFsiEnergyBalance;
use super::contract::{
    FixedReferenceFsiMaterial, FixedReferenceFsiState, FixedReferenceFsiStepConfig,
};
use super::element::{dot, fluid_local, local_velocity_dimension, solid_local};
use super::layout::FsiLayout;
use super::partition::{CellMaterial, FixedReferenceFsiPartition};
use super::{fluid_local_size, invalid, mini_count, p1_count};
use crate::affine_fem::physical_gradient;
use crate::discrete_space::{DiscreteSpace, SimplexP1BubbleSpace, SimplexP1Space};

pub(super) struct EnergyEvaluation<'a, const D: usize = 2> {
    pub(super) mesh: &'a SimplicialMesh,
    pub(super) partition: &'a FixedReferenceFsiPartition<D>,
    pub(super) previous: &'a FixedReferenceFsiState<D>,
    pub(super) next_vertex_velocity: &'a [[f64; D]],
    pub(super) next_bubbles: &'a [[f64; D]],
    pub(super) next_displacement: &'a [[f64; D]],
    pub(super) config: FixedReferenceFsiStepConfig<D>,
    pub(super) quadrature: &'a QuadratureRule,
}

pub(super) fn energy_balance<const D: usize>(
    evaluation: EnergyEvaluation<'_, D>,
) -> Result<FixedReferenceFsiEnergyBalance, Diagnostic> {
    let EnergyEvaluation {
        mesh,
        partition,
        previous,
        next_vertex_velocity,
        next_bubbles,
        next_displacement,
        config,
        quadrature,
    } = evaluation;
    let material = config.material();
    let mut previous_kinetic = 0.0;
    let mut next_kinetic = 0.0;
    let mut previous_elastic = 0.0;
    let mut next_elastic = 0.0;
    let mut kinetic_increment = 0.0;
    let mut elastic_increment = 0.0;
    let mut viscous_dissipation = 0.0;
    let p1_count = p1_count::<D>();
    let mini_count = mini_count::<D>();
    let mini = SimplexP1BubbleSpace::new(D)?;
    let p1 = SimplexP1Space::new(D)?;

    for (position, cell) in partition.fluid_cells().iter().copied().enumerate() {
        let entity = MeshEntity::new(D, cell.index());
        let geometry = mesh
            .geometry_map(entity)
            .expect("accepted fluid cell owns geometry");
        let inverse = geometry.inverse_jacobian()?;
        let vertices = mesh
            .entity_vertices(entity)
            .expect("accepted cell owns vertices");
        for point in quadrature.points() {
            let basis = mini.tabulate(&point.coordinates)?;
            let gradients = (0..mini_count)
                .map(|index| {
                    physical_gradient(
                        basis.gradient(index).expect("accepted MINI basis"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let mut old = [0.0; D];
            let mut new = [0.0; D];
            let mut new_gradient = [[0.0; D]; D];
            for local in 0..p1_count {
                for component in 0..D {
                    old[component] += basis.values()[local]
                        * previous.vertex_velocity()[vertices[local].index()][component];
                    new[component] += basis.values()[local]
                        * next_vertex_velocity[vertices[local].index()][component];
                    for axis in 0..D {
                        new_gradient[component][axis] += gradients[local][axis]
                            * next_vertex_velocity[vertices[local].index()][component];
                    }
                }
            }
            for component in 0..D {
                old[component] += basis.values()[p1_count]
                    * previous.fluid_cell_bubble_velocity()[position][component];
                new[component] += basis.values()[p1_count] * next_bubbles[position][component];
                for axis in 0..D {
                    new_gradient[component][axis] +=
                        gradients[p1_count][axis] * next_bubbles[position][component];
                }
            }
            let weight = point.weight * geometry.measure_scale();
            previous_kinetic += 0.5 * weight * material.fluid_density() * dot(&old, &old);
            next_kinetic += 0.5 * weight * material.fluid_density() * dot(&new, &new);
            let difference: [f64; D] =
                std::array::from_fn(|component| new[component] - old[component]);
            kinetic_increment +=
                0.5 * weight * material.fluid_density() * dot(&difference, &difference);
            viscous_dissipation += weight
                * material.fluid_dynamic_viscosity()
                * symmetric_gradient_twice_norm(&new_gradient);
        }
    }

    for cell in partition.solid_cells() {
        let entity = MeshEntity::new(D, cell.index());
        let geometry = mesh
            .geometry_map(entity)
            .expect("accepted solid cell owns geometry");
        let inverse = geometry.inverse_jacobian()?;
        let vertices = mesh
            .entity_vertices(entity)
            .expect("accepted cell owns vertices");
        for point in quadrature.points() {
            let basis = p1.tabulate(&point.coordinates)?;
            let gradients = (0..p1_count)
                .map(|index| {
                    physical_gradient(
                        basis.gradient(index).expect("accepted P1 basis"),
                        &inverse,
                        D,
                    )
                })
                .collect::<Vec<_>>();
            let mut old_velocity = [0.0; D];
            let mut new_velocity = [0.0; D];
            let mut old_displacement_gradient = [[0.0; D]; D];
            let mut new_displacement_gradient = [[0.0; D]; D];
            let mut increment_gradient = [[0.0; D]; D];
            for local in 0..p1_count {
                let vertex = vertices[local].index();
                for component in 0..D {
                    old_velocity[component] +=
                        basis.values()[local] * previous.vertex_velocity()[vertex][component];
                    new_velocity[component] +=
                        basis.values()[local] * next_vertex_velocity[vertex][component];
                    for axis in 0..D {
                        old_displacement_gradient[component][axis] += gradients[local][axis]
                            * previous.solid_displacement()[vertex][component];
                        new_displacement_gradient[component][axis] +=
                            gradients[local][axis] * next_displacement[vertex][component];
                        increment_gradient[component][axis] += gradients[local][axis]
                            * (next_displacement[vertex][component]
                                - previous.solid_displacement()[vertex][component]);
                    }
                }
            }
            let weight = point.weight * geometry.measure_scale();
            previous_kinetic +=
                0.5 * weight * material.solid_density() * dot(&old_velocity, &old_velocity);
            next_kinetic +=
                0.5 * weight * material.solid_density() * dot(&new_velocity, &new_velocity);
            let difference: [f64; D] =
                std::array::from_fn(|component| new_velocity[component] - old_velocity[component]);
            kinetic_increment +=
                0.5 * weight * material.solid_density() * dot(&difference, &difference);
            previous_elastic +=
                0.5 * weight * elastic_density(&old_displacement_gradient, material);
            next_elastic += 0.5 * weight * elastic_density(&new_displacement_gradient, material);
            elastic_increment += 0.5 * weight * elastic_density(&increment_gradient, material);
        }
    }
    let viscous_dissipation = config.time_step() * viscous_dissipation;
    let defect = next_kinetic - previous_kinetic + next_elastic - previous_elastic
        + kinetic_increment
        + elastic_increment
        + viscous_dissipation;
    let values = [
        previous_kinetic,
        next_kinetic,
        previous_elastic,
        next_elastic,
        kinetic_increment,
        elastic_increment,
        viscous_dissipation,
        defect,
    ];
    if values.into_iter().any(|value| !value.is_finite()) {
        return Err(invalid(
            "fixed-reference FSI energy evidence must be finite",
        ));
    }
    Ok(FixedReferenceFsiEnergyBalance {
        previous_kinetic,
        next_kinetic,
        previous_elastic,
        next_elastic,
        kinetic_increment,
        elastic_increment,
        viscous_dissipation,
        defect,
    })
}

fn elastic_density<const D: usize>(
    gradient: &[[f64; D]; D],
    material: FixedReferenceFsiMaterial<D>,
) -> f64 {
    material.solid_shear_modulus() * symmetric_gradient_twice_norm(gradient)
        + material.solid_first_lame_parameter() * trace(gradient).powi(2)
}

fn symmetric_gradient_twice_norm<const D: usize>(gradient: &[[f64; D]; D]) -> f64 {
    let mut squared_norm = gradient
        .iter()
        .enumerate()
        .map(|(axis, row)| 2.0 * row[axis].powi(2))
        .sum::<f64>();
    for (row, row_values) in gradient.iter().enumerate() {
        for (column, column_values) in gradient.iter().enumerate().skip(row + 1) {
            squared_norm += (row_values[column] + column_values[row]).powi(2);
        }
    }
    squared_norm
}

fn trace<const D: usize>(gradient: &[[f64; D]; D]) -> f64 {
    (0..D).map(|axis| gradient[axis][axis]).sum()
}

pub(super) fn require_pressure_closed_by_complete_operator<const D: usize>(
    system: &LinearSystem,
    layout: &FsiLayout<D>,
) -> Result<f64, Diagnostic> {
    let mut constant_pressure = vec![0.0; layout.reduced_size()];
    constant_pressure[layout.reduced_pressure_range()].fill(1.0);
    let action = system.matrix().multiply(&constant_pressure)?;
    let action_norm = norm(&action);
    let matrix_scale = system
        .matrix()
        .values()
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max)
        * (layout.reduced_size() as f64).sqrt();
    let tolerance = 8192.0 * f64::EPSILON * matrix_scale;
    if !action_norm.is_finite() || action_norm <= tolerance {
        return Err(invalid(format!(
            "fixed-reference FSI complete operator leaves constant pressure unclosed: action {action_norm:e}, threshold {tolerance:e}"
        )));
    }
    Ok(action_norm)
}

pub(super) fn require_symmetric(matrix: &CsrMatrix) -> Result<(), Diagnostic> {
    let scale = matrix
        .values()
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let tolerance = 4096.0 * f64::EPSILON * scale.max(1.0);
    for row in 0..matrix.rows() {
        for column in 0..matrix.columns() {
            let left = matrix.entry(row, column).expect("indices are in range");
            let right = matrix.entry(column, row).expect("indices are in range");
            if (left - right).abs() > tolerance {
                return Err(invalid(format!(
                    "fixed-reference FSI reduced operator is not symmetric at ({row}, {column})"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn recover_component_residuals<const D: usize>(
    mesh: &SimplicialMesh,
    partition: &FixedReferenceFsiPartition<D>,
    previous: &FixedReferenceFsiState<D>,
    config: FixedReferenceFsiStepConfig<D>,
    quadrature: &QuadratureRule,
    layout: &FsiLayout<D>,
    full_values: &[f64],
) -> Result<(Vec<f64>, Vec<f64>), Diagnostic> {
    let mut fluid_residual = vec![0.0; layout.full_size()];
    let mut solid_residual = vec![0.0; layout.full_size()];
    for cell_index in 0..partition.cell_count() {
        let cell = MeshEntity::new(D, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("accepted FSI cell owns affine geometry");
        let vertices = mesh
            .entity_vertices(cell)
            .expect("accepted FSI cell owns vertices");
        let (local, local_values, target) = match partition.material(cell_index) {
            CellMaterial::Fluid => {
                let position = partition
                    .fluid_position(cell_index)
                    .expect("fluid cell owns bubble position");
                (
                    fluid_local(&geometry, quadrature, config, &vertices, previous, position)?,
                    fluid_local_values(layout, full_values, position, &vertices),
                    &mut fluid_residual,
                )
            }
            CellMaterial::Solid => (
                solid_local(&geometry, quadrature, config, &vertices, previous)?,
                solid_local_values(layout, full_values, &vertices),
                &mut solid_residual,
            ),
            CellMaterial::Unassigned => unreachable!("partition is exhaustive"),
        };
        let local_residual = local_residual(&local, &local_values);
        for (local_vertex, vertex) in vertices.iter().enumerate() {
            for component in 0..D {
                target[layout.full_vertex_velocity(vertex.index(), component)] +=
                    local_residual[local_velocity_dimension::<D>(local_vertex, component)];
            }
        }
    }
    Ok((fluid_residual, solid_residual))
}

fn fluid_local_values<const D: usize>(
    layout: &FsiLayout<D>,
    full_values: &[f64],
    fluid_position: usize,
    vertices: &[MeshEntity],
) -> Vec<f64> {
    let mut values = solid_local_values(layout, full_values, vertices);
    for component in 0..D {
        values.push(full_values[layout.full_bubble_velocity(fluid_position, component)]);
    }
    for vertex in vertices {
        values.push(full_values[layout.full_pressure(vertex.index())]);
    }
    debug_assert_eq!(values.len(), fluid_local_size::<D>());
    values
}

fn solid_local_values<const D: usize>(
    layout: &FsiLayout<D>,
    full_values: &[f64],
    vertices: &[MeshEntity],
) -> Vec<f64> {
    vertices
        .iter()
        .flat_map(|vertex| {
            (0..D).map(move |component| {
                full_values[layout.full_vertex_velocity(vertex.index(), component)]
            })
        })
        .collect()
}

fn local_residual(local: &LocalContribution, values: &[f64]) -> Vec<f64> {
    (0..local.rows())
        .map(|row| {
            local.matrix()[row * local.columns()..(row + 1) * local.columns()]
                .iter()
                .zip(values)
                .map(|(entry, value)| entry * value)
                .sum::<f64>()
                - local.rhs()[row]
        })
        .collect()
}

pub(super) fn apply_canonical(
    system: &CanonicalCsrSystemView,
    values: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let mut output = vec![0.0; system.rows()];
    let problem = system.linear_problem()?;
    eqiora_solver::LinearOperator::apply(problem.operator(), values, &mut output)?;
    Ok(output)
}

pub(super) fn kinematic_residual_norm<const D: usize>(
    partition: &FixedReferenceFsiPartition<D>,
    previous: &FixedReferenceFsiState<D>,
    velocity: &[[f64; D]],
    displacement: &[[f64; D]],
    time_step: f64,
) -> f64 {
    partition
        .solid_vertices()
        .iter()
        .flat_map(|vertex| {
            (0..D).map(move |component| {
                displacement[vertex.index()][component]
                    - previous.solid_displacement()[vertex.index()][component]
                    - time_step * velocity[vertex.index()][component]
            })
        })
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
}

pub(super) fn norm(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::{symmetric_gradient_twice_norm, trace};

    #[test]
    fn three_dimensional_strain_invariants_include_every_axis_and_shear_pair() {
        let gradient = [[1.0, 2.0, 3.0], [5.0, 7.0, 11.0], [13.0, 17.0, 19.0]];

        assert_eq!(trace(&gradient), 1.0 + 7.0 + 19.0);
        assert_eq!(
            symmetric_gradient_twice_norm(&gradient),
            2.0 * (1.0_f64.powi(2) + 7.0_f64.powi(2) + 19.0_f64.powi(2))
                + (2.0_f64 + 5.0).powi(2)
                + (3.0_f64 + 13.0).powi(2)
                + (11.0_f64 + 17.0).powi(2)
        );
    }
}
