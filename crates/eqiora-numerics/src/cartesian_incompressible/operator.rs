use std::ops::Deref;
use std::sync::Arc;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_meshing::MeshEntity;

use super::pressure_coupling::MomentumWeightedPressureCoupling2d;
use super::replay::{CollocatedResidualReplay2d, replay_residual};
#[cfg(test)]
use crate::cartesian_fvm_geometry::cartesian_fvm_geometry_2d;
use crate::cartesian_fvm_geometry::{
    CartesianCellMetrics2d, CartesianFacetAdjacency2d, CartesianFacetMetrics2d,
};
use eqiora_meshing::CartesianMesh;

const DIMENSION: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CollocatedPoint2d {
    pub(crate) velocity: Vec<[f64; DIMENSION]>,
    pub(crate) pressure: Vec<f64>,
    pub(crate) gauge_multiplier: f64,
}

impl CollocatedPoint2d {
    pub(crate) fn packed(&self) -> Vec<f64> {
        let mut values = Vec::with_capacity(3 * self.velocity.len() + 1);
        for velocity in &self.velocity {
            values.extend_from_slice(velocity);
        }
        values.extend_from_slice(&self.pressure);
        values.push(self.gauge_multiplier);
        values
    }

    pub(crate) fn from_packed(values: &[f64], cell_count: usize) -> Result<Self, Diagnostic> {
        let expected = 3_usize
            .checked_mul(cell_count)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| invalid("collocated unknown count overflows usize"))?;
        if values.len() != expected || values.iter().any(|value| !value.is_finite()) {
            return Err(invalid(format!(
                "collocated point requires {expected} finite values, received {}",
                values.len()
            )));
        }
        let velocity = values[..2 * cell_count]
            .as_chunks::<DIMENSION>()
            .0
            .iter()
            .map(|value| [value[0], value[1]])
            .collect();
        Ok(Self {
            velocity,
            pressure: values[2 * cell_count..3 * cell_count].to_vec(),
            gauge_multiplier: values[3 * cell_count],
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CollocatedResidual2d {
    pub(crate) values: Vec<f64>,
    pub(super) momentum: Vec<[f64; DIMENSION]>,
    pub(super) physical_continuity: Vec<f64>,
    pub(super) face_actions: Vec<CollocatedFaceAction2d>,
    pub(crate) momentum_norm: f64,
    pub(crate) continuity_norm: f64,
    pub(crate) gauge_residual: f64,
}

impl CollocatedResidual2d {
    pub(crate) fn face_volume_fluxes(&self) -> Vec<f64> {
        self.face_actions
            .iter()
            .map(|action| match action {
                CollocatedFaceAction2d::Interior { volume_flux, .. } => *volume_flux,
                CollocatedFaceAction2d::Boundary { .. } => 0.0,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum CollocatedFaceAction2d {
    Interior {
        lower: usize,
        upper: usize,
        volume_flux: f64,
        face_velocity: [f64; DIMENSION],
        convective_momentum: [f64; DIMENSION],
        traction_momentum: [f64; DIMENSION],
    },
    Boundary {
        cell: usize,
        traction_momentum: [f64; DIMENSION],
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedCartesianIncompressibleOperator2d {
    mesh: CartesianMesh,
    cells: Vec<CartesianCellMetrics2d>,
    facets: Vec<CartesianFacetMetrics2d>,
    pressure_coupling: MomentumWeightedPressureCoupling2d,
    density: f64,
    viscosity: f64,
    duration: f64,
    body_force: Vec<[f64; DIMENSION]>,
    momentum_diagonal: Vec<[f64; DIMENSION]>,
}

impl PreparedCartesianIncompressibleOperator2d {
    #[cfg(test)]
    pub(crate) fn new(
        mesh: CartesianMesh,
        density: f64,
        viscosity: f64,
        duration: f64,
        body_force: Vec<[f64; DIMENSION]>,
    ) -> Result<Self, Diagnostic> {
        let (cells, facets) = cartesian_fvm_geometry_2d(&mesh)?;
        Self::from_geometry(
            mesh, cells, facets, density, viscosity, duration, body_force,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_geometry(
        mesh: CartesianMesh,
        cells: Vec<CartesianCellMetrics2d>,
        facets: Vec<CartesianFacetMetrics2d>,
        density: f64,
        viscosity: f64,
        duration: f64,
        body_force: Vec<[f64; DIMENSION]>,
    ) -> Result<Self, Diagnostic> {
        if !density.is_finite()
            || density <= 0.0
            || !viscosity.is_finite()
            || viscosity <= 0.0
            || !duration.is_finite()
            || duration <= 0.0
        {
            return Err(invalid(
                "collocated flow density, viscosity, and duration must be finite and positive",
            ));
        }
        if cells.len() != body_force.len() {
            return Err(invalid(
                "collocated body force must cover every cell exactly once",
            ));
        }
        if body_force.iter().flatten().any(|value| !value.is_finite()) {
            return Err(invalid("collocated body force must be finite"));
        }
        for axis in 0..DIMENSION {
            if mesh.axis_cell_count(axis).is_none_or(|count| count < 2) {
                return Err(invalid(
                    "linearly exact collocated flow requires at least two cells on each axis",
                ));
            }
        }
        let momentum_diagonal = momentum_diagonal(&cells, &facets, density, viscosity, duration)?;
        let pressure_coupling = MomentumWeightedPressureCoupling2d::new(&mesh)?;
        Ok(Self {
            mesh,
            cells,
            facets,
            pressure_coupling,
            density,
            viscosity,
            duration,
            body_force,
            momentum_diagonal,
        })
    }

    pub(crate) fn bind_action(
        self: &Arc<Self>,
        previous_velocity: Vec<[f64; DIMENSION]>,
        previous_face_volume_fluxes: Vec<f64>,
    ) -> Result<CartesianIncompressibleOperator2d, Diagnostic> {
        if self.cells.len() != previous_velocity.len()
            || previous_velocity
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "collocated previous velocity must contain one finite vector per cell",
            ));
        }
        if self.facets.len() != previous_face_volume_fluxes.len()
            || previous_face_volume_fluxes
                .iter()
                .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "collocated previous face-flux history must contain one finite value per finalized facet",
            ));
        }
        Ok(CartesianIncompressibleOperator2d {
            prepared: Arc::clone(self),
            previous_velocity,
            previous_face_volume_fluxes,
        })
    }

    pub(crate) fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    pub(crate) fn cells(&self) -> &[CartesianCellMetrics2d] {
        &self.cells
    }

    pub(crate) fn facet_count(&self) -> usize {
        self.facets.len()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CartesianIncompressibleOperator2d {
    prepared: Arc<PreparedCartesianIncompressibleOperator2d>,
    previous_velocity: Vec<[f64; DIMENSION]>,
    previous_face_volume_fluxes: Vec<f64>,
}

impl Deref for CartesianIncompressibleOperator2d {
    type Target = PreparedCartesianIncompressibleOperator2d;

    fn deref(&self) -> &Self::Target {
        &self.prepared
    }
}

#[cfg(test)]
impl CartesianIncompressibleOperator2d {
    pub(crate) fn new(
        mesh: CartesianMesh,
        density: f64,
        viscosity: f64,
        duration: f64,
        previous_velocity: Vec<[f64; DIMENSION]>,
        previous_face_volume_fluxes: Vec<f64>,
        body_force: Vec<[f64; DIMENSION]>,
    ) -> Result<Self, Diagnostic> {
        Arc::new(PreparedCartesianIncompressibleOperator2d::new(
            mesh, density, viscosity, duration, body_force,
        )?)
        .bind_action(previous_velocity, previous_face_volume_fluxes)
    }
}

impl CartesianIncompressibleOperator2d {
    pub(crate) fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub(crate) fn unknown_count(&self) -> usize {
        3 * self.cell_count() + 1
    }

    pub(crate) fn momentum_diagonal(&self) -> &[[f64; DIMENSION]] {
        &self.momentum_diagonal
    }

    pub(crate) fn pressure_corrections(&self, pressure: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        self.pressure_coupling
            .corrections(pressure, &self.momentum_diagonal)
    }

    pub(crate) fn replay(
        &self,
        point: &CollocatedPoint2d,
        residual: &CollocatedResidual2d,
    ) -> Result<CollocatedResidualReplay2d, Diagnostic> {
        self.validate_point(point)?;
        replay_residual(
            &self.facets,
            &self.cells,
            self.density,
            self.duration,
            &self.previous_velocity,
            &self.body_force,
            point,
            residual,
        )
    }

    pub(crate) fn evaluate(
        &self,
        point: &CollocatedPoint2d,
    ) -> Result<CollocatedResidual2d, Diagnostic> {
        self.validate_point(point)?;
        let pressure_corrections = self.pressure_coupling.transient_consistent_corrections(
            &point.pressure,
            &self.momentum_diagonal,
            &self.previous_face_volume_fluxes,
            &self.previous_velocity,
            self.density,
            self.duration,
        )?;
        let velocity_gradients = cell_vector_gradients(&self.mesh, &self.cells, &point.velocity)?;
        let pressure_gradients = self
            .pressure_coupling
            .cell_gradients(&point.pressure, "state")?;
        let mut momentum = vec![[0.0; DIMENSION]; self.cell_count()];
        let mut physical_continuity = vec![0.0; self.cell_count()];
        for (cell, cell_momentum) in momentum.iter_mut().enumerate() {
            let measure = self.cells[cell].measure;
            for (component, residual) in cell_momentum.iter_mut().enumerate() {
                *residual = self.density * measure / self.duration
                    * (point.velocity[cell][component] - self.previous_velocity[cell][component])
                    - measure * self.body_force[cell][component];
            }
        }
        let mut face_actions = Vec::with_capacity(self.facets.len());
        for (facet_index, facet) in self.facets.iter().enumerate() {
            match facet.adjacency {
                CartesianFacetAdjacency2d::Interior {
                    lower,
                    upper,
                    center_distance,
                } => {
                    let axis = facet.normal_axis;
                    let face_velocity = [
                        0.5 * (point.velocity[lower][0] + point.velocity[upper][0]),
                        0.5 * (point.velocity[lower][1] + point.velocity[upper][1]),
                    ];
                    let volume_flux =
                        facet.measure * face_velocity[axis] + pressure_corrections[facet_index];
                    physical_continuity[lower] += volume_flux;
                    physical_continuity[upper] -= volume_flux;
                    let convective_momentum =
                        face_velocity.map(|component| self.density * volume_flux * component);
                    let traction = interior_traction(
                        axis,
                        center_distance,
                        self.viscosity,
                        [point.velocity[lower], point.velocity[upper]],
                        [velocity_gradients[lower], velocity_gradients[upper]],
                        0.5 * (point.pressure[lower] + point.pressure[upper]),
                    );
                    let traction_momentum = traction.map(|component| facet.measure * component);
                    for component in 0..DIMENSION {
                        let action = convective_momentum[component] - traction_momentum[component];
                        momentum[lower][component] += action;
                        momentum[upper][component] -= action;
                    }
                    face_actions.push(CollocatedFaceAction2d::Interior {
                        lower,
                        upper,
                        volume_flux,
                        face_velocity,
                        convective_momentum,
                        traction_momentum,
                    });
                }
                CartesianFacetAdjacency2d::Boundary {
                    cell,
                    side,
                    center_distance,
                } => {
                    let outward_sign = match side {
                        eqiora_schema::kernel::BoundarySide::Lower => -1.0,
                        eqiora_schema::kernel::BoundarySide::Upper => 1.0,
                    };
                    let traction = zero_trace_boundary_traction(
                        facet.normal_axis,
                        outward_sign,
                        center_distance,
                        self.viscosity,
                        point.velocity[cell],
                        boundary_face_value(
                            point.pressure[cell],
                            pressure_gradients[cell][facet.normal_axis],
                            outward_sign,
                            center_distance,
                        ),
                    );
                    let traction_momentum = traction.map(|component| facet.measure * component);
                    for component in 0..DIMENSION {
                        momentum[cell][component] -= traction_momentum[component];
                    }
                    face_actions.push(CollocatedFaceAction2d::Boundary {
                        cell,
                        traction_momentum,
                    });
                }
            }
        }
        let gauge_residual = point
            .pressure
            .iter()
            .zip(&self.cells)
            .map(|(pressure, cell)| pressure * cell.measure)
            .sum::<f64>();
        let augmented_continuity = physical_continuity
            .iter()
            .zip(&self.cells)
            .map(|(residual, cell)| residual + point.gauge_multiplier * cell.measure)
            .collect::<Vec<_>>();
        let values = pack_residual(&momentum, &augmented_continuity, gauge_residual);
        require_finite(&values, "collocated residual")?;
        Ok(CollocatedResidual2d {
            momentum_norm: vector_norm(momentum.iter().flatten().copied())?,
            continuity_norm: vector_norm(physical_continuity.iter().copied())?,
            gauge_residual,
            values,
            momentum,
            physical_continuity,
            face_actions,
        })
    }

    pub(crate) fn apply_jvp(
        &self,
        point: &CollocatedPoint2d,
        direction: &CollocatedPoint2d,
    ) -> Result<Vec<f64>, Diagnostic> {
        self.validate_point(point)?;
        self.validate_point(direction)?;
        let pressure_corrections = self.pressure_coupling.transient_consistent_corrections(
            &point.pressure,
            &self.momentum_diagonal,
            &self.previous_face_volume_fluxes,
            &self.previous_velocity,
            self.density,
            self.duration,
        )?;
        let corrections = self
            .pressure_coupling
            .directional_corrections(&direction.pressure, &self.momentum_diagonal)?;
        let direction_gradients =
            cell_vector_gradients(&self.mesh, &self.cells, &direction.velocity)?;
        let pressure_direction_gradients = self
            .pressure_coupling
            .cell_gradients(&direction.pressure, "direction")?;
        let mut momentum = vec![[0.0; DIMENSION]; self.cell_count()];
        let mut continuity = vec![0.0; self.cell_count()];
        for cell in 0..self.cell_count() {
            let measure = self.cells[cell].measure;
            for (component, residual) in momentum[cell].iter_mut().enumerate() {
                *residual =
                    self.density * measure / self.duration * direction.velocity[cell][component];
            }
            continuity[cell] = direction.gauge_multiplier * measure;
        }
        for (facet_index, facet) in self.facets.iter().enumerate() {
            match facet.adjacency {
                CartesianFacetAdjacency2d::Interior {
                    lower,
                    upper,
                    center_distance,
                } => {
                    let axis = facet.normal_axis;
                    let face_velocity = average(point.velocity[lower], point.velocity[upper]);
                    let face_direction =
                        average(direction.velocity[lower], direction.velocity[upper]);
                    let volume_flux =
                        facet.measure * face_velocity[axis] + pressure_corrections[facet_index];
                    let volume_flux_direction =
                        facet.measure * face_direction[axis] + corrections[facet_index];
                    continuity[lower] += volume_flux_direction;
                    continuity[upper] -= volume_flux_direction;
                    for component in 0..DIMENSION {
                        let convective_direction = self.density
                            * (volume_flux_direction * face_velocity[component]
                                + volume_flux * face_direction[component]);
                        momentum[lower][component] += convective_direction;
                        momentum[upper][component] -= convective_direction;
                    }
                    let traction_direction = interior_traction(
                        axis,
                        center_distance,
                        self.viscosity,
                        [direction.velocity[lower], direction.velocity[upper]],
                        [direction_gradients[lower], direction_gradients[upper]],
                        0.5 * (direction.pressure[lower] + direction.pressure[upper]),
                    );
                    for component in 0..DIMENSION {
                        let action = facet.measure * traction_direction[component];
                        momentum[lower][component] -= action;
                        momentum[upper][component] += action;
                    }
                }
                CartesianFacetAdjacency2d::Boundary {
                    cell,
                    side,
                    center_distance,
                } => {
                    let outward_sign = match side {
                        eqiora_schema::kernel::BoundarySide::Lower => -1.0,
                        eqiora_schema::kernel::BoundarySide::Upper => 1.0,
                    };
                    let traction_direction = zero_trace_boundary_traction(
                        facet.normal_axis,
                        outward_sign,
                        center_distance,
                        self.viscosity,
                        direction.velocity[cell],
                        boundary_face_value(
                            direction.pressure[cell],
                            pressure_direction_gradients[cell][facet.normal_axis],
                            outward_sign,
                            center_distance,
                        ),
                    );
                    for component in 0..DIMENSION {
                        momentum[cell][component] -= facet.measure * traction_direction[component];
                    }
                }
            }
        }
        let gauge = direction
            .pressure
            .iter()
            .zip(&self.cells)
            .map(|(pressure, cell)| pressure * cell.measure)
            .sum();
        let values = pack_residual(&momentum, &continuity, gauge);
        require_finite(&values, "collocated analytic JVP")?;
        Ok(values)
    }

    fn validate_point(&self, point: &CollocatedPoint2d) -> Result<(), Diagnostic> {
        if point.velocity.len() != self.cell_count()
            || point.pressure.len() != self.cell_count()
            || !point.gauge_multiplier.is_finite()
            || point
                .velocity
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            || point.pressure.iter().any(|value| !value.is_finite())
        {
            return Err(invalid(
                "collocated point must contain one finite velocity and pressure per cell plus a finite gauge multiplier",
            ));
        }
        Ok(())
    }
}

fn momentum_diagonal(
    cells: &[CartesianCellMetrics2d],
    facets: &[CartesianFacetMetrics2d],
    density: f64,
    viscosity: f64,
    duration: f64,
) -> Result<Vec<[f64; DIMENSION]>, Diagnostic> {
    let mut diagonal = cells
        .iter()
        .map(|cell| [density * cell.measure / duration; DIMENSION])
        .collect::<Vec<_>>();
    for facet in facets {
        let coefficient = match facet.adjacency {
            CartesianFacetAdjacency2d::Interior {
                center_distance, ..
            }
            | CartesianFacetAdjacency2d::Boundary {
                center_distance, ..
            } => viscosity * facet.measure / center_distance,
        };
        let cells = match facet.adjacency {
            CartesianFacetAdjacency2d::Interior { lower, upper, .. } => [Some(lower), Some(upper)],
            CartesianFacetAdjacency2d::Boundary { cell, .. } => [Some(cell), None],
        };
        for cell in cells.into_iter().flatten() {
            for (component, diagonal) in diagonal[cell].iter_mut().enumerate() {
                *diagonal += coefficient
                    * if component == facet.normal_axis {
                        2.0
                    } else {
                        1.0
                    };
            }
        }
    }
    if diagonal
        .iter()
        .flatten()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid(
            "collocated momentum diagonal must remain finite and positive",
        ));
    }
    Ok(diagonal)
}

fn cell_vector_gradients(
    mesh: &CartesianMesh,
    cells: &[CartesianCellMetrics2d],
    values: &[[f64; DIMENSION]],
) -> Result<Vec<[[f64; DIMENSION]; DIMENSION]>, Diagnostic> {
    (0..cells.len())
        .map(|cell| {
            let entity = MeshEntity::new(DIMENSION, cell);
            let indices = mesh
                .cell_multi_index(entity)
                .ok_or_else(|| invalid("Cartesian cell multi-index is unavailable"))?;
            let mut gradient = [[0.0; DIMENSION]; DIMENSION];
            for axis in 0..DIMENSION {
                let count = mesh
                    .axis_cell_count(axis)
                    .ok_or_else(|| invalid("Cartesian axis cell count is unavailable"))?;
                let (lower, upper) = if indices[axis] == 0 {
                    (cell, neighbor(mesh, indices, axis, 1)?)
                } else if indices[axis] + 1 == count {
                    (neighbor(mesh, indices, axis, -1)?, cell)
                } else {
                    (
                        neighbor(mesh, indices, axis, -1)?,
                        neighbor(mesh, indices, axis, 1)?,
                    )
                };
                let distance = cells[upper].center[axis] - cells[lower].center[axis];
                if !distance.is_finite() || distance <= 0.0 {
                    return Err(invalid(
                        "Cartesian gradient stencil requires positive center distance",
                    ));
                }
                for component in 0..DIMENSION {
                    gradient[component][axis] =
                        (values[upper][component] - values[lower][component]) / distance;
                }
            }
            Ok(gradient)
        })
        .collect()
}

fn neighbor(
    mesh: &CartesianMesh,
    indices: &[usize],
    axis: usize,
    offset: isize,
) -> Result<usize, Diagnostic> {
    let mut neighbor = indices.to_vec();
    neighbor[axis] = neighbor[axis]
        .checked_add_signed(offset)
        .ok_or_else(|| invalid("Cartesian gradient neighbor index underflows"))?;
    mesh.cell_at(&neighbor)
        .map(MeshEntity::index)
        .ok_or_else(|| invalid("Cartesian gradient neighbor is unavailable"))
}

fn interior_traction(
    normal_axis: usize,
    center_distance: f64,
    viscosity: f64,
    velocity: [[f64; DIMENSION]; 2],
    gradient: [[[f64; DIMENSION]; DIMENSION]; 2],
    pressure: f64,
) -> [f64; DIMENSION] {
    let mut traction = [0.0; DIMENSION];
    for component in 0..DIMENSION {
        let normal_derivative = (velocity[1][component] - velocity[0][component]) / center_distance;
        let transposed_derivative =
            0.5 * (gradient[0][normal_axis][component] + gradient[1][normal_axis][component]);
        traction[component] = viscosity * (normal_derivative + transposed_derivative);
        if component == normal_axis {
            traction[component] -= pressure;
        }
    }
    traction
}

fn zero_trace_boundary_traction(
    normal_axis: usize,
    outward_sign: f64,
    center_distance: f64,
    viscosity: f64,
    cell_velocity: [f64; DIMENSION],
    pressure: f64,
) -> [f64; DIMENSION] {
    let mut traction = [0.0; DIMENSION];
    for component in 0..DIMENSION {
        let derivative = -outward_sign * cell_velocity[component] / center_distance;
        let mut stress_column = viscosity * derivative;
        if component == normal_axis {
            stress_column += viscosity * derivative;
            stress_column -= pressure;
        }
        traction[component] = outward_sign * stress_column;
    }
    traction
}

fn boundary_face_value(
    cell_value: f64,
    axis_derivative: f64,
    outward_sign: f64,
    center_distance: f64,
) -> f64 {
    outward_sign.mul_add(center_distance * axis_derivative, cell_value)
}

fn average(left: [f64; DIMENSION], right: [f64; DIMENSION]) -> [f64; DIMENSION] {
    [0.5 * (left[0] + right[0]), 0.5 * (left[1] + right[1])]
}

fn pack_residual(momentum: &[[f64; DIMENSION]], continuity: &[f64], gauge: f64) -> Vec<f64> {
    let mut values = Vec::with_capacity(3 * momentum.len() + 1);
    for value in momentum {
        values.extend_from_slice(value);
    }
    values.extend_from_slice(continuity);
    values.push(gauge);
    values
}

fn vector_norm(values: impl IntoIterator<Item = f64>) -> Result<f64, Diagnostic> {
    let squared = values
        .into_iter()
        .try_fold(0.0, |sum, value| {
            let next = value.mul_add(value, sum);
            next.is_finite().then_some(next)
        })
        .ok_or_else(|| invalid("collocated residual norm overflowed"))?;
    Ok(squared.sqrt())
}

fn require_finite(values: &[f64], role: &str) -> Result<(), Diagnostic> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid(format!("{role} contains a non-finite value")))
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-incompressible-operator-2d".to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian_fvm_geometry::cartesian_fvm_geometry_2d;

    #[test]
    fn affine_pressure_exactly_balances_its_canonical_body_force() {
        let mesh = CartesianMesh::uniform(&[[-1.0, 1.0], [-2.0, 2.0]], &[4, 3]).unwrap();
        let (_, facets) = cartesian_fvm_geometry_2d(&mesh).unwrap();
        let pressure = (0..12)
            .map(|cell| {
                let center = mesh.entity_center(MeshEntity::new(2, cell)).unwrap();
                1.25 * center[0] - 0.625 * center[1]
            })
            .collect::<Vec<_>>();
        let point = CollocatedPoint2d {
            velocity: vec![[0.0; DIMENSION]; 12],
            pressure,
            gauge_multiplier: 0.0,
        };
        let operator = Arc::new(
            PreparedCartesianIncompressibleOperator2d::new(
                mesh,
                1.0,
                0.05,
                0.01,
                vec![[1.25, -0.625]; 12],
            )
            .unwrap(),
        )
        .bind_action(point.velocity.clone(), vec![0.0; facets.len()])
        .unwrap();
        let residual = operator.evaluate(&point).unwrap();
        assert!(residual.momentum_norm < 8.0e-15);
        assert_eq!(residual.continuity_norm, 0.0);
        assert!(residual.gauge_residual.abs() < 1.0e-15);
        operator.replay(&point, &residual).unwrap();
    }
}
