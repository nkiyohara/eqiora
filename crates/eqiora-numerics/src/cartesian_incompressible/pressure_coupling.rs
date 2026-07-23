//! Linearly exact momentum-weighted pressure correction on Cartesian cells.

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};

use crate::cartesian_fvm_geometry::{CartesianFacetAdjacency2d, cartesian_fvm_geometry_2d};
use crate::{CartesianMesh, MeshEntity};

const DIMENSION: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
struct AxisGradientStencil {
    lower: usize,
    upper: usize,
    inverse_distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PressureCorrectionFacet {
    Interior {
        lower: usize,
        upper: usize,
        normal_axis: usize,
        inverse_center_distance: f64,
        measure: f64,
    },
    Boundary,
}

/// Fixed linear pressure action used by the bounded collocated FVM profile.
///
/// The operator owns no pressure values. It retains only Cartesian gradient
/// stencils and exact face geometry. Each application derives the symmetric
/// normal-momentum weight `d_f = 0.5 * (V_P / a_P + V_N / a_N)` from positive
/// per-cell, per-component momentum diagonals. It produces, for each interior
/// face oriented from its lower cell to its upper cell,
///
/// ```text
/// -A_f d_f ((p_N - p_P) / d_PN - avg(grad_h(p))_f[axis]).
/// ```
///
/// Boundary corrections are exactly zero. This type neither constructs the
/// velocity part of a face flux nor owns continuity assembly.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MomentumWeightedPressureCoupling2d {
    cell_count: usize,
    cell_measures: Vec<f64>,
    gradient_stencils: Vec<[AxisGradientStencil; DIMENSION]>,
    facets: Vec<PressureCorrectionFacet>,
}

impl MomentumWeightedPressureCoupling2d {
    /// Bind one supported 2D Cartesian mesh to its fixed linear pressure action.
    ///
    /// # Errors
    /// Rejects a non-2D mesh or fewer than two cells on either axis.
    pub(crate) fn new(mesh: &CartesianMesh) -> Result<Self, Diagnostic> {
        let axis_counts = require_supported_mesh(mesh)?;
        let (cells, geometry_facets) = cartesian_fvm_geometry_2d(mesh)?;

        let mut gradient_stencils = Vec::with_capacity(cells.len());
        for cell in 0..cells.len() {
            let entity = MeshEntity::new(DIMENSION, cell);
            let indices = mesh
                .cell_multi_index(entity)
                .ok_or_else(|| invalid("pressure correction cell has no Cartesian multi-index"))?;
            let mut stencils = [AxisGradientStencil {
                lower: 0,
                upper: 0,
                inverse_distance: 0.0,
            }; DIMENSION];
            for (axis, stencil) in stencils.iter_mut().enumerate() {
                let (lower_axis_index, upper_axis_index) = if indices[axis] == 0 {
                    (0, 1)
                } else if indices[axis] + 1 == axis_counts[axis] {
                    (axis_counts[axis] - 2, axis_counts[axis] - 1)
                } else {
                    (indices[axis] - 1, indices[axis] + 1)
                };
                let mut lower_indices = [indices[0], indices[1]];
                let mut upper_indices = lower_indices;
                lower_indices[axis] = lower_axis_index;
                upper_indices[axis] = upper_axis_index;
                let lower = mesh
                    .cell_at(&lower_indices)
                    .ok_or_else(|| invalid("pressure gradient lower support cell is unavailable"))?
                    .index();
                let upper = mesh
                    .cell_at(&upper_indices)
                    .ok_or_else(|| invalid("pressure gradient upper support cell is unavailable"))?
                    .index();
                let distance = cells[upper].center[axis] - cells[lower].center[axis];
                let inverse_distance = distance.recip();
                if !distance.is_finite()
                    || distance <= 0.0
                    || !inverse_distance.is_finite()
                    || inverse_distance <= 0.0
                {
                    return Err(invalid(
                        "pressure gradient support distance must be finite and positive",
                    ));
                }
                *stencil = AxisGradientStencil {
                    lower,
                    upper,
                    inverse_distance,
                };
            }
            gradient_stencils.push(stencils);
        }

        let mut facets = Vec::with_capacity(geometry_facets.len());
        for facet in geometry_facets {
            let pressure_facet = match facet.adjacency {
                CartesianFacetAdjacency2d::Interior {
                    lower,
                    upper,
                    center_distance,
                } => {
                    let inverse_center_distance = center_distance.recip();
                    if !inverse_center_distance.is_finite() || inverse_center_distance <= 0.0 {
                        return Err(invalid(
                            "pressure correction inverse face distance must be finite and positive",
                        ));
                    }
                    PressureCorrectionFacet::Interior {
                        lower,
                        upper,
                        normal_axis: facet.normal_axis,
                        inverse_center_distance,
                        measure: facet.measure,
                    }
                }
                CartesianFacetAdjacency2d::Boundary { .. } => PressureCorrectionFacet::Boundary,
            };
            facets.push(pressure_facet);
        }

        Ok(Self {
            cell_count: cells.len(),
            cell_measures: cells.iter().map(|cell| cell.measure).collect(),
            gradient_stencils,
            facets,
        })
    }

    /// Apply the fixed pressure action in complete geometry-facet order.
    ///
    /// Interior entries are oriented from the lower cell to the upper cell;
    /// exterior entries are exactly zero.
    pub(crate) fn corrections(
        &self,
        pressure: &[f64],
        momentum_diagonal: &[[f64; 2]],
    ) -> Result<Vec<f64>, Diagnostic> {
        self.apply_linear(pressure, momentum_diagonal, "pressure")
    }

    /// Apply the exact analytic directional action of this same linear operator.
    pub(crate) fn directional_corrections(
        &self,
        pressure_direction: &[f64],
        momentum_diagonal: &[[f64; 2]],
    ) -> Result<Vec<f64>, Diagnostic> {
        self.apply_linear(pressure_direction, momentum_diagonal, "pressure direction")
    }

    /// Add the BDF1 previous-face term required by time-consistent MWI.
    pub(crate) fn transient_consistent_corrections(
        &self,
        pressure: &[f64],
        momentum_diagonal: &[[f64; 2]],
        previous_face_volume_fluxes: &[f64],
        previous_velocity: &[[f64; 2]],
        density: f64,
        duration: f64,
    ) -> Result<Vec<f64>, Diagnostic> {
        let mut corrections = self.apply_linear(pressure, momentum_diagonal, "pressure")?;
        if previous_face_volume_fluxes.len() != self.facets.len()
            || previous_velocity.len() != self.cell_count
            || !density.is_finite()
            || density <= 0.0
            || !duration.is_finite()
            || duration <= 0.0
        {
            return Err(invalid(
                "transient-consistent pressure coupling requires one previous flux per facet, one previous velocity per cell, and positive finite density and duration",
            ));
        }
        for (correction, (facet, previous_flux)) in corrections
            .iter_mut()
            .zip(self.facets.iter().zip(previous_face_volume_fluxes))
        {
            if !previous_flux.is_finite() {
                return Err(invalid(
                    "transient-consistent previous face flux must be finite",
                ));
            }
            let PressureCorrectionFacet::Interior {
                lower,
                upper,
                normal_axis,
                measure,
                ..
            } = *facet
            else {
                if *previous_flux != 0.0 {
                    return Err(invalid(
                        "complete-zero-trace boundary history requires exact zero face flux",
                    ));
                }
                continue;
            };
            let lower_inverse_momentum =
                self.cell_measures[lower] / momentum_diagonal[lower][normal_axis];
            let upper_inverse_momentum =
                self.cell_measures[upper] / momentum_diagonal[upper][normal_axis];
            let face_weight = 0.5 * (lower_inverse_momentum + upper_inverse_momentum);
            let previous_face_velocity = 0.5
                * (previous_velocity[lower][normal_axis] + previous_velocity[upper][normal_axis]);
            let history_defect = previous_flux / measure - previous_face_velocity;
            *correction += measure * face_weight * density / duration * history_defect;
            if !correction.is_finite() {
                return Err(invalid(
                    "transient-consistent pressure correction is non-finite",
                ));
            }
        }
        Ok(corrections)
    }

    /// Reconstruct Cartesian cell gradients from the exact stencil used by
    /// the pressure-coupling action.
    pub(crate) fn cell_gradients(
        &self,
        values: &[f64],
        role: &str,
    ) -> Result<Vec<[f64; DIMENSION]>, Diagnostic> {
        if values.len() != self.cell_count {
            return Err(invalid(format!(
                "pressure {role} gradient has {} cells, expected {}",
                values.len(),
                self.cell_count
            )));
        }
        if let Some(cell) = values.iter().position(|value| !value.is_finite()) {
            return Err(invalid(format!(
                "pressure {role} gradient cell {cell} must be finite"
            )));
        }
        let mut cell_gradients = Vec::with_capacity(self.cell_count);
        for stencils in &self.gradient_stencils {
            let mut gradient = [0.0; DIMENSION];
            for (gradient, stencil) in gradient.iter_mut().zip(stencils) {
                *gradient =
                    (values[stencil.upper] - values[stencil.lower]) * stencil.inverse_distance;
                if !gradient.is_finite() {
                    return Err(invalid(format!("pressure {role} gradient is non-finite")));
                }
            }
            cell_gradients.push(gradient);
        }
        Ok(cell_gradients)
    }

    fn apply_linear(
        &self,
        values: &[f64],
        momentum_diagonal: &[[f64; 2]],
        role: &str,
    ) -> Result<Vec<f64>, Diagnostic> {
        if values.len() != self.cell_count {
            return Err(invalid(format!(
                "pressure correction {role} has {} cells, expected {}",
                values.len(),
                self.cell_count
            )));
        }
        if momentum_diagonal.len() != self.cell_count {
            return Err(invalid(format!(
                "pressure correction requires one two-component momentum diagonal per cell, received {} for {} cells",
                momentum_diagonal.len(),
                self.cell_count
            )));
        }
        if let Some(cell) = values.iter().position(|value| !value.is_finite()) {
            return Err(invalid(format!(
                "pressure correction {role} cell {cell} must be finite"
            )));
        }
        if let Some((cell, component)) =
            momentum_diagonal
                .iter()
                .enumerate()
                .find_map(|(cell, diagonal)| {
                    diagonal
                        .iter()
                        .position(|value| !value.is_finite() || *value <= 0.0)
                        .map(|component| (cell, component))
                })
        {
            return Err(invalid(format!(
                "pressure correction momentum diagonal cell {cell} component {component} must be finite and positive"
            )));
        }

        let cell_gradients = self.cell_gradients(values, role)?;

        let mut corrections = Vec::with_capacity(self.facets.len());
        for facet in &self.facets {
            let correction = match *facet {
                PressureCorrectionFacet::Interior {
                    lower,
                    upper,
                    normal_axis,
                    inverse_center_distance,
                    measure,
                } => {
                    let lower_inverse_momentum =
                        self.cell_measures[lower] / momentum_diagonal[lower][normal_axis];
                    let upper_inverse_momentum =
                        self.cell_measures[upper] / momentum_diagonal[upper][normal_axis];
                    let face_weight = 0.5 * (lower_inverse_momentum + upper_inverse_momentum);
                    if !lower_inverse_momentum.is_finite()
                        || lower_inverse_momentum <= 0.0
                        || !upper_inverse_momentum.is_finite()
                        || upper_inverse_momentum <= 0.0
                        || !face_weight.is_finite()
                        || face_weight <= 0.0
                    {
                        return Err(invalid(
                            "pressure correction derived normal-momentum weight must be finite and positive",
                        ));
                    }
                    let pressure_slope = (values[upper] - values[lower]) * inverse_center_distance;
                    let average_gradient = 0.5
                        * (cell_gradients[lower][normal_axis] + cell_gradients[upper][normal_axis]);
                    -measure * face_weight * (pressure_slope - average_gradient)
                }
                PressureCorrectionFacet::Boundary => 0.0,
            };
            if !correction.is_finite() {
                return Err(invalid(format!(
                    "pressure correction {role} facet action is non-finite"
                )));
            }
            corrections.push(correction);
        }
        Ok(corrections)
    }
}

fn require_supported_mesh(mesh: &CartesianMesh) -> Result<[usize; DIMENSION], Diagnostic> {
    let first = mesh
        .axis_cell_count(0)
        .ok_or_else(|| invalid("pressure correction requires a two-dimensional Cartesian mesh"))?;
    let second = mesh
        .axis_cell_count(1)
        .ok_or_else(|| invalid("pressure correction requires a two-dimensional Cartesian mesh"))?;
    if mesh.axis_cell_count(DIMENSION).is_some() {
        return Err(invalid(
            "pressure correction requires exactly two Cartesian dimensions",
        ));
    }
    if first < 2 || second < 2 {
        return Err(invalid(
            "pressure correction gradient requires at least two cells on each Cartesian axis",
        ));
    }
    Ok([first, second])
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-incompressible-fvm-2d".to_owned(),
        "pressure-coupling".to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_and_affine_pressure_have_roundoff_zero_correction() {
        let mesh = CartesianMesh::from_axes(vec![
            vec![-1.0, -0.4, 0.2, 1.5, 2.0],
            vec![-2.0, -0.5, 0.25, 1.0],
        ])
        .unwrap();
        let diagonals = vec![[2.5, 3.75]; 12];
        let operator = MomentumWeightedPressureCoupling2d::new(&mesh).unwrap();

        let constant = operator.corrections(&[3.25; 12], &diagonals).unwrap();
        assert!(constant.iter().all(|value| *value == 0.0));

        let gradient = [1.75, -0.625];
        let affine = cell_values(&mesh, |point, _| {
            -2.0 + gradient[0] * point[0] + gradient[1] * point[1]
        });
        let affine = operator.corrections(&affine, &diagonals).unwrap();
        let maximum_correction = affine
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max);
        assert!(maximum_correction < 2.0e-14);
    }

    #[test]
    fn both_cartesian_checkerboard_families_have_nonzero_action() {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[6, 5]).unwrap();
        let operator = MomentumWeightedPressureCoupling2d::new(&mesh).unwrap();
        let diagonals = vec![[1.0, 1.5]; 30];

        let one_axis = cell_values(
            &mesh,
            |_, indices| {
                if indices[0] % 2 == 0 { 1.0 } else { -1.0 }
            },
        );
        let two_axis = cell_values(&mesh, |_, indices| {
            if (indices[0] + indices[1]) % 2 == 0 {
                1.0
            } else {
                -1.0
            }
        });
        for pressure in [one_axis, two_axis] {
            let action = operator.corrections(&pressure, &diagonals).unwrap();
            let norm = action.iter().map(|value| value * value).sum::<f64>().sqrt();
            assert!(norm > 1.0e-6, "checkerboard pressure action was {norm:e}");
        }
    }

    #[test]
    fn directional_action_is_the_same_linear_operator() {
        let mesh = CartesianMesh::uniform(&[[-1.0, 2.0], [0.0, 2.0]], &[4, 3]).unwrap();
        let diagonals = (0..12)
            .map(|cell| [1.0 + 0.125 * cell as f64, 2.0 + 0.25 * cell as f64])
            .collect::<Vec<_>>();
        let operator = MomentumWeightedPressureCoupling2d::new(&mesh).unwrap();

        let pressure = cell_values(&mesh, |point, indices| {
            point[0].powi(2) - 0.25 * point[1] + indices[1] as f64
        });
        let direction = cell_values(&mesh, |point, indices| {
            -0.5 * point[0] + point[1].powi(2) + 0.1 * indices[0] as f64
        });
        let analytic = operator
            .directional_corrections(&direction, &diagonals)
            .unwrap();
        let epsilon = 1.0e-6;
        let plus = pressure
            .iter()
            .zip(&direction)
            .map(|(pressure, direction)| pressure + epsilon * direction)
            .collect::<Vec<_>>();
        let minus = pressure
            .iter()
            .zip(&direction)
            .map(|(pressure, direction)| pressure - epsilon * direction)
            .collect::<Vec<_>>();
        let plus = operator.corrections(&plus, &diagonals).unwrap();
        let minus = operator.corrections(&minus, &diagonals).unwrap();
        assert_eq!(plus.len(), 31);
        let maximum_error = plus
            .iter()
            .zip(&minus)
            .zip(&analytic)
            .map(|((plus, minus), analytic)| ((plus - minus) / (2.0 * epsilon) - analytic).abs())
            .fold(0.0_f64, f64::max);
        assert!(
            maximum_error < 2.0e-10,
            "directional error {maximum_error:e}"
        );
    }

    #[test]
    fn bdf1_face_history_is_active_and_zero_trace_boundaries_fail_closed() {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[3, 2]).unwrap();
        let operator = MomentumWeightedPressureCoupling2d::new(&mesh).unwrap();
        let diagonals = vec![[5.0, 7.0]; 6];
        let previous_velocity = vec![[0.0, 0.0]; 6];
        let (_, facets) = cartesian_fvm_geometry_2d(&mesh).unwrap();
        let interior = facets
            .iter()
            .position(|facet| matches!(facet.adjacency, CartesianFacetAdjacency2d::Interior { .. }))
            .unwrap();
        let boundary = facets
            .iter()
            .position(|facet| matches!(facet.adjacency, CartesianFacetAdjacency2d::Boundary { .. }))
            .unwrap();
        let mut history = vec![0.0; facets.len()];
        history[interior] = 0.25;
        let corrections = operator
            .transient_consistent_corrections(
                &[0.0; 6],
                &diagonals,
                &history,
                &previous_velocity,
                1.0,
                0.1,
            )
            .unwrap();
        assert!(corrections[interior].abs() > 1.0e-12);

        history[boundary] = 1.0;
        assert!(
            operator
                .transient_consistent_corrections(
                    &[0.0; 6],
                    &diagonals,
                    &history,
                    &previous_velocity,
                    1.0,
                    0.1,
                )
                .is_err()
        );
    }

    #[test]
    fn invalid_shapes_diagonals_and_axis_support_fail_closed() {
        let mesh = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[3, 2]).unwrap();
        let operator = MomentumWeightedPressureCoupling2d::new(&mesh).unwrap();
        assert!(operator.corrections(&[0.0; 6], &[[1.0; 2]; 5]).is_err());
        for invalid_diagonal in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            for component in 0..DIMENSION {
                let mut diagonals = vec![[1.0; DIMENSION]; 6];
                diagonals[2][component] = invalid_diagonal;
                assert!(operator.corrections(&[0.0; 6], &diagonals).is_err());
            }
        }

        let diagonals = [[1.0; DIMENSION]; 6];
        assert!(operator.corrections(&[0.0; 5], &diagonals).is_err());
        let mut nonfinite = vec![0.0; 6];
        nonfinite[4] = f64::NAN;
        assert!(operator.corrections(&nonfinite, &diagonals).is_err());
        assert!(
            operator
                .directional_corrections(&nonfinite, &diagonals)
                .is_err()
        );

        let insufficient = CartesianMesh::uniform(&[[0.0, 1.0], [0.0, 1.0]], &[1, 3]).unwrap();
        assert!(MomentumWeightedPressureCoupling2d::new(&insufficient).is_err());
    }

    fn cell_values(
        mesh: &CartesianMesh,
        value: impl Fn([f64; DIMENSION], [usize; DIMENSION]) -> f64,
    ) -> Vec<f64> {
        let count = mesh.axis_cell_count(0).unwrap() * mesh.axis_cell_count(1).unwrap();
        (0..count)
            .map(|cell| {
                let entity = MeshEntity::new(DIMENSION, cell);
                let indices: [usize; DIMENSION] =
                    mesh.cell_multi_index(entity).unwrap().try_into().unwrap();
                let point: [f64; DIMENSION] =
                    mesh.entity_center(entity).unwrap().try_into().unwrap();
                value(point, indices)
            })
            .collect()
    }
}
